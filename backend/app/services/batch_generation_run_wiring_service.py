from __future__ import annotations

from typing import TYPE_CHECKING, Any, Dict, Optional

from sqlalchemy.ext.asyncio import AsyncSession

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation route/read/runtime chain and this "
    "default-import wiring file is retained only as frozen "
    "rollback/source-map material after the batch retired-wiring closeout "
    "review."
)
SOURCE_MAP_RUST_OWNER = "backend-rs/src/api/health.rs"
SOURCE_MAP_ROLLBACK_FLAG = "aggregate_chapters_python_source_map"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from app.services.ai_service import AIService
    from app.services.chapter_quality_context_service import StoryPacket
    from app.services.story_repair_payload_service import StoryRepairPayload

CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS = 10.0


async def analyze_chapter_background(**kwargs):
    from app.services.manual_chapter_analysis_execution_service import (
        execute_chapter_analysis_background,
    )

    return await execute_chapter_analysis_background(**kwargs)


async def get_db_write_lock(user_id: str):
    from app.services.batch_generation_run_service import get_db_write_lock as get_db_write_lock_service

    return await get_db_write_lock_service(user_id)


async def _resolve_story_repair_state_for_batch_service(*args, **kwargs):
    from app.services.story_repair_payload_service import (
        resolve_generation_story_repair_state_for_batch,
    )

    return await resolve_generation_story_repair_state_for_batch(*args, **kwargs)


async def resolve_story_repair_state_for_batch(*args, **kwargs):
    return await _resolve_story_repair_state_for_batch_service(*args, **kwargs)


async def resolve_generation_story_repair_state_for_batch(*args, **kwargs):
    return await _resolve_story_repair_state_for_batch_service(*args, **kwargs)


def resolve_story_repair_prompt_kwargs(*args, **kwargs):
    from app.services.story_repair_payload_service import (
        resolve_story_repair_prompt_kwargs as resolve_story_repair_prompt_kwargs_impl,
    )

    return resolve_story_repair_prompt_kwargs_impl(*args, **kwargs)


def clone_chapter_quality_profile(*args, **kwargs):
    from app.services.chapter_quality_context_service import (
        clone_chapter_quality_profile as clone_chapter_quality_profile_impl,
    )

    return clone_chapter_quality_profile_impl(*args, **kwargs)


async def execute_batch_generation_in_order_workflow(*args, **kwargs):
    from app.services.batch_generation_run_service import (
        execute_batch_generation_in_order_workflow as execute_batch_generation_in_order_workflow_impl,
    )

    return await execute_batch_generation_in_order_workflow_impl(*args, **kwargs)


async def publish_task_stream_event_service(*args, **kwargs):
    from app.services.task_workflow_runtime_service import publish_task_stream_event

    return await publish_task_stream_event(*args, **kwargs)


async def sync_task_story_repair_state(*args, **kwargs):
    from app.services.task_workflow_runtime_service import (
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
    from app.services.batch_generation_run_service import (
        await_cancelable_batch_generation_result,
    )

    if poll_interval_seconds is None:
        poll_interval_seconds = CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS
    return await await_cancelable_batch_generation_result(
        generation_coro=generation_coro,
        task=task,
        db_session=db_session,
        poll_interval_seconds=poll_interval_seconds,
    )


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
    from app.services.batch_generation_analysis_service import (
        run_batch_chapter_analysis,
    )

    if analyze_chapter_background_fn is None:
        analyze_chapter_background_fn = analyze_chapter_background

    return await run_batch_chapter_analysis(
        db_session,
        write_lock=write_lock,
        batch_id=batch_id,
        chapter=chapter,
        user_id=user_id,
        project_id=project_id,
        retry_count=retry_count,
        max_retries=max_retries,
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
        analyze_chapter_background_fn=analyze_chapter_background_fn,
    )


async def run_single_chapter_generation_with_default_entry_seam(**kwargs):
    from app.services.batch_generation_single_chapter_entry_service import (
        generate_single_chapter_for_batch,
    )

    return await generate_single_chapter_for_batch(**kwargs)


async def execute_batch_generation_in_order_with_entry_service_seams(
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
):
    from app.services.task_workflow_runtime_service import sync_task_story_repair_state

    return await execute_batch_generation_in_order_with_default_wiring(
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
        get_db_write_lock_fn=get_db_write_lock,
        run_generation_fn=run_single_chapter_generation_with_default_entry_seam,
        await_generation_result_fn=await_cancelable_batch_generation_result_with_default_poll_interval,
        run_batch_analysis_fn=run_batch_chapter_analysis_with_background_seam,
        resolve_story_repair_state_fn=resolve_story_repair_state_for_batch,
        sync_task_story_repair_state_fn=sync_task_story_repair_state,
        publish_task_stream_event_fn=publish_task_stream_event_service,
    )


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
    get_db_write_lock_fn,
    run_generation_fn,
    await_generation_result_fn,
    run_batch_analysis_fn,
    resolve_story_repair_state_fn=None,
    sync_task_story_repair_state_fn=None,
    publish_task_stream_event_fn=None,
):
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
