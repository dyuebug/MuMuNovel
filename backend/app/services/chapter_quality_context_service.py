from __future__ import annotations

from dataclasses import dataclass, field, replace
from typing import Any, Dict, List, Mapping, Optional, Sequence
import json
import re

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.project import Project
from app.models.writing_style import WritingStyle
from app.services.mcp_tools_loader import mcp_tools_loader
from app.services.project_generation_defaults import resolve_project_generation_defaults
from app.services.project_continuity_ledger_service import build_project_continuity_ledger
from app.services.novel_quality_rules import detect_genre_profiles, detect_style_profile
from app.services.prompt_service import (
    build_creative_mode_block,
    build_narrative_blueprint_block,
    build_quality_preference_block,
    build_story_character_focus_anchor_block,
    build_story_character_state_ledger_block,
    build_story_creation_brief_block,
    build_story_focus_block,
    build_story_foreshadow_payoff_plan_block,
    build_story_foreshadow_state_ledger_block,
    build_story_long_term_goal_block,
    build_story_organization_state_ledger_block,
    build_story_relationship_state_ledger_block,
    build_story_career_state_ledger_block,
    build_story_pacing_budget_block,
    build_story_quality_trend_block,
    build_volume_pacing_block,
    build_story_repair_target_block,
)
from app.services.story_repair_payload_service import StoryRepairPayload, resolve_story_repair_prompt_kwargs
from app.services.writing_style_sync_service import sync_low_ai_presets

logger = get_logger(__name__)


@dataclass(frozen=True)
class StoryGenerationGuidance:
    creative_mode: Optional[str] = None
    story_focus: Optional[str] = None
    plot_stage: Optional[str] = None
    story_creation_brief: Optional[str] = None
    quality_preset: Optional[str] = None
    quality_notes: Optional[str] = None

    def to_generation_kwargs(self) -> Dict[str, Optional[str]]:
        return {
            "creative_mode": self.creative_mode,
            "story_focus": self.story_focus,
            "plot_stage": self.plot_stage,
            "story_creation_brief": self.story_creation_brief,
            "quality_preset": self.quality_preset,
            "quality_notes": self.quality_notes,
        }

    def to_prompt_fields(self) -> Dict[str, Any]:
        return {
            key: value or ""
            for key, value in self.to_generation_kwargs().items()
        }


STORY_GUIDANCE_FIELD_NAMES: tuple[str, ...] = (
    "creative_mode",
    "story_focus",
    "plot_stage",
    "story_creation_brief",
    "quality_preset",
    "quality_notes",
)


@dataclass(frozen=True)
class StoryBlueprint:
    long_term_goal: Optional[str] = None
    chapter_count: Optional[int] = None
    current_chapter_number: Optional[int] = None
    target_word_count: Optional[int] = None
    character_focus_names: tuple[str, ...] = ()
    foreshadow_payoff_plan: tuple[str, ...] = ()
    character_state_ledger: tuple[str, ...] = ()
    relationship_state_ledger: tuple[str, ...] = ()
    foreshadow_state_ledger: tuple[str, ...] = ()
    organization_state_ledger: tuple[str, ...] = ()
    career_state_ledger: tuple[str, ...] = ()

    def to_prompt_fields(self) -> Dict[str, Any]:
        return {
            "story_long_term_goal": self.long_term_goal or "",
            "chapter_count": self.chapter_count or "",
            "current_chapter_number": self.current_chapter_number or "",
            "target_word_count": self.target_word_count or "",
            "story_character_focus": list(self.character_focus_names),
            "story_foreshadow_payoff_plan": list(self.foreshadow_payoff_plan),
            "story_character_state_ledger": list(self.character_state_ledger),
            "story_relationship_state_ledger": list(self.relationship_state_ledger),
            "story_foreshadow_state_ledger": list(self.foreshadow_state_ledger),
            "story_organization_state_ledger": list(self.organization_state_ledger),
            "story_career_state_ledger": list(self.career_state_ledger),
        }


@dataclass(frozen=True)
class StoryPacket:
    guidance: StoryGenerationGuidance
    request_overrides: Dict[str, Optional[str]] = field(default_factory=dict)
    source: Optional[str] = None
    blueprint: StoryBlueprint = field(default_factory=StoryBlueprint)

    @classmethod
    def from_guidance(
        cls,
        guidance: StoryGenerationGuidance,
        *,
        request_overrides: Optional[Mapping[str, Any]] = None,
        source: Optional[str] = None,
        blueprint: Optional[StoryBlueprint] = None,
    ) -> "StoryPacket":
        normalized_overrides = _normalize_story_guidance_values(request_overrides or {})
        return cls(
            guidance=guidance,
            request_overrides={
                field_name: value
                for field_name, value in normalized_overrides.items()
                if value is not None
            },
            source=source,
            blueprint=blueprint or StoryBlueprint(),
        )

    def with_blueprint(
        self,
        *,
        long_term_goal: Optional[str] = None,
        chapter_count: Optional[int] = None,
        current_chapter_number: Optional[int] = None,
        target_word_count: Optional[int] = None,
        quality_metrics_summary: Optional[Mapping[str, Any]] = None,
        character_focus_source: Optional[Any] = None,
        foreshadow_payoff_source: Optional[Any] = None,
        character_state_source: Optional[Any] = None,
        relationship_state_source: Optional[Any] = None,
        foreshadow_state_source: Optional[Any] = None,
        organization_state_source: Optional[Any] = None,
        career_state_source: Optional[Any] = None,
    ) -> "StoryPacket":
        current = self.blueprint
        next_character_focus = _extract_story_packet_character_focus(character_focus_source)
        next_foreshadow_payoff_plan = _extract_story_packet_foreshadow_payoff_plan(foreshadow_payoff_source)
        next_character_state_ledger = _extract_story_packet_character_state_ledger(character_state_source)
        next_relationship_state_ledger = _extract_story_packet_relationship_state_ledger(relationship_state_source)
        next_foreshadow_state_ledger = _extract_story_packet_foreshadow_state_ledger(foreshadow_state_source)
        next_organization_state_ledger = _extract_story_packet_organization_state_ledger(organization_state_source)
        next_career_state_ledger = _extract_story_packet_career_state_ledger(career_state_source)
        blueprint = StoryBlueprint(
            long_term_goal=_normalize_optional_text(long_term_goal) or current.long_term_goal,
            chapter_count=_normalize_optional_int(chapter_count) if chapter_count is not None else current.chapter_count,
            current_chapter_number=(
                _normalize_optional_int(current_chapter_number)
                if current_chapter_number is not None
                else current.current_chapter_number
            ),
            target_word_count=(
                _normalize_optional_int(target_word_count)
                if target_word_count is not None
                else current.target_word_count
            ),
            character_focus_names=next_character_focus or current.character_focus_names,
            foreshadow_payoff_plan=next_foreshadow_payoff_plan or current.foreshadow_payoff_plan,
            character_state_ledger=next_character_state_ledger or current.character_state_ledger,
            relationship_state_ledger=next_relationship_state_ledger or current.relationship_state_ledger,
            foreshadow_state_ledger=next_foreshadow_state_ledger or current.foreshadow_state_ledger,
            organization_state_ledger=next_organization_state_ledger or current.organization_state_ledger,
            career_state_ledger=next_career_state_ledger or current.career_state_ledger,
        )
        return replace(self, blueprint=blueprint)

    def to_prompt_fields(self) -> Dict[str, Any]:
        fields = self.guidance.to_prompt_fields()
        fields.update(self.blueprint.to_prompt_fields())
        return fields

    def to_generation_kwargs(self) -> Dict[str, Optional[str]]:
        return self.guidance.to_generation_kwargs()

    def build_prompt_quality_kwargs(
        self,
        profile: Optional[Dict[str, Any]],
        *,
        scene: str = "chapter",
        story_repair_summary: Optional[str] = None,
        story_repair_targets: Optional[List[str]] = None,
        story_preserve_strengths: Optional[List[str]] = None,
        active_story_repair_payload: Optional[Mapping[str, Any]] = None,
        story_repair_payload: Optional[StoryRepairPayload] = None,
        chapter_count: Optional[int] = None,
        current_chapter_number: Optional[int] = None,
        target_word_count: Optional[int] = None,
        quality_metrics_summary: Optional[Mapping[str, Any]] = None,
        character_focus_source: Optional[Any] = None,
        foreshadow_payoff_source: Optional[Any] = None,
        character_state_source: Optional[Any] = None,
        relationship_state_source: Optional[Any] = None,
        foreshadow_state_source: Optional[Any] = None,
        organization_state_source: Optional[Any] = None,
        career_state_source: Optional[Any] = None,
    ) -> Dict[str, Any]:
        active_packet = self.with_blueprint(
            chapter_count=chapter_count,
            current_chapter_number=current_chapter_number,
            target_word_count=target_word_count,
            character_focus_source=character_focus_source,
            foreshadow_payoff_source=foreshadow_payoff_source,
            character_state_source=character_state_source,
            relationship_state_source=relationship_state_source,
            foreshadow_state_source=foreshadow_state_source,
            organization_state_source=organization_state_source,
            career_state_source=career_state_source,
        )
        blueprint = active_packet.blueprint
        return build_prompt_quality_kwargs(
            profile,
            guidance=active_packet.guidance,
            scene=scene,
            story_repair_summary=story_repair_summary,
            story_repair_targets=story_repair_targets,
            story_preserve_strengths=story_preserve_strengths,
            active_story_repair_payload=active_story_repair_payload,
            story_repair_payload=story_repair_payload,
            story_long_term_goal=blueprint.long_term_goal,
            story_character_focus=blueprint.character_focus_names,
            story_foreshadow_payoff_plan=blueprint.foreshadow_payoff_plan,
            chapter_count=blueprint.chapter_count,
            current_chapter_number=blueprint.current_chapter_number,
            target_word_count=blueprint.target_word_count,
            quality_metrics_summary=quality_metrics_summary,
            story_character_state_ledger=blueprint.character_state_ledger,
            story_relationship_state_ledger=blueprint.relationship_state_ledger,
            story_foreshadow_state_ledger=blueprint.foreshadow_state_ledger,
            story_organization_state_ledger=blueprint.organization_state_ledger,
            story_career_state_ledger=blueprint.career_state_ledger,
        )

    def build_quality_runtime_context(
        self,
        *,
        chapter_count: Optional[int] = None,
        current_chapter_number: Optional[int] = None,
        target_word_count: Optional[int] = None,
        character_focus_source: Optional[Any] = None,
        foreshadow_payoff_source: Optional[Any] = None,
        character_state_source: Optional[Any] = None,
        relationship_state_source: Optional[Any] = None,
        foreshadow_state_source: Optional[Any] = None,
        organization_state_source: Optional[Any] = None,
        career_state_source: Optional[Any] = None,
        genre: Optional[str] = None,
        style_name: Optional[str] = None,
        style_preset_id: Optional[str] = None,
        style_content: Optional[str] = None,
        style_profile: Optional[str] = None,
        genre_profiles: Optional[Sequence[str]] = None,
    ) -> Dict[str, Any]:
        active_packet = self.with_blueprint(
            chapter_count=chapter_count,
            current_chapter_number=current_chapter_number,
            target_word_count=target_word_count,
            character_focus_source=character_focus_source,
            foreshadow_payoff_source=foreshadow_payoff_source,
            character_state_source=character_state_source,
            relationship_state_source=relationship_state_source,
            foreshadow_state_source=foreshadow_state_source,
            organization_state_source=organization_state_source,
            career_state_source=career_state_source,
        )
        blueprint = active_packet.blueprint
        resolved_genre = str(genre or "").strip()
        resolved_style_name = str(style_name or "").strip()
        resolved_style_preset_id = str(style_preset_id or "").strip()
        resolved_style_profile = str(style_profile or "").strip().lower()
        if not resolved_style_profile:
            resolved_style_profile = detect_style_profile(
                style_name=resolved_style_name,
                style_preset_id=resolved_style_preset_id,
                style_content=style_content,
            )
        return {
            "creative_mode": active_packet.guidance.creative_mode or "",
            "story_focus": active_packet.guidance.story_focus or "",
            "plot_stage": active_packet.guidance.plot_stage or "",
            "quality_preset": active_packet.guidance.quality_preset or "",
            "story_long_term_goal": blueprint.long_term_goal or "",
            "chapter_count": blueprint.chapter_count,
            "current_chapter_number": blueprint.current_chapter_number,
            "target_word_count": blueprint.target_word_count,
            "character_focus": list(blueprint.character_focus_names),
            "foreshadow_payoff_plan": list(blueprint.foreshadow_payoff_plan),
            "character_state_ledger": list(blueprint.character_state_ledger),
            "relationship_state_ledger": list(blueprint.relationship_state_ledger),
            "foreshadow_state_ledger": list(blueprint.foreshadow_state_ledger),
            "organization_state_ledger": list(blueprint.organization_state_ledger),
            "career_state_ledger": list(blueprint.career_state_ledger),
            "genre": resolved_genre,
            "genre_profiles": _normalize_string_sequence(genre_profiles, limit=4) or list(detect_genre_profiles(resolved_genre)),
            "style_name": resolved_style_name,
            "style_preset_id": resolved_style_preset_id,
            "style_profile": resolved_style_profile,
        }

    def build_analysis_quality_kwargs(

        self,
        profile: Optional[Dict[str, Any]],
    ) -> Dict[str, Any]:
        return build_analysis_quality_kwargs(profile, guidance=self.guidance)


def build_story_runtime_requirement_text(
    base_requirements: Optional[str],
    *,
    guidance: Optional[StoryGenerationGuidance] = None,
    story_packet: Optional[StoryPacket] = None,
    chapter_count: Optional[int] = None,
    memory_guidance: Optional[str] = None,
    quality_repair_guidance: Optional[str] = None,
    scene: str = "outline",
    quality_trend_guidance: Optional[str] = None,
) -> str:
    """构建故事运行时要求文本，融合蓝图、记忆与修复提示。"""
    active_guidance = (story_packet.guidance if story_packet is not None else guidance) or StoryGenerationGuidance()
    blueprint = story_packet.blueprint if story_packet is not None else StoryBlueprint()
    resolved_chapter_count = blueprint.chapter_count or _normalize_optional_int(chapter_count)

    parts: List[str] = []

    def _append_block(block: Optional[str]) -> None:
        normalized = str(block or "").strip()
        if normalized:
            parts.append(normalized)

    _append_block(base_requirements)
    _append_block(build_story_creation_brief_block(active_guidance.story_creation_brief))
    _append_block(build_story_long_term_goal_block(blueprint.long_term_goal))
    _append_block(
        build_story_pacing_budget_block(
            resolved_chapter_count,
            plot_stage=active_guidance.plot_stage,
            scene=scene,
        )
    )
    _append_block(
        build_volume_pacing_block(
            resolved_chapter_count,
            plot_stage=active_guidance.plot_stage,
        )
    )
    _append_block(build_story_character_focus_anchor_block(blueprint.character_focus_names, scene=scene))
    _append_block(build_story_foreshadow_payoff_plan_block(blueprint.foreshadow_payoff_plan, scene=scene))
    _append_block(build_story_character_state_ledger_block(blueprint.character_state_ledger, scene=scene))
    _append_block(build_story_relationship_state_ledger_block(blueprint.relationship_state_ledger, scene=scene))
    _append_block(build_story_foreshadow_state_ledger_block(blueprint.foreshadow_state_ledger, scene=scene))
    _append_block(build_story_organization_state_ledger_block(blueprint.organization_state_ledger, scene=scene))
    _append_block(build_story_career_state_ledger_block(blueprint.career_state_ledger, scene=scene))
    _append_block(memory_guidance)
    _append_block(quality_repair_guidance)
    _append_block(quality_trend_guidance)
    return "\n\n".join(parts)


def _normalize_optional_text(value: Optional[str]) -> Optional[str]:
    if value is None:
        return None
    normalized = str(value).strip()
    return normalized or None


def _normalize_story_guidance_values(values: Mapping[str, Any]) -> Dict[str, Optional[str]]:
    return {
        field_name: _normalize_optional_text(values.get(field_name))
        for field_name in STORY_GUIDANCE_FIELD_NAMES
    }


def _normalize_optional_int(value: Optional[Any]) -> Optional[int]:
    if value is None:
        return None
    try:
        normalized = int(str(value).strip())
    except (TypeError, ValueError):
        return None
    return normalized if normalized > 0 else None


def _normalize_string_sequence(values: Optional[Any], *, limit: int = 4) -> tuple[str, ...]:
    if values is None:
        return ()
    if isinstance(values, str):
        raw_items = [values]
    elif isinstance(values, Sequence) and not isinstance(values, (str, bytes, bytearray)):
        raw_items = list(values)
    else:
        raw_items = [values]

    normalized: list[str] = []
    for item in raw_items:
        text = _normalize_optional_text(item)
        if not text:
            continue
        if text in normalized:
            continue
        normalized.append(text)
        if len(normalized) >= limit:
            break
    return tuple(normalized)


def _read_story_guidance_value(source: Optional[Any], field_name: str) -> Optional[str]:
    if source is None:
        return None
    if isinstance(source, Mapping):
        return source.get(field_name)
    return getattr(source, field_name, None)



def _extract_story_packet_character_focus(source: Optional[Any], *, limit: int = 4) -> tuple[str, ...]:
    if source is None:
        return ()

    candidate = source
    if isinstance(source, Mapping):
        candidate = source.get("character_focus")
        if not candidate:
            expansion_plan = source.get("expansion_plan")
            if isinstance(expansion_plan, str):
                try:
                    expansion_plan = json.loads(expansion_plan)
                except (TypeError, ValueError, json.JSONDecodeError):
                    expansion_plan = None
            if isinstance(expansion_plan, Mapping):
                candidate = expansion_plan.get("character_focus")
        if not candidate:
            candidate = source
    elif not isinstance(source, (str, Sequence)) or isinstance(source, (bytes, bytearray)):
        direct_focus = _read_story_guidance_value(source, "character_focus")
        if direct_focus:
            candidate = direct_focus
        else:
            expansion_plan = _read_story_guidance_value(source, "expansion_plan")
            if isinstance(expansion_plan, str):
                try:
                    expansion_plan = json.loads(expansion_plan)
                except (TypeError, ValueError, json.JSONDecodeError):
                    expansion_plan = None
            if isinstance(expansion_plan, Mapping):
                candidate = expansion_plan.get("character_focus")

    return _normalize_string_sequence(candidate, limit=limit)



def _extract_story_packet_foreshadow_payoff_plan(source: Optional[Any], *, limit: int = 3) -> tuple[str, ...]:
    if source is None:
        return ()

    if isinstance(source, Mapping):
        candidate = source.get("foreshadow_payoff_plan") or source.get("foreshadow_reminders")
    elif not isinstance(source, str) and not isinstance(source, Sequence):
        candidate = (
            _read_story_guidance_value(source, "foreshadow_payoff_plan")
            or _read_story_guidance_value(source, "foreshadow_reminders")
            or source
        )
    else:
        candidate = source

    if isinstance(candidate, Sequence) and not isinstance(candidate, (str, bytes, bytearray)):
        return _normalize_string_sequence(candidate, limit=limit)

    text = _normalize_optional_text(candidate)
    if not text:
        return ()

    normalized: list[str] = []
    current_title: Optional[str] = None
    for raw_line in text.splitlines():
        line = _normalize_optional_text(raw_line)
        if not line or line.startswith("【") or line.startswith("#"):
            continue

        cleaned = re.sub(r"^[-*\d\.\)\s]+", "", line).strip()
        if not cleaned:
            continue
        if cleaned.startswith("埋入章节") or cleaned.startswith("伏笔内容"):
            continue
        if cleaned.startswith("回收提示"):
            _, _, tip = cleaned.partition("：")
            tip_text = _normalize_optional_text(tip or cleaned.replace("回收提示", "", 1))
            entry = f"{current_title}：{tip_text}" if current_title and tip_text else tip_text
            if entry and entry not in normalized:
                normalized.append(entry)
            if len(normalized) >= limit:
                break
            continue

        current_title = cleaned
        if current_title not in normalized:
            normalized.append(current_title)
        if len(normalized) >= limit:
            break
    return tuple(normalized)


def _normalize_story_runtime_ledger_items(values: Optional[Any], *, limit: int = 4) -> tuple[str, ...]:
    return _normalize_string_sequence(values, limit=limit)


def _extract_story_packet_section_lines(
    text: Optional[Any],
    *,
    heading_prefix: Optional[str] = None,
    value_prefixes: Optional[Sequence[str]] = None,
    limit: int = 4,
) -> tuple[str, ...]:
    normalized_text = _normalize_optional_text(text)
    if not normalized_text:
        return ()

    matched: list[str] = []
    current_entity: Optional[str] = None
    normalized_prefixes = tuple(value_prefixes or ())
    heading = heading_prefix or ""

    for raw_line in normalized_text.splitlines():
        line = str(raw_line or "").rstrip()
        stripped = line.strip()
        if not stripped:
            continue
        if heading and stripped.startswith(heading):
            current_entity = stripped.replace("【", "").replace("】", "")
            current_entity = current_entity.split("(", 1)[0].strip() if current_entity else None
            continue
        if stripped.startswith("- "):
            entry = stripped[2:].strip()
            if entry and entry not in matched:
                matched.append(entry)
            if len(matched) >= limit:
                break
            continue
        for prefix in normalized_prefixes:
            if stripped.startswith(prefix):
                payload = stripped[len(prefix):].strip()
                if not payload:
                    break
                entry = f"{current_entity}：{payload}" if current_entity else payload
                if entry not in matched:
                    matched.append(entry)
                break
        if len(matched) >= limit:
            break

    return tuple(matched[:limit])


def _extract_story_packet_character_state_ledger(source: Optional[Any], *, limit: int = 4) -> tuple[str, ...]:
    if source is None:
        return ()
    if isinstance(source, Mapping):
        candidate = (
            source.get("story_character_state_ledger")
            or source.get("character_state_ledger")
            or source.get("character_arc_snapshot")
            or source.get("chapter_characters")
        )
    else:
        candidate = (
            _read_story_guidance_value(source, "story_character_state_ledger")
            or _read_story_guidance_value(source, "character_state_ledger")
            or _read_story_guidance_value(source, "character_arc_snapshot")
            or _read_story_guidance_value(source, "chapter_characters")
            or source
        )

    if isinstance(candidate, Sequence) and not isinstance(candidate, (str, bytes, bytearray)):
        return _normalize_story_runtime_ledger_items(candidate, limit=limit)

    return _extract_story_packet_section_lines(
        candidate,
        heading_prefix="【",
        value_prefixes=("当前状态", "当前状态:", "生存状态", "生存状态:"),
        limit=limit,
    )


def _extract_story_packet_relationship_state_ledger(source: Optional[Any], *, limit: int = 4) -> tuple[str, ...]:
    if source is None:
        return ()
    if isinstance(source, Mapping):
        candidate = (
            source.get("story_relationship_state_ledger")
            or source.get("relationship_state_ledger")
            or source.get("chapter_characters")
        )
    else:
        candidate = (
            _read_story_guidance_value(source, "story_relationship_state_ledger")
            or _read_story_guidance_value(source, "relationship_state_ledger")
            or _read_story_guidance_value(source, "chapter_characters")
            or source
        )

    if isinstance(candidate, Sequence) and not isinstance(candidate, (str, bytes, bytearray)):
        return _normalize_story_runtime_ledger_items(candidate, limit=limit)

    return _extract_story_packet_section_lines(
        candidate,
        heading_prefix="【",
        value_prefixes=("关系网络:", "关系网络"),
        limit=limit,
    )


def _extract_story_packet_foreshadow_state_ledger(source: Optional[Any], *, limit: int = 4) -> tuple[str, ...]:
    if source is None:
        return ()
    if isinstance(source, Mapping):
        candidate = (
            source.get("story_foreshadow_state_ledger")
            or source.get("foreshadow_state_ledger")
            or source.get("foreshadow_reminders")
            or source.get("story_foreshadow_payoff_plan")
        )
    else:
        candidate = (
            _read_story_guidance_value(source, "story_foreshadow_state_ledger")
            or _read_story_guidance_value(source, "foreshadow_state_ledger")
            or _read_story_guidance_value(source, "foreshadow_reminders")
            or _read_story_guidance_value(source, "story_foreshadow_payoff_plan")
            or source
        )

    if isinstance(candidate, Sequence) and not isinstance(candidate, (str, bytes, bytearray)):
        return _normalize_story_runtime_ledger_items(candidate, limit=limit)

    return _extract_story_packet_foreshadow_payoff_plan(candidate, limit=limit)


def _extract_story_packet_organization_state_ledger(source: Optional[Any], *, limit: int = 4) -> tuple[str, ...]:
    if source is None:
        return ()
    if isinstance(source, Mapping):
        candidate = (
            source.get("story_organization_state_ledger")
            or source.get("organization_state_ledger")
            or source.get("organization_states")
        )
    else:
        candidate = (
            _read_story_guidance_value(source, "story_organization_state_ledger")
            or _read_story_guidance_value(source, "organization_state_ledger")
            or _read_story_guidance_value(source, "organization_states")
            or source
        )

    if isinstance(candidate, Sequence) and not isinstance(candidate, (str, bytes, bytearray)):
        return _normalize_story_runtime_ledger_items(candidate, limit=limit)

    return _extract_story_packet_section_lines(
        candidate,
        heading_prefix="【",
        value_prefixes=("组织归属:", "组织归属", "当前势力:", "当前势力", "组织状态:", "组织状态"),
        limit=limit,
    )


def _extract_story_packet_career_state_ledger(source: Optional[Any], *, limit: int = 4) -> tuple[str, ...]:
    if source is None:
        return ()
    if isinstance(source, Mapping):
        candidate = (
            source.get("story_career_state_ledger")
            or source.get("career_state_ledger")
            or source.get("career_states")
        )
    else:
        candidate = (
            _read_story_guidance_value(source, "story_career_state_ledger")
            or _read_story_guidance_value(source, "career_state_ledger")
            or _read_story_guidance_value(source, "career_states")
            or source
        )

    if isinstance(candidate, Sequence) and not isinstance(candidate, (str, bytes, bytearray)):
        return _normalize_story_runtime_ledger_items(candidate, limit=limit)

    return _extract_story_packet_section_lines(
        candidate,
        heading_prefix="【",
        value_prefixes=("主职业:", "主职业", "副职业:", "副职业", "职业状态:", "职业状态"),
        limit=limit,
    )



def _build_story_long_term_goal_text(
    project: Optional[Project],
    guidance: StoryGenerationGuidance,
    *,
    chapter_count: Optional[int] = None,
    target_word_count: Optional[int] = None,
) -> Optional[str]:
    if project is None:
        return None

    parts: list[str] = []
    theme = _normalize_optional_text(getattr(project, "theme", None))
    description = _normalize_optional_text(getattr(project, "description", None))
    if theme:
        parts.append(f"主线主题：{theme}，整本书需要围绕它持续升级冲突与选择。")
    if description:
        parts.append(f"项目简介：{description[:90]}")
    if guidance.story_creation_brief:
        parts.append(f"创作总控：{guidance.story_creation_brief[:90]}")

    resolved_chapter_count = chapter_count or _normalize_optional_int(getattr(project, "chapter_count", None))
    if resolved_chapter_count:
        parts.append(f"整体篇幅预计约 {resolved_chapter_count} 章，推进时要兼顾起势、升级与回报收束。")
    elif target_word_count or _normalize_optional_int(getattr(project, "target_words", None)):
        total_words = target_word_count or _normalize_optional_int(getattr(project, "target_words", None))
        parts.append(f"整体体量预计约 {total_words} 字，推进时避免前松后挤。")

    return _compact_story_runtime_text(parts)



def _compact_story_runtime_text(parts: Sequence[str]) -> Optional[str]:
    normalized = [segment.strip() for segment in parts if str(segment or "").strip()]
    if not normalized:
        return None
    return " ".join(normalized)


def _extract_story_guidance_overrides(
    source: Optional[Any] = None,
    *,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
) -> Dict[str, Optional[str]]:
    raw_overrides = {
        "creative_mode": creative_mode,
        "story_focus": story_focus,
        "plot_stage": plot_stage,
        "story_creation_brief": story_creation_brief,
        "quality_preset": quality_preset,
        "quality_notes": quality_notes,
    }
    return {
        field_name: value if value is not None else _read_story_guidance_value(source, field_name)
        for field_name, value in raw_overrides.items()
    }


STORY_REPAIR_SOURCE_PROMPT_LABELS: Dict[str, str] = {
    "manual_request": "手动要求",
    "current_chapter_quality": "当前章节质量",
    "recent_history_summary": "近期质量趋势",
    "manual_plus_current_chapter_quality": "手动要求 + 当前章节质量",
    "manual_plus_recent_history_summary": "手动要求 + 近期质量趋势",
}
STORY_REPAIR_FOCUS_PROMPT_LABELS: Dict[str, str] = {
    "conflict": "冲突链推进",
    "rule_grounding": "规则落地",
    "outline": "大纲贴合",
    "dialogue": "对白自然度",
    "opening": "开场钩子",
    "payoff": "回报兑现",
    "cliffhanger": "章尾牵引",
    "pacing": "节奏稳定度",
}
STORY_REPAIR_QUALITY_SIGNAL_SOURCES = {
    "current_chapter_quality",
    "recent_history_summary",
    "manual_plus_current_chapter_quality",
    "manual_plus_recent_history_summary",
}


STORY_REPAIR_ACTION_GUIDANCE_DEFAULTS: Dict[str, Dict[str, str]] = {
    "rewrite_opening": {
        "creative_mode": "hook",
        "story_focus": "advance_plot",
        "quality_preset": "plot_drive",
        "story_creation_brief": "本轮优先重写开场钩子，尽快建立目标、悬念与冲突牵引。",
        "quality_notes": "强化开篇吸引力，确保前三段交代目标、阻力与读者期待。",
    },
    "strengthen_dialogue": {
        "creative_mode": "relationship",
        "story_focus": "relationship_shift",
        "quality_preset": "emotion_drama",
        "story_creation_brief": "本轮用对话推进关系变化，让冲突、试探与态度转折更可感。",
        "quality_notes": "提高对白张力与潜台词密度，让人物关系通过说话方式和行动反馈显性化。",
    },
    "patch_payoff": {
        "creative_mode": "payoff",
        "story_focus": "foreshadow_payoff",
        "quality_preset": "plot_drive",
        "story_creation_brief": "本轮优先补强伏笔兑现，让前文承诺在关键节点得到回应或反转。",
        "quality_notes": "强化回报兑现与因果闭环，明确伏笔触发、兑现结果与情绪释放。",
    },
    "bridge_scene": {
        "creative_mode": "suspense",
        "story_focus": "advance_plot",
        "quality_preset": "plot_drive",
        "story_creation_brief": "本轮补桥关键场景，把剧情推进、角色状态变化与章节衔接写扎实。",
        "quality_notes": "强化场景衔接与推进效率，确保目标、行动、阻碍和结果完整落地。",
    },
    "grounding_pass": {
        "creative_mode": "balanced",
        "quality_preset": "immersive",
        "story_creation_brief": "本轮先校准设定落地，让规则、组织、职业与资源约束真正进入情节。",
        "quality_notes": "强化设定约束在动作、对话和结果中的体现，避免信息悬空。",
    },
}
STORY_REPAIR_ACTION_MODE_FALLBACKS: Dict[str, str] = {
    "rewrite": "rewrite_opening",
    "dialogue": "strengthen_dialogue",
    "payoff": "patch_payoff",
    "bridge": "bridge_scene",
    "grounding": "grounding_pass",
}


def _resolve_story_repair_guidance_defaults(
    active_story_repair_payload: Optional[Mapping[str, Any]],
) -> Dict[str, Optional[str]]:
    if not isinstance(active_story_repair_payload, Mapping):
        return {}

    action_key = _normalize_optional_text(active_story_repair_payload.get("recommended_action"))
    if not action_key:
        mode_key = _normalize_optional_text(active_story_repair_payload.get("recommended_action_mode"))
        if mode_key:
            action_key = STORY_REPAIR_ACTION_MODE_FALLBACKS.get(mode_key)
    if not action_key:
        return {}

    base_defaults = STORY_REPAIR_ACTION_GUIDANCE_DEFAULTS.get(action_key)
    if not base_defaults:
        return {}

    resolved = dict(base_defaults)
    weakest_metric_label = _normalize_optional_text(active_story_repair_payload.get("weakest_metric_label"))
    focus_area = _normalize_optional_text(active_story_repair_payload.get("recommended_focus_area"))
    summary = _normalize_optional_text(active_story_repair_payload.get("summary"))
    focus_label = STORY_REPAIR_FOCUS_PROMPT_LABELS.get(focus_area, focus_area) if focus_area else None

    note_segments: List[str] = []
    if focus_label:
        note_segments.append(f"优先修补：{focus_label}。")
    if weakest_metric_label:
        note_segments.append(f"当前最弱指标：{weakest_metric_label}。")
    if summary:
        note_segments.append(summary)

    if note_segments:
        extra_notes = " ".join(note_segments)
        resolved["quality_notes"] = " ".join(
            segment for segment in [resolved.get("quality_notes"), extra_notes] if segment
        )

    return _normalize_story_guidance_values(resolved)


def _apply_story_repair_guidance_defaults(
    project: Optional[Project],
    story_packet: StoryPacket,
    active_story_repair_payload: Optional[Mapping[str, Any]],
) -> StoryPacket:
    derived_defaults = _resolve_story_repair_guidance_defaults(active_story_repair_payload)
    if not derived_defaults:
        return story_packet

    merged_overrides = dict(derived_defaults)
    merged_overrides.update(story_packet.request_overrides)
    next_guidance = resolve_story_generation_guidance(project, **merged_overrides)
    if next_guidance == story_packet.guidance:
        return story_packet
    return replace(story_packet, guidance=next_guidance)


def _normalize_story_repair_prompt_items(
    values: Optional[Sequence[str]],
    *,
    limit: int = 4,
) -> List[str]:
    if not values:
        return []

    items: List[str] = []
    seen: set[str] = set()
    for raw in values:
        text = str(raw or "").strip()
        if not text:
            continue
        label = STORY_REPAIR_FOCUS_PROMPT_LABELS.get(text, text)
        if label in seen:
            continue
        seen.add(label)
        items.append(label)
        if len(items) >= limit:
            break
    return items


def _format_story_repair_metric_value(value: Any) -> Optional[str]:
    if not isinstance(value, (int, float)):
        return None
    numeric = float(value)
    if numeric.is_integer():
        return str(int(numeric))
    return f"{numeric:.1f}"


def build_story_repair_diagnostic_context(
    active_story_repair_payload: Optional[Mapping[str, Any]],
    *,
    scene: str = "chapter",
) -> Dict[str, Any]:
    empty = {
        "story_repair_source": "",
        "story_repair_source_label": "",
        "story_repair_focus_areas": [],
        "story_repair_weakest_metric_label": "",
        "story_repair_weakest_metric_value": None,
        "story_repair_quality_gate_label": "",
        "story_repair_quality_gate_decision": "",
        "story_repair_quality_gate_summary": "",
        "story_repair_quality_gate_failed_metrics": [],
        "story_repair_diagnostic_block": "",
    }
    if not isinstance(active_story_repair_payload, Mapping):
        return empty

    source = str(active_story_repair_payload.get("source") or "").strip()
    fallback_source_label = str(active_story_repair_payload.get("source_label") or source or "").strip()
    source_label = STORY_REPAIR_SOURCE_PROMPT_LABELS.get(source, fallback_source_label)
    focus_areas = _normalize_story_repair_prompt_items(active_story_repair_payload.get("focus_areas"), limit=4)
    weakest_metric_label = str(active_story_repair_payload.get("weakest_metric_label") or "").strip()
    weakest_metric_value = (
        active_story_repair_payload.get("weakest_metric_value")
        if isinstance(active_story_repair_payload.get("weakest_metric_value"), (int, float))
        else None
    )
    weakest_metric_text = _format_story_repair_metric_value(weakest_metric_value)
    summary = str(active_story_repair_payload.get("summary") or "").strip()
    quality_gate_label = str(active_story_repair_payload.get("quality_gate_label") or "").strip()
    quality_gate_decision = str(active_story_repair_payload.get("quality_gate_decision") or "").strip()
    quality_gate_summary = str(active_story_repair_payload.get("quality_gate_summary") or "").strip()
    quality_gate_failed_metrics = _normalize_story_repair_prompt_items(
        active_story_repair_payload.get("quality_gate_failed_metrics"),
        limit=4,
    )

    closing_line = "先把最弱项拆成每章的目标、阻力、回报与章尾牵引，再统一分配节拍。" if scene == "outline" else "先修最弱项对应的事件、动作与后果，再统一润色语言。"
    diagnostic_block = ""
    if (
        weakest_metric_label
        or focus_areas
        or quality_gate_label
        or quality_gate_summary
        or source in STORY_REPAIR_QUALITY_SIGNAL_SOURCES
    ):
        lines = ["【诊断优先级卡】"]
        if source_label:
            lines.append(f"- 诊断来源：{source_label}")
        if quality_gate_label:
            gate_line = quality_gate_label
            if quality_gate_summary and quality_gate_summary != summary:
                gate_line = f"{gate_line}（{quality_gate_summary}）"
            lines.append(f"- 质量门禁：{gate_line}")
        elif quality_gate_summary:
            lines.append(f"- 质量门禁：{quality_gate_summary}")
        if quality_gate_failed_metrics:
            lines.append("- 门禁失败维度：" + " / ".join(quality_gate_failed_metrics))
        if weakest_metric_label:
            metric_line = weakest_metric_label
            if weakest_metric_text:
                metric_line = f"{metric_line}（当前值：{weakest_metric_text}）"
            lines.append(f"- 当前最弱项：{metric_line}")
        if focus_areas:
            lines.append("- 优先修复维度：" + " / ".join(focus_areas))
        if summary:
            lines.append(f"- 诊断结论：{summary}")
        lines.append(f"- {closing_line}")
        diagnostic_block = "\n".join(lines)

    return {
        "story_repair_source": source,
        "story_repair_source_label": source_label,
        "story_repair_focus_areas": focus_areas,
        "story_repair_weakest_metric_label": weakest_metric_label,
        "story_repair_weakest_metric_value": weakest_metric_value,
        "story_repair_quality_gate_label": quality_gate_label,
        "story_repair_quality_gate_decision": quality_gate_decision,
        "story_repair_quality_gate_summary": quality_gate_summary,
        "story_repair_quality_gate_failed_metrics": quality_gate_failed_metrics,
        "story_repair_diagnostic_block": diagnostic_block,
    }


def _build_story_repair_diagnostic_context(
    active_story_repair_payload: Optional[Mapping[str, Any]],
    *,
    scene: str = "chapter",
) -> Dict[str, Any]:
    return build_story_repair_diagnostic_context(active_story_repair_payload, scene=scene)


def resolve_story_generation_guidance(
    project: Optional[Project],
    *,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
) -> StoryGenerationGuidance:
    normalized_overrides = _normalize_story_guidance_values({
        "creative_mode": creative_mode,
        "story_focus": story_focus,
        "plot_stage": plot_stage,
        "story_creation_brief": story_creation_brief,
        "quality_preset": quality_preset,
        "quality_notes": quality_notes,
    })
    if project is None:
        return StoryGenerationGuidance(**normalized_overrides)

    resolved = resolve_project_generation_defaults(project, **normalized_overrides)
    return StoryGenerationGuidance(**resolved)


def build_story_generation_packet(
    project: Optional[Project],
    source: Optional[Any] = None,
    *,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
    source_label: Optional[str] = None,
) -> StoryPacket:
    raw_overrides = _extract_story_guidance_overrides(
        source,
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
    )
    guidance = resolve_story_generation_guidance(project, **raw_overrides)
    request_overrides = {
        field_name: value
        for field_name, value in _normalize_story_guidance_values(raw_overrides).items()
        if value is not None
    }
    resolved_chapter_count = (
        _normalize_optional_int(_read_story_guidance_value(source, "chapter_count"))
        or _normalize_optional_int(getattr(project, "chapter_count", None))
    )
    resolved_target_word_count = (
        _normalize_optional_int(_read_story_guidance_value(source, "target_word_count"))
        or _normalize_optional_int(_read_story_guidance_value(source, "target_words"))
        or _normalize_optional_int(getattr(project, "target_words", None))
    )
    blueprint = StoryBlueprint(
        long_term_goal=_build_story_long_term_goal_text(
            project,
            guidance,
            chapter_count=resolved_chapter_count,
            target_word_count=resolved_target_word_count,
        ),
        chapter_count=resolved_chapter_count,
        target_word_count=resolved_target_word_count,
        character_focus_names=_extract_story_packet_character_focus(source),
        foreshadow_payoff_plan=_extract_story_packet_foreshadow_payoff_plan(source),
        character_state_ledger=_extract_story_packet_character_state_ledger(source),
        relationship_state_ledger=_extract_story_packet_relationship_state_ledger(source),
        foreshadow_state_ledger=_extract_story_packet_foreshadow_state_ledger(source),
        organization_state_ledger=_extract_story_packet_organization_state_ledger(source),
        career_state_ledger=_extract_story_packet_career_state_ledger(source),
    )
    return StoryPacket(
        guidance=guidance,
        request_overrides=request_overrides,
        source=source_label,
        blueprint=blueprint,
    )


async def enrich_story_packet_with_project_continuity(
    db_session: AsyncSession,
    project: Optional[Project],
    story_packet: StoryPacket,
) -> StoryPacket:
    """在 story packet 缺少状态账本时，用项目 continuity ledger 回填。"""

    project_id = getattr(project, "id", None)
    if not project_id:
        return story_packet

    blueprint = story_packet.blueprint
    missing_character_state = not blueprint.character_state_ledger
    missing_relationship_state = not blueprint.relationship_state_ledger
    missing_foreshadow_state = not blueprint.foreshadow_state_ledger
    missing_organization_state = not blueprint.organization_state_ledger
    missing_career_state = not blueprint.career_state_ledger
    if not (
        missing_character_state
        or missing_relationship_state
        or missing_foreshadow_state
        or missing_organization_state
        or missing_career_state
    ):
        return story_packet

    continuity_ledger = await build_project_continuity_ledger(db_session, project_id)
    if not continuity_ledger.has_any_entries():
        return story_packet

    return story_packet.with_blueprint(
        character_state_source=(
            {"story_character_state_ledger": continuity_ledger.character_state_ledger}
            if missing_character_state and continuity_ledger.character_state_ledger
            else None
        ),
        relationship_state_source=(
            {"story_relationship_state_ledger": continuity_ledger.relationship_state_ledger}
            if missing_relationship_state and continuity_ledger.relationship_state_ledger
            else None
        ),
        foreshadow_state_source=(
            {"story_foreshadow_state_ledger": continuity_ledger.foreshadow_state_ledger}
            if missing_foreshadow_state and continuity_ledger.foreshadow_state_ledger
            else None
        ),
        organization_state_source=(
            {"story_organization_state_ledger": continuity_ledger.organization_state_ledger}
            if missing_organization_state and continuity_ledger.organization_state_ledger
            else None
        ),
        career_state_source=(
            {"story_career_state_ledger": continuity_ledger.career_state_ledger}
            if missing_career_state and continuity_ledger.career_state_ledger
            else None
        ),
    )


async def build_story_generation_packet_with_project_continuity(
    db_session: AsyncSession,
    project: Optional[Project],
    source: Optional[Any] = None,
    *,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
    source_label: Optional[str] = None,
) -> StoryPacket:
    """构建带项目 continuity ledger 回填能力的 story packet。"""

    packet = build_story_generation_packet(
        project,
        source=source,
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
        source_label=source_label,
    )
    return await enrich_story_packet_with_project_continuity(db_session, project, packet)


@dataclass(frozen=True)
class ChapterGenerationIntent:
    story_packet: StoryPacket
    quality_profile: Optional[Dict[str, Any]] = None
    project: Optional[Project] = None
    chapter: Optional[Any] = None
    chapter_context: Optional[Any] = None
    target_word_count: Optional[int] = None
    story_repair_payload: Optional[StoryRepairPayload] = None
    active_story_repair_payload: Optional[Mapping[str, Any]] = None
    quality_history_context: Optional[Mapping[str, Any]] = None
    quality_metrics_summary: Optional[Mapping[str, Any]] = None
    character_focus_source: Optional[Any] = None
    foreshadow_payoff_source: Optional[Any] = None
    character_state_source: Optional[Any] = None
    relationship_state_source: Optional[Any] = None
    foreshadow_state_source: Optional[Any] = None
    organization_state_source: Optional[Any] = None
    career_state_source: Optional[Any] = None

    def _build_runtime_source(self) -> Optional[Any]:
        merged: Dict[str, Any] = {}
        history_context = dict(self.quality_history_context) if isinstance(self.quality_history_context, Mapping) else {}
        if history_context:
            merged.update(history_context)

        chapter_context = self.chapter_context
        if chapter_context is not None:
            chapter_context_fields = {
                "character_arc_snapshot": getattr(chapter_context, "character_arc_snapshot", None),
                "foreshadow_reminders": getattr(chapter_context, "foreshadow_reminders", None),
                "chapter_careers": getattr(chapter_context, "chapter_careers", None),
                "chapter_characters": getattr(chapter_context, "chapter_characters", None),
            }
            for key, value in chapter_context_fields.items():
                if value:
                    merged[key] = value

        if merged:
            return merged
        if chapter_context is not None:
            return chapter_context
        if history_context:
            return history_context
        return None

    def build_prompt_quality_kwargs(self) -> Dict[str, Any]:
        runtime_source = self._build_runtime_source()
        return self.story_packet.build_prompt_quality_kwargs(
            self.quality_profile,
            story_repair_payload=self.story_repair_payload,
            active_story_repair_payload=self.active_story_repair_payload,
            chapter_count=getattr(self.project, "chapter_count", None),
            current_chapter_number=getattr(self.chapter, "chapter_number", None),
            target_word_count=self.target_word_count,
            quality_metrics_summary=self.quality_metrics_summary,
            character_focus_source=(
                self.character_focus_source
                if self.character_focus_source is not None
                else (self.chapter if self.chapter is not None else runtime_source)
            ),
            foreshadow_payoff_source=(
                self.foreshadow_payoff_source
                if self.foreshadow_payoff_source is not None
                else runtime_source
            ),
            character_state_source=(
                self.character_state_source
                if self.character_state_source is not None
                else runtime_source
            ),
            relationship_state_source=(
                self.relationship_state_source
                if self.relationship_state_source is not None
                else runtime_source
            ),
            foreshadow_state_source=(
                self.foreshadow_state_source
                if self.foreshadow_state_source is not None
                else runtime_source
            ),
            organization_state_source=(
                self.organization_state_source
                if self.organization_state_source is not None
                else runtime_source
            ),
            career_state_source=(
                self.career_state_source
                if self.career_state_source is not None
                else runtime_source
            ),
        )

    def build_quality_runtime_context(self) -> Dict[str, Any]:
        runtime_source = self._build_runtime_source()
        profile_source = self.quality_profile if isinstance(self.quality_profile, Mapping) else {}
        return self.story_packet.build_quality_runtime_context(
            chapter_count=getattr(self.project, "chapter_count", None),
            current_chapter_number=getattr(self.chapter, "chapter_number", None),
            target_word_count=self.target_word_count,
            character_focus_source=(
                self.character_focus_source
                if self.character_focus_source is not None
                else (self.chapter if self.chapter is not None else runtime_source)
            ),
            foreshadow_payoff_source=(
                self.foreshadow_payoff_source
                if self.foreshadow_payoff_source is not None
                else runtime_source
            ),
            character_state_source=(
                self.character_state_source
                if self.character_state_source is not None
                else runtime_source
            ),
            relationship_state_source=(
                self.relationship_state_source
                if self.relationship_state_source is not None
                else runtime_source
            ),
            foreshadow_state_source=(
                self.foreshadow_state_source
                if self.foreshadow_state_source is not None
                else runtime_source
            ),
            organization_state_source=(
                self.organization_state_source
                if self.organization_state_source is not None
                else runtime_source
            ),
            career_state_source=(
                self.career_state_source
                if self.career_state_source is not None
                else runtime_source
            ),
            genre=profile_source.get("genre") or getattr(self.project, "genre", None),
            style_name=profile_source.get("style_name"),
            style_preset_id=profile_source.get("style_preset_id"),
            style_content=profile_source.get("style_content"),
            style_profile=profile_source.get("style_profile"),
            genre_profiles=profile_source.get("genre_profiles"),
        )


def build_chapter_generation_intent(

    *,
    story_packet: Optional[StoryPacket],
    quality_profile: Optional[Dict[str, Any]],
    project: Optional[Project],
    chapter: Optional[Any],
    chapter_context: Optional[Any],
    target_word_count: Optional[int],
    story_repair_payload: Optional[StoryRepairPayload] = None,
    active_story_repair_payload: Optional[Mapping[str, Any]] = None,
    quality_history_context: Optional[Mapping[str, Any]] = None,
    quality_metrics_summary: Optional[Mapping[str, Any]] = None,
    character_focus_source: Optional[Any] = None,
    foreshadow_payoff_source: Optional[Any] = None,
    character_state_source: Optional[Any] = None,
    relationship_state_source: Optional[Any] = None,
    foreshadow_state_source: Optional[Any] = None,
    organization_state_source: Optional[Any] = None,
    career_state_source: Optional[Any] = None,
) -> ChapterGenerationIntent:
    active_story_packet = story_packet or build_story_generation_packet(
        project,
        source=chapter,
        source_label="chapter-generation-intent",
    )
    active_story_packet = _apply_story_repair_guidance_defaults(
        project,
        active_story_packet,
        active_story_repair_payload,
    )
    return ChapterGenerationIntent(
        story_packet=active_story_packet,
        quality_profile=quality_profile,
        project=project,
        chapter=chapter,
        chapter_context=chapter_context,
        target_word_count=target_word_count,
        story_repair_payload=story_repair_payload,
        active_story_repair_payload=active_story_repair_payload,
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


def build_prompt_quality_kwargs(
    profile: Optional[Dict[str, Any]],
    *,
    guidance: Optional[StoryGenerationGuidance] = None,
    scene: str = "chapter",
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[List[str]] = None,
    story_preserve_strengths: Optional[List[str]] = None,
    active_story_repair_payload: Optional[Mapping[str, Any]] = None,
    story_repair_payload: Optional[StoryRepairPayload] = None,
    story_long_term_goal: Optional[str] = None,
    story_character_focus: Optional[Sequence[str]] = None,
    story_foreshadow_payoff_plan: Optional[Sequence[str]] = None,
    chapter_count: Optional[int] = None,
    current_chapter_number: Optional[int] = None,
    target_word_count: Optional[int] = None,
    quality_metrics_summary: Optional[Mapping[str, Any]] = None,
    story_character_state_ledger: Optional[Sequence[str]] = None,
    story_relationship_state_ledger: Optional[Sequence[str]] = None,
    story_foreshadow_state_ledger: Optional[Sequence[str]] = None,
    story_organization_state_ledger: Optional[Sequence[str]] = None,
    story_career_state_ledger: Optional[Sequence[str]] = None,
) -> Dict[str, Any]:
    source = profile or {}
    active_guidance = guidance or StoryGenerationGuidance()
    external_assets = source.get("external_assets") or ()
    reference_assets = source.get("reference_assets") or external_assets or ()
    resolved_story_repair_kwargs = resolve_story_repair_prompt_kwargs(
        story_repair_payload,
        summary=story_repair_summary,
        targets=story_repair_targets,
        strengths=story_preserve_strengths,
    )
    story_repair_summary = resolved_story_repair_kwargs.get("story_repair_summary")
    story_repair_targets = resolved_story_repair_kwargs.get("story_repair_targets")
    story_preserve_strengths = resolved_story_repair_kwargs.get("story_preserve_strengths")
    repair_diagnostic_context = build_story_repair_diagnostic_context(active_story_repair_payload, scene=scene)
    resolved_story_long_term_goal = _normalize_optional_text(story_long_term_goal)
    resolved_story_character_focus = _normalize_string_sequence(story_character_focus, limit=4)
    resolved_story_foreshadow_payoff_plan = _normalize_string_sequence(story_foreshadow_payoff_plan, limit=3)
    resolved_story_character_state_ledger = _normalize_string_sequence(story_character_state_ledger, limit=4)
    resolved_story_relationship_state_ledger = _normalize_string_sequence(story_relationship_state_ledger, limit=4)
    resolved_story_foreshadow_state_ledger = _normalize_string_sequence(story_foreshadow_state_ledger, limit=4)
    resolved_story_organization_state_ledger = _normalize_string_sequence(story_organization_state_ledger, limit=4)
    resolved_story_career_state_ledger = _normalize_string_sequence(story_career_state_ledger, limit=4)
    resolved_chapter_count = _normalize_optional_int(chapter_count)
    resolved_current_chapter_number = _normalize_optional_int(current_chapter_number)
    resolved_target_word_count = _normalize_optional_int(target_word_count)
    resolved_quality_metrics_summary = dict(quality_metrics_summary) if isinstance(quality_metrics_summary, Mapping) else None

    return {
        "genre": source.get("genre") or "未设定",
        "style_name": source.get("style_name") or "",
        "style_preset_id": source.get("style_preset_id") or "",
        "style_content": source.get("style_content") or "",
        "external_assets": external_assets,
        "reference_assets": reference_assets,
        "mcp_guard": source.get("mcp_guard") or "",
        "mcp_references": source.get("mcp_references") or "",
        **active_guidance.to_prompt_fields(),
        "story_long_term_goal": resolved_story_long_term_goal or "",
        "story_character_focus": list(resolved_story_character_focus),
        "story_foreshadow_payoff_plan": list(resolved_story_foreshadow_payoff_plan),
        "story_character_state_ledger": list(resolved_story_character_state_ledger),
        "story_relationship_state_ledger": list(resolved_story_relationship_state_ledger),
        "story_foreshadow_state_ledger": list(resolved_story_foreshadow_state_ledger),
        "story_organization_state_ledger": list(resolved_story_organization_state_ledger),
        "story_career_state_ledger": list(resolved_story_career_state_ledger),
        "creative_mode_block": build_creative_mode_block(active_guidance.creative_mode, scene=scene),
        "story_focus_block": build_story_focus_block(active_guidance.story_focus, scene=scene),
        "story_creation_brief_block": build_story_creation_brief_block(active_guidance.story_creation_brief),
        "story_long_term_goal_block": build_story_long_term_goal_block(resolved_story_long_term_goal),
        "story_character_focus_anchor_block": build_story_character_focus_anchor_block(
            resolved_story_character_focus,
            scene=scene,
        ),
        "story_foreshadow_payoff_plan_block": build_story_foreshadow_payoff_plan_block(
            resolved_story_foreshadow_payoff_plan,
            scene=scene,
        ),
        "story_character_state_ledger_block": build_story_character_state_ledger_block(
            resolved_story_character_state_ledger,
            scene=scene,
        ),
        "story_relationship_state_ledger_block": build_story_relationship_state_ledger_block(
            resolved_story_relationship_state_ledger,
            scene=scene,
        ),
        "story_foreshadow_state_ledger_block": build_story_foreshadow_state_ledger_block(
            resolved_story_foreshadow_state_ledger,
            scene=scene,
        ),
        "story_organization_state_ledger_block": build_story_organization_state_ledger_block(
            resolved_story_organization_state_ledger,
            scene=scene,
        ),
        "story_career_state_ledger_block": build_story_career_state_ledger_block(
            resolved_story_career_state_ledger,
            scene=scene,
        ),
        "story_pacing_budget_block": build_story_pacing_budget_block(
            resolved_chapter_count,
            current_chapter_number=resolved_current_chapter_number,
            target_word_count=resolved_target_word_count,
            plot_stage=active_guidance.plot_stage,
            scene=scene,
        ),
        "story_volume_pacing_block": build_volume_pacing_block(
            resolved_chapter_count,
            plot_stage=active_guidance.plot_stage,
        ),
        "quality_metrics_summary": resolved_quality_metrics_summary,
        "story_quality_trend_block": build_story_quality_trend_block(
            resolved_quality_metrics_summary,
            scene=scene,
        ),
        "quality_preference_block": build_quality_preference_block(
            active_guidance.quality_preset,
            active_guidance.quality_notes,
            scene=scene,
        ),
        "story_repair_summary": story_repair_summary or "",
        "story_repair_targets": story_repair_targets or [],
        "story_preserve_strengths": story_preserve_strengths or [],
        "story_repair_target_block": build_story_repair_target_block(
            story_repair_summary,
            story_repair_targets,
            story_preserve_strengths,
        ),
        **repair_diagnostic_context,
        "narrative_blueprint_block": build_narrative_blueprint_block(
            active_guidance.creative_mode,
            active_guidance.story_focus,
            scene=scene,
            plot_stage=active_guidance.plot_stage,
        ),
    }

def build_analysis_quality_kwargs(
    profile: Optional[Dict[str, Any]],
    *,
    guidance: Optional[StoryGenerationGuidance] = None,
) -> Dict[str, Any]:
    source = profile or {}
    active_guidance = guidance or StoryGenerationGuidance()
    external_assets = source.get("external_assets") or ()
    reference_assets = source.get("reference_assets") or external_assets or ()
    return {
        "genre": source.get("genre") or "未设定",
        "style_name": source.get("style_name") or "",
        "style_preset_id": source.get("style_preset_id") or "",
        "style_content": source.get("style_content") or "",
        "external_assets": external_assets,
        "reference_assets": reference_assets,
        "mcp_references": source.get("mcp_references") or "",
        **active_guidance.to_prompt_fields(),
    }


async def _resolve_project_default_style_id(
    db_session: AsyncSession,
    project_id: str,
) -> Optional[int]:
    from app.models.project_default_style import ProjectDefaultStyle

    result = await db_session.execute(
        select(ProjectDefaultStyle.style_id)
        .where(ProjectDefaultStyle.project_id == project_id)
    )
    return result.scalar_one_or_none()


async def _resolve_effective_style_context(
    *,
    db_session: AsyncSession,
    user_id: str,
    project_id: str,
    style_id: Optional[int],
    prefer_project_default_style: bool = False,
    log_prefix: str = "章节",
) -> Dict[str, Any]:
    await sync_low_ai_presets(db_session)

    resolved_style_id = style_id
    if prefer_project_default_style and not resolved_style_id:
        resolved_style_id = await _resolve_project_default_style_id(db_session, project_id)
        if resolved_style_id:
            logger.info(f"📝 {log_prefix} - 使用项目默认写作风格: {resolved_style_id}")

    context = {
        "resolved_style_id": resolved_style_id,
        "style_name": "",
        "style_preset_id": "",
        "style_content": "",
    }
    if not resolved_style_id:
        return context

    style_result = await db_session.execute(
        select(WritingStyle).where(WritingStyle.id == resolved_style_id)
    )
    style = style_result.scalar_one_or_none()
    if not style:
        logger.warning(f"⚠️ {log_prefix} - 未找到风格 {resolved_style_id}")
        return context

    if style.user_id is not None and style.user_id != user_id:
        logger.warning(f"⚠️ {log_prefix} - 风格 {resolved_style_id} 不属于当前用户，跳过")
        return context

    context.update(
        {
            "resolved_style_id": resolved_style_id,
            "style_name": style.name or "",
            "style_preset_id": style.preset_id or "",
            "style_content": style.prompt_content or "",
        }
    )
    style_type = "全局预设" if style.user_id is None else "用户自定义"
    logger.info(f"✅ {log_prefix} - 使用写作风格: {style.name} ({style_type})")
    return context


async def resolve_chapter_quality_profile(
    *,
    db_session: AsyncSession,
    user_id: str,
    project: Optional[Project],
    style_id: Optional[int],
    enable_mcp: bool,
    prefer_project_default_style: bool = False,
    external_assets: Optional[List[Dict[str, Any]]] = None,
    reference_assets: Optional[List[Dict[str, Any]]] = None,
    log_prefix: str = "章节",
) -> Dict[str, Any]:
    normalized_external_assets = tuple(external_assets or ())
    normalized_reference_assets = tuple(reference_assets or normalized_external_assets or ())
    profile: Dict[str, Any] = {
        "genre": (project.genre if project else None) or "",
        "resolved_style_id": style_id,
        "style_name": "",
        "style_preset_id": "",
        "style_content": "",
        "external_assets": normalized_external_assets,
        "reference_assets": normalized_reference_assets,
        "mcp_guard": "",
        "mcp_references": "",
    }

    if project:
        profile.update(
            await _resolve_effective_style_context(
                db_session=db_session,
                user_id=user_id,
                project_id=project.id,
                style_id=style_id,
                prefer_project_default_style=prefer_project_default_style,
                log_prefix=log_prefix,
            )
        )

    if enable_mcp and user_id:
        try:
            prompt_blocks = await mcp_tools_loader.get_prompt_reference_blocks(
                user_id=user_id,
                db_session=db_session,
            )
            profile["mcp_guard"] = prompt_blocks.get("mcp_guard") or ""
            profile["mcp_references"] = prompt_blocks.get("mcp_references") or ""
        except Exception as mcp_error:
            logger.warning(f"⚠️ {log_prefix} - 获取MCP参考块失败，已回退为空: {mcp_error}")

    return profile


def clone_chapter_quality_profile(
    base_profile: Optional[Mapping[str, Any]],
    *,
    external_assets: Optional[List[Dict[str, Any]]] = None,
    reference_assets: Optional[List[Dict[str, Any]]] = None,
) -> Dict[str, Any]:
    """构建用于 Prompt 组装的章节质量运行时参数。"""
    profile = dict(base_profile or {})
    normalized_external_assets = tuple(external_assets or ())
    base_reference_assets = profile.get("reference_assets")
    if reference_assets is None:
        normalized_reference_assets = tuple(external_assets or base_reference_assets or ())
    else:
        normalized_reference_assets = tuple(reference_assets or ())

    profile["genre"] = str(profile.get("genre") or "")
    profile["resolved_style_id"] = profile.get("resolved_style_id")
    profile["style_name"] = str(profile.get("style_name") or "")
    profile["style_preset_id"] = str(profile.get("style_preset_id") or "")
    profile["style_content"] = str(profile.get("style_content") or "")
    profile["external_assets"] = normalized_external_assets
    profile["reference_assets"] = normalized_reference_assets
    profile["mcp_guard"] = str(profile.get("mcp_guard") or "")
    profile["mcp_references"] = str(profile.get("mcp_references") or "")
    return profile
