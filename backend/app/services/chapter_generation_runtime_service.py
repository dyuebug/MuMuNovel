from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Optional

from app.models.chapter import Chapter
from app.models.project import Project
from app.services.chapter_quality_context_service import (
    ChapterGenerationIntent,
    StoryPacket,
    StoryGenerationGuidance,
    build_chapter_generation_intent as build_base_chapter_generation_intent,
)
from app.services.story_repair_payload_service import StoryRepairPayload


@dataclass
class ChapterGenerationRuntimeBundle:
    generation_guidance: Optional[StoryGenerationGuidance]
    generation_intent: ChapterGenerationIntent
    prompt_quality_kwargs: Dict[str, Any]
    story_repair_payload: Optional[StoryRepairPayload] = None
    active_story_repair_payload: Optional[Dict[str, Any]] = None
    story_runtime_contract: Optional[Dict[str, Any]] = None


def create_chapter_generation_intent_from_runtime(
    *,
    story_packet: Optional[StoryPacket],
    quality_profile: Optional[Dict[str, Any]],
    project: Optional[Project],
    chapter: Chapter,
    chapter_context: Optional[Any],
    target_word_count: Optional[int],
    story_repair_state: Optional[Dict[str, Any]] = None,
    story_repair_payload: Optional[StoryRepairPayload] = None,
    active_story_repair_payload: Optional[Dict[str, Any]] = None,
    character_focus_source: Optional[Any] = None,
    foreshadow_payoff_source: Optional[Any] = None,
    character_state_source: Optional[Any] = None,
    relationship_state_source: Optional[Any] = None,
    foreshadow_state_source: Optional[Any] = None,
    organization_state_source: Optional[Any] = None,
    career_state_source: Optional[Any] = None,
) -> ChapterGenerationIntent:
    resolved_story_repair_payload = story_repair_payload
    resolved_active_story_repair_payload = active_story_repair_payload
    quality_history_context: Optional[Dict[str, Any]] = None
    quality_metrics_summary: Optional[Dict[str, Any]] = None

    if isinstance(story_repair_state, dict):
        if resolved_story_repair_payload is None:
            candidate_payload = story_repair_state.get("payload")
            if isinstance(candidate_payload, StoryRepairPayload) or candidate_payload is None:
                resolved_story_repair_payload = candidate_payload
        if resolved_active_story_repair_payload is None:
            candidate_active_payload = story_repair_state.get("active_story_repair_payload")
            if isinstance(candidate_active_payload, dict) or candidate_active_payload is None:
                resolved_active_story_repair_payload = candidate_active_payload
        candidate_quality_history_context = story_repair_state.get("quality_history_context")
        if isinstance(candidate_quality_history_context, dict):
            quality_history_context = candidate_quality_history_context
        candidate_quality_metrics_summary = story_repair_state.get("quality_metrics_summary")
        if isinstance(candidate_quality_metrics_summary, dict):
            quality_metrics_summary = candidate_quality_metrics_summary

    return build_base_chapter_generation_intent(
        story_packet=story_packet,
        quality_profile=quality_profile,
        project=project,
        chapter=chapter,
        chapter_context=chapter_context,
        target_word_count=target_word_count,
        story_repair_payload=resolved_story_repair_payload,
        active_story_repair_payload=resolved_active_story_repair_payload,
        quality_history_context=quality_history_context,
        quality_metrics_summary=quality_metrics_summary,
        character_focus_source=character_focus_source,
        foreshadow_payoff_source=foreshadow_payoff_source,
        character_state_source=character_state_source,
        relationship_state_source=relationship_state_source,
        foreshadow_state_source=foreshadow_state_source,
        organization_state_source=organization_state_source,
        career_state_source=career_state_source,
    )


def build_chapter_quality_runtime_context(
    *,
    story_packet: Optional[StoryPacket],
    project: Optional[Project],
    chapter: Chapter,
    chapter_context: Optional[Any],
    target_word_count: Optional[int],
    story_repair_state: Optional[Dict[str, Any]] = None,
    generation_intent: Optional[ChapterGenerationIntent] = None,
) -> Dict[str, Any]:
    active_generation_intent = generation_intent or create_chapter_generation_intent_from_runtime(
        story_packet=story_packet,
        quality_profile=None,
        project=project,
        chapter=chapter,
        chapter_context=chapter_context,
        target_word_count=target_word_count,
        story_repair_state=story_repair_state,
    )
    return active_generation_intent.build_quality_runtime_context()


def build_chapter_prompt_quality_kwargs_from_runtime(
    *,
    story_packet: Optional[StoryPacket],
    quality_profile: Optional[Dict[str, Any]],
    project: Optional[Project],
    chapter: Chapter,
    chapter_context: Optional[Any],
    target_word_count: Optional[int],
    story_repair_state: Optional[Dict[str, Any]] = None,
    story_repair_payload: Optional[StoryRepairPayload] = None,
    active_story_repair_payload: Optional[Dict[str, Any]] = None,
    character_focus_source: Optional[Any] = None,
    foreshadow_payoff_source: Optional[Any] = None,
    character_state_source: Optional[Any] = None,
    relationship_state_source: Optional[Any] = None,
    foreshadow_state_source: Optional[Any] = None,
    organization_state_source: Optional[Any] = None,
    career_state_source: Optional[Any] = None,
    generation_intent: Optional[ChapterGenerationIntent] = None,
) -> Dict[str, Any]:
    active_generation_intent = generation_intent or create_chapter_generation_intent_from_runtime(
        story_packet=story_packet,
        quality_profile=quality_profile,
        project=project,
        chapter=chapter,
        chapter_context=chapter_context,
        target_word_count=target_word_count,
        story_repair_state=story_repair_state,
        story_repair_payload=story_repair_payload,
        active_story_repair_payload=active_story_repair_payload,
        character_focus_source=chapter if character_focus_source is None else character_focus_source,
        foreshadow_payoff_source=foreshadow_payoff_source,
        character_state_source=character_state_source,
        relationship_state_source=relationship_state_source,
        foreshadow_state_source=foreshadow_state_source,
        organization_state_source=organization_state_source,
        career_state_source=career_state_source,
    )
    return active_generation_intent.build_prompt_quality_kwargs()


def build_chapter_generation_runtime_bundle(
    *,
    story_packet: Optional[StoryPacket],
    quality_profile: Optional[Dict[str, Any]],
    project: Optional[Project],
    chapter: Chapter,
    chapter_context: Optional[Any],
    target_word_count: Optional[int],
    story_repair_state: Optional[Dict[str, Any]] = None,
    story_repair_payload: Optional[StoryRepairPayload] = None,
    active_story_repair_payload: Optional[Dict[str, Any]] = None,
    character_focus_source: Optional[Any] = None,
    foreshadow_payoff_source: Optional[Any] = None,
    character_state_source: Optional[Any] = None,
    relationship_state_source: Optional[Any] = None,
    foreshadow_state_source: Optional[Any] = None,
    organization_state_source: Optional[Any] = None,
    career_state_source: Optional[Any] = None,
) -> ChapterGenerationRuntimeBundle:
    resolved_story_repair_payload = story_repair_payload
    resolved_active_story_repair_payload = active_story_repair_payload

    if isinstance(story_repair_state, dict):
        if resolved_story_repair_payload is None:
            candidate_payload = story_repair_state.get("payload")
            if isinstance(candidate_payload, StoryRepairPayload) or candidate_payload is None:
                resolved_story_repair_payload = candidate_payload
        if resolved_active_story_repair_payload is None:
            candidate_active_payload = story_repair_state.get("active_story_repair_payload")
            if isinstance(candidate_active_payload, dict) or candidate_active_payload is None:
                resolved_active_story_repair_payload = candidate_active_payload

    generation_intent = create_chapter_generation_intent_from_runtime(
        story_packet=story_packet,
        quality_profile=quality_profile,
        project=project,
        chapter=chapter,
        chapter_context=chapter_context,
        target_word_count=target_word_count,
        story_repair_state=story_repair_state,
        story_repair_payload=resolved_story_repair_payload,
        active_story_repair_payload=resolved_active_story_repair_payload,
        character_focus_source=character_focus_source,
        foreshadow_payoff_source=foreshadow_payoff_source,
        character_state_source=character_state_source,
        relationship_state_source=relationship_state_source,
        foreshadow_state_source=foreshadow_state_source,
        organization_state_source=organization_state_source,
        career_state_source=career_state_source,
    )
    prompt_quality_kwargs = build_chapter_prompt_quality_kwargs_from_runtime(
        story_packet=story_packet,
        quality_profile=quality_profile,
        project=project,
        chapter=chapter,
        chapter_context=chapter_context,
        target_word_count=target_word_count,
        story_repair_state=story_repair_state,
        story_repair_payload=resolved_story_repair_payload,
        active_story_repair_payload=resolved_active_story_repair_payload,
        character_focus_source=character_focus_source,
        foreshadow_payoff_source=foreshadow_payoff_source,
        character_state_source=character_state_source,
        relationship_state_source=relationship_state_source,
        foreshadow_state_source=foreshadow_state_source,
        organization_state_source=organization_state_source,
        career_state_source=career_state_source,
        generation_intent=generation_intent,
    )
    return ChapterGenerationRuntimeBundle(
        generation_guidance=generation_intent.story_packet.guidance if generation_intent else None,
        generation_intent=generation_intent,
        prompt_quality_kwargs=prompt_quality_kwargs,
        story_repair_payload=resolved_story_repair_payload,
        active_story_repair_payload=resolved_active_story_repair_payload,
        story_runtime_contract=(
            generation_intent.build_story_runtime_contract()
            if generation_intent is not None
            else None
        ),
    )
