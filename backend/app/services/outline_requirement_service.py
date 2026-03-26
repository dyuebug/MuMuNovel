"""大纲生成要求组装服务。"""

from __future__ import annotations

from typing import Optional

from app.services.chapter_quality_context_service import (
    StoryGenerationGuidance,
    StoryPacket,
    build_story_runtime_requirement_text,
)
from app.services.prompt_service import (
    build_creative_mode_block,
    build_narrative_blueprint_block,
    build_quality_preference_block,
    build_story_acceptance_card_block,
    build_story_action_rendering_card_block,
    build_story_character_arc_card_block,
    build_story_cliffhanger_card_block,
    build_story_dialogue_advancement_card_block,
    build_story_emotion_landing_card_block,
    build_story_execution_checklist_block,
    build_story_focus_block,
    build_story_information_release_card_block,
    build_story_objective_card_block,
    build_story_opening_hook_card_block,
    build_story_payoff_chain_card_block,
    build_story_repetition_control_card_block,
    build_story_repetition_risk_block,
    build_story_result_card_block,
    build_story_rule_grounding_card_block,
    build_story_scene_anchor_card_block,
    build_story_scene_density_card_block,
    build_story_summary_tone_control_card_block,
    build_story_viewpoint_discipline_card_block,
)

OUTLINE_SCENE = "outline"


def _append_if_present(parts: list[str], block: Optional[str]) -> None:
    normalized = str(block or "").strip()
    if normalized:
        parts.append(normalized)


def resolve_outline_guidance(
    *,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
    guidance: Optional[StoryGenerationGuidance] = None,
    story_packet: Optional[StoryPacket] = None,
) -> StoryGenerationGuidance:
    return (story_packet.guidance if story_packet is not None else guidance) or StoryGenerationGuidance(
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
    )


def build_outline_guidance_blocks(active_guidance: StoryGenerationGuidance) -> list[str]:
    creative_mode = active_guidance.creative_mode
    story_focus = active_guidance.story_focus
    plot_stage = active_guidance.plot_stage

    blocks = [
        build_quality_preference_block(active_guidance.quality_preset, active_guidance.quality_notes, scene=OUTLINE_SCENE),
        build_creative_mode_block(creative_mode, scene=OUTLINE_SCENE),
        build_story_focus_block(story_focus, scene=OUTLINE_SCENE),
        build_narrative_blueprint_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_objective_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_result_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_payoff_chain_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_rule_grounding_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_information_release_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_emotion_landing_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_action_rendering_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_summary_tone_control_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_repetition_control_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_viewpoint_discipline_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_dialogue_advancement_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_opening_hook_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_execution_checklist_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_scene_anchor_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_scene_density_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_repetition_risk_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_acceptance_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_cliffhanger_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_character_arc_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
    ]
    return [str(block).strip() for block in blocks if str(block or "").strip()]


def build_opening_outline_constraints_block(outline_count: int) -> str:
    return (
        f"【开局大纲约束】这是小说的开局部分，请生成{outline_count}个大纲节点，重点关注：\n"
        "1. 引入主要角色和世界观设定\n"
        "2. 建立主线冲突和故事钩子\n"
        "3. 展开初期情节，为后续发展埋下伏笔\n"
        "4. 若包含第1-3章，尽量体现黄金三章节奏（钩子→升级→小高潮）\n"
        "5. 每章至少一个小爽点与一个章尾钩子，避免平推\n"
        "6. 不要试图完结故事，这只是开始部分\n"
        "7. 不要在JSON字符串值中使用中文引号（\"\"''），请使用【】或《》标记"
    )


def build_outline_generation_requirements(
    base_requirements: Optional[str],
    *,
    chapter_count: Optional[int] = None,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
    memory_guidance: Optional[str] = None,
    quality_repair_guidance: Optional[str] = None,
    quality_trend_guidance: Optional[str] = None,
    guidance: Optional[StoryGenerationGuidance] = None,
    story_packet: Optional[StoryPacket] = None,
    opening_outline_count: Optional[int] = None,
) -> str:
    active_guidance = resolve_outline_guidance(
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
        guidance=guidance,
        story_packet=story_packet,
    )

    parts: list[str] = []
    _append_if_present(
        parts,
        build_story_runtime_requirement_text(
            base_requirements,
            guidance=active_guidance,
            story_packet=story_packet,
            chapter_count=chapter_count,
            memory_guidance=memory_guidance,
            quality_repair_guidance=quality_repair_guidance,
            quality_trend_guidance=quality_trend_guidance,
            scene=OUTLINE_SCENE,
        ),
    )
    parts.extend(build_outline_guidance_blocks(active_guidance))
    if opening_outline_count is not None and opening_outline_count > 0:
        _append_if_present(parts, build_opening_outline_constraints_block(opening_outline_count))
    return "\n\n".join(parts)
