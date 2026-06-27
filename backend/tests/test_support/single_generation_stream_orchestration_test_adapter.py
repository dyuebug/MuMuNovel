from __future__ import annotations

import asyncio
import inspect
from typing import Any, Callable, Optional

from tests.test_support.chapter_generation_stream_types import (
    ChapterGenerationAnalysisFollowupPlan,
    ChapterGenerationAnalysisScheduling,
    ChapterGenerationEmissionStep,
    ChapterGenerationPersistencePreparation,
    ChapterGenerationPostPersistEffects,
    ChapterGenerationStreamCandidateStageResult,
    ChapterGenerationStreamBuiltContext,
    ChapterGenerationStreamExecutionSetup,
    ChapterGenerationStreamPrompt,
    ChapterGenerationStreamRequestPayload,
    ChapterGenerationStreamRuntimeContext,
    ChapterGenerationStreamResponseArtifacts,
)


async def _resolve_maybe_await(result: Any) -> Any:
    if inspect.isawaitable(result):
        return await result
    return result


async def _load_generation_outline(
    db_session,
    *,
    chapter,
):
    from migrator_app.models.outline import Outline
    from sqlalchemy import select

    if chapter.outline_id:
        outline_result = await db_session.execute(
            select(Outline)
            .where(Outline.id == chapter.outline_id)
            .execution_options(populate_existing=True)
        )
        return outline_result.scalar_one_or_none()

    outline_result = await db_session.execute(
        select(Outline)
        .where(Outline.project_id == chapter.project_id)
        .where(Outline.order_index == chapter.chapter_number)
        .execution_options(populate_existing=True)
    )
    return outline_result.scalar_one_or_none()


def _resolve_chapter_perspective(
    *,
    project,
    temp_narrative_perspective: Optional[str],
) -> str:
    return temp_narrative_perspective or project.narrative_perspective or "第三人称"


async def build_chapter_generation_event_stream_with_explicit_wiring(
    *,
    db_session_source: Callable[[], Any],
    chapter_id: str,
    current_user_id: str,
    generate_request: Any,
    background_tasks: Any,
    user_ai_service: Any,
    target_word_count: int,
    enable_analysis: bool,
    heartbeat_interval_seconds: float,
    custom_model: Optional[str],
    temp_narrative_perspective: Optional[str],
    style_id: Optional[int],
    dependencies: Any,
    prepare_stream_execution_fn,
    execute_candidate_stage_fn,
    finalize_stream_result_fn,
    emit_stream_plan_fn,
    tracker_factory,
    format_sse_fn,
    send_event_fn,
    build_progress_kwargs_fn,
    result_type,
):
    from tests.test_support.retired_runtime_test_support import get_logger

    logger = get_logger(__name__)
    db_session = None
    db_committed = False
    tracker = tracker_factory("章节生成")

    try:
        yield await tracker.start()

        async for db_session in db_session_source():
            yield await tracker.loading("Loading generation context...", 0.2)

            try:
                execution_setup = await prepare_stream_execution_fn(
                    db_session=db_session,
                    chapter_id=chapter_id,
                    current_user_id=current_user_id,
                    generate_request=generate_request,
                    user_ai_service=user_ai_service,
                    target_word_count=target_word_count,
                    custom_model=custom_model,
                    temp_narrative_perspective=temp_narrative_perspective,
                    style_id=style_id,
                    dependencies=dependencies.execution,
                )
            except ValueError as exc:
                detail = str(exc)
                error_code = 404 if ("未找到" in detail or "不存在" in detail) else 400
                yield await tracker.error(detail, error_code)
                return

            yield await tracker.loading("Chapter context built", 0.8)
            yield await tracker.preparing("Preparing AI prompts...")
            logger.info("Starting chapter stream generation: %s", chapter_id)
            yield await tracker.generating(
                current_chars=0,
                estimated_total=target_word_count,
            )

            candidate_stage_result = await execute_candidate_stage_fn(
                chapter_id=chapter_id,
                user_ai_service=user_ai_service,
                target_word_count=target_word_count,
                heartbeat_interval_seconds=heartbeat_interval_seconds,
                execution_setup=execution_setup,
                dependencies=dependencies.candidate,
                emit_generating_fn=lambda **kwargs: tracker.generating(**kwargs),
                emit_heartbeat_fn=tracker.heartbeat,
                emit_chunk_fn=tracker.generating_chunk,
                build_progress_kwargs_fn=build_progress_kwargs_fn,
                result_type=result_type,
            )
            for chunk_payload in candidate_stage_result.chunk_payloads:
                yield chunk_payload

            saving_payload, emission_plan = await finalize_stream_result_fn(
                db_session=db_session,
                chapter_id=chapter_id,
                current_user_id=current_user_id,
                background_tasks=background_tasks,
                user_ai_service=user_ai_service,
                enable_analysis=enable_analysis,
                execution_setup=execution_setup,
                candidate_stage_result=candidate_stage_result,
                dependencies=dependencies.finalize,
                emit_saving_fn=tracker.saving,
            )
            yield saving_payload
            db_committed = True

            async for emitted_payload in emit_stream_plan_fn(
                emission_plan=emission_plan,
                tracker_complete_fn=tracker.complete,
                tracker_result_fn=tracker.result,
                tracker_done_fn=tracker.done,
                format_sse_fn=format_sse_fn,
                send_event_fn=send_event_fn,
            ):
                yield emitted_payload

            break

    except GeneratorExit:
        logger.warning("Chapter stream generator closed early (SSE disconnect)")
        db_session = None
    except Exception as exc:
        logger.error("Chapter stream generation failed: %s", exc)
        if db_session and not db_committed:
            try:
                if db_session.in_transaction():
                    await db_session.rollback()
                    logger.info("Rolled back uncommitted chapter stream transaction")
            except Exception as rollback_error:
                logger.error("Chapter stream rollback failed: %s", rollback_error)
        db_session = None
        yield await tracker.error(str(exc))
    finally:
        if db_session:
            db_session = None


async def build_chapter_generation_event_stream_with_default_wiring(
    *,
    db_session_source: Callable[[], Any],
    chapter_id: str,
    current_user_id: str,
    generate_request: Any,
    background_tasks: Any,
    user_ai_service: Any,
    target_word_count: int,
    enable_analysis: bool,
    heartbeat_interval_seconds: float,
    custom_model: Optional[str],
    temp_narrative_perspective: Optional[str],
    style_id: Optional[int],
    dependencies: Any,
):
    from tests.test_support.batch_generation_single_chapter_wiring_test_adapter import (
        build_chapter_generation_progress_kwargs,
    )
    from tests.test_support.single_generation_stream_candidate_test_adapter import (
        execute_chapter_generation_candidate_stage,
    )
    from tests.test_support.utils.sse_response import SSEResponse, WizardProgressTracker

    async for payload in build_chapter_generation_event_stream_with_explicit_wiring(
        db_session_source=db_session_source,
        chapter_id=chapter_id,
        current_user_id=current_user_id,
        generate_request=generate_request,
        background_tasks=background_tasks,
        user_ai_service=user_ai_service,
        target_word_count=target_word_count,
        enable_analysis=enable_analysis,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
        custom_model=custom_model,
        temp_narrative_perspective=temp_narrative_perspective,
        style_id=style_id,
        dependencies=dependencies,
        prepare_stream_execution_fn=prepare_chapter_generation_stream_execution,
        execute_candidate_stage_fn=execute_chapter_generation_candidate_stage,
        finalize_stream_result_fn=finalize_chapter_generation_stream_result,
        emit_stream_plan_fn=emit_chapter_generation_stream_plan,
        tracker_factory=WizardProgressTracker,
        format_sse_fn=SSEResponse.format_sse,
        send_event_fn=SSEResponse.send_event,
        build_progress_kwargs_fn=build_chapter_generation_progress_kwargs,
        result_type=ChapterGenerationStreamCandidateStageResult,
    ):
        yield payload


async def _default_resolve_quality_profile(*args, **kwargs):
    from tests.test_support.chapter_prompt_quality_test_support import (
        resolve_chapter_quality_profile,
    )

    if args:
        kwargs = {"db_session": args[0], **kwargs}

    return await resolve_chapter_quality_profile(**kwargs)


async def _default_build_story_packet(*args, **kwargs):
    from tests.test_support.story_continuity_ledger_test_support import (
        build_story_generation_packet_with_project_continuity,
    )

    return await build_story_generation_packet_with_project_continuity(
        *args,
        **kwargs,
    )


async def load_chapter_generation_stream_runtime_context(
    db_session,
    *,
    chapter_id: str,
    user_id: str,
    generate_request: Any,
    style_id: Optional[int],
    resolve_story_repair_state_fn,
    cancel_outline_postprocess_tasks_fn,
    resolve_quality_profile_fn: Optional[Callable[..., Any]] = None,
    build_story_packet_fn: Optional[Callable[..., Any]] = None,
) -> ChapterGenerationStreamRuntimeContext:
    from migrator_app.models.chapter import Chapter
    from migrator_app.models.project import Project
    from tests.test_support.story_continuity_ledger_test_support import (
        build_story_generation_packet_with_project_continuity,
    )
    from tests.test_support.chapter_prompt_quality_test_support import (
        resolve_chapter_quality_profile,
    )
    from sqlalchemy import select

    chapter_result = await db_session.execute(
        select(Chapter).where(Chapter.id == chapter_id)
    )
    chapter = chapter_result.scalar_one_or_none()
    if chapter is None:
        raise ValueError("章节不存在")

    project_result = await db_session.execute(
        select(Project).where(Project.id == chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project is None:
        raise ValueError("项目不存在")

    outline_mode = project.outline_mode or "one-to-many"
    cancel_outline_postprocess_tasks_fn(chapter.project_id)
    outline = await _load_generation_outline(db_session, chapter=chapter)
    quality_profile_resolver = (
        resolve_quality_profile_fn or resolve_chapter_quality_profile
    )
    story_packet_builder = (
        build_story_packet_fn
        or build_story_generation_packet_with_project_continuity
    )
    quality_profile = await quality_profile_resolver(
        db_session=db_session,
        user_id=user_id,
        project=project,
        style_id=style_id,
        enable_mcp=bool(getattr(generate_request, "enable_mcp", True)),
        prefer_project_default_style=not bool(style_id),
        log_prefix="chapter-generate",
    )
    story_packet = await story_packet_builder(
        db_session=db_session,
        project=project,
        source=generate_request,
        source_label="chapter-generate-request",
    )
    story_repair_state = await resolve_story_repair_state_fn(
        db_session,
        chapter=chapter,
        story_repair_summary=getattr(generate_request, "story_repair_summary", None),
        story_repair_targets=getattr(generate_request, "story_repair_targets", None),
        story_preserve_strengths=getattr(
            generate_request,
            "story_preserve_strengths",
            None,
        ),
    )
    story_repair_payload = story_repair_state.get("payload")
    return ChapterGenerationStreamRuntimeContext(
        chapter=chapter,
        project=project,
        outline=outline,
        outline_mode=outline_mode,
        quality_profile=quality_profile,
        story_packet=story_packet,
        generation_guidance=story_packet.guidance,
        story_repair_state=story_repair_state,
        story_repair_payload=story_repair_payload,
        resolved_style_id=quality_profile.get("resolved_style_id"),
        style_content=quality_profile.get("style_content") or "",
        style_name=quality_profile.get("style_name") or "",
        style_preset_id=quality_profile.get("style_preset_id") or "",
    )


async def build_chapter_generation_stream_context(
    *,
    db_session,
    runtime_context: ChapterGenerationStreamRuntimeContext,
    user_id: str,
    target_word_count: int,
    temp_narrative_perspective: Optional[str],
    memory_service: Any,
    foreshadow_service: Any,
    one_to_one_builder_cls: Any,
    one_to_many_builder_cls: Any,
    build_outline_structure_runtime_sources_fn,
    build_generation_runtime_bundle_fn,
) -> ChapterGenerationStreamBuiltContext:
    chapter = runtime_context.chapter
    project = runtime_context.project
    outline = runtime_context.outline

    if runtime_context.outline_mode == "one-to-one":
        context_builder = one_to_one_builder_cls(
            memory_service=memory_service,
            foreshadow_service=foreshadow_service,
        )
        chapter_context = await context_builder.build(
            chapter=chapter,
            project=project,
            outline=outline,
            user_id=user_id,
            db=db_session,
            target_word_count=target_word_count,
        )
    else:
        context_builder = one_to_many_builder_cls(
            memory_service=memory_service,
            foreshadow_service=foreshadow_service,
        )
        chapter_context = await context_builder.build(
            chapter=chapter,
            project=project,
            outline=outline,
            user_id=user_id,
            db=db_session,
            target_word_count=target_word_count,
            style_content=runtime_context.style_content,
            temp_narrative_perspective=temp_narrative_perspective,
        )

    outline_runtime_sources = build_outline_structure_runtime_sources_fn(outline)
    generation_runtime_bundle = await _resolve_maybe_await(
        build_generation_runtime_bundle_fn(
            story_packet=runtime_context.story_packet,
            quality_profile=runtime_context.quality_profile,
            project=project,
            chapter=chapter,
            chapter_context=chapter_context,
            target_word_count=target_word_count,
            story_repair_state=runtime_context.story_repair_state,
            story_repair_payload=runtime_context.story_repair_payload,
            active_story_repair_payload=(
                runtime_context.story_repair_state.get("active_story_repair_payload")
                if isinstance(runtime_context.story_repair_state, dict)
                else None
            ),
            character_focus_source=outline_runtime_sources or None,
            character_state_source=outline_runtime_sources or None,
            organization_state_source=outline_runtime_sources or None,
        )
    )
    return ChapterGenerationStreamBuiltContext(
        chapter_context=chapter_context,
        generation_intent=generation_runtime_bundle.generation_intent,
        prompt_quality_kwargs=generation_runtime_bundle.prompt_quality_kwargs,
        story_runtime_contract=generation_runtime_bundle.story_runtime_contract,
    )


async def build_chapter_generation_stream_prompt(
    *,
    db_session,
    runtime_context: ChapterGenerationStreamRuntimeContext,
    built_context: ChapterGenerationStreamBuiltContext,
    current_user_id: str,
    target_word_count: int,
    temp_narrative_perspective: Optional[str],
    get_template_fn,
    format_prompt_fn,
    apply_style_to_prompt_fn,
) -> ChapterGenerationStreamPrompt:
    chapter = runtime_context.chapter
    project = runtime_context.project
    chapter_context = built_context.chapter_context
    prompt_quality_kwargs = built_context.prompt_quality_kwargs
    chapter_perspective = _resolve_chapter_perspective(
        project=project,
        temp_narrative_perspective=temp_narrative_perspective,
    )

    common_kwargs = {
        "chapter_title": chapter.title,
        "chapter_number": chapter.chapter_number,
        "chapter_outline": chapter_context.chapter_outline,
        "target_word_count": target_word_count,
        "narrative_perspective": chapter_perspective,
        "world_time_period": project.world_time_period or "",
        "world_location": project.world_location or "",
        "world_atmosphere": project.world_atmosphere or "",
        "world_rules": project.world_rules or "",
        "characters_info": chapter_context.chapter_characters or "",
        "chapter_careers": chapter_context.chapter_careers or "",
        "foreshadow_reminders": chapter_context.foreshadow_reminders or "",
        **prompt_quality_kwargs,
    }

    if runtime_context.outline_mode == "one-to-one":
        if chapter_context.continuation_point:
            template = await get_template_fn(
                "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
                current_user_id,
                db_session,
            )
            base_prompt = format_prompt_fn(
                template,
                previous_chapter_content=chapter_context.continuation_point,
                previous_chapter_summary=chapter_context.previous_chapter_summary or "",
                relevant_memories=chapter_context.relevant_memories or "",
                **common_kwargs,
            )
        else:
            template = await get_template_fn(
                "CHAPTER_GENERATION_ONE_TO_ONE",
                current_user_id,
                db_session,
            )
            base_prompt = format_prompt_fn(
                template,
                relevant_memories=chapter_context.relevant_memories or "",
                **common_kwargs,
            )
    else:
        if chapter_context.continuation_point:
            template = await get_template_fn(
                "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
                current_user_id,
                db_session,
            )
            base_prompt = format_prompt_fn(
                template,
                continuation_point=chapter_context.continuation_point,
                previous_chapter_summary=chapter_context.previous_chapter_summary or "",
                recent_chapters_context=chapter_context.recent_chapters_context or "",
                relevant_memories=chapter_context.relevant_memories or "",
                **common_kwargs,
            )
        else:
            template = await get_template_fn(
                "CHAPTER_GENERATION_ONE_TO_MANY",
                current_user_id,
                db_session,
            )
            base_prompt = format_prompt_fn(
                template,
                relevant_memories=chapter_context.relevant_memories or "",
                **common_kwargs,
            )

    prompt = (
        apply_style_to_prompt_fn(base_prompt, runtime_context.style_content)
        if runtime_context.style_content
        else base_prompt
    )
    return ChapterGenerationStreamPrompt(
        chapter_perspective=chapter_perspective,
        base_prompt=base_prompt,
        prompt=prompt,
    )


def build_chapter_generation_stream_request_payload(
    *,
    runtime_context: ChapterGenerationStreamRuntimeContext,
    built_context: ChapterGenerationStreamBuiltContext,
    stream_prompt: ChapterGenerationStreamPrompt,
    project: Any,
    target_word_count: int,
    enable_mcp: bool,
    custom_model: Optional[str],
    ai_service: Any,
    build_runtime_system_prompt_fn,
    calculate_max_tokens_fn,
    build_request_options_fn,
    detect_style_profile_fn,
    resolve_generation_temperature_fn,
) -> ChapterGenerationStreamRequestPayload:
    chapter_context = built_context.chapter_context
    style_content = runtime_context.style_content
    style_name = runtime_context.style_name
    style_preset_id = runtime_context.style_preset_id
    story_runtime_contract = built_context.story_runtime_contract

    system_prompt = build_runtime_system_prompt_fn(
        project=project,
        style_content=style_content,
        chapter_outline=chapter_context.chapter_outline,
        previous_summary=chapter_context.previous_chapter_summary,
        style_name=style_name,
        style_preset_id=style_preset_id,
        target_word_count=target_word_count,
        story_runtime_contract=story_runtime_contract,
    )
    max_tokens = calculate_max_tokens_fn(target_word_count)
    style_profile = detect_style_profile_fn(
        style_name=style_name,
        style_preset_id=style_preset_id,
        style_content=style_content,
    )
    generate_kwargs: dict[str, Any] = {
        "prompt": stream_prompt.prompt,
        "system_prompt": system_prompt,
        "tool_choice": "auto",
        "auto_mcp": enable_mcp,
        "max_tokens": max_tokens,
        "temperature": resolve_generation_temperature_fn(style_profile),
    }
    request_options = build_request_options_fn(ai_service)
    if request_options is not None:
        generate_kwargs["request_options"] = request_options
    if custom_model:
        generate_kwargs["model"] = custom_model

    return ChapterGenerationStreamRequestPayload(
        system_prompt=system_prompt,
        max_tokens=max_tokens,
        generate_kwargs=generate_kwargs,
    )


async def prepare_chapter_generation_stream_execution(
    *,
    db_session,
    chapter_id: str,
    current_user_id: str,
    generate_request: Any,
    user_ai_service: Any,
    target_word_count: int,
    custom_model: Optional[str],
    temp_narrative_perspective: Optional[str],
    style_id: Optional[int],
    dependencies: Any,
    resolve_quality_profile_fn: Optional[Callable[..., Any]] = None,
    build_story_packet_fn: Optional[Callable[..., Any]] = None,
) -> ChapterGenerationStreamExecutionSetup:
    stream_runtime_context = await load_chapter_generation_stream_runtime_context(
        db_session,
        chapter_id=chapter_id,
        user_id=current_user_id,
        generate_request=generate_request,
        style_id=style_id,
        resolve_story_repair_state_fn=dependencies.resolve_story_repair_state_fn,
        cancel_outline_postprocess_tasks_fn=(
            dependencies.cancel_outline_postprocess_tasks_fn
        ),
        resolve_quality_profile_fn=resolve_quality_profile_fn,
        build_story_packet_fn=build_story_packet_fn,
    )
    current_chapter = stream_runtime_context.chapter
    project = stream_runtime_context.project

    built_stream_context = await build_chapter_generation_stream_context(
        db_session=db_session,
        runtime_context=stream_runtime_context,
        user_id=current_user_id,
        target_word_count=target_word_count,
        temp_narrative_perspective=temp_narrative_perspective,
        memory_service=dependencies.memory_service,
        foreshadow_service=dependencies.foreshadow_service,
        one_to_one_builder_cls=dependencies.one_to_one_builder_cls,
        one_to_many_builder_cls=dependencies.one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=(
            dependencies.build_outline_structure_runtime_sources_fn
        ),
        build_generation_runtime_bundle_fn=(
            dependencies.build_generation_runtime_bundle_fn
        ),
    )
    story_runtime_contract = built_stream_context.story_runtime_contract

    stream_prompt = await build_chapter_generation_stream_prompt(
        db_session=db_session,
        runtime_context=stream_runtime_context,
        built_context=built_stream_context,
        current_user_id=current_user_id,
        target_word_count=target_word_count,
        temp_narrative_perspective=temp_narrative_perspective,
        get_template_fn=dependencies.get_template_fn,
        format_prompt_fn=dependencies.format_prompt_fn,
        apply_style_to_prompt_fn=dependencies.apply_style_to_prompt_fn,
    )
    request_payload = build_chapter_generation_stream_request_payload(
        runtime_context=stream_runtime_context,
        built_context=built_stream_context,
        stream_prompt=stream_prompt,
        project=project,
        target_word_count=target_word_count,
        enable_mcp=generate_request.enable_mcp,
        custom_model=custom_model,
        ai_service=user_ai_service,
        build_runtime_system_prompt_fn=(
            dependencies.build_runtime_system_prompt_fn
        ),
        calculate_max_tokens_fn=dependencies.calculate_max_tokens_fn,
        build_request_options_fn=dependencies.build_request_options_fn,
        detect_style_profile_fn=dependencies.detect_style_profile_fn,
        resolve_generation_temperature_fn=(
            dependencies.resolve_generation_temperature_fn
        ),
    )
    return ChapterGenerationStreamExecutionSetup(
        stream_runtime_context=stream_runtime_context,
        built_stream_context=built_stream_context,
        current_chapter=current_chapter,
        project=project,
        quality_profile=stream_runtime_context.quality_profile,
        story_packet=stream_runtime_context.story_packet,
        story_runtime_contract=story_runtime_contract,
        request_payload=request_payload,
    )


def apply_chapter_generation_outcome_and_build_history(
    *,
    chapter: Any,
    project: Any,
    outcome: Any,
    story_runtime_contract: Optional[dict[str, Any]],
    build_generation_history_payload_fn: Callable[..., str],
    history_model: str = "default",
) -> ChapterGenerationPersistencePreparation:
    from migrator_app.models import GenerationHistory

    previous_content = chapter.content or ""
    previous_word_count = int(chapter.word_count or len(previous_content))
    previous_status = chapter.status

    provisional_draft_saved = bool(
        outcome.provisional_draft_allowed and not outcome.content_applied
    )
    if outcome.content_applied or provisional_draft_saved:
        chapter.content = outcome.full_content
        chapter.word_count = outcome.candidate_word_count
        chapter.status = "completed" if outcome.content_applied else "draft"
        project.current_words = (
            int(project.current_words or 0)
            - previous_word_count
            + outcome.candidate_word_count
        )

    saved_word_count = int(chapter.word_count or 0)
    history_payload = build_generation_history_payload_fn(
        outcome.full_content,
        outcome.quality_metrics,
        content_applied=outcome.content_applied,
        attempt_state=outcome.attempt_state,
        story_runtime_contract=story_runtime_contract,
    )
    history = GenerationHistory(
        project_id=chapter.project_id,
        chapter_id=chapter.id,
        prompt=f"chapter:{chapter.chapter_number}:{chapter.title}",
        generated_content=history_payload,
        model=history_model,
    )
    return ChapterGenerationPersistencePreparation(
        previous_content=previous_content,
        previous_word_count=previous_word_count,
        previous_status=previous_status,
        saved_word_count=saved_word_count,
        provisional_draft_saved=provisional_draft_saved,
        history=history,
    )


def build_chapter_generation_analysis_followup_plan(
    *,
    enable_analysis: bool,
    quality_gate_action: Optional[str],
    quality_gate_requires_followup: bool,
    full_content: str,
    candidate_word_count: int,
) -> ChapterGenerationAnalysisFollowupPlan:
    resolved_action = str(quality_gate_action or "continue")
    should_schedule_analysis = bool(
        enable_analysis or quality_gate_requires_followup
    )

    analysis_reason: Optional[str] = None
    if should_schedule_analysis:
        analysis_reason = (
            "manual_analysis" if enable_analysis else "quality_gate_followup"
        )
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
        chapter_content_override=(
            full_content if quality_gate_requires_followup else None
        ),
        chapter_word_count_override=(
            candidate_word_count if quality_gate_requires_followup else None
        ),
        completion_message=completion_message,
        analysis_started_message=analysis_started_message,
    )


def build_chapter_generation_stream_response_artifacts(
    *,
    chapter: Any,
    draft_attempt: Any,
    quality_metrics: Optional[dict[str, Any]],
    quality_gate_action: Optional[str],
    quality_gate_message: Optional[str],
    quality_gate_snapshot: Optional[dict[str, Any]],
    quality_gate_requires_followup: bool,
    content_applied: bool,
    saved_word_count: int,
    task_id: Optional[str],
    story_runtime_contract: Optional[dict[str, Any]],
    analysis_started_message: Optional[str],
    build_candidate_draft_payload_fn: Callable[..., Optional[dict[str, Any]]],
    build_stream_result_payload_fn: Callable[..., dict[str, Any]],
) -> ChapterGenerationStreamResponseArtifacts:
    candidate_draft_summary = None
    if quality_gate_requires_followup and draft_attempt is not None:
        candidate_draft_summary = build_candidate_draft_payload_fn(
            draft_attempt=draft_attempt,
            chapter_updated_at=chapter.updated_at,
            include_full_text=False,
        )

    quality_metrics_event_payload: dict[str, Any] = {
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
            "type": (
                "quality_gate_retry"
                if resolved_action == "retry"
                else "quality_gate_blocked"
            ),
            "chapter_id": chapter.id,
            "chapter_number": chapter.chapter_number,
            "message": quality_gate_message,
            "progress": 88 if resolved_action == "retry" else 95,
            "quality_gate": (
                quality_gate_snapshot
                if isinstance(quality_gate_snapshot, dict)
                else None
            ),
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
    db_session: Any,
    *,
    chapter_id: str,
    user_id: str,
    project_id: str,
    followup_plan: ChapterGenerationAnalysisFollowupPlan,
    ai_service: Any,
    quality_profile: dict[str, Any],
    story_packet: Any,
    create_analysis_task_fn: Callable[..., Any],
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
    db_session: Any,
    *,
    chapter_id: str,
    chapter: Any,
    project: Any,
    full_content: str,
    candidate_word_count: int,
    content_applied: bool,
    provisional_draft_saved: bool,
    previous_status: Optional[str],
    auto_plant_pending_foreshadows_fn: Callable[..., Any],
) -> ChapterGenerationPostPersistEffects:
    from tests.test_support.retired_runtime_test_support import get_logger

    logger = get_logger(__name__)
    if content_applied:
        logger.info(f"✅ 章节 {chapter_id} 已保存，共 {candidate_word_count} 字")
    elif provisional_draft_saved:
        logger.info(
            f"⚠️ 章节 {chapter_id} 已保存候选草稿，共 {candidate_word_count} 字"
        )
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
) -> list[ChapterGenerationEmissionStep]:
    steps: list[ChapterGenerationEmissionStep] = [
        ChapterGenerationEmissionStep(
            kind="tracker_complete", message=completion_message
        ),
        ChapterGenerationEmissionStep(
            kind="sse_payload",
            payload=response_artifacts.quality_metrics_event_payload,
        ),
    ]
    if response_artifacts.quality_gate_event_payload:
        steps.append(
            ChapterGenerationEmissionStep(
                kind="sse_payload",
                payload=response_artifacts.quality_gate_event_payload,
            )
        )
    steps.append(
        ChapterGenerationEmissionStep(
            kind="tracker_result",
            payload=response_artifacts.result_payload,
        )
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
    emission_plan,
    tracker_complete_fn: Callable[[str], Any],
    tracker_result_fn: Callable[[dict[str, Any]], Any],
    tracker_done_fn: Callable[[], Any],
    format_sse_fn: Callable[[dict[str, Any]], Any],
    send_event_fn: Callable[..., Any],
):
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
    db_session: Any,
    chapter_id: str,
    current_user_id: str,
    background_tasks: Any,
    user_ai_service: Any,
    enable_analysis: bool,
    execution_setup: Any,
    candidate_stage_result: Any,
    dependencies: Any,
    emit_saving_fn: Callable[[str, float], Any],
    apply_outcome_and_build_history_fn: Callable[..., Any] = (
        apply_chapter_generation_outcome_and_build_history
    ),
) -> tuple[Any, list[ChapterGenerationEmissionStep]]:
    from tests.test_support.retired_runtime_test_support import get_logger

    logger = get_logger(__name__)
    saving_payload = await emit_saving_fn(
        "Saving chapter content and quality results...", 0.3
    )
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
        auto_plant_pending_foreshadows_fn=(
            dependencies.foreshadow_service.auto_plant_pending_foreshadows
        ),
    )

    followup_plan = build_chapter_generation_analysis_followup_plan(
        enable_analysis=enable_analysis,
        quality_gate_action=candidate_stage_result.quality_gate_action,
        quality_gate_requires_followup=(
            candidate_stage_result.quality_gate_requires_followup
        ),
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
            logger.info(
                f"Created analysis task: {task_id} (reason={followup_plan.analysis_reason})"
            )

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
        quality_metrics=(
            candidate_stage_result.quality_metrics
            if isinstance(candidate_stage_result.quality_metrics, dict)
            else None
        ),
        quality_gate_action=candidate_stage_result.quality_gate_action,
        quality_gate_message=candidate_stage_result.quality_gate_message,
        quality_gate_snapshot=candidate_stage_result.quality_gate_snapshot,
        quality_gate_requires_followup=(
            candidate_stage_result.quality_gate_requires_followup
        ),
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




