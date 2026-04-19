from __future__ import annotations

import asyncio
from typing import Any, AsyncIterator, Awaitable, Callable, Dict, List, Optional, Sequence

from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.chapter import Chapter
from app.models.project import Project
from app.services.chapter_generation_stream_models import (
    ChapterGenerationAnalysisFollowupPlan,
    ChapterGenerationAnalysisScheduling,
    ChapterGenerationEmissionStep,
    ChapterGenerationPostPersistEffects,
    ChapterGenerationStreamFinalizeDependencies,
    ChapterGenerationStreamResponseArtifacts,
)

logger = get_logger(__name__)


def build_chapter_generation_analysis_followup_plan(
    *,
    enable_analysis: bool,
    quality_gate_action: Optional[str],
    quality_gate_requires_followup: bool,
    full_content: str,
    candidate_word_count: int,
) -> ChapterGenerationAnalysisFollowupPlan:
    resolved_action = str(quality_gate_action or "continue")
    should_schedule_analysis = bool(enable_analysis or quality_gate_requires_followup)

    analysis_reason: Optional[str] = None
    if should_schedule_analysis:
        analysis_reason = "manual_analysis" if enable_analysis else "quality_gate_followup"
        if resolved_action == "retry":
            analysis_reason = "quality_gate_auto_repair"
        elif resolved_action == "manual_review":
            analysis_reason = "quality_gate_manual_review"

    completion_message = "章节生成完成"
    if resolved_action == "retry":
        completion_message = "章节生成完成，已转入质量修复"
    elif resolved_action == "manual_review":
        completion_message = "章节生成完成，已转入人工复核"

    analysis_started_message: Optional[str] = None
    if should_schedule_analysis:
        analysis_started_message = "章节分析任务已启动"
        if resolved_action == "retry":
            analysis_started_message = "质量修复分析任务已启动"
        elif resolved_action == "manual_review":
            analysis_started_message = "人工复核分析任务已启动"

    return ChapterGenerationAnalysisFollowupPlan(
        should_schedule_analysis=should_schedule_analysis,
        analysis_reason=analysis_reason,
        chapter_content_override=full_content if quality_gate_requires_followup else None,
        chapter_word_count_override=candidate_word_count if quality_gate_requires_followup else None,
        completion_message=completion_message,
        analysis_started_message=analysis_started_message,
    )


def build_chapter_generation_stream_response_artifacts(
    *,
    chapter: Chapter,
    draft_attempt: Any,
    quality_metrics: Optional[Dict[str, Any]],
    quality_gate_action: Optional[str],
    quality_gate_message: Optional[str],
    quality_gate_snapshot: Optional[Dict[str, Any]],
    quality_gate_requires_followup: bool,
    content_applied: bool,
    saved_word_count: int,
    task_id: Optional[str],
    story_runtime_contract: Optional[Dict[str, Any]],
    analysis_started_message: Optional[str],
    build_candidate_draft_payload_fn: Callable[..., Optional[Dict[str, Any]]],
    build_stream_result_payload_fn: Callable[..., Dict[str, Any]],
) -> ChapterGenerationStreamResponseArtifacts:
    candidate_draft_summary = None
    if quality_gate_requires_followup and draft_attempt is not None:
        candidate_draft_summary = build_candidate_draft_payload_fn(
            draft_attempt=draft_attempt,
            chapter_updated_at=chapter.updated_at,
            include_full_text=False,
        )

    quality_metrics_event_payload: Dict[str, Any] = {
        "type": "quality_metrics",
        "chapter_id": chapter.id,
        "chapter_number": chapter.chapter_number,
    }
    if isinstance(quality_metrics, dict):
        quality_metrics_event_payload.update(quality_metrics)

    quality_gate_event_payload = None
    if quality_gate_requires_followup:
        resolved_action = str(quality_gate_action or "continue")
        quality_gate_event_payload = {
            "type": "quality_gate_retry" if resolved_action == "retry" else "quality_gate_blocked",
            "chapter_id": chapter.id,
            "chapter_number": chapter.chapter_number,
            "message": quality_gate_message,
            "progress": 88 if resolved_action == "retry" else 95,
            "quality_gate": quality_gate_snapshot if isinstance(quality_gate_snapshot, dict) else None,
        }

    result_payload = build_stream_result_payload_fn(
        word_count=saved_word_count,
        analysis_task_id=task_id,
        quality_metrics=quality_metrics if isinstance(quality_metrics, dict) else None,
        quality_gate_action=quality_gate_action,
        quality_gate_message=quality_gate_message,
        content_applied=content_applied,
        chapter_status=chapter.status or "draft",
        saved_word_count=saved_word_count,
        hard_gate_blocked=quality_gate_requires_followup,
        story_runtime_contract=story_runtime_contract,
        candidate_draft=candidate_draft_summary,
    )

    analysis_started_event_data = None
    if task_id and analysis_started_message:
        analysis_started_event_data = {
            "task_id": task_id,
            "message": analysis_started_message,
        }

    return ChapterGenerationStreamResponseArtifacts(
        quality_metrics_event_payload=quality_metrics_event_payload,
        quality_gate_event_payload=quality_gate_event_payload,
        result_payload=result_payload,
        analysis_started_event_data=analysis_started_event_data,
    )


async def prepare_chapter_generation_analysis_scheduling(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    user_id: str,
    project_id: str,
    followup_plan: ChapterGenerationAnalysisFollowupPlan,
    ai_service: Any,
    quality_profile: Dict[str, Any],
    story_packet: Any,
    create_analysis_task_fn: Callable[..., Awaitable[Any]],
) -> ChapterGenerationAnalysisScheduling:
    if not followup_plan.should_schedule_analysis:
        return ChapterGenerationAnalysisScheduling(
            task_id=None,
            background_task_kwargs=None,
        )

    analysis_task = await create_analysis_task_fn(
        db_session,
        chapter_id=chapter_id,
        user_id=user_id,
        project_id=project_id,
        log_context=f"stream:{followup_plan.analysis_reason}",
    )
    task_id = getattr(analysis_task, "id", None) if analysis_task is not None else None
    return ChapterGenerationAnalysisScheduling(
        task_id=task_id,
        background_task_kwargs={
            "chapter_id": chapter_id,
            "user_id": user_id,
            "project_id": project_id,
            "task_id": task_id,
            "ai_service": ai_service,
            "quality_profile": quality_profile,
            "story_packet": story_packet,
            "chapter_content_override": followup_plan.chapter_content_override,
            "chapter_word_count_override": followup_plan.chapter_word_count_override,
        },
    )


async def run_chapter_generation_post_persist_effects(
    db_session: AsyncSession,
    *,
    chapter_id: str,
    chapter: Chapter,
    project: Project,
    full_content: str,
    candidate_word_count: int,
    content_applied: bool,
    provisional_draft_saved: bool,
    previous_status: Optional[str],
    auto_plant_pending_foreshadows_fn: Callable[..., Awaitable[Dict[str, Any]]],
) -> ChapterGenerationPostPersistEffects:
    if content_applied:
        logger.info(f"✅ 章节 {chapter_id} 已保存，共 {candidate_word_count} 字")
    elif provisional_draft_saved:
        logger.info(f"⚠️ 章节 {chapter_id} 已保存候选草稿，共 {candidate_word_count} 字")
    else:
        logger.info(
            f"⚠️ 章节 {chapter_id} 未落库，保留候选草稿，共 {candidate_word_count} 字，previous_status={previous_status}"
        )

    planted_count = 0
    plant_error: Optional[str] = None
    if content_applied:
        try:
            plant_result = await auto_plant_pending_foreshadows_fn(
                db=db_session,
                project_id=project.id,
                chapter_id=chapter_id,
                chapter_number=chapter.chapter_number,
                chapter_content=full_content,
            )
            planted_count = int((plant_result or {}).get("planted_count") or 0)
            if planted_count > 0:
                logger.info(f"✅ 已成功埋入伏笔: {planted_count}")
        except Exception as exc:
            plant_error = str(exc)
            logger.warning(f"⚠️ 自动埋入伏笔失败: {plant_error}")

    return ChapterGenerationPostPersistEffects(
        planted_count=planted_count,
        plant_error=plant_error,
    )


def build_chapter_generation_stream_emission_plan(
    *,
    completion_message: str,
    response_artifacts: ChapterGenerationStreamResponseArtifacts,
) -> List[ChapterGenerationEmissionStep]:
    steps: List[ChapterGenerationEmissionStep] = [
        ChapterGenerationEmissionStep(kind="tracker_complete", message=completion_message),
        ChapterGenerationEmissionStep(kind="sse_payload", payload=response_artifacts.quality_metrics_event_payload),
    ]
    if response_artifacts.quality_gate_event_payload:
        steps.append(
            ChapterGenerationEmissionStep(
                kind="sse_payload",
                payload=response_artifacts.quality_gate_event_payload,
            )
        )
    steps.append(
        ChapterGenerationEmissionStep(kind="tracker_result", payload=response_artifacts.result_payload)
    )
    if response_artifacts.analysis_started_event_data:
        steps.append(
            ChapterGenerationEmissionStep(
                kind="sse_event",
                event="analysis_started",
                payload=response_artifacts.analysis_started_event_data,
            )
        )
    steps.append(ChapterGenerationEmissionStep(kind="tracker_done"))
    return steps


async def emit_chapter_generation_stream_plan(
    *,
    emission_plan: Sequence[ChapterGenerationEmissionStep],
    tracker_complete_fn: Callable[[str], Awaitable[Any]],
    tracker_result_fn: Callable[[Dict[str, Any]], Awaitable[Any]],
    tracker_done_fn: Callable[[], Awaitable[Any]],
    format_sse_fn: Callable[[Dict[str, Any]], Any],
    send_event_fn: Callable[..., Awaitable[Any]],
) -> AsyncIterator[Any]:
    for emission_step in emission_plan:
        if emission_step.kind == "tracker_complete":
            yield await tracker_complete_fn(emission_step.message or "")
        elif emission_step.kind == "sse_payload":
            yield format_sse_fn(emission_step.payload or {})
        elif emission_step.kind == "tracker_result":
            yield await tracker_result_fn(emission_step.payload or {})
        elif emission_step.kind == "sse_event":
            yield await send_event_fn(
                event=emission_step.event or "message",
                data=emission_step.payload or {},
            )
        elif emission_step.kind == "tracker_done":
            yield await tracker_done_fn()



async def finalize_chapter_generation_stream_result(
    *,
    db_session: AsyncSession,
    chapter_id: str,
    current_user_id: str,
    background_tasks: Any,
    user_ai_service: Any,
    enable_analysis: bool,
    execution_setup: Any,
    candidate_stage_result: Any,
    dependencies: ChapterGenerationStreamFinalizeDependencies,
    emit_saving_fn: Callable[[str, float], Awaitable[Any]],
    apply_outcome_and_build_history_fn: Callable[..., Any],
) -> tuple[Any, List[ChapterGenerationEmissionStep]]:
    saving_payload = await emit_saving_fn("Saving chapter content and quality results...", 0.3)
    persistence_preparation = apply_outcome_and_build_history_fn(
        chapter=execution_setup.current_chapter,
        project=execution_setup.project,
        outcome=candidate_stage_result.selected_candidate_outcome,
        story_runtime_contract=execution_setup.story_runtime_contract,
        build_generation_history_payload_fn=dependencies.build_generation_history_payload_fn,
        history_model="default",
    )
    provisional_draft_saved = persistence_preparation.provisional_draft_saved
    db_session.add(persistence_preparation.history)
    if candidate_stage_result.draft_attempt is not None:
        db_session.add(candidate_stage_result.draft_attempt)
    await db_session.commit()
    await db_session.refresh(execution_setup.current_chapter)

    await run_chapter_generation_post_persist_effects(
        db_session,
        chapter_id=chapter_id,
        chapter=execution_setup.current_chapter,
        project=execution_setup.project,
        full_content=candidate_stage_result.full_content,
        candidate_word_count=candidate_stage_result.candidate_word_count,
        content_applied=candidate_stage_result.content_applied,
        provisional_draft_saved=provisional_draft_saved,
        previous_status=candidate_stage_result.previous_status,
        auto_plant_pending_foreshadows_fn=dependencies.foreshadow_service.auto_plant_pending_foreshadows,
    )

    followup_plan = build_chapter_generation_analysis_followup_plan(
        enable_analysis=enable_analysis,
        quality_gate_action=candidate_stage_result.quality_gate_action,
        quality_gate_requires_followup=candidate_stage_result.quality_gate_requires_followup,
        full_content=candidate_stage_result.full_content,
        candidate_word_count=candidate_stage_result.candidate_word_count,
    )
    analysis_scheduling = await prepare_chapter_generation_analysis_scheduling(
        db_session,
        chapter_id=chapter_id,
        user_id=current_user_id,
        project_id=execution_setup.project.id,
        followup_plan=followup_plan,
        ai_service=user_ai_service,
        quality_profile=execution_setup.quality_profile,
        story_packet=execution_setup.story_packet,
        create_analysis_task_fn=dependencies.create_analysis_task_fn,
    )
    task_id = analysis_scheduling.task_id
    if analysis_scheduling.background_task_kwargs is not None:
        if task_id is not None:
            logger.info(f"Created analysis task: {task_id} (reason={followup_plan.analysis_reason})")

        await asyncio.sleep(0.05)
        background_tasks.add_task(
            dependencies.analyze_chapter_background_fn,
            **analysis_scheduling.background_task_kwargs,
        )
    else:
        logger.info("No follow-up analysis scheduled")

    response_artifacts = build_chapter_generation_stream_response_artifacts(
        chapter=execution_setup.current_chapter,
        draft_attempt=candidate_stage_result.draft_attempt,
        quality_metrics=candidate_stage_result.quality_metrics if isinstance(candidate_stage_result.quality_metrics, dict) else None,
        quality_gate_action=candidate_stage_result.quality_gate_action,
        quality_gate_message=candidate_stage_result.quality_gate_message,
        quality_gate_snapshot=candidate_stage_result.quality_gate_snapshot,
        quality_gate_requires_followup=candidate_stage_result.quality_gate_requires_followup,
        content_applied=candidate_stage_result.content_applied,
        saved_word_count=execution_setup.current_chapter.word_count or 0,
        task_id=task_id,
        story_runtime_contract=execution_setup.story_runtime_contract,
        analysis_started_message=followup_plan.analysis_started_message,
        build_candidate_draft_payload_fn=dependencies.build_candidate_draft_payload_fn,
        build_stream_result_payload_fn=dependencies.build_stream_result_payload_fn,
    )
    emission_plan = build_chapter_generation_stream_emission_plan(
        completion_message=followup_plan.completion_message,
        response_artifacts=response_artifacts,
    )
    return saving_payload, emission_plan
