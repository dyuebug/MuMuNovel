"""批量生成章节分析 helper。"""
from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING, Any, Callable, Dict, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation analysis and failure-handling "
    "chain; this Python helper is retained only as frozen "
    "rollback/source-map material after the batch retired-support-shell "
    "closeout review."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/api/health.rs"
SOURCE_MAP_ROLLBACK_FLAG = "aggregate_chapters_python_source_map"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger

if TYPE_CHECKING:
    from app.models.chapter import Chapter
    from app.services.ai_service import AIService
    from app.services.chapter_quality_context_service import (
        StoryGenerationGuidance,
        StoryPacket,
    )
    from app.services.story_repair_payload_service import StoryRepairPayload


logger = get_logger(__name__)


async def create_analysis_task_safely(*args, **kwargs):
    from app.services.analysis_task_service import (
        create_analysis_task_safely as create_analysis_task_safely_impl,
    )

    return await create_analysis_task_safely_impl(*args, **kwargs)


async def execute_chapter_analysis_background(*args, **kwargs):
    from app.services.manual_chapter_analysis_execution_service import (
        execute_chapter_analysis_background as execute_chapter_analysis_background_impl,
    )

    return await execute_chapter_analysis_background_impl(*args, **kwargs)


async def publish_task_stream_event(*args, **kwargs):
    from app.services.task_workflow_runtime_service import (
        publish_task_stream_event as publish_task_stream_event_impl,
    )

    return await publish_task_stream_event_impl(*args, **kwargs)


async def run_batch_chapter_analysis(
    db_session: AsyncSession,
    *,
    write_lock,
    batch_id: str,
    chapter: "Chapter",
    user_id: str,
    project_id: str,
    retry_count: int,
    max_retries: int,
    ai_service: "AIService",
    quality_profile: Optional[Dict[str, Any]] = None,
    story_packet: Optional["StoryPacket"] = None,
    generation_guidance: Optional["StoryGenerationGuidance"] = None,
    chapter_content_override: Optional[str] = None,
    chapter_word_count_override: Optional[int] = None,
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[list[str]] = None,
    story_preserve_strengths: Optional[list[str]] = None,
    story_repair_payload: Optional["StoryRepairPayload"] = None,
    create_analysis_task_fn: Optional[Callable[..., Any]] = None,
    analyze_chapter_background_fn: Optional[Callable[..., Any]] = None,
) -> tuple[bool, Optional[str]]:
    logger.info(f"开始章节分析: 第{chapter.chapter_number}章")
    await publish_task_stream_event(
        batch_id,
        {
            "type": "analysis_started",
            "chapter_id": chapter.id,
            "chapter_number": chapter.chapter_number,
            "message": "正在分析章节",
            "progress": 85,
            "phase": "parsing",
            "current_retry_count": retry_count,
            "max_retries": max_retries,
        },
        db_session=db_session,
    )

    analysis_retry_count = 0
    last_analysis_error = None
    resolved_create_analysis_task_fn = create_analysis_task_fn or create_analysis_task_safely
    resolved_analyze_chapter_background_fn = (
        analyze_chapter_background_fn or execute_chapter_analysis_background
    )

    while analysis_retry_count < 3:
        try:
            if analysis_retry_count > 0:
                logger.info(f"章节分析重试(第{analysis_retry_count}次): 第{chapter.chapter_number}章")

            async with write_lock:
                analysis_task = await resolved_create_analysis_task_fn(
                    db_session,
                    chapter_id=chapter.id,
                    user_id=user_id,
                    project_id=project_id,
                    log_context=f"batch:{batch_id}",
                )
            if analysis_task is None:
                return False, "Chapter or project was deleted before analysis"

            analysis_result = await resolved_analyze_chapter_background_fn(
                chapter_id=chapter.id,
                user_id=user_id,
                project_id=project_id,
                task_id=analysis_task.id,
                ai_service=ai_service,
                quality_profile=quality_profile,
                story_packet=story_packet,
                generation_guidance=generation_guidance,
                chapter_content_override=chapter_content_override,
                chapter_word_count_override=chapter_word_count_override,
                story_repair_summary=story_repair_summary,
                story_repair_targets=story_repair_targets,
                story_preserve_strengths=story_preserve_strengths,
                story_repair_payload=story_repair_payload,
            )
            if not analysis_result:
                raise Exception("章节分析结果为空")

            logger.info(f"开始章节分析: 第{chapter.chapter_number}章")
            return True, None
        except Exception as analysis_error:
            last_analysis_error = str(analysis_error)
            analysis_retry_count += 1

            if analysis_retry_count < 3:
                wait_time = min(2 ** analysis_retry_count, 10)
                logger.warning(f"章节分析将在 {wait_time} 秒后重试...")
                await asyncio.sleep(wait_time)

    return False, last_analysis_error or "章节分析失败"
