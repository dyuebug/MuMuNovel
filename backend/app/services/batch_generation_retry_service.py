"""批量生成重试调度 helper。"""
from __future__ import annotations

import asyncio
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any, Callable, Dict, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch retry/runtime orchestration chain; this "
    "Python helper is kept only as frozen rollback/source-map material for "
    "legacy batch retry fallback."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from app.logger import get_logger

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from app.models.batch_generation_task import BatchGenerationTask
    from app.models.chapter import Chapter
    from app.models.project import Project
    from app.services.ai_service import AIService
    from app.services.chapter_quality_context_service import StoryPacket
    from app.services.story_repair_payload_service import StoryRepairPayload


logger = get_logger(__name__)


def _batch_generation_chapter_execution_service():
    from app.services import batch_generation_chapter_execution_service

    return batch_generation_chapter_execution_service


def _batch_generation_chapter_failure_state_service():
    from app.services import batch_generation_chapter_failure_state_service

    return batch_generation_chapter_failure_state_service


def _batch_generation_chapter_success_state_service():
    from app.services import batch_generation_chapter_success_state_service

    return batch_generation_chapter_success_state_service


def _chapter_generation_prerequisite_service():
    from app.services.chapter_generation import prerequisite_service

    return prerequisite_service


def _chapter_quality_context_service():
    from app.services import chapter_quality_context_service

    return chapter_quality_context_service


def _story_repair_payload_service():
    from app.services import story_repair_payload_service

    return story_repair_payload_service


def _story_runtime_serialization_service():
    from app.services import story_runtime_serialization_service

    return story_runtime_serialization_service


def _task_quality_snapshot_service():
    from app.services import task_quality_snapshot_service

    return task_quality_snapshot_service


def _task_workflow_runtime_service():
    from app.services import task_workflow_runtime_service

    return task_workflow_runtime_service


def clear_batch_generation_execution_caches(*args, **kwargs):
    return _batch_generation_chapter_execution_service().clear_batch_generation_execution_caches(*args, **kwargs)


async def prepare_batch_generation_chapter_attempt(*args, **kwargs):
    return await _batch_generation_chapter_execution_service().prepare_batch_generation_chapter_attempt(*args, **kwargs)


def prepare_batch_generation_chapter_result(*args, **kwargs):
    return _batch_generation_chapter_execution_service().prepare_batch_generation_chapter_result(*args, **kwargs)


async def fail_batch_generation_after_analysis(*args, **kwargs):
    return await _batch_generation_chapter_failure_state_service().fail_batch_generation_after_analysis(*args, **kwargs)


async def fail_batch_generation_after_max_retries(*args, **kwargs):
    return await _batch_generation_chapter_failure_state_service().fail_batch_generation_after_max_retries(*args, **kwargs)


async def apply_successful_batch_generation_chapter(*args, **kwargs):
    return await _batch_generation_chapter_success_state_service().apply_successful_batch_generation_chapter(*args, **kwargs)


async def finalize_successful_batch_generation_chapter(*args, **kwargs):
    return await _batch_generation_chapter_success_state_service().finalize_successful_batch_generation_chapter(*args, **kwargs)


async def handle_batch_generation_quality_gate_retry(*args, **kwargs):
    return await _batch_generation_chapter_success_state_service().handle_batch_generation_quality_gate_retry(*args, **kwargs)


async def check_chapter_generation_prerequisites(*args, **kwargs):
    return await _chapter_generation_prerequisite_service().check_chapter_generation_prerequisites(*args, **kwargs)


def clone_chapter_quality_profile(*args, **kwargs):
    return _chapter_quality_context_service().clone_chapter_quality_profile(*args, **kwargs)


async def resolve_generation_story_repair_state_for_batch(*args, **kwargs):
    return await _story_repair_payload_service().resolve_generation_story_repair_state_for_batch(*args, **kwargs)


def resolve_quality_gate_execution_plan(*args, **kwargs):
    return _story_repair_payload_service().resolve_quality_gate_execution_plan(*args, **kwargs)


def attach_story_runtime_contract(*args, **kwargs):
    return _story_runtime_serialization_service().attach_story_runtime_contract(*args, **kwargs)


async def record_task_quality_metrics(*args, **kwargs):
    return await _task_quality_snapshot_service().record_task_quality_metrics(*args, **kwargs)


async def batch_task_exists(*args, **kwargs):
    return await _task_workflow_runtime_service().batch_task_exists(*args, **kwargs)


async def sync_task_story_repair_state(*args, **kwargs):
    return await _task_workflow_runtime_service().sync_task_story_repair_state(*args, **kwargs)


@dataclass(frozen=True)
class BatchGenerationExecutionEnvironment:
    batch_id: str
    user_id: str
    ai_service: "AIService"
    write_lock: Any
    emit_event: Callable[..., Any]
    batch_story_packet: "StoryPacket"
    task_base_quality_profile: Dict[str, Any]
    cached_analysis_quality_profile: Dict[str, Any]
    custom_model: Optional[str]
    temp_narrative_perspective: Optional[str]
    creative_mode: Optional[str]
    story_focus: Optional[str]
    plot_stage: Optional[str]
    story_creation_brief: Optional[str]
    quality_preset: Optional[str]
    quality_notes: Optional[str]
    enable_web_research: Optional[bool]
    web_research_query: Optional[str]
    story_repair_summary: Optional[str]
    story_repair_targets: Optional[list[str]]
    story_preserve_strengths: Optional[list[str]]
    stream_chunks: bool
    run_generation_fn: Callable[..., Any]
    await_generation_result_fn: Callable[..., Any]
    run_batch_analysis_fn: Callable[..., Any]


@dataclass(frozen=True)
class BatchGenerationChapterRuntimeState:
    chapter_id: str
    chapter_index: int
    last_generated_summary: Optional[str]
    active_story_repair_state: Dict[str, Any]
    active_story_repair_payload: Optional["StoryRepairPayload"]


@dataclass(frozen=True)
class BatchGenerationChapterExecutionOutcome:
    active_story_repair_state: Dict[str, Any]
    active_story_repair_payload: Optional[StoryRepairPayload]
    last_generated_summary: Optional[str]


async def execute_batch_generation_chapter_with_retries(
    db_session: "AsyncSession",
    *,
    task: "BatchGenerationTask",
    project: "Project",
    execution_context: BatchGenerationExecutionEnvironment,
    runtime_state: BatchGenerationChapterRuntimeState,
) -> Optional[BatchGenerationChapterExecutionOutcome]:
    from sqlalchemy import select
    from app.models.chapter import Chapter

    retry_count = 0
    chapter_success = False
    chapter = None
    current_last_generated_summary = runtime_state.last_generated_summary
    current_active_story_repair_state = runtime_state.active_story_repair_state
    current_active_story_repair_payload = runtime_state.active_story_repair_payload

    while retry_count <= task.max_retries and not chapter_success:
        try:
            attempt_preparation = await prepare_batch_generation_chapter_attempt(
                db_session,
                task=task,
                project=project,
                chapter_id=runtime_state.chapter_id,
                retry_count=retry_count,
                write_lock=execution_context.write_lock,
                emit_event=execution_context.emit_event,
                cached_analysis_quality_profile=execution_context.cached_analysis_quality_profile,
                clone_quality_profile_fn=clone_chapter_quality_profile,
            )
            chapter = attempt_preparation.chapter
            analysis_quality_profile = attempt_preparation.analysis_quality_profile

            if retry_count > 0:
                logger.info(
                    f"重试 [{runtime_state.chapter_index}/{task.total_chapters}] 继续生成 (第{retry_count}次): "
                    f"第{chapter.chapter_number}章《{chapter.title}》"
                )
            else:
                logger.info(
                    f"开始 [{runtime_state.chapter_index}/{task.total_chapters}] 生成章节: "
                    f"第{chapter.chapter_number}章《{chapter.title}》"
                )

            can_generate, error_msg, _ = await check_chapter_generation_prerequisites(db_session, chapter)
            if not can_generate:
                raise Exception(f"章节生成失败: {error_msg}")

            generation_result = await execution_context.await_generation_result_fn(
                generation_coro=execution_context.run_generation_fn(
                    db_session=db_session,
                    chapter=chapter,
                    user_id=execution_context.user_id,
                    style_id=task.style_id,
                    target_word_count=task.target_word_count,
                    ai_service=execution_context.ai_service,
                    write_lock=execution_context.write_lock,
                    story_packet=execution_context.batch_story_packet,
                    base_quality_profile=execution_context.task_base_quality_profile,
                    custom_model=execution_context.custom_model,
                    previous_summary_context=current_last_generated_summary,
                    temp_narrative_perspective=execution_context.temp_narrative_perspective,
                    creative_mode=execution_context.creative_mode,
                    story_focus=execution_context.story_focus,
                    plot_stage=execution_context.plot_stage,
                    story_creation_brief=execution_context.story_creation_brief,
                    quality_preset=execution_context.quality_preset,
                    quality_notes=execution_context.quality_notes,
                    enable_web_research=execution_context.enable_web_research,
                    web_research_query=execution_context.web_research_query,
                    story_repair_summary=execution_context.story_repair_summary,
                    story_repair_targets=execution_context.story_repair_targets,
                    story_preserve_strengths=execution_context.story_preserve_strengths,
                    story_repair_payload=current_active_story_repair_payload,
                    active_story_repair_snapshot=current_active_story_repair_state.get("active_story_repair_payload"),
                    story_repair_state=current_active_story_repair_state,
                    stream_task_id=execution_context.batch_id,
                    stream_chunks=execution_context.stream_chunks,
                    retry_count=retry_count,
                    max_retries=task.max_retries,
                ),
                task=task,
                db_session=db_session,
            )

            prepared_result = prepare_batch_generation_chapter_result(
                generation_result,
                chapter=chapter,
                retry_count=retry_count,
                max_retries=task.max_retries,
                active_story_repair_payload=current_active_story_repair_payload,
                enable_analysis=task.enable_analysis,
                resolve_quality_gate_plan_fn=resolve_quality_gate_execution_plan,
                attach_story_runtime_contract_fn=attach_story_runtime_contract,
            )
            generated_summary = prepared_result.generated_summary
            generated_content = prepared_result.generated_content
            generated_word_count = prepared_result.generated_word_count
            generation_quality_metrics = prepared_result.generation_quality_metrics
            generation_story_runtime_contract = prepared_result.generation_story_runtime_contract
            quality_gate_plan = prepared_result.quality_gate_plan
            quality_gate_snapshot = prepared_result.quality_gate_snapshot
            quality_gate_action = prepared_result.quality_gate_action
            should_run_analysis = prepared_result.should_run_analysis

            if prepared_result.metrics_event is not None:
                await execution_context.emit_event(prepared_result.metrics_event)
                await record_task_quality_metrics(
                    execution_context.batch_id,
                    prepared_result.metrics_event,
                    db_session=db_session,
                )

            retry_preparation = None
            if quality_gate_action == "retry":
                retry_preparation = await handle_batch_generation_quality_gate_retry(
                    db_session,
                    task=task,
                    chapter=chapter,
                    project=project,
                    batch_id=execution_context.batch_id,
                    chapter_id=runtime_state.chapter_id,
                    retry_count=retry_count,
                    write_lock=execution_context.write_lock,
                    emit_event=execution_context.emit_event,
                    sync_task_story_repair_state_fn=sync_task_story_repair_state,
                                        quality_gate_plan=quality_gate_plan,
                    quality_gate_snapshot=quality_gate_snapshot,
                    generated_content=generated_content,
                    generated_summary=generated_summary,
                    generation_quality_metrics=generation_quality_metrics,
                    active_story_repair_payload=current_active_story_repair_payload,
                    active_story_repair_state=current_active_story_repair_state,
                )
                current_active_story_repair_state = retry_preparation.active_story_repair_state
                current_active_story_repair_payload = retry_preparation.active_story_repair_payload
            else:
                if quality_gate_action == "manual_review":
                    current_active_story_repair_payload = (
                        quality_gate_plan.get("repair_payload") or current_active_story_repair_payload
                    )
                    current_active_story_repair_state = await sync_task_story_repair_state(
                        execution_context.batch_id,
                        payload=current_active_story_repair_payload,
                        active_story_repair_payload=quality_gate_plan.get("active_story_repair_payload"),
                        db_session=db_session,
                    )
                    notice_message = quality_gate_plan.get("message") or (
                        f"Chapter {chapter.chapter_number} hit the quality gate; content is kept and optimization is recommended."
                    )
                    await execution_context.emit_event(
                        {
                            "type": "progress",
                            "chapter_id": runtime_state.chapter_id,
                            "chapter_number": chapter.chapter_number,
                            "message": notice_message,
                            "progress": 76,
                            "status": "running",
                            "phase": "saving",
                            "current_retry_count": retry_count,
                            "max_retries": task.max_retries,
                            "quality_gate": quality_gate_snapshot,
                            "active_story_repair_payload": quality_gate_plan.get("active_story_repair_payload"),
                        }
                    )
                applied_state = await apply_successful_batch_generation_chapter(
                    db_session,
                    task=task,
                    chapter=chapter,
                    batch_id=execution_context.batch_id,
                    retry_count=retry_count,
                    emit_event=execution_context.emit_event,
                                    project=project,
                    write_lock=execution_context.write_lock,
                    resolve_story_repair_state_fn=resolve_generation_story_repair_state_for_batch,
                    sync_task_story_repair_state_fn=sync_task_story_repair_state,
                    generated_content=generated_content,
                    generated_word_count=generated_word_count,
                    generation_quality_metrics=generation_quality_metrics,
                    generation_story_runtime_contract=generation_story_runtime_contract,
                    generated_summary=generated_summary,
                    should_run_analysis=should_run_analysis,
                    story_repair_summary=execution_context.story_repair_summary,
                    story_repair_targets=execution_context.story_repair_targets,
                    story_preserve_strengths=execution_context.story_preserve_strengths,
                )
                current_active_story_repair_state = applied_state.active_story_repair_state
                current_active_story_repair_payload = applied_state.active_story_repair_payload
                if applied_state.last_generated_summary:
                    current_last_generated_summary = applied_state.last_generated_summary

            if not await batch_task_exists(db_session, execution_context.batch_id):
                await clear_batch_generation_execution_caches(execution_context.batch_id)
                logger.info(f"Stop batch generation because task no longer exists: {execution_context.batch_id}")
                return None

            chapter_exists_result = await db_session.execute(
                select(Chapter.id).where(Chapter.id == runtime_state.chapter_id)
            )
            if chapter_exists_result.scalar_one_or_none() is None:
                await clear_batch_generation_execution_caches(execution_context.batch_id)
                logger.info(f"Stop batch generation because chapter no longer exists: {runtime_state.chapter_id}")
                return None

            if should_run_analysis:
                analysis_success, analysis_error = await execution_context.run_batch_analysis_fn(
                    db_session=db_session,
                    write_lock=execution_context.write_lock,
                    batch_id=execution_context.batch_id,
                    chapter=chapter,
                    user_id=execution_context.user_id,
                    project_id=task.project_id,
                    retry_count=retry_count,
                    max_retries=task.max_retries,
                    ai_service=execution_context.ai_service,
                    quality_profile=analysis_quality_profile,
                    story_packet=execution_context.batch_story_packet,
                    chapter_content_override=generated_content,
                    chapter_word_count_override=generated_word_count,
                    story_repair_summary=execution_context.story_repair_summary,
                    story_repair_targets=execution_context.story_repair_targets,
                    story_preserve_strengths=execution_context.story_preserve_strengths,
                    story_repair_payload=current_active_story_repair_payload,
                )
                if not analysis_success:
                    await fail_batch_generation_after_analysis(
                        db_session,
                        task=task,
                        chapter=chapter,
                        chapter_id=runtime_state.chapter_id,
                        analysis_error=analysis_error,
                        write_lock=execution_context.write_lock,
                        emit_event=execution_context.emit_event,
                    )
                    return None

            if quality_gate_action == "retry":
                next_retry_count = retry_preparation.next_retry_count
                wait_time = min(2 ** next_retry_count, 10)
                logger.info(
                    f"Quality-gate retry for chapter {chapter.chapter_number}; "
                    f"waiting {wait_time}s before retry #{next_retry_count}"
                )
                await asyncio.sleep(wait_time)
                retry_count = next_retry_count
                continue

            chapter_success = True
            await finalize_successful_batch_generation_chapter(
                db_session,
                task=task,
                emit_event=execution_context.emit_event,
                write_lock=execution_context.write_lock,
            )
        except Exception as e:
            last_error = str(e)
            error_msg = f"Chapter {chapter.chapter_number if chapter else '?'} generation failed: {last_error}"
            logger.error(f"批量生成错误: {error_msg}")

            retry_count += 1

            if retry_count <= task.max_retries:
                wait_time = min(2 ** retry_count, 10)
                logger.info(f"将在 {wait_time} 秒后重试...")
                await asyncio.sleep(wait_time)
            else:
                logger.error(
                    f"❌ 已超过最大重试次数({task.max_retries}): "
                    f"第{chapter.chapter_number if chapter else '?'}章"
                )
                await fail_batch_generation_after_max_retries(
                    db_session,
                    task=task,
                    chapter=chapter,
                    chapter_id=runtime_state.chapter_id,
                    last_error=last_error,
                    retry_count=retry_count,
                    write_lock=execution_context.write_lock,
                    emit_event=execution_context.emit_event,
                )
                return None

    if not chapter_success:
        return None

    return BatchGenerationChapterExecutionOutcome(
        active_story_repair_state=current_active_story_repair_state,
        active_story_repair_payload=current_active_story_repair_payload,
        last_generated_summary=current_last_generated_summary,
    )
