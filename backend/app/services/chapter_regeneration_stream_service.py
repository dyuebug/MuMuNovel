from __future__ import annotations

import asyncio
from dataclasses import dataclass
from datetime import datetime
from typing import TYPE_CHECKING, Any, AsyncGenerator, AsyncIterator, Callable, Dict, List, Optional, Sequence

from app.logger import get_logger
from app.schemas.generation_payload import build_chapter_regeneration_stream_result_payload
from app.schemas.regeneration import ChapterRegenerateRequest
from app.services.ai_service import AIService
from app.services.regeneration_task_service import (
    create_regeneration_task,
    mark_latest_regeneration_task_failed,
)
from app.utils.sse_response import SSEResponse, WizardProgressTracker

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from app.models.chapter import Chapter
    from app.models.memory import PlotAnalysis

logger = get_logger(__name__)


@dataclass(frozen=True)
class ChapterRegenerationStreamContext:
    chapter: Chapter
    analysis: Optional[PlotAnalysis]
    user_id: str
    regenerate_request: ChapterRegenerateRequest
    effective_regenerate_request: ChapterRegenerateRequest
    project_context: Dict[str, Any]
    style_content: str
    style_id: Optional[int]
    story_runtime_contract: Optional[Dict[str, Any]]


@dataclass(frozen=True)
class ChapterRegenerationSanitizedContent:
    full_content: str
    removed_meta_lines: int


@dataclass(frozen=True)
class ChapterRegenerationCompletion:
    word_count: int
    diff_stats: Dict[str, Any]
    result_payload: Dict[str, Any]


@dataclass(frozen=True)
class ChapterRegenerationEmissionStep:
    kind: str
    payload: Optional[Dict[str, Any]] = None
    message: Optional[str] = None
    event: Optional[str] = None
    progress: Optional[float] = None


@dataclass
class ChapterRegenerationStreamingState:
    full_content: str = ""


async def build_chapter_regeneration_event_stream(
    *,
    db_session_source: Callable[[], AsyncGenerator[AsyncSession, None]],
    context: ChapterRegenerationStreamContext,
    user_ai_service: AIService,
    regenerator_factory: Callable[[AIService], Any],
    sanitize_generated_text: Callable[[str], tuple[str, int]],
    contains_workflow_meta_text: Callable[[str], bool],
) -> AsyncGenerator[str, None]:
    tracker = WizardProgressTracker("章节重写")
    yield await tracker.start()

    async for db_session in db_session_source():
        db_committed = False
        try:
            yield await tracker.loading("正在准备重写上下文...", 0.5)

            regeneration_task = await create_regeneration_task(
                db_session,
                chapter=context.chapter,
                analysis=context.analysis,
                user_id=context.user_id,
                regenerate_request=context.regenerate_request,
                style_id=context.style_id,
            )
            task_id = regeneration_task.id
            logger.info(f"已创建章节重写任务: {task_id}")

            yield await tracker.preparing("正在生成重写提示词...")
            yield await SSEResponse.send_event(
                event="task_created",
                data={"task_id": task_id},
            )

            regenerator = regenerator_factory(user_ai_service)
            streaming_state = ChapterRegenerationStreamingState()
            estimated_total = resolve_chapter_regeneration_estimated_total(context)

            yield await tracker.generating(
                current_chars=0,
                estimated_total=estimated_total,
            )

            async for streamed_payload in stream_chapter_regeneration_feedback(
                regenerator=regenerator,
                context=context,
                db_session=db_session,
                estimated_total=estimated_total,
                streaming_state=streaming_state,
                tracker_generating_chunk_fn=tracker.generating_chunk,
                tracker_preparing_fn=tracker.preparing,
                tracker_generating_fn=tracker.generating,
                tracker_parsing_fn=tracker.parsing,
            ):
                yield streamed_payload

            yield await tracker.saving("正在保存重写结果...", 0.5)

            full_content = streaming_state.full_content
            sanitized_content = sanitize_chapter_regeneration_content(
                full_content,
                chapter_id=context.chapter.id,
                sanitize_generated_text=sanitize_generated_text,
                contains_workflow_meta_text=contains_workflow_meta_text,
            )
            full_content = sanitized_content.full_content

            completion = finalize_chapter_regeneration_completion(
                regeneration_task=regeneration_task,
                original_content=context.chapter.content,
                regenerated_content=full_content,
                regenerator=regenerator,
                regenerate_request=context.regenerate_request,
                story_runtime_contract=context.story_runtime_contract,
            )

            await db_session.commit()
            db_committed = True

            emission_plan = build_chapter_regeneration_emission_plan(
                result_payload=completion.result_payload,
            )
            async for emitted_payload in emit_chapter_regeneration_plan(
                emission_plan=emission_plan,
                tracker_saving_fn=tracker.saving,
                tracker_complete_fn=tracker.complete,
                tracker_result_fn=tracker.result,
                tracker_done_fn=tracker.done,
            ):
                yield emitted_payload

            logger.info(f"章节重写完成: {context.chapter.id}, 任务: {task_id}")
        except Exception as exc:
            logger.error(f"章节重写失败: {str(exc)}", exc_info=True)

            if not db_committed:
                try:
                    await handle_chapter_regeneration_failure(
                        db_session,
                        chapter_id=context.chapter.id,
                        error_message=str(exc),
                    )
                except Exception as update_error:
                    logger.error(f"更新章节重写任务状态失败: {str(update_error)}")

            yield await tracker.error(str(exc))
        break


def resolve_chapter_regeneration_estimated_total(
    context: ChapterRegenerationStreamContext,
) -> int:
    return int(
        context.effective_regenerate_request.target_word_count
        or context.regenerate_request.target_word_count
        or len(context.chapter.content or "")
    )


def sanitize_chapter_regeneration_content(
    full_content: str,
    *,
    chapter_id: str,
    sanitize_generated_text: Callable[[str], tuple[str, int]],
    contains_workflow_meta_text: Callable[[str], bool],
) -> ChapterRegenerationSanitizedContent:
    sanitized_content, removed_meta_lines = sanitize_generated_text(full_content)
    if removed_meta_lines > 0:
        logger.warning(
            f"章节重写检测到流程化元文本，已清理 {removed_meta_lines} 行: chapter_id={chapter_id}"
        )
    if not sanitized_content.strip():
        raise ValueError("重写结果为空或仅包含流程化元文本")
    if contains_workflow_meta_text(sanitized_content):
        raise ValueError("重写结果包含流程化元文本")
    return ChapterRegenerationSanitizedContent(
        full_content=sanitized_content,
        removed_meta_lines=removed_meta_lines,
    )


def finalize_chapter_regeneration_completion(
    *,
    regeneration_task: Any,
    original_content: Optional[str],
    regenerated_content: str,
    regenerator: Any,
    regenerate_request: ChapterRegenerateRequest,
    story_runtime_contract: Optional[Dict[str, Any]],
    build_result_payload_fn: Callable[..., Dict[str, Any]] = build_chapter_regeneration_stream_result_payload,
) -> ChapterRegenerationCompletion:
    regeneration_task.status = "completed"
    regeneration_task.regenerated_content = regenerated_content
    regeneration_task.regenerated_word_count = len(regenerated_content)
    regeneration_task.completed_at = datetime.now()

    diff_stats = regenerator.calculate_content_diff(
        original_content,
        regenerated_content,
    )
    result_payload = build_result_payload_fn(
        task_id=regeneration_task.id,
        word_count=len(regenerated_content),
        version_number=regeneration_task.version_number,
        auto_applied=regenerate_request.auto_apply,
        diff_stats=diff_stats,
        story_runtime_contract=story_runtime_contract,
    )
    return ChapterRegenerationCompletion(
        word_count=len(regenerated_content),
        diff_stats=diff_stats if isinstance(diff_stats, dict) else {},
        result_payload=result_payload,
    )


def build_chapter_regeneration_emission_plan(
    *,
    result_payload: Dict[str, Any],
) -> List[ChapterRegenerationEmissionStep]:
    return [
        ChapterRegenerationEmissionStep(kind="tracker_saving", message="正在保存", progress=0.9),
        ChapterRegenerationEmissionStep(kind="tracker_complete", message="章节重写完成"),
        ChapterRegenerationEmissionStep(kind="tracker_result", payload=result_payload),
        ChapterRegenerationEmissionStep(kind="tracker_done"),
    ]


async def emit_chapter_regeneration_plan(
    *,
    emission_plan: Sequence[ChapterRegenerationEmissionStep],
    tracker_saving_fn: Callable[[str, float], Awaitable[Any]],
    tracker_complete_fn: Callable[[str], Awaitable[Any]],
    tracker_result_fn: Callable[[Dict[str, Any]], Awaitable[Any]],
    tracker_done_fn: Callable[[], Awaitable[Any]],
) -> AsyncIterator[Any]:
    for emission_step in emission_plan:
        if emission_step.kind == "tracker_saving":
            yield await tracker_saving_fn(emission_step.message or "", float(emission_step.progress or 0))
        elif emission_step.kind == "tracker_complete":
            yield await tracker_complete_fn(emission_step.message or "")
        elif emission_step.kind == "tracker_result":
            yield await tracker_result_fn(emission_step.payload or {})
        elif emission_step.kind == "tracker_done":
            yield await tracker_done_fn()


async def handle_chapter_regeneration_failure(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    error_message: str,
    mark_failed_fn: Callable[..., Awaitable[Any]] = mark_latest_regeneration_task_failed,
) -> None:
    await mark_failed_fn(
        db_session,
        chapter_id=chapter_id,
        error_message=error_message,
    )


async def stream_chapter_regeneration_feedback(
    *,
    regenerator: Any,
    context: ChapterRegenerationStreamContext,
    db_session: AsyncSession,
    estimated_total: int,
    streaming_state: ChapterRegenerationStreamingState,
    tracker_generating_chunk_fn: Callable[[str], Awaitable[Any]],
    tracker_preparing_fn: Callable[[str], Awaitable[Any]],
    tracker_generating_fn: Callable[..., Awaitable[Any]],
    tracker_parsing_fn: Callable[[str], Awaitable[Any]],
) -> AsyncIterator[Any]:
    async for event in regenerator.regenerate_with_feedback(
        chapter=context.chapter,
        analysis=context.analysis,
        regenerate_request=context.effective_regenerate_request,
        project_context=context.project_context,
        style_content=context.style_content,
        user_id=context.user_id,
        db=db_session,
    ):
        if event["type"] == "chunk":
            chunk = str(event.get("content") or "")
            streaming_state.full_content += chunk
            yield await tracker_generating_chunk_fn(chunk)

            if streaming_state.full_content and len(streaming_state.full_content) % 500 == 0:
                yield await tracker_generating_fn(
                    current_chars=len(streaming_state.full_content),
                    estimated_total=estimated_total,
                    message=f"正在重写中... 已生成 {len(streaming_state.full_content)} 字",
                )
        elif event["type"] == "progress":
            progress = float(event.get("progress") or 0)
            message = str(event.get("message") or "")
            if progress < 20:
                yield await tracker_preparing_fn(message)
            elif progress < 85:
                yield await tracker_generating_fn(
                    current_chars=len(streaming_state.full_content),
                    estimated_total=estimated_total,
                    message=message,
                )
            else:
                yield await tracker_parsing_fn(message)

        await asyncio.sleep(0)
