"""批量生成运行 service。"""
from __future__ import annotations

import asyncio
from contextlib import suppress
from typing import TYPE_CHECKING, Any, Callable, Dict, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation runtime lifecycle and dispatch "
    "chain; this Python runtime shell is retained only as frozen "
    "rollback/source-map material after the batch retired-support-shell "
    "closeout review."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/api/health.rs"
SOURCE_MAP_ROLLBACK_FLAG = "aggregate_chapters_python_source_map"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from app.logger import get_logger
from sqlalchemy.ext.asyncio import AsyncSession

if TYPE_CHECKING:
    from app.models.batch_generation_task import BatchGenerationTask
    from app.services.ai_service import AIService
    from app.services.chapter_quality_context_service import StoryPacket
    from app.services.story_repair_payload_service import StoryRepairPayload

logger = get_logger(__name__)


async def get_db_write_lock(user_id: str):
    from app.services.chapter_analysis_support_service import get_chapter_analysis_write_lock

    return await get_chapter_analysis_write_lock(user_id)


async def await_cancelable_batch_generation_result(
    *,
    generation_coro,
    task: "BatchGenerationTask",
    db_session: AsyncSession,
    poll_interval_seconds: float,
):
    generation_task = asyncio.create_task(generation_coro)
    try:
        while True:
            try:
                return await asyncio.wait_for(
                    asyncio.shield(generation_task),
                    timeout=poll_interval_seconds,
                )
            except asyncio.TimeoutError:
                await db_session.refresh(task)
                if task.status == 'cancelled':
                    generation_task.cancel()
                    with suppress(asyncio.CancelledError):
                        await generation_task
                    raise asyncio.CancelledError()
    finally:
        if not generation_task.done():
            generation_task.cancel()
            with suppress(asyncio.CancelledError):
                await generation_task


async def execute_batch_generation_in_order_workflow(
    *,
    batch_id: str,
    user_id: str,
    ai_service: "AIService",
    custom_model: Optional[str] = None,
    temp_narrative_perspective: Optional[str] = None,
    story_packet: Optional["StoryPacket"] = None,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
    enable_web_research: Optional[bool] = None,
    web_research_query: Optional[str] = None,
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[list[str]] = None,
    story_preserve_strengths: Optional[list[str]] = None,
    story_repair_payload: Optional["StoryRepairPayload"] = None,
    base_quality_profile: Optional[Dict[str, Any]] = None,
    get_db_write_lock_fn,
    resolve_story_repair_prompt_kwargs_fn,
    clone_quality_profile_fn,
    resolve_story_repair_state_fn,
    sync_task_story_repair_state_fn,
    run_generation_fn: Callable[..., Any],
    await_generation_result_fn: Callable[..., Any],
    run_batch_analysis_fn: Callable[..., Any],
    publish_task_stream_event_fn: Callable[..., Any],
):
    from app.services.batch_generation_retry_service import (
        BatchGenerationChapterRuntimeState,
        BatchGenerationExecutionEnvironment,
        execute_batch_generation_chapter_with_retries,
    )
    from app.services.batch_generation_workflow_service import (
        complete_batch_generation_execution,
        fail_batch_generation_on_unhandled_exception,
        handle_cancelled_batch_generation_execution,
        initialize_batch_generation_execution,
        mark_batch_generation_current_chapter,
    )

    db_session = None
    task = None
    emit_batch_event = None
    write_lock = await get_db_write_lock_fn(user_id)
    resolved_story_repair_kwargs = resolve_story_repair_prompt_kwargs_fn(
        story_repair_payload,
        summary=story_repair_summary,
        targets=story_repair_targets,
        strengths=story_preserve_strengths,
    )
    story_repair_summary = resolved_story_repair_kwargs.get("story_repair_summary")
    story_repair_targets = resolved_story_repair_kwargs.get("story_repair_targets")
    story_preserve_strengths = resolved_story_repair_kwargs.get("story_preserve_strengths")

    try:
        logger.info(f"开始执行批量生成任务: {batch_id}")

        from app.database import get_engine
        from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

        engine = await get_engine(user_id)
        async_session_local = async_sessionmaker(
            engine,
            class_=AsyncSession,
            expire_on_commit=False,
        )
        db_session = async_session_local()

        async def emit_batch_event(event: Dict[str, Any]):
            await publish_task_stream_event_fn(batch_id, event, db_session=db_session)

        initialization = await initialize_batch_generation_execution(
            db_session,
            batch_id=batch_id,
            user_id=user_id,
            write_lock=write_lock,
            emit_event=emit_batch_event,
            story_packet=story_packet,
            creative_mode=creative_mode,
            story_focus=story_focus,
            plot_stage=plot_stage,
            story_creation_brief=story_creation_brief,
            quality_preset=quality_preset,
            quality_notes=quality_notes,
            base_quality_profile=base_quality_profile,
            story_repair_summary=story_repair_summary,
            story_repair_targets=story_repair_targets,
            story_preserve_strengths=story_preserve_strengths,
            clone_quality_profile_fn=clone_quality_profile_fn,
            resolve_story_repair_state_fn=resolve_story_repair_state_fn,
            sync_task_story_repair_state_fn=sync_task_story_repair_state_fn,
        )
        if initialization is None:
            return

        task = initialization.task
        project = initialization.project
        batch_story_packet = initialization.batch_story_packet
        task_base_quality_profile = initialization.task_base_quality_profile
        cached_analysis_quality_profile = initialization.cached_analysis_quality_profile
        active_story_repair_state = initialization.active_story_repair_state
        active_story_repair_payload = initialization.active_story_repair_payload
        stream_chunks = initialization.stream_chunks

        last_generated_summary = None
        execution_context = BatchGenerationExecutionEnvironment(
            batch_id=batch_id,
            user_id=user_id,
            ai_service=ai_service,
            write_lock=write_lock,
            emit_event=emit_batch_event,
            batch_story_packet=batch_story_packet,
            task_base_quality_profile=task_base_quality_profile,
            cached_analysis_quality_profile=cached_analysis_quality_profile,
            custom_model=custom_model,
            temp_narrative_perspective=temp_narrative_perspective,
            creative_mode=creative_mode,
            story_focus=story_focus,
            plot_stage=plot_stage,
            story_creation_brief=story_creation_brief,
            quality_preset=quality_preset,
            quality_notes=quality_notes,
            enable_web_research=enable_web_research,
            web_research_query=web_research_query,
            story_repair_summary=story_repair_summary,
            story_repair_targets=story_repair_targets,
            story_preserve_strengths=story_preserve_strengths,
            stream_chunks=stream_chunks,
            run_generation_fn=run_generation_fn,
            await_generation_result_fn=await_generation_result_fn,
            run_batch_analysis_fn=run_batch_analysis_fn,
        )

        for idx, chapter_id in enumerate(task.chapter_ids, 1):
            await db_session.refresh(task)
            if task.status == 'cancelled':
                await handle_cancelled_batch_generation_execution(
                    db_session,
                    task=task,
                    batch_id=batch_id,
                    write_lock=write_lock,
                    emit_event=emit_batch_event,
                )
                return

            await mark_batch_generation_current_chapter(
                db_session,
                task=task,
                chapter_id=chapter_id,
                write_lock=write_lock,
            )

            chapter_outcome = await execute_batch_generation_chapter_with_retries(
                db_session,
                task=task,
                project=project,
                execution_context=execution_context,
                runtime_state=BatchGenerationChapterRuntimeState(
                    chapter_id=chapter_id,
                    chapter_index=idx,
                    last_generated_summary=last_generated_summary,
                    active_story_repair_state=active_story_repair_state,
                    active_story_repair_payload=active_story_repair_payload,
                ),
            )
            if chapter_outcome is None:
                return

            active_story_repair_state = chapter_outcome.active_story_repair_state
            active_story_repair_payload = chapter_outcome.active_story_repair_payload
            last_generated_summary = chapter_outcome.last_generated_summary

        await complete_batch_generation_execution(
            db_session,
            task=task,
            batch_id=batch_id,
            write_lock=write_lock,
            emit_event=emit_batch_event,
        )
    except asyncio.CancelledError:
        if db_session and task and emit_batch_event is not None:
            try:
                await handle_cancelled_batch_generation_execution(
                    db_session,
                    task=task,
                    batch_id=batch_id,
                    write_lock=write_lock,
                    emit_event=emit_batch_event,
                    refresh_before_commit=True,
                )
            except Exception as cancel_error:
                logger.error(f"处理中断取消逻辑失败: {str(cancel_error)}")
        return
    except Exception as error:
        logger.error(f"批量生成执行异常: {str(error)}", exc_info=True)
        if db_session and task:
            try:
                await fail_batch_generation_on_unhandled_exception(
                    db_session,
                    task=task,
                    error=error,
                    write_lock=write_lock,
                    emit_event=emit_batch_event,
                )
            except Exception as commit_error:
                logger.error(f"更新批量生成失败状态时提交失败: {str(commit_error)}")
    finally:
        if db_session:
            await db_session.close()
