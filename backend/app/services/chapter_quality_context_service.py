from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, List, Mapping, Optional, Sequence

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.project import Project
from app.models.writing_style import WritingStyle
from app.services.mcp_tools_loader import mcp_tools_loader
from app.services.project_generation_defaults import resolve_project_generation_defaults
from app.services.prompt_service import (
    build_creative_mode_block,
    build_narrative_blueprint_block,
    build_quality_preference_block,
    build_story_creation_brief_block,
    build_story_focus_block,
    build_story_repair_target_block,
)
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
class StoryPacket:
    guidance: StoryGenerationGuidance
    request_overrides: Dict[str, Optional[str]] = field(default_factory=dict)
    source: Optional[str] = None

    def to_prompt_fields(self) -> Dict[str, Any]:
        return self.guidance.to_prompt_fields()

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
    ) -> Dict[str, Any]:
        return build_prompt_quality_kwargs(
            profile,
            guidance=self.guidance,
            scene=scene,
            story_repair_summary=story_repair_summary,
            story_repair_targets=story_repair_targets,
            story_preserve_strengths=story_preserve_strengths,
            active_story_repair_payload=active_story_repair_payload,
        )

    def build_analysis_quality_kwargs(
        self,
        profile: Optional[Dict[str, Any]],
    ) -> Dict[str, Any]:
        return build_analysis_quality_kwargs(profile, guidance=self.guidance)


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


def _read_story_guidance_value(source: Optional[Any], field_name: str) -> Optional[str]:
    if source is None:
        return None
    if isinstance(source, Mapping):
        return source.get(field_name)
    return getattr(source, field_name, None)


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

    closing_line = "先把最弱项拆成每章的目标、阻力、回报与章尾牵引，再统一分配节拍。" if scene == "outline" else "先修最弱项对应的事件、动作与后果，再统一润色语言。"
    diagnostic_block = ""
    if weakest_metric_label or focus_areas or source in STORY_REPAIR_QUALITY_SIGNAL_SOURCES:
        lines = ["【诊断优先级卡】"]
        if source_label:
            lines.append(f"- 诊断来源：{source_label}")
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
    return StoryPacket(
        guidance=guidance,
        request_overrides=request_overrides,
        source=source_label,
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
) -> Dict[str, Any]:
    source = profile or {}
    active_guidance = guidance or StoryGenerationGuidance()
    external_assets = source.get("external_assets") or ()
    reference_assets = source.get("reference_assets") or external_assets or ()
    repair_diagnostic_context = build_story_repair_diagnostic_context(active_story_repair_payload, scene=scene)

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
        "creative_mode_block": build_creative_mode_block(active_guidance.creative_mode, scene=scene),
        "story_focus_block": build_story_focus_block(active_guidance.story_focus, scene=scene),
        "story_creation_brief_block": build_story_creation_brief_block(active_guidance.story_creation_brief),
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
