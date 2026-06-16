"""批量生成执行入口冻结 shim。

该文件保留给 rollback/source-map 和历史 patch surface 使用，
真实 owner 已收口到 batch run wiring / task runtime / story repair 模块。
"""
from __future__ import annotations

from typing import TYPE_CHECKING

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch route/runtime chain; this Python entry module "
    "is kept only as frozen rollback/source-map material for aggregate and "
    "legacy route fallback wiring."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/chapter_batch_generation.rs; "
    "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "legacy_batch_generation_python_routes_enabled"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

if TYPE_CHECKING:
    from app.services.ai_service import AIService
    from app.services.chapter_quality_context_service import StoryPacket
    from app.services.story_repair_payload_service import StoryRepairPayload


CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS = 10.0


def _run_wiring_service():
    from app.services import batch_generation_run_wiring_service

    return batch_generation_run_wiring_service


def _task_runtime_service():
    from app.services import task_workflow_runtime_service

    return task_workflow_runtime_service


def _story_repair_payload_service():
    from app.services import story_repair_payload_service

    return story_repair_payload_service


async def analyze_chapter_background(**kwargs):
    return await _run_wiring_service().analyze_chapter_background(**kwargs)


async def get_db_write_lock(user_id: str):
    return await _run_wiring_service().get_db_write_lock(user_id)


async def resolve_story_repair_state_for_batch(*args, **kwargs):
    return await _story_repair_payload_service().resolve_generation_story_repair_state_for_batch(
        *args,
        **kwargs,
    )


async def sync_task_story_repair_state(*args, **kwargs):
    return await _task_runtime_service().sync_task_story_repair_state(*args, **kwargs)


async def publish_task_stream_event_service(*args, **kwargs):
    return await _task_runtime_service().publish_task_stream_event(*args, **kwargs)


async def _await_cancelable_batch_generation_result(
    *,
    generation_coro,
    task,
    db_session,
    poll_interval_seconds=None,
):
    return await _run_wiring_service().await_cancelable_batch_generation_result_with_default_poll_interval(
        generation_coro=generation_coro,
        task=task,
        db_session=db_session,
        poll_interval_seconds=(
            CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS
            if poll_interval_seconds is None
            else poll_interval_seconds
        ),
    )


async def _run_batch_chapter_analysis(
    db_session,
    *,
    write_lock,
    batch_id: str,
    chapter,
    user_id: str,
    project_id: str,
    retry_count: int,
    max_retries: int,
    ai_service: "AIService",
    quality_profile=None,
    story_packet: "StoryPacket" | None = None,
    generation_guidance=None,
    chapter_content_override=None,
    chapter_word_count_override=None,
    story_repair_summary=None,
    story_repair_targets=None,
    story_preserve_strengths=None,
    story_repair_payload: "StoryRepairPayload" | None = None,
):
    return await _run_wiring_service().run_batch_chapter_analysis_with_background_seam(
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
        analyze_chapter_background_fn=analyze_chapter_background,
    )


async def execute_batch_generation_in_order(
    batch_id: str,
    user_id: str,
    ai_service: "AIService",
    custom_model=None,
    temp_narrative_perspective=None,
    story_packet: "StoryPacket" | None = None,
    creative_mode=None,
    story_focus=None,
    plot_stage=None,
    story_creation_brief=None,
    quality_preset=None,
    quality_notes=None,
    enable_web_research=None,
    web_research_query=None,
    story_repair_summary=None,
    story_repair_targets=None,
    story_preserve_strengths=None,
    story_repair_payload: "StoryRepairPayload" | None = None,
    base_quality_profile=None,
):
    return await _run_wiring_service().execute_batch_generation_in_order_with_entry_service_seams(
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
    )


async def _run_single_chapter_generation(**kwargs):
    return await _run_wiring_service().run_single_chapter_generation_with_default_entry_seam(
        **kwargs,
    )
