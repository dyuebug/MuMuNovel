"""Test-only adapter for retired batch generation runtime orchestration."""
from __future__ import annotations

import asyncio
from contextlib import suppress
from dataclasses import dataclass
from datetime import datetime
from typing import TYPE_CHECKING, Any, Dict, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.retired_runtime_test_support import get_logger

if TYPE_CHECKING:
    from migrator_app.models.batch_generation_task import BatchGenerationTask
    from migrator_app.models.project import Project
    from tests.test_support.ai_gateway.ai_service import AIService
    from tests.test_support.story_packet_test_support import StoryPacket
    from tests.test_support.story_repair_payload_test_support import StoryRepairPayload

CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS = 10.0
logger = get_logger(__name__)


@dataclass(frozen=True)
class BatchGenerationExecutionInitialization:
    task: "BatchGenerationTask"
    project: "Project"
    batch_story_packet: "StoryPacket"
    task_base_quality_profile: Dict[str, Any]
    cached_analysis_quality_profile: Dict[str, Any]
    active_story_repair_state: Dict[str, Any]
    active_story_repair_payload: Optional["StoryRepairPayload"]
    stream_chunks: bool


async def analyze_chapter_background(**kwargs):
    from tests.test_support.manual_chapter_analysis_execution_test_support import (
        execute_chapter_analysis_background,
    )

    return await execute_chapter_analysis_background(**kwargs)


async def get_db_write_lock(user_id: str):
    from tests.test_support.manual_chapter_analysis_execution_test_support import (
        get_chapter_analysis_write_lock,
    )

    return await get_chapter_analysis_write_lock(user_id)


async def _resolve_story_repair_state_for_batch_service(*args, **kwargs):
    from tests.test_support.story_repair_payload_test_support import (
        resolve_generation_story_repair_state_for_batch,
    )

    return await resolve_generation_story_repair_state_for_batch(*args, **kwargs)


async def resolve_story_repair_state_for_batch(*args, **kwargs):
    return await _resolve_story_repair_state_for_batch_service(*args, **kwargs)


async def resolve_generation_story_repair_state_for_batch(*args, **kwargs):
    return await _resolve_story_repair_state_for_batch_service(*args, **kwargs)


def resolve_story_repair_prompt_kwargs(*args, **kwargs):
    from tests.test_support.story_repair_payload_test_support import (
        resolve_story_repair_prompt_kwargs as resolve_story_repair_prompt_kwargs_impl,
    )

    return resolve_story_repair_prompt_kwargs_impl(*args, **kwargs)


def clone_chapter_quality_profile(*args, **kwargs):
    from tests.test_support.chapter_prompt_quality_test_support import (
        clone_chapter_quality_profile as clone_chapter_quality_profile_impl,
    )

    return clone_chapter_quality_profile_impl(*args, **kwargs)


async def build_story_generation_packet_with_project_continuity(*args, **kwargs):
    from tests.test_support.story_continuity_ledger_test_support import (
        build_story_generation_packet_with_project_continuity as impl,
    )

    return await impl(*args, **kwargs)


async def resolve_chapter_quality_profile(*args, **kwargs):
    from tests.test_support.chapter_prompt_quality_test_support import (
        resolve_chapter_quality_profile as impl,
    )

    return await impl(*args, **kwargs)


async def mark_batch_generation_current_chapter(
    db_session: AsyncSession,
    *,
    task: "BatchGenerationTask",
    chapter_id: str,
    write_lock,
) -> None:
    async with write_lock:
        task.current_chapter_id = chapter_id
        task.current_retry_count = 0
        await db_session.commit()


async def handle_cancelled_batch_generation_execution(
    db_session: AsyncSession,
    *,
    task: "BatchGenerationTask",
    batch_id: str,
    write_lock,
    emit_event,
    refresh_before_commit: bool = False,
) -> None:
    if refresh_before_commit:
        await db_session.refresh(task)

    if task.status != "cancelled":
        async with write_lock:
            task.status = "cancelled"
            task.completed_at = datetime.now()
            task.current_chapter_id = None
            task.current_chapter_number = None
            await db_session.commit()

    await emit_event(
        {
            "type": "error",
            "error": "项目不存在",
            "code": 400,
            "phase": "cancelled",
        }
    )
    await emit_event({"type": "done"})


async def complete_batch_generation_execution(
    db_session: AsyncSession,
    *,
    task: "BatchGenerationTask",
    batch_id: str,
    write_lock,
    emit_event,
) -> None:
    async with write_lock:
        task.status = "completed"
        task.completed_at = datetime.now()
        task.current_chapter_id = None
        task.current_chapter_number = None
        await db_session.commit()

    await emit_event(
        {
            "type": "progress",
            "message": "批量生成完成",
            "progress": 100,
            "status": "success",
            "phase": "complete",
        }
    )
    await emit_event({"type": "done"})


async def fail_batch_generation_on_unhandled_exception(
    db_session: AsyncSession,
    *,
    task: "BatchGenerationTask",
    error: Exception,
    write_lock,
    emit_event,
) -> None:
    async with write_lock:
        task.status = "failed"
        task.error_message = str(error)[:500]
        task.completed_at = datetime.now()
        await db_session.commit()

    await emit_event(
        {
            "type": "error",
            "error": task.error_message or str(error),
            "code": 500,
            "phase": "failed",
        }
    )
    await emit_event({"type": "done"})


async def initialize_batch_generation_execution(
    db_session: AsyncSession,
    *,
    batch_id: str,
    user_id: str,
    write_lock,
    emit_event,
    story_packet: Optional["StoryPacket"],
    creative_mode: Optional[str],
    story_focus: Optional[str],
    plot_stage: Optional[str],
    story_creation_brief: Optional[str],
    quality_preset: Optional[str],
    quality_notes: Optional[str],
    base_quality_profile: Optional[Dict[str, Any]],
    story_repair_summary: Optional[str],
    story_repair_targets: Optional[list[str]],
    story_preserve_strengths: Optional[list[str]],
    clone_quality_profile_fn,
    resolve_story_repair_state_fn,
    sync_task_story_repair_state_fn,
):
    from sqlalchemy import select

    from migrator_app.models.batch_generation_task import BatchGenerationTask
    from migrator_app.models.project import Project

    task_result = await db_session.execute(
        select(BatchGenerationTask).where(BatchGenerationTask.id == batch_id)
    )
    task = task_result.scalar_one_or_none()
    if task is None:
        return None

    project_result = await db_session.execute(
        select(Project).where(Project.id == task.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project is None:
        async with write_lock:
            task.status = "failed"
            task.error_message = "项目不存在"
            task.completed_at = datetime.now()
            await db_session.commit()
        await emit_event(
            {
                "type": "error",
                "error": "项目不存在",
                "code": 400,
                "phase": "validation",
            }
        )
        await emit_event({"type": "done"})
        return None

    batch_story_packet = story_packet
    if batch_story_packet is None:
        batch_story_packet = await build_story_generation_packet_with_project_continuity(
            db_session,
            project,
            source_label="batch-generation-runtime",
            source=type(
                "_BatchRuntimeSource",
                (),
                {
                    "creative_mode": creative_mode,
                    "story_focus": story_focus,
                    "plot_stage": plot_stage,
                    "story_creation_brief": story_creation_brief,
                    "quality_preset": quality_preset,
                    "quality_notes": quality_notes,
                },
            )(),
        )

    task_base_quality_profile = clone_quality_profile_fn(base_quality_profile)
    if not task_base_quality_profile:
        task_base_quality_profile = await resolve_chapter_quality_profile(
            db_session=db_session,
            user_id=user_id,
            project=project,
            style_id=task.style_id,
            enable_mcp=True,
            prefer_project_default_style=not bool(task.style_id),
            log_prefix="批量生成运行时",
        )
    cached_analysis_quality_profile = clone_quality_profile_fn(task_base_quality_profile)

    active_story_repair_state = await resolve_story_repair_state_fn(
        db_session,
        project_id=project.id,
        before_chapter_number=task.start_chapter_number,
        story_repair_summary=story_repair_summary,
        story_repair_targets=story_repair_targets,
        story_preserve_strengths=story_preserve_strengths,
    )
    await sync_task_story_repair_state_fn(
        batch_id,
        story_repair_state=active_story_repair_state,
        db_session=db_session,
    )
    active_story_repair_payload = active_story_repair_state.get("payload")

    return BatchGenerationExecutionInitialization(
        task=task,
        project=project,
        batch_story_packet=batch_story_packet,
        task_base_quality_profile=(
            dict(task_base_quality_profile)
            if isinstance(task_base_quality_profile, dict)
            else {}
        ),
        cached_analysis_quality_profile=(
            dict(cached_analysis_quality_profile)
            if isinstance(cached_analysis_quality_profile, dict)
            else {}
        ),
        active_story_repair_state=(
            dict(active_story_repair_state)
            if isinstance(active_story_repair_state, dict)
            else {}
        ),
        active_story_repair_payload=active_story_repair_payload,
        stream_chunks=True,
    )


async def publish_task_stream_event_service(*args, **kwargs):
    from tests.test_support.task_system import publish_task_stream_event

    return await publish_task_stream_event(*args, **kwargs)


async def sync_task_story_repair_state(*args, **kwargs):
    from tests.test_support.task_system import (
        sync_task_story_repair_state as sync_task_story_repair_state_impl,
    )

    return await sync_task_story_repair_state_impl(*args, **kwargs)


async def publish_task_stream_event(*args, **kwargs):
    return await publish_task_stream_event_service(*args, **kwargs)


async def await_cancelable_batch_generation_result_with_default_poll_interval(
    *,
    generation_coro,
    task,
    db_session: AsyncSession,
    poll_interval_seconds: Optional[float] = None,
):
    if poll_interval_seconds is None:
        poll_interval_seconds = CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS
    return await await_cancelable_batch_generation_result(
        generation_coro=generation_coro,
        task=task,
        db_session=db_session,
        poll_interval_seconds=poll_interval_seconds,
    )


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
                if task.status == "cancelled":
                    generation_task.cancel()
                    with suppress(asyncio.CancelledError):
                        await generation_task
                    raise asyncio.CancelledError()
    finally:
        if not generation_task.done():
            generation_task.cancel()
            with suppress(asyncio.CancelledError):
                await generation_task


async def run_batch_chapter_analysis_with_background_seam(
    db_session: AsyncSession,
    *,
    write_lock,
    batch_id: str,
    chapter,
    user_id: str,
    project_id: str,
    retry_count: int,
    max_retries: int,
    ai_service: "AIService",
    quality_profile: Optional[Dict[str, object]] = None,
    story_packet: Optional["StoryPacket"] = None,
    generation_guidance=None,
    chapter_content_override: Optional[str] = None,
    chapter_word_count_override: Optional[int] = None,
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[list[str]] = None,
    story_preserve_strengths: Optional[list[str]] = None,
    story_repair_payload: Optional["StoryRepairPayload"] = None,
    analyze_chapter_background_fn=None,
) -> tuple[bool, Optional[str]]:
    from tests.test_support.analysis_task_test_support import (
        create_analysis_task_safely,
    )

    if analyze_chapter_background_fn is None:
        analyze_chapter_background_fn = analyze_chapter_background

    await publish_task_stream_event_service(
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

    while analysis_retry_count < 3:
        try:
            if analysis_retry_count > 0:
                logger.info(f"章节分析重试(第{analysis_retry_count}次): 第{chapter.chapter_number}章")

            async with write_lock:
                analysis_task = await create_analysis_task_safely(
                    db_session,
                    chapter_id=chapter.id,
                    user_id=user_id,
                    project_id=project_id,
                    log_context=f"batch:{batch_id}",
                )
            if analysis_task is None:
                return False, "Chapter or project was deleted before analysis"

            analysis_result = await analyze_chapter_background_fn(
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


async def execute_batch_generation_in_order_with_default_wiring(
    *,
    batch_id: str,
    user_id: str,
    ai_service,
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
    get_db_write_lock_fn=None,
    run_generation_fn=None,
    await_generation_result_fn=None,
    run_batch_analysis_fn=None,
    resolve_story_repair_state_fn=None,
    sync_task_story_repair_state_fn=None,
    publish_task_stream_event_fn=None,
):
    from tests.test_support.batch_generation_single_chapter_wiring_test_adapter import (
        generate_single_chapter_for_batch,
    )

    if get_db_write_lock_fn is None:
        get_db_write_lock_fn = get_db_write_lock
    if run_generation_fn is None:
        run_generation_fn = generate_single_chapter_for_batch
    if await_generation_result_fn is None:
        await_generation_result_fn = (
            await_cancelable_batch_generation_result_with_default_poll_interval
        )
    if run_batch_analysis_fn is None:
        run_batch_analysis_fn = run_batch_chapter_analysis_with_background_seam
    if resolve_story_repair_state_fn is None:
        resolve_story_repair_state_fn = resolve_generation_story_repair_state_for_batch
    if sync_task_story_repair_state_fn is None:
        sync_task_story_repair_state_fn = sync_task_story_repair_state
    if publish_task_stream_event_fn is None:
        publish_task_stream_event_fn = publish_task_stream_event

    return await execute_batch_generation_in_order_workflow(
        batch_id=batch_id,
        user_id=user_id,
        ai_service=ai_service,
        custom_model=custom_model,
        temp_narrative_perspective=temp_narrative_perspective,
        story_packet=story_packet,
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
        story_repair_payload=story_repair_payload,
        base_quality_profile=base_quality_profile,
        get_db_write_lock_fn=get_db_write_lock_fn,
        resolve_story_repair_prompt_kwargs_fn=resolve_story_repair_prompt_kwargs,
        clone_quality_profile_fn=clone_chapter_quality_profile,
        resolve_story_repair_state_fn=resolve_story_repair_state_fn,
        sync_task_story_repair_state_fn=sync_task_story_repair_state_fn,
        run_generation_fn=run_generation_fn,
        await_generation_result_fn=await_generation_result_fn,
        run_batch_analysis_fn=run_batch_analysis_fn,
        publish_task_stream_event_fn=publish_task_stream_event_fn,
    )


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
    get_db_write_lock_fn=None,
    resolve_story_repair_prompt_kwargs_fn=None,
    clone_quality_profile_fn=None,
    resolve_story_repair_state_fn=None,
    sync_task_story_repair_state_fn=None,
    run_generation_fn=None,
    await_generation_result_fn=None,
    run_batch_analysis_fn=None,
    publish_task_stream_event_fn=None,
):
    from tests.test_support.database_test_support import get_engine

    from tests.test_support.batch_generation_retry_test_adapter import (
        BatchGenerationChapterRuntimeState,
        BatchGenerationExecutionEnvironment,
        execute_batch_generation_chapter_with_retries,
    )
    from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

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
            if task.status == "cancelled":
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
            except Exception:
                pass
        return
    except Exception as error:
        logger.exception("Batch generation execution failed")
        if db_session and task and emit_batch_event is not None:
            await fail_batch_generation_on_unhandled_exception(
                db_session,
                task=task,
                error=error,
                write_lock=write_lock,
                emit_event=emit_batch_event,
            )
    finally:
        if db_session is not None:
            await db_session.close()




