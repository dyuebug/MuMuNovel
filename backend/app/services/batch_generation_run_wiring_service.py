from __future__ import annotations

from typing import Any, Dict, Optional

from app.services.batch_generation_run_service import execute_batch_generation_in_order_workflow
from app.services.chapter_quality_context_service import StoryPacket, clone_chapter_quality_profile
from app.services.story_repair_payload_service import (
    StoryRepairPayload,
    resolve_generation_story_repair_state_for_batch,
    resolve_story_repair_prompt_kwargs,
)
from app.services.task_workflow_runtime_service import (
    publish_task_stream_event,
    sync_task_story_repair_state,
)


async def execute_batch_generation_in_order_with_default_wiring(
    *,
    batch_id: str,
    user_id: str,
    ai_service,
    custom_model: Optional[str] = None,
    temp_narrative_perspective: Optional[str] = None,
    story_packet: Optional[StoryPacket] = None,
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
    story_repair_payload: Optional[StoryRepairPayload] = None,
    base_quality_profile: Optional[Dict[str, Any]] = None,
    get_db_write_lock_fn,
    run_generation_fn,
    await_generation_result_fn,
    run_batch_analysis_fn,
    resolve_story_repair_state_fn=resolve_generation_story_repair_state_for_batch,
    sync_task_story_repair_state_fn=sync_task_story_repair_state,
    publish_task_stream_event_fn=publish_task_stream_event,
):
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
