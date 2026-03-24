from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, List, Optional

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

    def to_prompt_fields(self) -> Dict[str, Any]:
        return {
            "creative_mode": self.creative_mode or "",
            "story_focus": self.story_focus or "",
            "plot_stage": self.plot_stage or "",
            "story_creation_brief": self.story_creation_brief or "",
            "quality_preset": self.quality_preset or "",
            "quality_notes": self.quality_notes or "",
        }


def _normalize_optional_text(value: Optional[str]) -> Optional[str]:
    if value is None:
        return None
    normalized = str(value).strip()
    return normalized or None


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
    if project is None:
        return StoryGenerationGuidance(
            creative_mode=creative_mode or None,
            story_focus=story_focus or None,
            plot_stage=plot_stage or None,
            story_creation_brief=_normalize_optional_text(story_creation_brief),
            quality_preset=quality_preset or None,
            quality_notes=_normalize_optional_text(quality_notes),
        )

    resolved = resolve_project_generation_defaults(
        project,
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
    )
    return StoryGenerationGuidance(**resolved)


def build_prompt_quality_kwargs(
    profile: Optional[Dict[str, Any]],
    *,
    guidance: Optional[StoryGenerationGuidance] = None,
    scene: str = "chapter",
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[List[str]] = None,
    story_preserve_strengths: Optional[List[str]] = None,
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
