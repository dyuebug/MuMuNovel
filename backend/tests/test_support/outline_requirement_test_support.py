"""大纲 requirement 测试支持 owner。"""

from __future__ import annotations

import re
from typing import List, Optional

from tests.test_support.story_packet_test_support import (
    StoryBlueprint,
    StoryGenerationGuidance,
    StoryPacket,
    _normalize_optional_int,
)
from tests.test_support.story_prompt_block_test_support import (
    build_creative_mode_block,
    build_narrative_blueprint_block,
    build_quality_preference_block,
    build_story_acceptance_card_block,
    build_story_action_rendering_card_block,
    build_story_career_state_ledger_block,
    build_story_character_arc_card_block,
    build_story_character_focus_anchor_block,
    build_story_character_state_ledger_block,
    build_story_cliffhanger_card_block,
    build_story_creation_brief_block,
    build_story_dialogue_advancement_card_block,
    build_story_emotion_landing_card_block,
    build_story_execution_checklist_block,
    build_story_focus_block,
    build_story_foreshadow_payoff_plan_block,
    build_story_foreshadow_state_ledger_block,
    build_story_information_release_card_block,
    build_story_long_term_goal_block,
    build_story_objective_card_block,
    build_story_opening_hook_card_block,
    build_story_organization_state_ledger_block,
    build_story_payoff_chain_card_block,
    build_story_pacing_budget_block,
    build_story_relationship_state_ledger_block,
    build_story_repetition_control_card_block,
    build_story_repetition_risk_block,
    build_story_result_card_block,
    build_story_rule_grounding_card_block,
    build_story_scene_anchor_card_block,
    build_story_scene_density_card_block,
    build_story_summary_tone_control_card_block,
    build_story_viewpoint_discipline_card_block,
    build_volume_pacing_block,
)

OUTLINE_SCENE = "outline"
OUTLINE_RUNTIME_REQUIREMENT_BLOCK_LIMITS: dict[str, int] = {
    "base_requirements": 520,
    "story_creation_brief": 220,
    "quality_repair_guidance": 320,
    "memory_guidance": 820,
    "story_long_term_goal": 220,
    "story_character_focus_anchor": 180,
    "story_foreshadow_payoff_plan": 240,
    "story_relationship_state_ledger": 220,
    "story_character_state_ledger": 220,
    "quality_trend_guidance": 240,
    "story_organization_state_ledger": 200,
    "story_career_state_ledger": 200,
    "story_foreshadow_state_ledger": 200,
    "story_pacing_budget": 180,
    "story_volume_pacing": 160,
}
OUTLINE_RUNTIME_REQUIREMENT_TOTAL_LIMIT = 3600


def _split_sentences(text: str) -> List[str]:
    parts = re.split(r"[。！？!?；;\n]+", text)
    return [part.strip() for part in parts if part.strip()]


def _append_if_present(parts: list[str], block: Optional[str]) -> None:
    normalized = str(block or "").strip()
    if normalized:
        parts.append(normalized)


def _ellipsize_story_runtime_text(text: str, limit: int) -> str:
    normalized = str(text or "").strip()
    if limit <= 0:
        return ""
    if len(normalized) <= limit:
        return normalized
    if limit <= 3:
        return normalized[:limit]
    return normalized[: limit - 3].rstrip() + "..."


def _truncate_story_runtime_block(block: Optional[str], limit: int) -> str:
    normalized = str(block or "").strip()
    if not normalized or limit <= 0 or len(normalized) <= limit:
        return normalized

    lines = [line.strip() for line in normalized.splitlines() if line.strip()]
    if not lines:
        return ""
    if len(lines) == 1:
        return _ellipsize_story_runtime_text(lines[0], limit)

    head = lines[0]
    if len(head) >= limit:
        return _ellipsize_story_runtime_text(head, limit)

    kept_lines = [head]
    current_length = len(head)
    for line in lines[1:]:
        separator_length = 1
        projected_length = current_length + separator_length + len(line)
        if projected_length <= limit:
            kept_lines.append(line)
            current_length = projected_length
            continue

        remaining = limit - current_length - separator_length
        if remaining > 6:
            kept_lines.append(_ellipsize_story_runtime_text(line, remaining))
        elif kept_lines:
            kept_lines[-1] = kept_lines[-1].rstrip(".") + "..."
        break

    return "\n".join(kept_lines)


def _join_story_runtime_blocks_with_budget(
    blocks: list[str],
    *,
    total_limit: Optional[int] = None,
) -> str:
    normalized_blocks = [str(block).strip() for block in blocks if str(block or "").strip()]
    if not normalized_blocks:
        return ""
    if total_limit is None or total_limit <= 0:
        return "\n\n".join(normalized_blocks)

    merged_blocks: list[str] = []
    current_length = 0
    for block in normalized_blocks:
        separator_length = 2 if merged_blocks else 0
        projected_length = current_length + separator_length + len(block)
        if projected_length <= total_limit:
            merged_blocks.append(block)
            current_length = projected_length
            continue

        remaining = total_limit - current_length - separator_length
        if remaining < 80:
            break

        truncated = _truncate_story_runtime_block(block, remaining)
        if truncated:
            merged_blocks.append(truncated)
        break

    return "\n\n".join(merged_blocks)


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
    packet_guidance = getattr(story_packet, "guidance", None) if story_packet is not None else None
    return packet_guidance or guidance or StoryGenerationGuidance(
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
    )


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
    compact_mode: bool = False,
) -> str:
    packet_guidance = getattr(story_packet, "guidance", None) if story_packet is not None else None
    packet_blueprint = getattr(story_packet, "blueprint", None) if story_packet is not None else None
    active_guidance = packet_guidance or guidance or StoryGenerationGuidance()
    blueprint = packet_blueprint or StoryBlueprint()
    resolved_chapter_count = blueprint.chapter_count or _normalize_optional_int(chapter_count)

    block_specs = [
        ("base_requirements", base_requirements),
        ("story_creation_brief", build_story_creation_brief_block(active_guidance.story_creation_brief)),
        ("quality_repair_guidance", quality_repair_guidance),
        ("memory_guidance", memory_guidance),
        ("story_long_term_goal", build_story_long_term_goal_block(blueprint.long_term_goal)),
        ("story_character_focus_anchor", build_story_character_focus_anchor_block(blueprint.character_focus_names, scene=scene)),
        ("story_foreshadow_payoff_plan", build_story_foreshadow_payoff_plan_block(blueprint.foreshadow_payoff_plan, scene=scene)),
        ("story_relationship_state_ledger", build_story_relationship_state_ledger_block(blueprint.relationship_state_ledger, scene=scene)),
        ("story_character_state_ledger", build_story_character_state_ledger_block(blueprint.character_state_ledger, scene=scene)),
        ("quality_trend_guidance", quality_trend_guidance),
        ("story_organization_state_ledger", build_story_organization_state_ledger_block(blueprint.organization_state_ledger, scene=scene)),
        ("story_career_state_ledger", build_story_career_state_ledger_block(blueprint.career_state_ledger, scene=scene)),
        ("story_foreshadow_state_ledger", build_story_foreshadow_state_ledger_block(blueprint.foreshadow_state_ledger, scene=scene)),
        (
            "story_pacing_budget",
            build_story_pacing_budget_block(
                resolved_chapter_count,
                plot_stage=active_guidance.plot_stage,
                scene=scene,
            ),
        ),
        (
            "story_volume_pacing",
            build_volume_pacing_block(
                resolved_chapter_count,
                plot_stage=active_guidance.plot_stage,
            ),
        ),
    ]

    blocks: list[str] = []
    for block_name, block in block_specs:
        normalized = str(block or "").strip()
        if not normalized:
            continue
        if compact_mode and scene == "outline":
            normalized = _truncate_story_runtime_block(
                normalized,
                OUTLINE_RUNTIME_REQUIREMENT_BLOCK_LIMITS.get(block_name, len(normalized)),
            )
        if normalized:
            blocks.append(normalized)

    if compact_mode and scene == "outline":
        return _join_story_runtime_blocks_with_budget(
            blocks,
            total_limit=OUTLINE_RUNTIME_REQUIREMENT_TOTAL_LIMIT,
        )
    return "\n\n".join(blocks)


def build_compact_outline_guidance_blocks(active_guidance: StoryGenerationGuidance) -> list[str]:
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
        build_story_opening_hook_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_cliffhanger_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_character_arc_card_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
        build_story_execution_checklist_block(creative_mode, story_focus, scene=OUTLINE_SCENE, plot_stage=plot_stage),
    ]
    return [str(block).strip() for block in blocks if str(block or "").strip()]


def build_outline_guidance_blocks(
    active_guidance: StoryGenerationGuidance,
    *,
    compact_mode: bool = False,
) -> list[str]:
    creative_mode = active_guidance.creative_mode
    story_focus = active_guidance.story_focus
    plot_stage = active_guidance.plot_stage

    if compact_mode:
        return build_compact_outline_guidance_blocks(active_guidance)

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
    compact_mode: bool = False,
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
            compact_mode=compact_mode,
        ),
    )
    parts.extend(build_outline_guidance_blocks(active_guidance, compact_mode=compact_mode))
    if opening_outline_count is not None and opening_outline_count > 0:
        _append_if_present(parts, build_opening_outline_constraints_block(opening_outline_count))
    return "\n\n".join(parts)
