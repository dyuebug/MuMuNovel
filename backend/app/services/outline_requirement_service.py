"""大纲生成要求组装服务。"""

from __future__ import annotations

import re

from typing import Optional, List

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


def _split_sentences(text: str) -> List[str]:
    parts = re.split(r"[。！？!?；;\n]+", text)
    return [part.strip() for part in parts if part.strip()]
    return [part.strip() for part in parts if part.strip()]


def _append_if_present(parts: list[str], block: Optional[str]) -> None:
    normalized = str(block or "").strip()
    if normalized:
        parts.append(normalized)


def extract_outline_anchor_lines(chapter_outline: Optional[str], max_lines: int = 10) -> List[str]:
    """Extract outline anchors from both headed and prose summaries."""
    if not chapter_outline:
        return []

    section_capture_limits = {
        "章节概要": 1,
        "剧情摘要": 1,
        "场景设定": 2,
        "关键事件": 4,
        "情节要点": 5,
        "叙事目标": 1,
        "冲突主线": 2,
        "角色抉择": 2,
        "代价/风险": 2,
        "规则影响点": 2,
        "对话钩子": 2,
        "人物转折": 2,
        "角色焦点": 2,
        "情感基调": 1,
    }
    keywords = (
        "章节概要", "剧情摘要", "关键事件", "情节要点", "叙事目标",
        "冲突", "规则影响", "角色投择", "角色抉择", "代价", "人物转折",
        "对话钩子", "角色焦点", "场景设定", "情感基调",
    )
    sentence_cues = (
        "目标", "冲突", "阻力", "规则", "决定", "代价", "反馈", "小爽点", "悬念", "章尾",
        "反转", "异常", "认主", "借书证", "页印", "回声", "机位", "禁播", "校对", "封门",
    )

    raw_lines = [line.strip() for line in chapter_outline.splitlines() if line.strip()]
    section_anchors: List[str] = []
    capture_bullet_count = 0

    for line in raw_lines:
        if line.startswith("【") and line.endswith("】"):
            section_name = line[1:-1].strip()
            if any(key in section_name for key in keywords):
                capture_bullet_count = section_capture_limits.get(section_name, 3)
            else:
                capture_bullet_count = 0
            continue

        cleaned = line.lstrip("- ").strip()
        if not cleaned:
            continue

        if capture_bullet_count > 0:
            section_anchors.append(cleaned[:120])
            capture_bullet_count -= 1
            continue

        if any(key in cleaned for key in keywords):
            parts = [part.strip() for part in re.split(r"[:：]", cleaned, maxsplit=1)]
            if len(parts) == 2 and parts[1] and any(key in parts[0] for key in keywords):
                section_anchors.append(parts[1][:120])
                continue
            if cleaned.endswith((":", "：")):
                continue
            section_anchors.append(cleaned[:120])

    sentence_anchors: List[str] = []
    for sentence in _split_sentences(chapter_outline):
        normalized = sentence.lstrip("- ").strip()
        if len(normalized) < 8:
            continue
        cue_score = sum(1 for cue in sentence_cues if cue in normalized)
        if cue_score <= 0 and len(normalized) < 24:
            continue
        sentence_anchors.append(normalized[:120])

    source_anchors = section_anchors if section_anchors else sentence_anchors

    deduped: List[str] = []
    seen: set[str] = set()
    for item in [*source_anchors, *sentence_anchors]:
        normalized = item.strip()
        if normalized and normalized not in seen:
            seen.add(normalized)
            deduped.append(normalized[:120])
        if len(deduped) >= max_lines:
            break

    return deduped

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


def build_outline_guidance_blocks(active_guidance: StoryGenerationGuidance, *, compact_mode: bool = False) -> list[str]:
    creative_mode = active_guidance.creative_mode
    story_focus = active_guidance.story_focus
    plot_stage = active_guidance.plot_stage

    if compact_mode:
        return build_compact_outline_guidance_blocks(active_guidance)

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
