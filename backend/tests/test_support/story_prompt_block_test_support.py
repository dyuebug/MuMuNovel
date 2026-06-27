"""Story prompt block owner used by story packet runtime assembly."""

from __future__ import annotations

import logging
import re
from typing import Any, Dict, Mapping, Optional, Sequence

from tests.test_support.retired_runtime_test_support import get_logger
from tests.test_support.schemas.novel_quality_profile_service import novel_quality_profile_service
from tests.test_support.story_prompt_template_support_test_support import (
    QUALITY_RUNTIME_TRACKING_TAG,
    QUALITY_TEMPLATE_INSERTIONS,
    compact_prompt_text as _shared_compact_prompt_text,
)

# Keep the legacy logger name so existing debug assertions still see the same source.
logger = get_logger("app.services.prompt_service")

try:
    from tests.test_support.ai_gateway.mcp_tools_loader import (
        MCP_CANON_PRIORITY_RULE,
        MCP_SOURCE_DISCLOSURE_RULE,
    )
except Exception:
    MCP_CANON_PRIORITY_RULE = "项目 canon（既有设定、角色关系、本章大纲）优先级高于一切外部参考。"
    MCP_SOURCE_DISCLOSURE_RULE = "最终输出禁止暴露 MCP、工具名、检索过程或来源站点。"

CREATIVE_MODE_SPECS = {
    "balanced": {
        "label": "均衡推进",
        "outline": [
            "同时照顾钩子、推进、情绪和信息释放，不偏科。",
            "每章都要既能往下推，又能留下后续空间。",
        ],
        "chapter": [
            "兼顾推进效率、情绪余韵和章尾牵引，不让单一节拍统治全文。",
            "既要有动作落点，也要有关系或情绪反馈。",
        ],
    },
    "hook": {
        "label": "钩子优先",
        "outline": [
            "每章优先设计读者会想点下一章的悬挂点和动作牵引。",
            "关键信息不要一次讲透，尽量把转折放在章尾或场尾。",
        ],
        "chapter": [
            "开场尽快抛出异常、任务或危险，章尾优先落在未解动作上。",
            "减少平铺解释，多用突发变化和信息缺口带动阅读。",
        ],
    },
    "emotion": {
        "label": "情绪沉浸",
        "outline": [
            "每章都明确情绪波峰波谷，让冲突带出人物内在变化。",
            "安排能让人物情绪外露的场面，不只给事件结果。",
        ],
        "chapter": [
            "强化人物情绪的触发、压抑、外露和余震过程。",
            "多写反应、动作和潜台词，少写统一口径的抒情总结。",
        ],
    },
    "suspense": {
        "label": "悬念拉满",
        "outline": [
            "优先铺信息差、误导、遮蔽与逐层揭开，避免过早讲透底牌。",
            "每章至少留一个会迫使角色继续追查的新疑点。",
        ],
        "chapter": [
            "控制信息披露节奏，把真相拆成连续可追的碎片。",
            "对白和动作里埋认知偏差，让读者和角色都处在半知状态。",
        ],
    },
    "relationship": {
        "label": "关系张力",
        "outline": [
            "每章尽量让人物关系产生位移：靠近、撕裂、试探或互相利用。",
            "冲突优先落在人与人之间的立场差和利益差上。",
        ],
        "chapter": [
            "强化角色之间的试探、误解、压制、让步和反击。",
            "至少让一段关键互动同时推动剧情与关系变化。",
        ],
    },
    "payoff": {
        "label": "爽点推进",
        "outline": [
            "优先规划反转、收获、打脸、突破等即时反馈，避免一直憋压不放。",
            "每章都给读者一个清晰可感的阶段性兑现点。",
        ],
        "chapter": [
            "强化铺垫→爆发→反馈链条，让爽点有落地动作和后续影响。",
            "减少空转拉扯，关键节点尽量让角色主动出手换结果。",
        ],
    },
}

CREATIVE_MODE_ALIASES = {
    "balanced": "balanced",
    "均衡": "balanced",
    "均衡推进": "balanced",
    "hook": "hook",
    "钩子": "hook",
    "钩子优先": "hook",
    "emotion": "emotion",
    "情绪": "emotion",
    "情绪沉浸": "emotion",
    "suspense": "suspense",
    "悬念": "suspense",
    "悬念拉满": "suspense",
    "relationship": "relationship",
    "关系": "relationship",
    "关系张力": "relationship",
    "payoff": "payoff",
    "爽点": "payoff",
    "爽点推进": "payoff",
}

STORY_FOCUS_SPECS = {
    "advance_plot": {
        "label": "主线推进",
        "outline": [
            "本轮大纲优先让事件产生明确推进，不要原地打转。",
            "每章都要形成新的行动结果、局势变化或任务升级。",
        ],
        "chapter": [
            "优先写清角色做了什么、局势如何变化、下一步被逼向哪里。",
            "减少原地解释和重复抒情，让情节真正往前走。",
        ],
    },
    "deepen_character": {
        "label": "人物塑形",
        "outline": [
            "本轮优先安排能暴露人物选择、弱点、执念与成长代价的章节。",
            "不要只给事件节点，要给人物变化节点。",
        ],
        "chapter": [
            "优先通过选择、反应、失误和坚持来立住人物。",
            "让角色的独特声音、习惯与价值判断真正显形。",
        ],
    },
    "escalate_conflict": {
        "label": "冲突升级",
        "outline": [
            "本轮优先让阻力变强、代价变高、对立面更具体。",
            "章节之间要形成持续抬升的压力链，而不是重复同级冲突。",
        ],
        "chapter": [
            "优先写出目标受阻、局面恶化、选择更难的过程。",
            "让冲突产生即时后果，不要只停留在嘴上对抗。",
        ],
    },
    "reveal_mystery": {
        "label": "谜团揭示",
        "outline": [
            "本轮优先安排线索出现、误导修正和真相推进的章节。",
            "揭示要分层，不要一口气把所有底牌讲透。",
        ],
        "chapter": [
            "优先通过调查、对质、异常细节与证据变化推进认知。",
            "每章至少让读者比上一章多知道一点关键东西。",
        ],
    },
    "relationship_shift": {
        "label": "关系转折",
        "outline": [
            "本轮优先安排人物关系发生靠近、破裂、试探或重组。",
            "让关系变化能反向影响后续行动，而不只是情绪点缀。",
        ],
        "chapter": [
            "优先写互动中的试探、让步、误判、亏欠或立场重排。",
            "对话和行动都要服务关系变化，不只写结果。",
        ],
    },
    "foreshadow_payoff": {
        "label": "伏笔回收",
        "outline": [
            "本轮优先处理前文埋下的信息、承诺、物件或关系线索。",
            "回收时既要兑现，也要顺手打开新的后续空间。",
        ],
        "chapter": [
            "优先让前文埋下的悬念、承诺或能力产生可感的回报。",
            "回收不能只靠说明，要落在事件结果和人物反馈上。",
        ],
    },
}

STORY_FOCUS_ALIASES = {
    "advance_plot": "advance_plot",
    "主线": "advance_plot",
    "主线推进": "advance_plot",
    "推进剧情": "advance_plot",
    "deepen_character": "deepen_character",
    "人物": "deepen_character",
    "人物塑形": "deepen_character",
    "塑造人物": "deepen_character",
    "escalate_conflict": "escalate_conflict",
    "冲突": "escalate_conflict",
    "冲突升级": "escalate_conflict",
    "升级冲突": "escalate_conflict",
    "reveal_mystery": "reveal_mystery",
    "谜团": "reveal_mystery",
    "谜团揭示": "reveal_mystery",
    "揭示真相": "reveal_mystery",
    "relationship_shift": "relationship_shift",
    "关系": "relationship_shift",
    "关系转折": "relationship_shift",
    "关系变化": "relationship_shift",
    "foreshadow_payoff": "foreshadow_payoff",
    "伏笔": "foreshadow_payoff",
    "伏笔回收": "foreshadow_payoff",
    "回收伏笔": "foreshadow_payoff",
}

PLOT_STAGE_LABELS = {
    "development": "发展阶段",
    "climax": "高潮阶段",
    "ending": "结局阶段",
}

PLOT_STAGE_MISSIONS = {
    "development": "立局、铺变量、建立目标与第一轮压力。",
    "climax": "持续抬压、逼近正面碰撞、推动关键反转。",
    "ending": "回收承诺、兑现伏笔、收束关系并留下余味。",
}

PLOT_STAGE_ALIASES = {
    "development": "development",
    "发展": "development",
    "发展阶段": "development",
    "climax": "climax",
    "高潮": "climax",
    "高潮阶段": "climax",
    "ending": "ending",
    "结局": "ending",
    "结局阶段": "ending",
}

QUALITY_OPTIONAL_CARD_BLOCK_BUDGETS = {
    "development": 2600,
    "climax": 3000,
    "ending": 2800,
}

QUALITY_OPTIONAL_CARD_DEFAULT_BUDGET = 2800
QUALITY_REGENERATION_OPTIONAL_CARD_BUDGET = 2200

QUALITY_OPTIONAL_CARD_DROP_ORDER = {
    "development": (
        "story_acceptance_card_block",
        "story_repetition_risk_block",
        "story_opening_hook_card_block",
        "story_cliffhanger_card_block",
        "story_result_card_block",
        "story_payoff_chain_card_block",
        "story_summary_tone_control_card_block",
        "story_repetition_control_card_block",
        "story_scene_density_card_block",
        "story_scene_anchor_card_block",
        "story_information_release_card_block",
        "story_emotion_landing_card_block",
        "story_viewpoint_discipline_card_block",
        "story_rule_grounding_card_block",
        "story_objective_card_block",
        "story_action_rendering_card_block",
        "story_dialogue_advancement_card_block",
        "story_character_arc_card_block",
    ),
    "climax": (
        "story_acceptance_card_block",
        "story_repetition_risk_block",
        "story_summary_tone_control_card_block",
        "story_repetition_control_card_block",
        "story_viewpoint_discipline_card_block",
        "story_scene_density_card_block",
        "story_scene_anchor_card_block",
        "story_information_release_card_block",
        "story_opening_hook_card_block",
        "story_emotion_landing_card_block",
        "story_character_arc_card_block",
        "story_dialogue_advancement_card_block",
        "story_objective_card_block",
        "story_rule_grounding_card_block",
        "story_result_card_block",
        "story_action_rendering_card_block",
        "story_cliffhanger_card_block",
        "story_payoff_chain_card_block",
    ),
    "ending": (
        "story_opening_hook_card_block",
        "story_repetition_risk_block",
        "story_scene_density_card_block",
        "story_scene_anchor_card_block",
        "story_repetition_control_card_block",
        "story_viewpoint_discipline_card_block",
        "story_action_rendering_card_block",
        "story_dialogue_advancement_card_block",
        "story_objective_card_block",
        "story_character_arc_card_block",
        "story_summary_tone_control_card_block",
        "story_information_release_card_block",
        "story_rule_grounding_card_block",
        "story_result_card_block",
        "story_cliffhanger_card_block",
        "story_emotion_landing_card_block",
        "story_acceptance_card_block",
        "story_payoff_chain_card_block",
    ),
}

QUALITY_FOCUS_PROTECTED_BLOCKS = {
    "payoff": (
        "story_payoff_chain_card_block",
        "story_result_card_block",
    ),
    "foreshadow_payoff": (
        "story_payoff_chain_card_block",
        "story_result_card_block",
    ),
    "cliffhanger": (
        "story_cliffhanger_card_block",
        "story_opening_hook_card_block",
    ),
    "opening": (
        "story_opening_hook_card_block",
    ),
    "hook": (
        "story_opening_hook_card_block",
        "story_cliffhanger_card_block",
    ),
    "dialogue": (
        "story_dialogue_advancement_card_block",
    ),
    "rule_grounding": (
        "story_rule_grounding_card_block",
    ),
    "continuity": (
        "story_scene_anchor_card_block",
        "story_information_release_card_block",
        "story_character_arc_card_block",
    ),
    "character_continuity": (
        "story_scene_anchor_card_block",
        "story_character_arc_card_block",
    ),
    "relationship_continuity": (
        "story_scene_anchor_card_block",
        "story_dialogue_advancement_card_block",
    ),
    "organization_continuity": (
        "story_scene_anchor_card_block",
        "story_information_release_card_block",
    ),
    "career_continuity": (
        "story_scene_anchor_card_block",
        "story_objective_card_block",
    ),
    "pacing": (
        "story_scene_density_card_block",
        "story_objective_card_block",
    ),
    "conflict": (
        "story_objective_card_block",
        "story_action_rendering_card_block",
    ),
}

QUALITY_PREFERENCE_SPECS = {
    "balanced": {
        "label": "均衡质感",
        "outline": [
            "兼顾推进、情绪、场景和信息释放，不让单一维度长期压过其他维度。",
            "每轮最好既有推进结果，也有可感回报和后续余味。",
        ],
        "chapter": [
            "兼顾抓力、推进、情绪和信息密度，不让正文只剩单项发力。",
            "每章最好既有局势变化，也有读者能感到的回报与余味。",
        ],
    },
    "plot_drive": {
        "label": "强情节回报",
        "outline": [
            "优先强化开头抓力、动作桥段、爽点回收和章尾牵引。",
            "减少空转解释和过度铺垫，让大纲更偏可追读连载感。",
        ],
        "chapter": [
            "优先强化开头抓力、动作现场化、回报节点和章尾追读牵引。",
            "减少空转解释、慢热预热和没有反馈的过程性段落。",
        ],
    },
    "immersive": {
        "label": "沉浸场景感",
        "outline": [
            "优先强化设定落地、视角稳定、场景密度与空间感。",
            "信息解释尽量压进事件和场景里，减少说明书式铺陈。",
        ],
        "chapter": [
            "优先强化设定落地、视角纪律、场景密度和现场感。",
            "解释尽量嵌进动作、对白和环境反馈里，减少飘在空中的说明。",
        ],
    },
    "emotion_drama": {
        "label": "情绪关系向",
        "outline": [
            "优先强化情绪落点、对白推进、关系余波和误伤后的后效。",
            "让人物关系变化真正反向推动下一轮行动。",
        ],
        "chapter": [
            "优先强化情绪触发、外显反应、对白张力和关系余波。",
            "让人物靠近、误伤、试探和迟来的理解都落在现场里。",
        ],
    },
    "clean_prose": {
        "label": "克制干净文风",
        "outline": [
            "优先强化信息压缩、重复压缩、总结腔抑制和表达克制。",
            "减少花哨总结和自我解释，让结构更清楚干净。",
        ],
        "chapter": [
            "优先强化信息压缩、重复压缩、少盖章、少同义复述。",
            "减少油腻金句、过度解释和模板连接词，让正文更利落。",
        ],
    },
}

QUALITY_PREFERENCE_ALIASES = {
    "balanced": "balanced",
    "均衡": "balanced",
    "均衡质感": "balanced",
    "plot_drive": "plot_drive",
    "强情节": "plot_drive",
    "强情节回报": "plot_drive",
    "immersive": "immersive",
    "沉浸": "immersive",
    "沉浸场景感": "immersive",
    "emotion_drama": "emotion_drama",
    "情绪关系": "emotion_drama",
    "情绪关系向": "emotion_drama",
    "clean_prose": "clean_prose",
    "克制文风": "clean_prose",
    "克制干净文风": "clean_prose",
}


def _compact_prompt_text(value: Any) -> str:
    return _shared_compact_prompt_text(value)


def _dedupe_prompt_items(items: list[str]) -> list[str]:
    seen: set[str] = set()
    deduped: list[str] = []
    for item in items:
        normalized = str(item or "").strip()
        if not normalized or normalized in seen:
            continue
        seen.add(normalized)
        deduped.append(normalized)
    return deduped


def _coerce_positive_int(value: Optional[Any]) -> Optional[int]:
    if value is None:
        return None
    try:
        normalized = int(str(value).strip())
    except (TypeError, ValueError):
        return None
    return normalized if normalized > 0 else None


def _trim_prompt_terminal_punctuation(value: Any) -> str:
    text = _compact_prompt_text(value)
    return text.rstrip("。！？!?；;,.，、 ")


def _normalize_prompt_sentence_fragments(values: Any) -> list[str]:
    normalized: list[str] = []
    for value in values or ():
        cleaned = _trim_prompt_terminal_punctuation(value)
        if cleaned:
            normalized.append(cleaned)
    return normalized


def _normalize_runtime_prompt_items(values: Optional[Any], *, limit: int = 4) -> list[str]:
    if values is None:
        return []

    if isinstance(values, str):
        raw_items = re.split(r"[\n;]+", values)
    elif isinstance(values, Sequence) and not isinstance(values, (str, bytes, bytearray)):
        raw_items = list(values)
    else:
        raw_items = [values]

    normalized: list[str] = []
    for raw in raw_items:
        if isinstance(raw, Mapping):
            label = str(raw.get("label") or raw.get("name") or raw.get("title") or "").strip()
            summary = str(
                raw.get("summary")
                or raw.get("content")
                or raw.get("item")
                or raw.get("value")
                or ""
            ).strip()
            status = str(raw.get("status") or "").strip()
            target_chapter = raw.get("target_chapter")
            text = f"{label}: {summary}" if label and summary else (summary or label)
            meta_parts: list[str] = []
            if status:
                meta_parts.append(f"status={status}")
            if target_chapter not in (None, ""):
                meta_parts.append(f"target_chapter={target_chapter}")
            if meta_parts:
                text = f"{text}; {'; '.join(meta_parts)}" if text else "; ".join(meta_parts)
        else:
            text = str(raw or "").strip()
        if not text:
            continue
        text = re.sub(r"^[-•*·\d\.\)\s]+", "", text).strip()
        if not text or text.startswith("【"):
            continue
        normalized.append(text)

    return _dedupe_prompt_items(normalized)[:limit]


def _normalize_quality_focus_tags(values: Optional[Any]) -> list[str]:
    tags: list[str] = []
    for item in _normalize_runtime_prompt_items(values, limit=6):
        normalized = re.sub(r"[^a-z0-9_\u4e00-\u9fff]+", "_", item.strip().lower()).strip("_")
        if not normalized:
            continue
        tag = normalized
        if any(token in normalized for token in ("payoff", "回报", "兑现", "伏笔回收")):
            tag = "payoff"
        elif any(token in normalized for token in ("cliffhanger", "尾钩", "章尾", "牵引", "追读")):
            tag = "cliffhanger"
        elif any(token in normalized for token in ("opening", "开头", "开场", "hook", "钩子")):
            tag = "opening"
        elif any(token in normalized for token in ("dialogue", "对话", "对白")):
            tag = "dialogue"
        elif any(token in normalized for token in ("rule_grounding", "grounding", "设定落地", "规则落地")):
            tag = "rule_grounding"
        elif any(token in normalized for token in ("organization_continuity", "组织连续性")):
            tag = "organization_continuity"
        elif any(token in normalized for token in ("career_continuity", "职业连续性")):
            tag = "career_continuity"
        elif any(token in normalized for token in ("relationship_continuity", "关系连续性")):
            tag = "relationship_continuity"
        elif any(token in normalized for token in ("character_continuity", "人物连续性")):
            tag = "character_continuity"
        elif any(token in normalized for token in ("continuity", "连续性", "衔接", "接力")):
            tag = "continuity"
        elif any(token in normalized for token in ("pacing", "节奏")):
            tag = "pacing"
        elif any(token in normalized for token in ("conflict", "冲突")):
            tag = "conflict"
        if tag not in tags:
            tags.append(tag)
    return tags


def resolve_quality_focus_protected_blocks(summary: Optional[Any]) -> tuple[str, ...]:
    if not isinstance(summary, Mapping):
        return ()

    focus_tags = _normalize_quality_focus_tags(summary.get("recent_focus_areas"))
    continuity_preflight = (
        summary.get("continuity_preflight")
        if isinstance(summary.get("continuity_preflight"), Mapping)
        else {}
    )
    for tag in _normalize_quality_focus_tags(continuity_preflight.get("focus_areas")):
        if tag not in focus_tags:
            focus_tags.append(tag)

    if (
        continuity_preflight.get("summary")
        or _normalize_runtime_prompt_items(continuity_preflight.get("repair_targets"), limit=3)
    ) and "continuity" not in focus_tags:
        focus_tags.append("continuity")

    protected_blocks: list[str] = []
    for tag in focus_tags:
        for block in QUALITY_FOCUS_PROTECTED_BLOCKS.get(tag, ()):
            if block not in protected_blocks:
                protected_blocks.append(block)
    return tuple(protected_blocks)


def _allocate_volume_segments(chapter_count: int) -> list[tuple[str, int]]:
    total = max(0, int(chapter_count or 0))
    if total <= 0:
        return []
    if total == 1:
        return [("development", 1)]
    if total == 2:
        return [("development", 1), ("ending", 1)]
    if total == 3:
        return [("development", 1), ("climax", 1), ("ending", 1)]

    development_count = max(1, round(total * 0.45))
    climax_count = max(1, round(total * 0.35))
    ending_count = total - development_count - climax_count

    if ending_count < 1:
        ending_count = 1
        if development_count >= climax_count and development_count > 1:
            development_count -= 1
        elif climax_count > 1:
            climax_count -= 1

    segments: list[tuple[str, int]] = []
    if development_count > 0:
        segments.append(("development", development_count))
    if climax_count > 0:
        segments.append(("climax", climax_count))
    if ending_count > 0:
        segments.append(("ending", ending_count))
    return segments


def normalize_creative_mode(mode: Optional[str]) -> Optional[str]:
    cleaned = str(mode or "").strip()
    if not cleaned:
        return None
    return CREATIVE_MODE_ALIASES.get(cleaned) or CREATIVE_MODE_ALIASES.get(cleaned.lower())


def normalize_story_focus(value: Optional[str]) -> Optional[str]:
    cleaned = str(value or "").strip()
    if not cleaned:
        return None
    return STORY_FOCUS_ALIASES.get(cleaned) or STORY_FOCUS_ALIASES.get(cleaned.lower())


def normalize_quality_preset(value: Optional[str]) -> Optional[str]:
    cleaned = str(value or "").strip()
    if not cleaned:
        return None
    return QUALITY_PREFERENCE_ALIASES.get(cleaned) or QUALITY_PREFERENCE_ALIASES.get(cleaned.lower())


def normalize_plot_stage(value: Optional[str]) -> Optional[str]:
    cleaned = str(value or "").strip()
    if not cleaned:
        return None
    return PLOT_STAGE_ALIASES.get(cleaned) or PLOT_STAGE_ALIASES.get(cleaned.lower())


def _split_quality_preference_note_items(
    quality_notes: Optional[str],
    *,
    limit: int = 4,
) -> list[str]:
    notes = _compact_prompt_text(quality_notes)
    if not notes:
        return []

    items: list[str] = []
    seen: set[str] = set()
    for raw in re.split(r"(?:\r?\n|[；;]+)", notes):
        normalized = re.sub(r"^[\s\-\*•·\d\.\)\(、]+", "", str(raw or "")).strip()
        if not normalized or normalized in seen:
            continue
        seen.add(normalized)
        items.append(normalized)
        if len(items) >= limit:
            break
    return items


def compact_prompt_text(value: Any) -> str:
    return _compact_prompt_text(value)


def dedupe_prompt_items(items: list[str]) -> list[str]:
    return _dedupe_prompt_items(items)


def coerce_positive_int(value: Optional[Any]) -> Optional[int]:
    return _coerce_positive_int(value)


def trim_prompt_terminal_punctuation(value: Any) -> str:
    return _trim_prompt_terminal_punctuation(value)


def normalize_prompt_sentence_fragments(values: Any) -> list[str]:
    return _normalize_prompt_sentence_fragments(values)


def normalize_runtime_prompt_items(values: Optional[Any], *, limit: int = 4) -> list[str]:
    return _normalize_runtime_prompt_items(values, limit=limit)


def split_quality_preference_note_items(
    quality_notes: Optional[str],
    *,
    limit: int = 4,
) -> list[str]:
    return _split_quality_preference_note_items(quality_notes, limit=limit)


def build_quality_profile_context(**kwargs) -> Dict[str, Any]:
    external_assets = kwargs.get("external_assets") or kwargs.get("reference_assets") or ()
    return novel_quality_profile_service.build_profile_dict(
        {
            "genre": kwargs.get("genre"),
            "style_name": kwargs.get("style_name"),
            "style_preset_id": kwargs.get("style_preset_id"),
            "style_content": kwargs.get("style_content"),
            "external_assets": external_assets,
        }
    )


def resolve_quality_optional_block_budget(
    template_key: Optional[str],
    template_insertion: Optional[str],
    plot_stage: Optional[str],
    *,
    optional_card_block_budgets: Mapping[str, int],
    optional_card_default_budget: int,
    regeneration_optional_card_budget: int,
    budget_override: Optional[Any] = None,
    continuity_density: int = 0,
) -> Optional[int]:
    if budget_override not in (None, ""):
        try:
            return max(int(budget_override), 0)
        except (TypeError, ValueError):
            return None
    if not template_key or not template_insertion or "{story_objective_card_block}" not in template_insertion:
        return None
    if continuity_density < 2:
        return None
    normalized_stage = normalize_plot_stage(plot_stage) or "development"
    resolved_budget = optional_card_block_budgets.get(
        normalized_stage,
        optional_card_default_budget,
    )
    if template_key == "CHAPTER_REGENERATION_SYSTEM":
        return min(resolved_budget, regeneration_optional_card_budget)
    return resolved_budget


def resolve_quality_optional_block_drop_order(
    plot_stage: Optional[str],
    *,
    optional_card_drop_order: Mapping[str, tuple[str, ...]],
    protected_blocks: Sequence[str] = (),
) -> tuple[str, ...]:
    normalized_stage = normalize_plot_stage(plot_stage) or "development"
    base_order = optional_card_drop_order.get(
        normalized_stage,
        optional_card_drop_order["development"],
    )
    protected_set = {key for key in protected_blocks if key}
    if not protected_set:
        return base_order
    return tuple(
        [key for key in base_order if key not in protected_set]
        + [key for key in base_order if key in protected_set]
    )


def apply_quality_optional_block_budget(
    blocks: Dict[str, str],
    *,
    template_key: Optional[str],
    template_insertion: Optional[str],
    plot_stage: Optional[str],
    optional_card_block_budgets: Mapping[str, int],
    optional_card_default_budget: int,
    regeneration_optional_card_budget: int,
    optional_card_drop_order: Mapping[str, tuple[str, ...]],
    budget_override: Optional[Any] = None,
    quality_metrics_summary: Optional[Any] = None,
) -> Dict[str, str]:
    continuity_density = sum(
        1
        for key in (
            "story_character_focus_anchor_block",
            "story_foreshadow_payoff_plan_block",
            "story_character_state_ledger_block",
            "story_relationship_state_ledger_block",
            "story_foreshadow_state_ledger_block",
            "story_organization_state_ledger_block",
            "story_career_state_ledger_block",
        )
        if blocks.get(key)
    )
    budget = resolve_quality_optional_block_budget(
        template_key,
        template_insertion,
        plot_stage,
        optional_card_block_budgets=optional_card_block_budgets,
        optional_card_default_budget=optional_card_default_budget,
        regeneration_optional_card_budget=regeneration_optional_card_budget,
        budget_override=budget_override,
        continuity_density=continuity_density,
    )
    if budget is None:
        return blocks

    placeholders = {
        match.group(1)
        for match in re.finditer(r"\{([A-Za-z0-9_]+)\}", template_insertion or "")
    }
    protected_blocks = resolve_quality_focus_protected_blocks(quality_metrics_summary)
    protected_set = {key for key in protected_blocks if key}
    drop_order = resolve_quality_optional_block_drop_order(
        plot_stage,
        optional_card_drop_order=optional_card_drop_order,
        protected_blocks=protected_blocks,
    )
    current_size = sum(
        len(blocks.get(key) or "")
        for key in drop_order
        if key in placeholders and blocks.get(key)
    )
    if current_size <= budget:
        return blocks

    trimmed = dict(blocks)
    for key in drop_order:
        if key in protected_set:
            continue
        value = trimmed.get(key) or ""
        if key not in placeholders or not value:
            continue
        trimmed[key] = ""
        current_size -= len(value)
        if current_size <= budget:
            break
    return trimmed


def build_quality_runtime_blocks(
    template_key: Optional[str],
    *,
    template_insertions: Optional[Mapping[str, str]] = None,
    optional_card_block_budgets: Optional[Mapping[str, int]] = None,
    optional_card_default_budget: Optional[int] = None,
    regeneration_optional_card_budget: Optional[int] = None,
    optional_card_drop_order: Optional[Mapping[str, tuple[str, ...]]] = None,
    quality_runtime_tracking_tag: str = QUALITY_RUNTIME_TRACKING_TAG,
    mcp_canon_priority_rule: str = MCP_CANON_PRIORITY_RULE,
    mcp_source_disclosure_rule: str = MCP_SOURCE_DISCLOSURE_RULE,
    **kwargs,
) -> Dict[str, str]:
    template_insertions = template_insertions or QUALITY_TEMPLATE_INSERTIONS
    optional_card_block_budgets = (
        optional_card_block_budgets or QUALITY_OPTIONAL_CARD_BLOCK_BUDGETS
    )
    optional_card_default_budget = (
        QUALITY_OPTIONAL_CARD_DEFAULT_BUDGET
        if optional_card_default_budget is None
        else optional_card_default_budget
    )
    regeneration_optional_card_budget = (
        QUALITY_REGENERATION_OPTIONAL_CARD_BUDGET
        if regeneration_optional_card_budget is None
        else regeneration_optional_card_budget
    )
    optional_card_drop_order = (
        optional_card_drop_order or QUALITY_OPTIONAL_CARD_DROP_ORDER
    )

    profile = build_quality_profile_context(**kwargs)
    prompt_blocks = profile.get("prompt_blocks") or {}
    quality_metrics_summary = (
        kwargs.get("quality_metrics_summary")
        if isinstance(kwargs.get("quality_metrics_summary"), Mapping)
        else {}
    )
    continuity_preflight = (
        quality_metrics_summary.get("continuity_preflight")
        if isinstance(quality_metrics_summary.get("continuity_preflight"), Mapping)
        else None
    )

    generation_block = compact_prompt_text(prompt_blocks.get("generation"))
    checker_block = compact_prompt_text(prompt_blocks.get("checker"))
    reviser_block = compact_prompt_text(prompt_blocks.get("reviser"))
    mcp_guard_block = compact_prompt_text(
        kwargs.get("mcp_guard") or kwargs.get("quality_mcp_guard") or prompt_blocks.get("mcp_guard")
    )
    external_assets_block = compact_prompt_text(prompt_blocks.get("external_assets"))
    mcp_references = compact_prompt_text(
        kwargs.get("mcp_references") or kwargs.get("quality_mcp_references")
    )
    creative_mode_block = compact_prompt_text(
        kwargs.get("creative_mode_block") or build_creative_mode_block(kwargs.get("creative_mode"), scene="chapter")
    )
    story_focus_block = compact_prompt_text(
        kwargs.get("story_focus_block") or build_story_focus_block(kwargs.get("story_focus"), scene="chapter")
    )
    narrative_blueprint_block = compact_prompt_text(
        kwargs.get("narrative_blueprint_block")
        or build_narrative_blueprint_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_creation_brief_block = compact_prompt_text(
        kwargs.get("story_creation_brief_block")
        or build_story_creation_brief_block(kwargs.get("story_creation_brief"))
    )
    story_long_term_goal_block = compact_prompt_text(
        kwargs.get("story_long_term_goal_block")
        or build_story_long_term_goal_block(kwargs.get("story_long_term_goal"))
    )
    story_quality_hard_guard_block = compact_prompt_text(kwargs.get("story_quality_hard_guard_block"))
    story_pacing_budget_block = compact_prompt_text(
        kwargs.get("story_pacing_budget_block")
        or build_story_pacing_budget_block(
            kwargs.get("chapter_count"),
            current_chapter_number=kwargs.get("current_chapter_number"),
            target_word_count=kwargs.get("target_word_count"),
            plot_stage=kwargs.get("plot_stage"),
            scene="chapter",
        )
    )
    story_volume_pacing_block = compact_prompt_text(
        kwargs.get("story_volume_pacing_block")
        or build_volume_pacing_block(
            kwargs.get("chapter_count"),
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_quality_trend_block = compact_prompt_text(
        kwargs.get("story_quality_trend_block")
        or build_story_quality_trend_block(
            quality_metrics_summary,
            scene="chapter",
        )
    )
    story_character_focus_anchor_block = compact_prompt_text(
        kwargs.get("story_character_focus_anchor_block")
        or build_story_character_focus_anchor_block(
            kwargs.get("story_character_focus"),
            scene="chapter",
        )
    )
    story_foreshadow_payoff_plan_block = compact_prompt_text(
        kwargs.get("story_foreshadow_payoff_plan_block")
        or build_story_foreshadow_payoff_plan_block(
            kwargs.get("story_foreshadow_payoff_plan"),
            scene="chapter",
        )
    )
    story_character_state_ledger_block = compact_prompt_text(
        kwargs.get("story_character_state_ledger_block")
        or build_story_character_state_ledger_block(
            kwargs.get("story_character_state_ledger"),
            scene="chapter",
        )
    )
    story_relationship_state_ledger_block = compact_prompt_text(
        kwargs.get("story_relationship_state_ledger_block")
        or build_story_relationship_state_ledger_block(
            kwargs.get("story_relationship_state_ledger"),
            scene="chapter",
        )
    )
    story_foreshadow_state_ledger_block = compact_prompt_text(
        kwargs.get("story_foreshadow_state_ledger_block")
        or build_story_foreshadow_state_ledger_block(
            kwargs.get("story_foreshadow_state_ledger"),
            scene="chapter",
        )
    )
    story_organization_state_ledger_block = compact_prompt_text(
        kwargs.get("story_organization_state_ledger_block")
        or build_story_organization_state_ledger_block(
            kwargs.get("story_organization_state_ledger"),
            scene="chapter",
        )
    )
    story_career_state_ledger_block = compact_prompt_text(
        kwargs.get("story_career_state_ledger_block")
        or build_story_career_state_ledger_block(
            kwargs.get("story_career_state_ledger"),
            scene="chapter",
        )
    )
    quality_preference_block = compact_prompt_text(
        kwargs.get("quality_preference_block")
        or build_quality_preference_block(
            kwargs.get("quality_preset"),
            kwargs.get("quality_notes"),
            scene="chapter",
        )
    )
    story_objective_card_block = compact_prompt_text(
        kwargs.get("story_objective_card_block")
        or build_story_objective_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_result_card_block = compact_prompt_text(
        kwargs.get("story_result_card_block")
        or build_story_result_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_payoff_chain_card_block = compact_prompt_text(
        kwargs.get("story_payoff_chain_card_block")
        or build_story_payoff_chain_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_rule_grounding_card_block = compact_prompt_text(
        kwargs.get("story_rule_grounding_card_block")
        or build_story_rule_grounding_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_information_release_card_block = compact_prompt_text(
        kwargs.get("story_information_release_card_block")
        or build_story_information_release_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_emotion_landing_card_block = compact_prompt_text(
        kwargs.get("story_emotion_landing_card_block")
        or build_story_emotion_landing_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_action_rendering_card_block = compact_prompt_text(
        kwargs.get("story_action_rendering_card_block")
        or build_story_action_rendering_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_summary_tone_control_card_block = compact_prompt_text(
        kwargs.get("story_summary_tone_control_card_block")
        or build_story_summary_tone_control_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_repetition_control_card_block = compact_prompt_text(
        kwargs.get("story_repetition_control_card_block")
        or build_story_repetition_control_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_viewpoint_discipline_card_block = compact_prompt_text(
        kwargs.get("story_viewpoint_discipline_card_block")
        or build_story_viewpoint_discipline_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_dialogue_advancement_card_block = compact_prompt_text(
        kwargs.get("story_dialogue_advancement_card_block")
        or build_story_dialogue_advancement_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_opening_hook_card_block = compact_prompt_text(
        kwargs.get("story_opening_hook_card_block")
        or build_story_opening_hook_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_repair_target_block = compact_prompt_text(
        kwargs.get("story_repair_target_block")
        or build_story_repair_target_block(
            kwargs.get("story_repair_summary"),
            kwargs.get("story_repair_targets"),
            kwargs.get("story_preserve_strengths"),
        )
    )
    story_repair_diagnostic_block = compact_prompt_text(kwargs.get("story_repair_diagnostic_block"))
    story_execution_checklist_block = compact_prompt_text(
        kwargs.get("story_execution_checklist_block")
        or build_story_execution_checklist_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
            continuity_preflight=continuity_preflight,
        )
    )
    story_scene_anchor_card_block = compact_prompt_text(
        kwargs.get("story_scene_anchor_card_block")
        or build_story_scene_anchor_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_scene_density_card_block = compact_prompt_text(
        kwargs.get("story_scene_density_card_block")
        or build_story_scene_density_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_repetition_risk_block = compact_prompt_text(
        kwargs.get("story_repetition_risk_block")
        or build_story_repetition_risk_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_acceptance_card_block = compact_prompt_text(
        kwargs.get("story_acceptance_card_block")
        or build_story_acceptance_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_cliffhanger_card_block = compact_prompt_text(
        kwargs.get("story_cliffhanger_card_block")
        or build_story_cliffhanger_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )
    story_character_arc_card_block = compact_prompt_text(
        kwargs.get("story_character_arc_card_block")
        or build_story_character_arc_card_block(
            kwargs.get("creative_mode"),
            kwargs.get("story_focus"),
            scene="chapter",
            plot_stage=kwargs.get("plot_stage"),
        )
    )

    quality_generation_protocol_block = compact_prompt_text(
        "\n".join(
            [
                "【统一协议护栏】",
                f"- 质量块追踪标签：{quality_runtime_tracking_tag}",
                "- 统一吸收第三版规则摘要，不在各链路重复手写散落逻辑。",
                "- runtime 质量块只补充规则来源，不覆盖用户模板主体与业务上下文。",
                f"- {mcp_canon_priority_rule}",
                f"- {mcp_source_disclosure_rule}",
                "- 禁止输出流程化元文本、调度说明、自我评注与来源暴露。",
            ]
        )
    )
    quality_json_protocol_block = compact_prompt_text(
        "\n".join(
            [
                "【统一JSON协议护栏】",
                f"- 质量块追踪标签：{quality_runtime_tracking_tag}",
                "- 维持纯 JSON 输出，不追加 markdown、解释说明、流程文本或来源披露。",
                f"- {mcp_canon_priority_rule}",
                f"- {mcp_source_disclosure_rule}",
                "- 若证据不足，使用 null / 空数组 / 保守结论，不臆造事实。",
            ]
        )
    )
    quality_analysis_block = checker_block or generation_block
    quality_regeneration_block = generation_block or reviser_block

    blocks = {
        "quality_generation_block": generation_block,
        "quality_analysis_block": quality_analysis_block,
        "quality_checker_block": checker_block,
        "quality_reviser_block": reviser_block,
        "quality_regeneration_block": quality_regeneration_block,
        "quality_generation_protocol_block": quality_generation_protocol_block,
        "quality_json_protocol_block": quality_json_protocol_block,
        "quality_mcp_guard_block": mcp_guard_block,
        "quality_external_assets_block": external_assets_block,
        "quality_mcp_references_block": mcp_references,
        "creative_mode_block": creative_mode_block,
        "story_focus_block": story_focus_block,
        "narrative_blueprint_block": narrative_blueprint_block,
        "story_creation_brief_block": story_creation_brief_block,
        "story_long_term_goal_block": story_long_term_goal_block,
        "story_quality_hard_guard_block": story_quality_hard_guard_block,
        "story_pacing_budget_block": story_pacing_budget_block,
        "story_volume_pacing_block": story_volume_pacing_block,
        "story_quality_trend_block": story_quality_trend_block,
        "story_character_focus_anchor_block": story_character_focus_anchor_block,
        "story_foreshadow_payoff_plan_block": story_foreshadow_payoff_plan_block,
        "story_character_state_ledger_block": story_character_state_ledger_block,
        "story_relationship_state_ledger_block": story_relationship_state_ledger_block,
        "story_foreshadow_state_ledger_block": story_foreshadow_state_ledger_block,
        "story_organization_state_ledger_block": story_organization_state_ledger_block,
        "story_career_state_ledger_block": story_career_state_ledger_block,
        "quality_preference_block": quality_preference_block,
        "story_objective_card_block": story_objective_card_block,
        "story_result_card_block": story_result_card_block,
        "story_payoff_chain_card_block": story_payoff_chain_card_block,
        "story_rule_grounding_card_block": story_rule_grounding_card_block,
        "story_information_release_card_block": story_information_release_card_block,
        "story_emotion_landing_card_block": story_emotion_landing_card_block,
        "story_action_rendering_card_block": story_action_rendering_card_block,
        "story_summary_tone_control_card_block": story_summary_tone_control_card_block,
        "story_repetition_control_card_block": story_repetition_control_card_block,
        "story_viewpoint_discipline_card_block": story_viewpoint_discipline_card_block,
        "story_dialogue_advancement_card_block": story_dialogue_advancement_card_block,
        "story_opening_hook_card_block": story_opening_hook_card_block,
        "story_repair_target_block": story_repair_target_block,
        "story_repair_diagnostic_block": story_repair_diagnostic_block,
        "story_execution_checklist_block": story_execution_checklist_block,
        "story_scene_anchor_card_block": story_scene_anchor_card_block,
        "story_scene_density_card_block": story_scene_density_card_block,
        "story_repetition_risk_block": story_repetition_risk_block,
        "story_acceptance_card_block": story_acceptance_card_block,
        "story_cliffhanger_card_block": story_cliffhanger_card_block,
        "story_character_arc_card_block": story_character_arc_card_block,
    }

    template_insertion = template_insertions.get(template_key or "")
    blocks = apply_quality_optional_block_budget(
        blocks,
        template_key=template_key,
        template_insertion=template_insertion,
        plot_stage=kwargs.get("plot_stage"),
        optional_card_block_budgets=optional_card_block_budgets,
        optional_card_default_budget=optional_card_default_budget,
        regeneration_optional_card_budget=regeneration_optional_card_budget,
        optional_card_drop_order=optional_card_drop_order,
        budget_override=kwargs.get("quality_optional_block_budget"),
        quality_metrics_summary=kwargs.get("quality_metrics_summary"),
    )
    if template_insertion:
        blocks["quality_contract_block"] = template_insertion.format(**blocks)
    else:
        blocks["quality_contract_block"] = ""
    return blocks


def build_creative_mode_block(mode: Optional[str], *, scene: str) -> str:
    normalized = normalize_creative_mode(mode)
    if not normalized:
        return ""

    spec = CREATIVE_MODE_SPECS.get(normalized)
    if not spec:
        return ""

    bullets = spec.get(scene) or []
    if not bullets:
        return ""

    lines = [f"【创作模式】当前采用“{spec['label']}”"]
    lines.extend(f"- {item}" for item in bullets)
    return _compact_prompt_text("\n".join(lines))


def build_story_focus_block(value: Optional[str], *, scene: str) -> str:
    normalized = normalize_story_focus(value)
    if not normalized:
        return ""

    spec = STORY_FOCUS_SPECS.get(normalized)
    if not spec:
        return ""

    bullets = spec.get(scene) or []
    if not bullets:
        return ""

    lines = [f"【结构侧重点】当前优先“{spec['label']}”"]
    lines.extend(f"- {item}" for item in bullets)
    return _compact_prompt_text("\n".join(lines))


def build_quality_preference_block(
    quality_preset: Optional[str],
    quality_notes: Optional[str],
    *,
    scene: str,
) -> str:
    normalized_preset = normalize_quality_preset(quality_preset)
    note_items = _split_quality_preference_note_items(quality_notes)

    spec = QUALITY_PREFERENCE_SPECS.get(normalized_preset) if normalized_preset else None
    bullets = spec.get(scene) if spec else []

    if not bullets and not note_items:
        return ""

    if spec:
        lines = [f"【质量预设】当前采用“{spec['label']}”"]
        lines.extend(f"- {item}" for item in bullets)
    else:
        lines = ["【质量偏好补充】"]

    if note_items:
        if len(note_items) == 1:
            lines.append(f"- 补充偏好：{note_items[0]}")
        else:
            lines.append("- 补充偏好：")
            lines.extend(f"  - {item}" for item in note_items)

    return _compact_prompt_text("\n".join(lines))


def build_narrative_blueprint_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    priority_beats: list[str] = []
    priority_risks: list[str] = []

    if normalized_mode == "hook":
        priority_beats.extend([
            "开场更早抛出异常、危险或未完成目标，先抓住读者注意力。",
            "尾段优先保留信息缺口、危险临门或选择未决，不要平收。",
        ])
        priority_risks.append("不要只堆钩子和异常，却缺少实质推进。")
    elif normalized_mode == "emotion":
        priority_beats.extend([
            "关键转折后要写出人物情绪余震和关系反应，不只交代结果。",
            "让动作、停顿和对白共同承载情绪，而不是全靠抒情说明。",
        ])
        priority_risks.append("不要让情绪独自悬空，必须落回选择与后果。")
    elif normalized_mode == "suspense":
        priority_beats.extend([
            "中前段持续制造信息差、误判或证据变化，让压力逐步抬升。",
            "每个阶段都给出一点新认知，但不要一次讲透底牌。",
        ])
        priority_risks.append("避免把悬念写成纯遮掩，读者需要看到有效推进。")
    elif normalized_mode == "relationship":
        priority_beats.extend([
            "把关键冲突尽量落在人与人之间的立场差、亏欠感或试探上。",
            "安排一次关系位移，让后续行动因为关系变化而改道。",
        ])
        priority_risks.append("不要只有关系情绪，没有行动层面的后续影响。")
    elif normalized_mode == "payoff":
        priority_beats.extend([
            "优先安排前文铺垫兑现、收获反馈或阶段性反转，给读者明确回报。",
            "兑现后顺手打开下一轮更大的目标或麻烦，不把气口写死。",
        ])
        priority_risks.append("不要只顾爽点回收，忽略代价与后续空间。")
    elif normalized_mode == "balanced":
        priority_beats.extend([
            "推进、情绪、信息释放和回报要彼此穿插，不让单一节拍统治全文。",
        ])

    if normalized_focus == "advance_plot":
        priority_beats.extend([
            "每个关键段都要写出行动结果和局势变化，避免原地解释。",
        ])
        priority_risks.append("避免设定说明和情绪回旋挤压主线推进。")
    elif normalized_focus == "deepen_character":
        priority_beats.extend([
            "至少安排一次能暴露人物弱点、执念或价值判断的选择。",
        ])
        priority_risks.append("不要把人物塑形写成静态介绍，必须落到行为上。")
    elif normalized_focus == "escalate_conflict":
        priority_beats.extend([
            "让阻力、代价和对立面逐段变强，形成持续抬压链条。",
        ])
        priority_risks.append("避免重复同级冲突，读者会觉得原地踏步。")
    elif normalized_focus == "reveal_mystery":
        priority_beats.extend([
            "优先安排线索出现、误导修正和认知刷新，至少推进一点真相。",
        ])
        priority_risks.append("不要把揭示写成解释堆叠，尽量通过事件和证据推进。")
    elif normalized_focus == "relationship_shift":
        priority_beats.extend([
            "对话、动作和站队变化都要服务关系转折，而不只是口头表态。",
        ])
        priority_risks.append("不要让关系变化只停留在情绪层，没有后续选择代价。")
    elif normalized_focus == "foreshadow_payoff":
        priority_beats.extend([
            "回收时既要兑现前文承诺，也要带出新的悬念或任务。",
        ])
        priority_risks.append("避免只用说明句回收伏笔，最好落在事件结果上。")

    if normalized_stage == "development":
        priority_beats.append("当前阶段优先扩张局势、铺开变量，并把选择成本逐章抬高。")
        priority_risks.append("避免太早交底或提前透支高潮。")
    elif normalized_stage == "climax":
        priority_beats.append("当前阶段要让核心矛盾正面碰撞，把选择逼到无法拖延的节点。")
        priority_risks.append("避免高潮只有声量，没有清晰结果与代价。")
    elif normalized_stage == "ending":
        priority_beats.append("当前阶段要优先收束主承诺、主悬念和关键关系线，再留余味。")
        priority_risks.append("避免只顾收尾，忘了兑现前文最重要的铺垫。")

    if scene == "outline":
        base_beats = [
            "前段先放出主目标、局势缺口或新任务，不要直接堆设定。",
            "中段持续抬高阻力、代价或信息差，让章节彼此形成递进关系。",
            "后段安排一次明显转折、揭示或关系位移，改变后续走向。",
            "收尾既给阶段性结果，也留下下一轮想追下去的问题。",
        ]
        base_risks = ["不要把整轮大纲写成同一种功能，节拍必须有起伏。"]
        scene_label = "大纲"
    else:
        base_beats = [
            "开场尽快抛出异常、目标或受阻点，不做平铺导入。",
            "中段用连续动作推进局势，并让阻力或代价升级。",
            "后段安排一次局势改写、信息刷新或关系位移。",
            "结尾保留明确追读牵引，不要平收。",
        ]
        base_risks = ["不要把节拍写成说明书，关键节点都要有动作和即时结果。"]
        scene_label = "章节"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    beats = _dedupe_prompt_items(priority_beats + base_beats)[:4]
    risks = _dedupe_prompt_items(priority_risks + base_risks)
    combo_text = " / ".join(combo_labels) if combo_labels else "默认结构"

    lines = [f"【结构蓝图】本轮按“{combo_text}”组织{scene_label}节拍"]
    lines.extend(f"- {item}" for item in beats)
    if risks:
        lines.append(f"- 重点避免：{risks[0]}")
    return _compact_prompt_text("\n".join(lines))


def build_story_objective_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        objective = "让本轮章节承担清晰主任务，不平均摊功能。"
        obstacle = "让中段持续抬压，每一章都比上一章更难一点。"
        turn = "在后段安排一次会改写后续走向的结构转折。"
        hook = "尾段留下下一轮章节必须回应的问题或新任务。"
        scene_label = "大纲"
    else:
        objective = "让本章推动一个看得见的目标，不写空转段落。"
        obstacle = "安排一次明确受阻、代价上升或信息错位。"
        turn = "在中后段安排一次认知或局面改写。"
        hook = "章尾留下追读牵引，不平收。"
        scene_label = "章节"

    if normalized_mode == "hook":
        hook = "把钩子放在异常、危险或未决选择上，尽量做到前段抓人、尾段牵引。"
        turn = "转折优先用信息缺口扩大、危险临门或局势突然偏转来触发。"
    elif normalized_mode == "emotion":
        objective = (
            "目标除了推进事件，还要逼出人物情绪波动和关系反馈。"
            if scene == "outline"
            else "让本章既推进事件，也逼出人物情绪与关系反应。"
        )
        turn = "转折优先落在情绪反噬、误伤、和解受阻或认知偏移上。"
        hook = "钩子留在情绪未落地、关系未说破或选择仍有余震处。"
    elif normalized_mode == "suspense":
        obstacle = "阻力优先来自信息差、误判、证据反噬或真相未全。"
        turn = "转折通过线索翻面、认知刷新、身份异动或危险升级完成。"
        hook = "钩子留在新疑点、半揭开的答案或更近一步的危险上。"
    elif normalized_mode == "relationship":
        objective = (
            "本轮重点推动人物关系位移，让站队和信任结构发生变化。"
            if scene == "outline"
            else "让本章推动一次明确的关系位移，而不只是情绪点缀。"
        )
        obstacle = "阻力来自立场差、亏欠、信任裂缝或试探失手。"
        turn = "转折优先用关系破裂、突然靠近、站队变化或误会反转来完成。"
        hook = "钩子留在关系未定、话没说透、立场悬空的地方。"
    elif normalized_mode == "payoff":
        objective = (
            "本轮重点兑现前文铺垫、承诺或能力，并带出更大后果。"
            if scene == "outline"
            else "让本章承担一次明确兑现，让读者感到回报落地。"
        )
        turn = "转折优先让兑现带出更大代价、更高目标或新的麻烦。"
        hook = "钩子放在回报之后的新失衡上，而不是只停在爽点本身。"

    if normalized_focus == "advance_plot":
        objective = "核心目标是把局势往前推一格，至少形成新的行动结果。"
    elif normalized_focus == "deepen_character":
        objective = "核心目标是让角色在选择里显形，暴露弱点、执念或价值判断。"
    elif normalized_focus == "escalate_conflict":
        obstacle = "阻力必须逐层变强，让代价和对立面都更具体。"
    elif normalized_focus == "reveal_mystery":
        turn = "转折优先通过线索出现、误导修正和认知刷新来完成。"
    elif normalized_focus == "relationship_shift":
        turn = "转折必须带来关系位移、立场重排或信任结构变化。"
    elif normalized_focus == "foreshadow_payoff":
        objective = "核心目标是兑现前文埋设，并顺手打开新的后续空间。"
        hook = "钩子留在兑现后的新承诺、新麻烦或更大代价上。"

    if normalized_stage == "development":
        objective = (
            "当前阶段先立局、铺变量和主任务，把后续压力链搭起来。"
            if scene == "outline"
            else "当前阶段先把局势和眼前目标推到更难的位置。"
        )
    elif normalized_stage == "climax":
        obstacle = "阻力要逼近正面碰撞，选择代价必须明显抬高。"
        turn = "转折要接近核心碰撞点，不能只是小波动。"
    elif normalized_stage == "ending":
        objective = (
            "当前阶段优先回收主承诺、主悬念和关键关系线。"
            if scene == "outline"
            else "当前阶段让本章承担主承诺或关键关系线的回收职责。"
        )
        hook = "钩子更适合留余味、次级悬念或收束后的新失衡，不能抢走主收束。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认任务"
    lines = [f"【{scene_label}目标卡】本轮按“{combo_text}”优先落实以下叙事任务"]
    lines.append(f"- 目标：{objective}")
    lines.append(f"- 阻力：{obstacle}")
    lines.append(f"- 转折：{turn}")
    lines.append(f"- 钩子：{hook}")
    return _compact_prompt_text("\n".join(lines))


def build_story_result_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        progress = "这一轮结束后，主线应进入一个更具体、更难回头的新局面。"
        reveal = "至少释放一轮信息、真相碎片或兑现回报，避免纯拖延。"
        relationship = "关键人物关系、站队或信任结构要出现可见位移。"
        fallout = "尾段要把下一轮章节必须回应的压力、问题或任务钉住。"
        scene_label = "大纲"
    else:
        progress = "这一章结束后，局势应明确前移，人物不能还停在原地。"
        reveal = "至少交付一个新认知、新线索或一次有效兑现。"
        relationship = "至少有一条人物关系线出现可见变化，而不是只说情绪。"
        fallout = "章尾要留下一个会逼出下章动作的余波，而不是平稳收住。"
        scene_label = "章节"

    if normalized_mode == "hook":
        progress = (
            "本轮结束后，读者要感到故事被明显拽进下一段更危险的局面。"
            if scene == "outline"
            else "本章结束后，局势必须被推到一个不继续看就会难受的节点。"
        )
        fallout = "余波优先落在未决选择、临门危险或刚被挑开的异常上。"
    elif normalized_mode == "emotion":
        reveal = "结果里要能看到情绪代价、误伤、和解受阻或内心认知变化。"
        relationship = "关系结果要落到互动后果上，让人物之后的做法因此改变。"
    elif normalized_mode == "suspense":
        reveal = "至少留下一个更接近真相的新证据，同时制造新的误判空间。"
        fallout = "余波留在新疑点、身份异动或危险升级上，不能只剩空白遮掩。"
    elif normalized_mode == "relationship":
        relationship = "结果里必须出现一次明确的关系位移、立场变化或信任重排。"
        fallout = "余波最好落在关系未定、话未说透或站队未稳上。"
    elif normalized_mode == "payoff":
        reveal = "结果要让读者看到铺垫兑现、回报落地，并感到不是白等。"
        progress = (
            "兑现之后，主线要进入一个新的阶段，而不是只做结算。"
            if scene == "outline"
            else "兑现之后，局势要被顺势推向更高目标或更大麻烦。"
        )

    if normalized_focus == "advance_plot":
        progress = "推进结果必须清晰可见：行动产生了后果，局势换了位置。"
    elif normalized_focus == "deepen_character":
        reveal = "结果要让人物的弱点、执念或价值判断真正显形，而非停在说明。"
        relationship = "人物变化要影响他与他人的互动方式或后续选择。"
    elif normalized_focus == "escalate_conflict":
        progress = "推进结果不是前进一步，而是把人推入更高代价的冲突区。"
        fallout = "余波要把冲突继续抬高，让下一轮没有轻松退路。"
    elif normalized_focus == "reveal_mystery":
        reveal = "揭示结果必须真实推进谜团，不只是制造更多模糊表述。"
    elif normalized_focus == "relationship_shift":
        relationship = "关系结果必须足够明确，能改变两人之后的说话方式、站位或合作条件。"
    elif normalized_focus == "foreshadow_payoff":
        reveal = "结果要让前文埋设获得兑现，同时打开新的后续空间。"
        fallout = "余波放在兑现后的新承诺、新代价或更大失衡上。"

    if normalized_stage == "development":
        progress = (
            "这一轮结束后，故事应完成立局并把压力链真正搭起来。"
            if scene == "outline"
            else "这一章结束后，故事要进入一个更难但更清晰的推进区。"
        )
        fallout = "余波要把后续任务钉住，让读者知道下一章不是重复上一章。"
    elif normalized_stage == "climax":
        progress = "推进结果要逼近或触发正面碰撞，不能只是外围晃动。"
        reveal = "揭示结果要掀开关键底牌、核心真相或决定性误判。"
    elif normalized_stage == "ending":
        reveal = "揭示结果优先服务主承诺、主悬念与关键伏笔的回收。"
        relationship = "关系结果要体现收束、定局或带余温的最终位移。"
        fallout = "余波更适合留余味、后效和新失衡，不能抢走主收束。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认结果"
    lines = [f"【{scene_label}结果卡】本轮写完后，至少让读者感知到以下结果变化（{combo_text}）"]
    lines.append(f"- 推进：{progress}")
    lines.append(f"- 揭示：{reveal}")
    lines.append(f"- 关系：{relationship}")
    lines.append(f"- 余波：{fallout}")
    return _compact_prompt_text("\n".join(lines))


def build_story_creation_brief_block(creation_brief: Optional[str]) -> str:
    brief = _compact_prompt_text(creation_brief)
    if not brief:
        return ""
    lines = ["【本轮创作总控】"]
    lines.append(f"- 执行摘要：{brief}")
    lines.append("- 先按总控摘要定目标、推进与收束，再参考后续卡片补细节，不要彼此打架。")
    return _compact_prompt_text("\n".join(lines))


def build_story_repair_target_block(
    repair_summary: Optional[str],
    repair_targets: Optional[Sequence[str]],
    preserve_strengths: Optional[Sequence[str]] = None,
) -> str:
    summary = str(repair_summary or "").strip()
    targets = _dedupe_prompt_items([str(item or "").strip() for item in (repair_targets or [])])
    strengths = _dedupe_prompt_items([str(item or "").strip() for item in (preserve_strengths or [])])

    if not summary and not targets and not strengths:
        return ""

    lines = ["【修复目标卡】"]
    if summary:
        lines.append(f"- 当前问题：{summary}")
    if targets:
        lines.append("- 本轮动作：")
        lines.extend(f"  - {item}" for item in targets)
    if strengths:
        lines.append("- 保留优势：")
        lines.extend(f"  - {item}" for item in strengths)
    target_text = "\n".join([summary, *targets])
    if any(token in target_text for token in ("冲突", "阻碍", "代价", "升级", "受阻", "conflict")):
        lines.append("- Conflict repair hard rule: include at least one obstacle -> choice -> cost sequence in the middle; do not only add explanation or circular arguing.")
    if any(token in target_text for token in ("章尾", "钩子", "悬念", "未决", "cliffhanger")):
        lines.append("- Ending repair hard rule: the final paragraph must leave an info gap, approaching danger, identity shift, or pending choice, and the last line cannot soften the landing.")
    if any(token in target_text for token in ("角色状态", "人物状态", "角色连续性", "character continuity ledger", "carry forward the character continuity ledger")):
        lines.append("- Character continuity hard rule: explicitly land at least one character-state ledger item as action, hesitation, failure, or cost.")
    if any(token in target_text for token in ("关系状态", "关系连续性", "relationship continuity ledger", "relationship ledger", "互信", "站位")):
        lines.append("- Relationship continuity hard rule: explicitly land at least one relationship-state ledger item through dialogue probing, position shift, or trust wobble.")
    lines.append("- 修复必须落到具体事件、动作和后果，不要只加解释或换说法。")
    return _compact_prompt_text("\n".join(lines))


def build_story_execution_checklist_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
    continuity_preflight: Optional[Any] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        opening = "前段先用 1-2 章立主任务、人物站位和局势缺口，尽快进入事件。"
        pressure = "中段持续加压，每一章追加一个新阻力、代价或变量。"
        pivot = "后段安排一次会改写路线的关键转折、揭示或站队变化。"
        closing = "尾段先给阶段性结果，再把下一轮问题抛实。"
        scene_label = "大纲执行清单"
    else:
        opening = "开场 30% 内抛出目标、异常或受阻点，不平铺背景。"
        pressure = "中段用动作、对话和反馈连续加压，避免解释停顿。"
        pivot = "中后段安排一次改写认知或局面的关键动作。"
        closing = "收尾先落结果，再留下逼出下章的余波。"
        scene_label = "章节执行清单"

    if normalized_mode == "hook":
        opening = (
            "前段优先让异常、危险或未决任务尽快冒头，不慢热铺垫。"
            if scene == "outline"
            else "开场尽快抛出异常、险情或未决选择，让读者立刻进入状态。"
        )
        closing = "收尾把悬而未决的危险、选择或信息缺口钉牢，形成追读牵引。"
    elif normalized_mode == "emotion":
        pressure = "中段用互动、误伤、退让受阻或情绪回弹来持续加压。"
        pivot = "关键转折优先落在情绪爆裂、和解失败或认知刺痛上。"
        closing = "收尾保留情绪余震，让人物无法当场彻底消化。"
    elif normalized_mode == "suspense":
        opening = "开场先扔出异常线索、误判苗头或危险信号，再补背景。"
        pressure = "中段不断扩大信息差、证据变化和错误判断的代价。"
        pivot = "转折优先让线索翻面、身份异动或危险升级来改写局面。"
        closing = "收尾留下更尖锐的新疑点，而不是只把答案藏起来。"
    elif normalized_mode == "relationship":
        opening = "开场先把关系张力、站位差或试探动作摆上台面。"
        pressure = "中段持续通过对话、行动和站队测试来挤压关系。"
        pivot = "转折优先用关系破裂、突然靠近或立场变化来触发。"
        closing = "收尾把关系悬在未定状态，逼出下一轮互动。"
    elif normalized_mode == "payoff":
        opening = "开场尽快回扣前文埋设，提醒读者这轮会有兑现。"
        pressure = "中段不断把兑现条件推近，同时抬高兑现所需代价。"
        pivot = "转折优先让铺垫兑现落地，但必须伴随新后果。"
        closing = "收尾不要停在爽点，要顺手抛出兑现后的新失衡。"

    if normalized_focus == "advance_plot":
        opening = "开场先亮明本轮要推进的事，别让读者等太久才知道这章要干嘛。"
        pressure = "中段每次推进都要带来新结果，避免原地解释和空转。"
    elif normalized_focus == "deepen_character":
        pressure = "中段把压力尽量变成选择题，让人物性格在决策里显形。"
        pivot = "关键转折最好来自人物自己的选择、软肋或价值判断。"
        closing = "收尾保留人物做完选择后的余震，而不是只交代事件结束。"
    elif normalized_focus == "escalate_conflict":
        pressure = "中段每一轮加压都要比上一轮更狠，别重复同级冲突。"
        pivot = "转折要把冲突推向正面碰撞，而不是继续绕圈。"
        closing = "收尾把人物钉在更高代价区，确保下一轮没法轻退。"
    elif normalized_focus == "reveal_mystery":
        opening = "开场尽快抛出线索、异常或疑点，别先讲设定。"
        pressure = "中段通过调查、误导修正和证据变化推进认知。"
        pivot = "转折要真正修正一次认知，而不是只多说一点背景。"
    elif normalized_focus == "relationship_shift":
        pressure = "中段每次互动都要推动信任、亏欠、戒备或站队发生位移。"
        pivot = "转折要让关系位置真正改变，而不是嘴上吵完又回原点。"
        closing = "收尾留下新的关系姿态或未兑现承诺，逼出后续互动。"
    elif normalized_focus == "foreshadow_payoff":
        opening = "开场尽快把前文埋下的人、物、承诺或代价重新拉回现场。"
        pivot = "关键转折优先落实伏笔兑现，并让读者看见兑现后的连锁反应。"
        closing = "收尾保留回收后的新缺口，避免把兑现写成句号。"

    if normalized_stage == "development":
        opening = (
            "前几章先把高频场景、常驻人物和主要行动空间固定下来，再持续加变量。"
            if scene == "outline"
            else "发展阶段先把当前场景秩序和人物站位立稳，再推进变量入场。"
        )
        pivot = "发展阶段至少安排一次让局面升级或关系改写的关键动作。"
        closing = "收尾先压实当前推进结果，再给后续升级留口。"
    elif normalized_stage == "climax":
        opening = "高潮阶段开场尽快把人物推到主碰撞现场，不再外围试探。"
        pressure = "中段持续抬高代价、时限和压迫，不能退回解释区。"
        pivot = "转折必须推动正面碰撞、关键反转或局势翻面。"
        closing = "收尾先落下当前碰撞结果，再把更大的余波推向下章。"
    elif normalized_stage == "ending":
        opening = "收束阶段开场尽快把待回收的承诺、关系或真相重新拉回台面。"
        pressure = "中段围绕最终代价、兑现与收束推进，不再横生新主枝线。"
        pivot = "关键转折优先完成回收并揭示最后代价，别再新开大主线。"
        closing = "收尾要完成阶段性回收，同时留下明确余味或尾问。"

    if scene != "outline":
        opening = f"{opening} 前 20%-25% 内至少给出目标、异常或受阻点之一。"
        pressure = f"{pressure} 中段至少完成一次“推进→受阻→决断→代价/反弹”的冲突链。"
        pivot = f"{pivot} 关键动作最好伴随一条设定规则的触发、限制或反噬。"
        closing = f"{closing} 最后一段必须留下新的信息缺口、危险逼近、身份位移或待做选择之一。"
        opening = f"{opening} 最好前 120-180 字内就同时出现两类抓手（异常 / 任务 / 受阻 / 倒计时 / 强制选择）。"
        closing = f"{closing} 最后一行禁止复盘解释或抒情软收，优先落在指令、锁定、翻面信息、逼近危险或未完成选择上。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认执行节奏"
    lines = [f"【{scene_label}】本轮优先按以下节奏执行（{combo_text}）"]
    lines.append(f"- 开场：{opening}")
    lines.append(f"- 加压：{pressure}")
    lines.append(f"- 转折：{pivot}")
    lines.append(f"- 收束：{closing}")
    continuity_info = continuity_preflight if isinstance(continuity_preflight, Mapping) else {}
    continuity_summary = str(continuity_info.get("summary") or "").strip()
    continuity_targets = _normalize_runtime_prompt_items(continuity_info.get("repair_targets"), limit=3)
    if continuity_targets:
        lines.append("- 连续性接力：优先补齐以下跨章承接点，至少落实 1 项到动作、对白或场景变化里。")
        lines.extend(f"  - {item}" for item in continuity_targets)
    elif continuity_summary:
        lines.append(f"- 连续性接力：{continuity_summary}；本章至少把其中一个承接点写成可见行动。")

    return _compact_prompt_text("\n".join(lines))


def build_story_scene_anchor_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        entry_anchor = "每一章先写清谁在场、身处何处、眼前要做什么，让事件有落地空间。"
        lens_focus = "单章优先只设一个主镜头重心（主行动/主关系/主线索其一），别平均撒给所有元素。"
        info_release = "新设定、新背景和新关系判断分批投放，优先绑在事件推进节点上。"
        transition_rule = "章节与章节之间的换场要写明触发动作、时间位移或局势变化，不空跳。"
        scene_label = "大纲场景调度卡"
    else:
        entry_anchor = "开场3-5句内交代人在何处、正在做什么、眼前压力从哪来，让读者先站稳。"
        lens_focus = "单场景优先盯住一个镜头重心（动作推进/关系碰撞/线索识别其一），别四处撒。"
        info_release = "新信息优先嵌进动作、观察、对白和即时反应里，一次只释放一层。"
        transition_rule = "切换时间、地点或行动阶段时，用简短动作或环境变化做承接，避免镜头空跳。"
        scene_label = "章节场景调度卡"

    if normalized_mode == "hook":
        entry_anchor = (
            "每章开头优先把异常、危险或任务阻力放进当前场景，不靠背景慢慢预热。"
            if scene == "outline"
            else "开场第一时间让异常、危险或任务阻力进入场内，别先讲完整背景。"
        )
        lens_focus = "镜头优先跟着最能制造牵引的问题走，别被枝节说明抢掉主注意力。"
        info_release = "关键情报分两步以内放出，不一次把答案和解释全说透。"
    elif normalized_mode == "emotion":
        lens_focus = "镜头优先盯动作停顿、身体距离、视线变化和话没说满的地方。"
        info_release = "情绪信息优先藏在回避、试探、失控边缘和即时反应里，不整段抒情讲完。"
    elif normalized_mode == "suspense":
        entry_anchor = "先把异常细节、危险信号或错误判断的触发点放进场，再补必要背景。"
        lens_focus = "镜头优先盯可疑细节、认知偏差和证据变化，不被大段说明拖停。"
        info_release = "线索一次只推进半步到一步，并配一个读者可验证的细节支点。"
    elif normalized_mode == "relationship":
        lens_focus = "镜头优先盯站位、语气、视线和试探动作，让关系张力有身体感。"
        transition_rule = "换场要让读者明白关系位置为什么变了，而不是人物凭空突然亲疏变化。"
    elif normalized_mode == "payoff":
        entry_anchor = "让待兑现的人、物、承诺或麻烦尽快回到场内，别临时凭空冒出。"
        info_release = "先让兑现条件现身，再给爆发反馈与余波，不把回报写成一句结果通知。"

    if normalized_focus == "advance_plot":
        lens_focus = "镜头重心跟主任务走，和主推进无关的抒情或设定只保留必要量。"
    elif normalized_focus == "deepen_character":
        lens_focus = "镜头贴近人物决策前后的犹疑、反应和自控失效，让性格在现场显形。"
        info_release = "人物信息通过选择、动作和反应露出，不靠整段自述讲完。"
    elif normalized_focus == "escalate_conflict":
        transition_rule = "每次换场都要把压力抬高一级，不重复同级拉扯或相似争执。"
    elif normalized_focus == "reveal_mystery":
        info_release = "线索一次只推进一层，且必须挂在可见证据、异常反应或判断修正上。"
    elif normalized_focus == "relationship_shift":
        lens_focus = "镜头重点盯说话方式、身体距离和站队动作的变化，让关系位移可见。"
    elif normalized_focus == "foreshadow_payoff":
        entry_anchor = "让前文埋下的人、物、承诺或代价尽早回到场内，别临时补设定。"
        info_release = "兑现信息要让读者能认出回扣来源，再补当下反馈与新后果。"

    if normalized_stage == "development":
        entry_anchor = (
            "前几章先把高频场景、常驻人物和主要行动空间固定下来，再持续加变量。"
            if scene == "outline"
            else "发展阶段先把当前场景秩序和人物站位立稳，再推进变量入场。"
        )
    elif normalized_stage == "climax":
        lens_focus = "高潮阶段镜头尽量贴近最核心的碰撞点，不频繁切旁枝和外围观察。"
        transition_rule = "高潮阶段减少无效横移，切换要短促直接，始终围着主碰撞服务。"
    elif normalized_stage == "ending":
        info_release = "收束阶段优先回收主承诺、主关系和主真相，不再新开大块信息池。"
        transition_rule = "结尾换场要服务收束或余味，别再把战线铺散到新的主空间。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认场景调度"
    lines = [f"【{scene_label}】本轮优先按以下场景调度执行（{combo_text}）"]
    lines.append(f"- 入场锚点：{entry_anchor}")
    lines.append(f"- 镜头重心：{lens_focus}")
    lines.append(f"- 信息投放：{info_release}")
    lines.append(f"- 切换规则：{transition_rule}")
    return _compact_prompt_text("\n".join(lines))


def build_story_scene_density_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        scene_task = "每个关键场景尽量同时承担推进、揭示、冲突、情绪中的两项以上，不让章节只剩单一功能。"
        live_action = "该现场化的节点尽量现场化：动作发起、受阻、反馈和局面变化要能被看见。"
        load_mix = "信息、情绪和关系变化尽量压在事件里完成，不把它们拆成单独的大段说明。"
        rhythm_breath = "推进段更利落，余波段可以稍停，但不要连续多个场景都只有解释或复盘。"
        avoid_line = "不要把整轮剧情排成“解释场—聊天场—回忆场”串联，却迟迟没有真正局势变化。"
        scene_label = "大纲"
    else:
        scene_task = "本章每个重要场景都要有明确任务：推进局势、抬高压力、揭一层信息或改动关系。"
        live_action = "关键冲突、破局和兑现尽量写出动作链和现场反馈，不要一笔带过最该看的过程。"
        load_mix = "把信息、情绪和关系变化嵌进动作与对白里，减少大段静态解释。"
        rhythm_breath = "短段推进、必要停顿、再继续推进，让读者有气口但不掉线。"
        avoid_line = "不要连续几段都在讲、想、回忆、解释，却没有动作、反馈和局势移动。"
        scene_label = "章节"

    if normalized_mode == "hook":
        scene_task = "开场场景尽量尽快入事，让第一个场景就承担抓人和立压任务。"
    elif normalized_mode == "emotion":
        load_mix = "情绪密度来自互动、误伤、靠近失败和余波，不是单靠大段抒情。"
        rhythm_breath = "情绪段可以稍慢，但必须有新的触发、反应或关系变化支撑。"
    elif normalized_mode == "suspense":
        scene_task = "悬念型场景最好每场至少多出一个新线索、新反常或新风险。"
        live_action = "危险与调查尽量现场发生，不要只在事后总结“原来很危险”。"
    elif normalized_mode == "relationship":
        load_mix = "关系戏也要有事件支点：试探、合作、冲突、靠近或决裂，而不是纯聊天。"
    elif normalized_mode == "payoff":
        live_action = "兑现型场景优先把最值钱的动作、反应和反馈写在台前，不要藏在摘要句里。"

    if normalized_focus == "advance_plot":
        scene_task = "场景结束后最好能看到主线确实前进了一格，而不是忙完还在原地。"
    elif normalized_focus == "deepen_character":
        load_mix = "人物塑形最好落在选择和反应里，不要把场景停下来专门写人物说明书。"
    elif normalized_focus == "escalate_conflict":
        live_action = "冲突升级优先靠更难的现场碰撞和更贵的代价，不靠口头宣布升级。"
    elif normalized_focus == "reveal_mystery":
        scene_task = "每个关键场景最好都让谜团多推进半步，而不是只在个别节点突然集中补答案。"
    elif normalized_focus == "relationship_shift":
        rhythm_breath = "关系变化要有拉扯节奏：试探、误判、碰撞、余波，不要一句话突然完成。"
    elif normalized_focus == "foreshadow_payoff":
        scene_task = "尽量让某个场景承担伏笔兑现或预埋，不要全章都没有回报节点。"

    stage_line = ""
    if normalized_stage == "development":
        stage_line = "发展阶段重在把场景链铺密：每场都给一点推进，不让中段发空。"
    elif normalized_stage == "climax":
        stage_line = "高潮阶段要提高现场化比例，压缩解释和复盘，让动作、决断与后果顶上来。"
        avoid_line = "不要在高潮章连续堆长段回忆、讲解和心理总结，把冲击拆散。"
    elif normalized_stage == "ending":
        stage_line = "结局阶段的场景密度重点是回收与余波并存：既要落地，也要留一丝回味。"
        avoid_line = "不要在收尾阶段继续用很多过渡场把关键回收往后拖。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认密度"
    lines = [f"【{scene_label}场景密度卡】本轮请提升每个场景的有效载荷与节奏（{combo_text}）"]
    lines.append(f"- 场景任务：{scene_task}")
    lines.append(f"- 现场化：{live_action}")
    lines.append(f"- 装载方式：{load_mix}")
    lines.append(f"- 节奏呼吸：{rhythm_breath}")
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


def build_story_payoff_chain_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        seed_point = "这一轮至少承接一个已有铺垫，或埋下一个后续能尽快回收的小钩点，不让整卷只会铺不会收。"
        payoff_point = "安排一个具体兑现节点：能力见效、关系翻面、计划得手、误判反噬或线索翻正。"
        feedback_chain = "兑现之后要带出局势变化、关系余震、资源得失或新的行动压力。"
        reader_reward = "让读者明显感到这轮有收获，不是纯过渡章群。"
        avoid_line = "不要把所有回收都推到很后面，也不要只在总结句里宣布“某伏笔终于兑现”。"
        scene_label = "大纲"
    else:
        seed_point = "本章最好承接一个前文钩点，或提前挂出一个本章内/近章可回收的小铺垫。"
        payoff_point = "给读者一个看得见的兑现瞬间：动作打中、关系变位、计划起效、真相掀半层、承诺终于落地。"
        feedback_chain = "兑现后立刻写反馈和余波，不只报结果，要让人物和局面都跟着变。"
        reader_reward = "让追更读者在本章拿到一个明确回报，而不是一直被要求耐心等待。"
        avoid_line = "不要只铺不收，也不要把兑现写成一句轻飘飘的结果播报。"
        scene_label = "章节"

    if normalized_mode == "hook":
        payoff_point = "钩子型兑现最好来得更快，让读者早一点尝到“这章真的有事发生”的回报。"
    elif normalized_mode == "emotion":
        payoff_point = "情绪型兑现可以落在一句没说出口的话被说出、一次误解被捅破，或一次安慰彻底失败。"
        feedback_chain = "兑现后的余波优先写关系温差、情绪后坐力和人物自我认知变化。"
    elif normalized_mode == "suspense":
        payoff_point = "悬念型兑现更适合“揭半层真相 + 打开更危险缺口”，既满足又继续勾人。"
    elif normalized_mode == "relationship":
        payoff_point = "关系型兑现优先落在站位变化、信任转移、边界突破或彻底决裂。"
    elif normalized_mode == "payoff":
        seed_point = "优先锁定前文明确埋过的承诺、伏笔或能力点，不要再临时找替身回收。"
        reader_reward = "兑现时让读者清楚感到“前面那些铺垫没有白等”。"

    if normalized_focus == "advance_plot":
        feedback_chain = "兑现后的反馈必须推动主线进入下一格，别回收完又回到原地。"
    elif normalized_focus == "deepen_character":
        payoff_point = "兑现瞬间最好顺便照出人物的底线、成长、执念或迟来的代价感。"
    elif normalized_focus == "escalate_conflict":
        feedback_chain = "回收后不要泄压，最好把人物推进更难的冲突层级。"
    elif normalized_focus == "reveal_mystery":
        payoff_point = "优先给一个有效答案，但同时暴露更关键的缺口或更大的反常。"
    elif normalized_focus == "relationship_shift":
        reader_reward = "读者要能明显看见关系不一样了，而不是只在心理旁白里说“其实变了”。"
    elif normalized_focus == "foreshadow_payoff":
        seed_point = "尽量指定哪条旧伏笔要回收，不要泛泛地说“注意前后呼应”。"

    stage_line = ""
    if normalized_stage == "development":
        stage_line = "发展阶段也要给小回收，让读者持续获得推进感，别把所有满足感都压后。"
    elif normalized_stage == "climax":
        stage_line = "高潮阶段优先回收最值钱的承诺和冲突，不要只继续预热更大的后面。"
        avoid_line = "不要在高潮里还只会继续铺垫和预告，却不给真正爆发与反馈。"
    elif normalized_stage == "ending":
        stage_line = "结局阶段优先回收主承诺、主关系和主谜面，再保留必要余波。"
        avoid_line = "不要在结局阶段把核心伏笔继续往后拖，削弱收束满足感。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认回收"
    lines = [f"【{scene_label}爽点回收卡】本轮请形成可感知的“铺垫→兑现→反馈”链条（{combo_text}）"]
    lines.append(f"- 预埋点：{seed_point}")
    lines.append(f"- 兑现点：{payoff_point}")
    lines.append(f"- 反馈链：{feedback_chain}")
    lines.append(f"- 读者回报：{reader_reward}")
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


def build_story_rule_grounding_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        rule_landing = "每轮至少让一个世界规则、能力边界、职业机制或组织秩序真正参与情节推进，而不是停在设定表。"
        trigger_condition = "规则先绑在具体触发条件上：谁做了什么、碰到了什么、违反了什么，规则才生效。"
        cost_limit = "每次规则介入尽量带出限制、代价、门槛或副作用，不给无代价万能设定。"
        scene_manifestation = "设定要通过事件结果、场景反馈、人物应对和局势变化显形，不靠大段讲解。"
        avoid_line = "不要让设定只在需要时突然出现救场，也不要讲完规则却看不到它如何改变局势。"
        scene_label = "大纲"
    else:
        rule_landing = "本章至少让一个规则/能力/机制在现场真正出手，而不是只被提到名字。"
        trigger_condition = "规则生效前先写清触发条件：谁做了什么、碰到了什么、付了什么，别让效果凭空发生。"
        cost_limit = "规则一旦介入，尽量附带代价、冷却、限制、风险或资源消耗。"
        scene_manifestation = "把设定表现落在动作、受阻、反馈、物理后果和人物应对上，不要只在旁白里解释。"
        avoid_line = "不要把规则写成临时外挂，也不要每次到关键处才想起补一段机制说明。"
        scene_label = "章节"

    if normalized_mode == "hook":
        trigger_condition = "钩子型规则尽量尽快触发，让异常、危险或麻烦先真实发生。"
    elif normalized_mode == "emotion":
        scene_manifestation = "设定效果最好压到人物情绪和关系余波上，让规则不是冷冰冰地“说明一下”。"
    elif normalized_mode == "suspense":
        rule_landing = "悬念型规则优先只显露最危险、最反常或最让人误判的一层。"
    elif normalized_mode == "relationship":
        scene_manifestation = "设定效果最好改写人与人之间的信任、合作权限或站队关系。"
    elif normalized_mode == "payoff":
        rule_landing = "优先回收前文提过的规则伏笔，让读者感到“原来之前那句设定现在真有用”。"

    if normalized_focus == "advance_plot":
        rule_landing = "优先让能推动主线前进的规则进场，别把篇幅浪费在旁枝设定展示。"
    elif normalized_focus == "deepen_character":
        scene_manifestation = "设定效果最好顺便暴露人物怎么理解规则、利用规则、畏惧规则或误判规则。"
    elif normalized_focus == "escalate_conflict":
        cost_limit = "冲突升级时，规则代价和限制要更咬人，不能只有强度没有代价。"
    elif normalized_focus == "reveal_mystery":
        trigger_condition = "优先让规则通过异常、误差、漏洞或例外来推进谜团。"
    elif normalized_focus == "relationship_shift":
        scene_manifestation = "设定效果最好改写人与人之间的信任、合作权限或站队关系。"
    elif normalized_focus == "foreshadow_payoff":
        rule_landing = "优先回收前文提过的规则伏笔，让读者感到“原来之前那句设定现在真有用”。"

    stage_line = ""
    if normalized_stage == "development":
        stage_line = "发展阶段先把最常用、最会咬人的规则边界立清楚，后面推进才有稳定抓手。"
    elif normalized_stage == "climax":
        stage_line = "高潮阶段让规则真正咬人或兑现，不要临近决战才重新解释一整套世界观。"
        avoid_line = "不要在高潮段落里突然停下来长讲机制说明，优先让规则直接在碰撞中显形。"
    elif normalized_stage == "ending":
        stage_line = "结局阶段优先回收最核心的规则承诺与代价，不要再抛全新体系。"
        avoid_line = "不要在结局阶段新增大块设定补丁，把收束重心冲散。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认设定落地"
    lines = [f"【{scene_label}设定落地卡】本轮请让规则与设定真正进场（{combo_text}）"]
    lines.append(f"- 规则着陆：{rule_landing}")
    lines.append(f"- 触发条件：{trigger_condition}")
    lines.append(f"- 代价/限制：{cost_limit}")
    lines.append(f"- 场景表现：{scene_manifestation}")
    if scene != "outline":
        lines.append("- 硬指标：至少完成一条“触发条件→规则生效→限制/代价→局势变化”的完整链，禁止只讲设定不让设定出手。")
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


def build_story_information_release_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        new_info = "每轮优先只放一层最必要的新信息：新规则、新背景、新动机里选最该知道的一层即可。"
        carrier = "优先通过动作结果、人物观察、关系碰撞和对白交换带出信息，不单列说明段。"
        explanation_limit = "解释只够读者跟上当前局势，不需要一次性讲完整个体系。"
        reader_handle = "复杂术语或新概念最好在三句内给一个人话抓手，让读者知道它对眼前事情意味着什么。"
        avoid_line = "不要把这一轮当设定百科补丁包，一口气倾倒多层背景。"
        scene_label = "大纲"
    else:
        new_info = "本章新信息尽量只命中一层：让读者明白当前最关键的规则、背景或动机即可。"
        carrier = "把信息拆进动作、观察、对白和即时后果里，尽量让读者边看事边懂事。"
        explanation_limit = "解释到能支撑当前冲突和理解即可，剩下的留给后续场景继续补。"
        reader_handle = "新词、新职业、新力量或新关系出现时，尽快补一句读者能立刻听懂的人话。"
        avoid_line = "不要在高潮动作中间突然插整段背景介绍，也不要连着三段都在解释。"
        scene_label = "章节"

    if normalized_mode == "hook":
        carrier = "先抓事件，再补信息；解释要贴着异常、危险或选择出现，别抢在钩子前面。"
    elif normalized_mode == "emotion":
        carrier = "信息最好从争执、试探、隐瞒、误解或安慰失败里漏出来，而不是平铺直叙。"
    elif normalized_mode == "suspense":
        new_info = "悬念型信息优先只揭半层：给可追踪的新线索，不把底牌一口气翻完。"
        explanation_limit = "解释要刚好够读者继续猜，不要把所有反常都立刻讲穿。"
    elif normalized_mode == "relationship":
        carrier = "信息最好挂在关系互动里，用谁敢说、谁不肯说、谁故意隐瞒来制造张力。"
    elif normalized_mode == "payoff":
        new_info = "优先释放与兑现直接相关的信息，让读者知道这次回收了什么、又打开了什么后效。"

    if normalized_focus == "advance_plot":
        new_info = "只放能推动主线前进的信息，和当前推进无关的设定先别急着补。"
    elif normalized_focus == "deepen_character":
        carrier = "信息最好通过人物选择、口误、回避和偏见露出来，而不是作者代说。"
    elif normalized_focus == "escalate_conflict":
        reader_handle = "让读者迅速明白这条信息为什么会让局势更糟、更难、更贵。"
    elif normalized_focus == "reveal_mystery":
        new_info = "优先放能推进谜团的一小块有效信息，而不是旁枝背景。"
        explanation_limit = "每次只多揭一层，不要直接把谜底和世界观补课一起打包端上来。"
    elif normalized_focus == "relationship_shift":
        carrier = "信息最好通过立场变化、试探问答、隐瞒失效或关系破口流出来。"
    elif normalized_focus == "foreshadow_payoff":
        new_info = "信息释放要服务于伏笔回收，让读者在“原来如此”和“接下来怎么办”之间获得连锁反馈。"

    stage_line = ""
    if normalized_stage == "development":
        stage_line = "发展阶段重点是把任务所需的最小信息量说清，别一开始就把整套世界全摊开。"
    elif normalized_stage == "climax":
        stage_line = "高潮阶段压缩说明比例，优先用已建立的信息打架，让新增解释只服务当下决断。"
        avoid_line = "不要在高潮关键碰撞前后连续长讲设定，把情绪和动作气口掐断。"
    elif normalized_stage == "ending":
        stage_line = "结局阶段优先投放回收性信息和结果性信息，不要突然补大量新设定。"
        avoid_line = "不要在结局处开启新的百科讲解，避免把收束拉回说明书。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认投放"
    lines = [f"【{scene_label}信息投放卡】本轮请控制信息释放方式与密度（{combo_text}）"]
    lines.append(f"- 本轮信息：{new_info}")
    lines.append(f"- 承载方式：{carrier}")
    lines.append(f"- 解释上限：{explanation_limit}")
    lines.append(f"- 读者抓手：{reader_handle}")
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


def build_story_emotion_landing_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        trigger_point = "每轮关键情绪先绑定一个具体触发：受伤、误会、失手、迟到的安慰、失去或看见了不该看见的东西。"
        outer_reaction = "情绪尽量落在动作停顿、生理反应、说话方式变化和选择偏移上，不只写抽象结论。"
        relationship_wave = "安排情绪在关系里留下余波：更靠近、更疏远、嘴硬、误伤、补偿失败或信任松动。"
        layered_shift = "同一轮情绪最好有层次变化，不要一上来就把人物情绪和主题判断全部说透。"
        avoid_line = "不要用“他很难过/她非常愤怒/他忽然明白了一切”直接代替现场表达。"
        scene_label = "大纲"
    else:
        trigger_point = "本章关键情绪先落在明确触发事件上，别让情绪像凭空冒出来。"
        outer_reaction = "优先写呼吸、停顿、动作错位、措辞变化、沉默和失控边缘，而不是直接给标签。"
        relationship_wave = "让情绪改变人与人之间的距离、说话方式、信任程度或之后的选择。"
        layered_shift = "情绪推进尽量分层：先忍、再裂、再回避/反击/崩掉，不要一步到顶。"
        avoid_line = "不要连续几句旁白都在盖章人物心情，也不要把复杂情绪一句话写死。"
        scene_label = "章节"

    if normalized_mode == "hook":
        trigger_point = "开场情绪最好直接绑定险情、麻烦或打断，让压力先压到人物身上。"
    elif normalized_mode == "emotion":
        outer_reaction = "情绪型段落更要靠停顿、改口、嘴硬、回避和细小动作发声，而不是抒情盖章。"
        layered_shift = "情绪最好出现误伤、自我压抑、短暂失控和余波回流的层次。"
    elif normalized_mode == "suspense":
        trigger_point = "悬念型情绪优先来自异常、误判、恐惧和答案缺口，而不是纯抒情。"
    elif normalized_mode == "relationship":
        relationship_wave = "关系戏里的情绪重点是靠近失败、信任松动、边界被碰、迟到的理解或不肯承认。"
    elif normalized_mode == "payoff":
        layered_shift = "兑现后的情绪别只停在爽或痛，要继续写余震、亏欠、松一口气后的空心或新责任。"

    if normalized_focus == "advance_plot":
        outer_reaction = "情绪反应之后最好立刻影响下一步行动，不让情绪段和主线脱节。"
    elif normalized_focus == "deepen_character":
        layered_shift = "人物塑形时优先写他怎么忍、怎么装、怎么解释自己，而不是作者替他总结性格。"
    elif normalized_focus == "escalate_conflict":
        relationship_wave = "冲突升级时让情绪带来误伤、顶撞、失控或撤回援手，而不是只提高音量。"
    elif normalized_focus == "reveal_mystery":
        trigger_point = "谜团推进时把情绪绑定到“看懂了一半”和“更不安了”这种认知落差上。"
    elif normalized_focus == "relationship_shift":
        relationship_wave = "关系变化重点写温差、试探落空、迟疑和态度微偏，不只写一句“关系变了”。"
    elif normalized_focus == "foreshadow_payoff":
        trigger_point = "伏笔兑现时优先写人物对旧承诺、旧创伤、旧误解被碰到时的即时反应。"

    stage_line = ""
    if normalized_stage == "development":
        stage_line = "发展阶段先把情绪触发与余波立住，让后续人物线有持续发酵空间。"
    elif normalized_stage == "climax":
        stage_line = "高潮阶段情绪要跟着碰撞一起爆，不要躲回长段抒情和解释。"
        avoid_line = "不要在高潮情绪点后立刻用旁白把人物全部解释完，冲掉现场余震。"
    elif normalized_stage == "ending":
        stage_line = "结局阶段的情绪更适合落在余波、代价、和解未尽或迟来的理解上。"
        avoid_line = "不要在结尾把所有情绪做成统一口号式总结，留一点人味和回声。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认情绪落点"
    lines = [f"【{scene_label}情绪落点卡】本轮请把情绪压回现场与关系里（{combo_text}）"]
    lines.append(f"- 触发点：{trigger_point}")
    lines.append(f"- 外显反应：{outer_reaction}")
    lines.append(f"- 关系余波：{relationship_wave}")
    lines.append(f"- 层次推进：{layered_shift}")
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


def build_story_action_rendering_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        action_start = "这一轮最值钱的桥段优先写成可视化动作：谁先动、怎么动、为什么现在动。"
        collision_feedback = "动作之后要有受阻、反击、误差、意外或被迫变招，别一键直达结果。"
        visible_change = "关键动作必须改变局面：位置变了、关系变了、危险级别变了、代价落下来了。"
        lens_priority = "需要现场化的节点优先给镜头，不要把最该看的桥段压成摘要。"
        avoid_line = "不要用“随后/很快/最终”一句话带过最关键的碰撞、破局或兑现。"
        scene_label = "大纲"
    else:
        action_start = "本章关键桥段先写动作发起：谁出手、谁试探、谁先失手、谁先顶上去。"
        collision_feedback = "动作里要有碰撞反馈：被挡住、打偏、误判、迟疑、反咬、变招或代价。"
        visible_change = "动作之后必须带来可见变化，不只报结果，要看见场面怎么被改写。"
        lens_priority = "最值钱的冲突、破局、兑现和危险临门尽量给现场镜头，不要躲去摘要句。"
        avoid_line = "不要把整场关键动作压成“他们打了一阵”“事情很快解决了”这种概述。"
        scene_label = "章节"

    if normalized_mode == "hook":
        action_start = "钩子段优先让动作先响，先让事情发生，再补解释。"
    elif normalized_mode == "emotion":
        collision_feedback = "情绪戏里的动作也要显形：推开、停住、没接住、想碰又收回，比抽象形容更有劲。"
    elif normalized_mode == "suspense":
        visible_change = "悬念型动作优先留下新反常、新危险或新证据，不要动作做完什么都没变。"
    elif normalized_mode == "relationship":
        action_start = "关系戏里的关键动作可以是靠近、退开、挡住、递回去、没接、转身或越界。"
    elif normalized_mode == "payoff":
        lens_priority = "兑现型桥段更要现场化，把最值钱的那一下真正写在台前。"

    if normalized_focus == "advance_plot":
        visible_change = "动作结束后主线最好明确前进一格，而不是热闹完还在原地。"
    elif normalized_focus == "deepen_character":
        collision_feedback = "动作反馈要顺手照出人物习惯、底线、软肋和犹豫，不只看热闹。"
    elif normalized_focus == "escalate_conflict":
        action_start = "冲突升级时优先写更难的现场碰撞，不靠旁白宣布“局势更严重了”。"
    elif normalized_focus == "reveal_mystery":
        visible_change = "动作之后最好掉出线索、破绽、证据或更大的缺口。"
    elif normalized_focus == "relationship_shift":
        collision_feedback = "关系变化尽量通过动作错位、接与不接、站位变化和边界碰撞来显形。"
    elif normalized_focus == "foreshadow_payoff":
        lens_priority = "伏笔兑现时优先写兑现发生的那一刻，不要只在事后回顾“原来如此”。"

    stage_line = ""
    if normalized_stage == "development":
        stage_line = "发展阶段先把关键动作链写清，别让中段长期停在说明和准备态。"
    elif normalized_stage == "climax":
        stage_line = "高潮阶段的动作要更现场、更具体、更有反馈，不要只剩结果播报。"
        avoid_line = "不要在高潮关键桥段里大量省略动作过程，让最该爆的地方直接哑火。"
    elif normalized_stage == "ending":
        stage_line = "结局阶段优先现场化最重要的兑现、告别、冲突终局和代价落地。"
        avoid_line = "不要在收尾阶段把关键回收全写成叙述总结，削弱满足感。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认动作显影"
    lines = [f"【{scene_label}动作显影卡】本轮请把关键桥段写成可见动作链（{combo_text}）"]
    lines.append(f"- 起手动作：{action_start}")
    lines.append(f"- 碰撞反馈：{collision_feedback}")
    lines.append(f"- 局面变化：{visible_change}")
    lines.append(f"- 镜头优先：{lens_priority}")
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


def build_story_summary_tone_control_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        conclusion_hold = "主题、人物判断和关系结论尽量少直接盖章，优先让事件走向和余波自己说话。"
        replacement_path = "如果需要表达判断，优先用动作、对白、物件、站位变化和后果来替代总结句。"
        blank_space = "关键段落保留一点留白，让读者自己接上“原来是这样”，不要每次都替读者总结。"
        sentence_control = "压缩“他知道/她明白/这意味着/从此以后/命运注定”这类盖章句频率。"
        avoid_line = "不要把每个转折都写成作者点评，更不要在段尾连发金句式结论。"
        scene_label = "大纲"
    else:
        conclusion_hold = "本章少直接宣布人物心境、关系定性和主题意义，优先把判断埋进现场。"
        replacement_path = "该写结论时，尽量换成动作停顿、没说出口的话、被看见的物件和局面变化。"
        blank_space = "给读者留一点自己体会的空间，不要刚发生完就立刻替他总结感受。"
        sentence_control = "少用抽象总结句和命运句，尤其别用旁白把人物成长、爱情或主题一次性说穿。"
        avoid_line = "不要连续用“他终于明白”“她忽然懂得”“这意味着一切都变了”收段。"
        scene_label = "章节"

    if normalized_mode == "hook":
        conclusion_hold = "钩子段更要少总结，优先把问题留在事件和动作上。"
    elif normalized_mode == "emotion":
        replacement_path = "情绪结论尽量改成呼吸、目光、错开的动作、答非所问和沉默。"
        blank_space = "情绪戏别刚掀起就旁白总结，给余波一点扩散空间。"
    elif normalized_mode == "suspense":
        sentence_control = "悬念段更要克制解释性总结，别一边卖疑一边把答案和意义都旁白清楚。"
    elif normalized_mode == "relationship":
        replacement_path = "关系变化尽量通过称呼、距离、口气、是否接话和是否站到一起表现，不靠盖章。"
    elif normalized_mode == "payoff":
        conclusion_hold = "兑现后少讲大道理，优先让反馈和代价证明这次回收值不值。"

    if normalized_focus == "advance_plot":
        replacement_path = "主线推进时用“发生了什么变化”代替“这意味着什么”，让局势自己发声。"
    elif normalized_focus == "deepen_character":
        blank_space = "人物塑形时少替人物写人物小结，保留一些矛盾和自欺让读者自己品。"
    elif normalized_focus == "escalate_conflict":
        sentence_control = "冲突升级时少复盘和评点，让更贵的动作和后果承担说服力。"
    elif normalized_focus == "reveal_mystery":
        conclusion_hold = "揭谜时只给必要答案，不顺手把主题点评和全部意义打包讲完。"
    elif normalized_focus == "relationship_shift":
        replacement_path = "关系变化更适合落在没接住的话、退后的半步、迟疑和让步上，而不是口头定性。"
    elif normalized_focus == "foreshadow_payoff":
        blank_space = "回收伏笔时让“原来如此”的快感由前后呼应产生，不用旁白替读者喊出来。"

    stage_line = ""
    if normalized_stage == "development":
        stage_line = "发展阶段先克制解释欲，让读者跟着事件自己建立判断。"
    elif normalized_stage == "climax":
        stage_line = "高潮阶段尤其要少讲道理，让碰撞、代价和沉默承担重量。"
        avoid_line = "不要在高潮关键段突然插长句评语，把现场冲击改写成作者感悟。"
    elif normalized_stage == "ending":
        stage_line = "结局阶段允许有余味，但不等于大段讲主题总结，优先让结尾意象和余波说话。"
        avoid_line = "不要在收尾用旁白把所有主题、成长和命运一次性解释完。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认抑制"
    lines = [f"【{scene_label}总结腔抑制卡】本轮请减少作者盖章式结论（{combo_text}）"]
    lines.append(f"- 结论克制：{conclusion_hold}")
    lines.append(f"- 替代表现：{replacement_path}")
    lines.append(f"- 留白位置：{blank_space}")
    lines.append(f"- 句式控制：{sentence_control}")
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


def build_story_repetition_control_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        repeat_target = "同一轮里同一信息、情绪判断、人物动机和风险提醒尽量只命中一次，不连续换说法重讲。"
        first_hit = "第一次命中时尽量写到位：要么最清楚、要么最有劲，让后面不必重复提醒。"
        later_handle = "后续再提时优先推进新变化、新后果或新角度，不重复旧结论本身。"
        merge_rule = "相邻场景若承担同一功能，优先并掉弱的那次表达，把篇幅留给新推进。"
        avoid_line = "不要把同一个担心、同一个设定、同一个情绪在三段里换着词反复说。"
        scene_label = "大纲"
    else:
        repeat_target = "本章同一信息、情绪、设定提醒和人物判断尽量只打一次重击，别连着复述。"
        first_hit = "第一次出现时尽量让它足够清晰、足够具体，后面就用动作和后果承接。"
        later_handle = "后续若必须再提，最好带出升级、反转、误差或代价，不只原话重来。"
        merge_rule = "相邻段若在做同一件事，优先删掉弱重复，保留最有效的一次表达。"
        avoid_line = "不要前一段刚说完人物害怕、设定危险或任务困难，后一段马上换说法再提醒一遍。"
        scene_label = "章节"

    if normalized_mode == "hook":
        first_hit = "钩子信息第一次出现就要够尖，别靠反复提醒硬撑抓力。"
    elif normalized_mode == "emotion":
        repeat_target = "情绪不要连着用近义词重复盖章，优先让余波和动作替情绪继续发声。"
    elif normalized_mode == "suspense":
        later_handle = "悬念再提时要带新反常或新缺口，别只是重复“事情不对劲”。"
    elif normalized_mode == "relationship":
        merge_rule = "关系拉扯不要连续两三轮都在说同一种疏离或暧昧，要让关系位置真的变。"
    elif normalized_mode == "payoff":
        first_hit = "回收点第一次兑现时就把满足感打满，别后面再靠解释重复证明它很重要。"

    if normalized_focus == "advance_plot":
        later_handle = "主线推进时，重复提旧问题不如让问题进入新阶段。"
    elif normalized_focus == "deepen_character":
        repeat_target = "人物塑形别反复旁白同一性格标签，优先换成不同场景下的新选择来证明。"
    elif normalized_focus == "escalate_conflict":
        later_handle = "冲突升级时要给更高代价和新碰撞，不要只反复提醒“矛盾很激烈”。"
    elif normalized_focus == "reveal_mystery":
        merge_rule = "谜团提示要层层推进，不重复播报同一团迷雾。"
    elif normalized_focus == "relationship_shift":
        later_handle = "关系变化再提时要让说话方式、站位或行动条件变化，而不是重说“他们变了”。"
    elif normalized_focus == "foreshadow_payoff":
        first_hit = "伏笔第一次埋下就尽量精准，后面少反复提醒存在感。"

    stage_line = ""
    if normalized_stage == "development":
        stage_line = "发展阶段尤其容易水在重复提醒里，要尽快把同类信息压缩成一次有效命中。"
    elif normalized_stage == "climax":
        stage_line = "高潮阶段少复盘、少重复解释，让碰撞和后果接管篇幅。"
        avoid_line = "不要在高潮段落连续复述同一危险、同一情绪和同一动机，削弱冲击。"
    elif normalized_stage == "ending":
        stage_line = "结局阶段优先用结果和余波说话，不要反复回顾已经兑现的东西。"
        avoid_line = "不要在收尾用多段重复总结同一主题和同一成长，拖慢收束。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认压缩"
    lines = [f"【{scene_label}重复压缩卡】本轮请减少同义复述与连续提醒（{combo_text}）"]
    lines.append(f"- 重复对象：{repeat_target}")
    lines.append(f"- 首次命中：{first_hit}")
    lines.append(f"- 后续处理：{later_handle}")
    lines.append(f"- 删并原则：{merge_rule}")
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


def build_story_viewpoint_discipline_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        camera_focus = "这一轮默认贴住一个主镜头人物推进，除非明确设计，否则不要在同一关键段里随意钻入多人内心。"
        visible_boundary = "只写主镜头此刻能看见、听见、推断到的内容，未知就保留未知。"
        inner_access = "内心优先给当前主视角人物，其他人物更多通过动作、失言、停顿和选择显形。"
        switch_rule = "如果必须切视角，最好让章节分隔、场景切换或明确标识承担切换。"
        avoid_line = "不要用作者口吻替所有角色下判断，也不要一句话把每个人真实心思都说穿。"
        scene_label = "大纲"
    else:
        camera_focus = "本章关键场景尽量贴住一个主视角，让读者跟着同一双眼睛承受信息差和压力。"
        visible_boundary = "当前人物不知道的东西，尽量不要直接盖章给读者，先通过异常、动作和线索侧写。"
        inner_access = "内心戏优先写主视角人物的当下反应，不要一句话顺手把周围所有人都看透。"
        switch_rule = "要切视角时，尽量借章节断点、明确场景跳转或强需求切换，不在紧张现场横跳。"
        avoid_line = "不要上一句还在甲的脑子里，下一句就跳进乙的内心，再下一句作者来总结真相。"
        scene_label = "章节"

    if normalized_mode == "hook":
        camera_focus = "钩子段尽量贴住最先承受异常、危险或任务压力的人，让抓力更直接。"
    elif normalized_mode == "emotion":
        inner_access = "情绪型段落优先写体感、误读、嘴硬和停顿，不要全靠作者替人物命名情绪。"
    elif normalized_mode == "suspense":
        visible_boundary = "悬念型段落更要守住可见边界，不要为了省事提前透出标准答案。"
        avoid_line = "不要一边让人物发懵，一边又让旁白抢先把谜底和真意解释完。"
    elif normalized_mode == "relationship":
        inner_access = "关系戏里更适合通过对视、回避、打断和措辞变化显露双方状态，而不是双向内心旁白轮流讲解。"
    elif normalized_mode == "payoff":
        camera_focus = "兑现瞬间尽量贴住最能感到“终于到了”的人物，让回报更有代入感。"

    if normalized_focus == "advance_plot":
        camera_focus = "优先跟随最能推动主线下一步的人物视角，少切去旁支人物分散推进。"
    elif normalized_focus == "deepen_character":
        inner_access = "聚焦人物做选择时的偏见、软肋和自我辩解，不用全知口吻替他写人物小传。"
    elif normalized_focus == "escalate_conflict":
        visible_boundary = "冲突升级时更要守住局中人视角，让错误判断和迟来的发现保留张力。"
    elif normalized_focus == "reveal_mystery":
        switch_rule = "如需切视角揭新线索，必须让切换本身带来新证据，而不是单纯替作者补课。"
    elif normalized_focus == "relationship_shift":
        inner_access = "关系变化优先让读者从主视角的误判、迟疑、试探和受伤里感到变化。"
    elif normalized_focus == "foreshadow_payoff":
        camera_focus = "回收伏笔时尽量站在最受那条伏笔影响的人物身上，让兑现更有分量。"

    stage_line = ""
    if normalized_stage == "development":
        stage_line = "发展阶段先把主镜头稳定住，让读者知道该跟谁看、跟谁担心。"
    elif normalized_stage == "climax":
        stage_line = "高潮阶段更要贴住最疼、最险、最难选的那个视角，少横跳、少俯视。"
        avoid_line = "不要在高潮现场频繁切镜头解释全局，导致碰撞被切碎、情绪被稀释。"
    elif normalized_stage == "ending":
        stage_line = "结局阶段的视角切换应服务收束与余味，不要为了补信息乱开上帝视角。"
        avoid_line = "不要在结尾靠作者总结式全知旁白把人物命运一次性说教完。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认视角"
    lines = [f"【{scene_label}视角纪律卡】本轮请稳定镜头与信息边界（{combo_text}）"]
    lines.append(f"- 主镜头：{camera_focus}")
    lines.append(f"- 可见边界：{visible_boundary}")
    lines.append(f"- 内心准入：{inner_access}")
    lines.append(f"- 切换条件：{switch_rule}")
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


def build_story_dialogue_advancement_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        dialogue_task = "本轮关键对白至少承担一个明确任务：试探、施压、谈判、套话、摊牌或关系重排，别只做信息搬运。"
        information_gap = "想清谁知道得更多、谁在隐瞒、谁在误解，让对白自带信息差。"
        voice_split = "不同角色的句长、措辞、回避方式和情绪爆点要能分开，不要所有人轮流讲道理。"
        action_support = "对白最好配合停顿、动作、眼神、打断或环境反馈，让潜台词落地。"
        avoid_line = "不要让整段对白变成背景说明会，也不要每个人都说得完整、正确、体面。"
        scene_label = "大纲"
    else:
        dialogue_task = "本章关键对白要推动局势、关系或选择，不要只是把读者已经知道的信息再说一遍。"
        information_gap = "对白里要有信息差：有人在试探、有人在藏、有人没听懂、有人故意说半句。"
        voice_split = "角色说话方式要分得开：句长、词汇、礼貌度、火气、停顿和潜台词都别一样。"
        action_support = "对白之间穿插动作、表情、环境反应和沉默，让说出口和没说出口的东西一起工作。"
        avoid_line = "不要一轮对白全是完整长句和总结句，也不要让角色轮流替作者解释世界观。"
        scene_label = "章节"

    if normalized_mode == "hook":
        dialogue_task = "对白最好一开口就带压力、问题或威胁，让读者立刻感觉有事要炸。"
    elif normalized_mode == "emotion":
        information_gap = "情绪型对白重点不在“说清楚”，而在谁嘴硬、谁避重就轻、谁说了反话。"
        action_support = "动作陪跑优先写停顿、改口、没接住的安慰和说完后的余震。"
    elif normalized_mode == "suspense":
        information_gap = "悬念型对白要保留缺口：一句话只揭半层，最好带出新疑点或相互矛盾。"
    elif normalized_mode == "relationship":
        dialogue_task = "对白要承担站位试探、边界确认或关系升降温，别只是客观交流信息。"
        voice_split = "关系越近越敢打断、绕弯、戳痛点；关系越远越讲分寸、试探和保留。"
    elif normalized_mode == "payoff":
        dialogue_task = "兑现型对白要让人物对结果作出反应：承认、嘴硬、错愕、反咬或迟来的理解。"

    if normalized_focus == "advance_plot":
        dialogue_task = "对白结束后应推动行动计划、立场判断或主线下一步，而不是原地聊完。"
    elif normalized_focus == "deepen_character":
        voice_split = "对白重点是把人物软肋、执念、教养和惯性露出来，不是统一输出正确答案。"
    elif normalized_focus == "escalate_conflict":
        information_gap = "冲突型对白要让误解更深、底牌更露或退路更少，别聊完反而泄压。"
    elif normalized_focus == "reveal_mystery":
        dialogue_task = "对白里优先放试探、交叉验证和半真半假的线索，不要直接口述谜底。"
    elif normalized_focus == "relationship_shift":
        action_support = "对话结束后最好能看见站位变化、沉默拉长、目光回避或合作条件改变。"
    elif normalized_focus == "foreshadow_payoff":
        dialogue_task = "对白可以顺手回收旧台词、旧承诺或旧误会，让熟悉信息产生新含义。"

    stage_line = ""
    if normalized_stage == "development":
        stage_line = "发展阶段的对白重点是尽快立清关系、任务和信息差，让后续冲突有抓手。"
    elif normalized_stage == "climax":
        stage_line = "高潮阶段对白要短、狠、准，优先服务摊牌、碰撞和底线暴露。"
        avoid_line = "不要在高潮对白里长篇复盘前情或讲大道理，把碰撞气口拖死。"
    elif normalized_stage == "ending":
        stage_line = "结局阶段对白更适合落在承认、告别、没说完的余味或代价后的新关系。"
        avoid_line = "不要在结局里靠大段解释把所有情绪说穿，留一点人味和余波。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认对白推进"
    lines = [f"【{scene_label}对白推进卡】本轮请让关键对白真正推动故事（{combo_text}）"]
    lines.append(f"- 对话任务：{dialogue_task}")
    lines.append(f"- 信息落差：{information_gap}")
    lines.append(f"- 声线区分：{voice_split}")
    lines.append(f"- 动作陪跑：{action_support}")
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


def build_story_repetition_risk_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        opening_risk = "不要每轮前段都只做设定铺陈，读者会感觉整轮大纲在原地起步。"
        pressure_risk = "不要每章都用同一级别阻力灌水，中段会失去递进感。"
        pivot_risk = "不要把每次转折都写成临时加设定或生硬插入新人物。"
        closing_risk = "不要每轮都只用“下回更精彩”式尾章，下一轮任务必须具体。"
        scene_label = "大纲"
    else:
        opening_risk = "不要反复用回忆、说明或同一种异常开场，容易让章节起手发闷。"
        pressure_risk = "不要把受阻写成同一种争吵、误会或嘴上发狠，压力会显得空。"
        pivot_risk = "不要把转折写成假反转、硬转念或只靠旁白解释。"
        closing_risk = "不要每章都用同一种问句、敲门声或电话铃收尾，钩子会疲劳。"
        scene_label = "章节"

    if normalized_mode == "hook":
        opening_risk = "钩子模式下不要每次都靠突发危险硬拽开场，异常类型需要变化。"
        closing_risk = "不要连续多章都用悬空危险硬切章尾，读者会识别套路。"
    elif normalized_mode == "emotion":
        pressure_risk = "不要反复靠争吵、沉默或内心独白制造情绪，否则张力会钝化。"
        pivot_risk = "不要把情绪转折写成突然想通，缺少事件触发会显得虚。"
    elif normalized_mode == "suspense":
        opening_risk = "悬念模式下不要只会丢疑点不交代有效信息，否则会像故意遮掩。"
        pivot_risk = "不要连续用“其实另有隐情”做反转，真相推进需要层次。"
        closing_risk = "不要只留空白疑问而不给新证据，悬念会变成拖延。"
    elif normalized_mode == "relationship":
        pressure_risk = "不要把关系推进写成重复拉扯却没有立场后果，读者会觉得没变化。"
        pivot_risk = "不要每次都靠误会触发关系变化，站队和选择也要轮换。"
    elif normalized_mode == "payoff":
        opening_risk = "回收模式下不要一上来就罗列旧伏笔目录，读者需要事件化兑现。"
        closing_risk = "不要每次回收完都再塞一个更大的谜团，容易冲淡回报感。"

    if normalized_focus == "advance_plot":
        pressure_risk = "主线推进不要只做位移和赶路，缺少阻力变化会像流水账。"
    elif normalized_focus == "deepen_character":
        opening_risk = "人物塑形不要总从心理描写起手，最好让性格先在动作里显形。"
        pressure_risk = "不要把成长写成同一种自责或回忆，人物弧线会发虚。"
    elif normalized_focus == "escalate_conflict":
        pressure_risk = "冲突升级不要一直放大音量不抬高代价，否则只是吵得更大声。"
        pivot_risk = "不要把冲突转折只写成新敌人登场，最好让旧矛盾也发生质变。"
    elif normalized_focus == "reveal_mystery":
        pivot_risk = "谜团揭示不要总靠旁人解释，证据和事件本身也要承担揭示功能。"
        closing_risk = "不要连续多次只留下谜面不回收谜底，读者会怀疑作者在拖。"
    elif normalized_focus == "relationship_shift":
        pressure_risk = "关系转折不要只换台词腔调，最好同步改变合作方式和站位。"
    elif normalized_focus == "foreshadow_payoff":
        closing_risk = "伏笔回收不要每次都变成新伏笔发射器，需保留真正落地的满足。"

    if normalized_stage == "development":
        opening_risk = "发展阶段不要长时间停在铺垫准备态，必须尽快把变量推上桌。"
        closing_risk = "发展阶段不要每章都只留一个模糊目标，任务应逐步具体化。"
    elif normalized_stage == "climax":
        pressure_risk = "高潮阶段不要反复假装要碰撞却不断拖开，读者会明显感到泄劲。"
        pivot_risk = "高潮阶段不要只有大声量和快节奏，没有决定性变化就不算高潮。"
    elif normalized_stage == "ending":
        opening_risk = "结局阶段不要又重新搭新盘子，优先收最重要的旧承诺。"
        closing_risk = "结局阶段不要为了续作感强行再开主线，否则会稀释收束力度。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认避重"
    lines = [f"【{scene_label}重复风险卡】本轮需主动规避以下高频套路（{combo_text}）"]
    lines.append(f"- 开场风险：{opening_risk}")
    lines.append(f"- 加压风险：{pressure_risk}")
    lines.append(f"- 转折风险：{pivot_risk}")
    lines.append(f"- 收尾风险：{closing_risk}")
    return _compact_prompt_text("\n".join(lines))


def build_story_acceptance_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        mission_check = "验收时先看这轮章节是否承担了明确主任务，而不是平均摊功能。"
        change_check = "至少要看到局势、关系或认知层面的阶段性变化，不能只搭台。"
        freshness_check = "检查本轮关键章法是否和上一轮过度同构，避免整卷节拍重复。"
        closing_check = "尾段既要交代阶段结果，也要给下一轮留下具体任务。"
        scene_label = "大纲"
    else:
        mission_check = "验收时先看本章是否完成了一个清晰主任务，而不是热闹但空转。"
        change_check = "至少要看到局势、关系或认知有一项明确变化，不能原地踏步。"
        freshness_check = "检查开场、加压、转折、收尾是否又落回同一种旧套路。"
        closing_check = "章尾既要完成本章收束，也要留下合适的追读牵引或余味。"
        scene_label = "章节"

    if normalized_mode == "hook":
        mission_check = "验收时重点看开场和章尾是否真正形成牵引，而不只是制造噪音。"
        closing_check = "结尾要让读者有继续读的冲动，但不能只有硬切和悬空。"
    elif normalized_mode == "emotion":
        change_check = "验收时要看到情绪余震和关系后果，而不是只有一段抒情。"
        freshness_check = "检查情绪推进是否又只是争吵、沉默或内心独白轮换。"
    elif normalized_mode == "suspense":
        change_check = "验收时至少要有一个有效线索、认知刷新或危险升级真正落地。"
        closing_check = "结尾要留下更尖锐的问题，但不能完全不给有效信息。"
    elif normalized_mode == "relationship":
        mission_check = "验收时看人物关系是否真的发生位移，而不是只多说了几句狠话。"
        change_check = "关系变化最好能改动人物之后的站位、合作或信任条件。"
    elif normalized_mode == "payoff":
        mission_check = "验收时要确认前文铺垫是否真正兑现，而不是只口头提到。"
        closing_check = "兑现之后要有后效和新失衡，不能只停在一次性爽点。"

    if normalized_focus == "advance_plot":
        mission_check = "验收时先看主线是否实打实前进，而不是忙了很多事却没推局势。"
    elif normalized_focus == "deepen_character":
        change_check = "验收时看人物是否在选择里显形，而不是只补充背景说明。"
        freshness_check = "检查人物塑形是否又回到同一种回忆、自责或旁白总结。"
    elif normalized_focus == "escalate_conflict":
        change_check = "验收时要能看见代价升级、对立加深或冲突进入新层级。"
        closing_check = "本轮结束后人物应被留在更难的位置，而不是轻松退回安全区。"
    elif normalized_focus == "reveal_mystery":
        mission_check = "验收时必须确认谜团有真实推进，而不是只多堆了一层雾。"
    elif normalized_focus == "relationship_shift":
        change_check = "验收时看关系是否足以改变说话方式、行动选择或站队逻辑。"
    elif normalized_focus == "foreshadow_payoff":
        mission_check = "验收时确认伏笔是否兑现落地，同时打开了新的后续空间。"

    if normalized_stage == "development":
        mission_check = "发展阶段验收重点是：有没有把局势、变量和主任务真正搭起来。"
        closing_check = "收尾应让下一轮任务更具体，而不是继续停留在准备态。"
    elif normalized_stage == "climax":
        change_check = "高潮阶段验收重点是：有没有形成决定性碰撞、底牌掀开或局势断裂。"
        freshness_check = "检查高潮是否只是声量更大，还是确实发生了不可逆变化。"
    elif normalized_stage == "ending":
        mission_check = "结局阶段验收重点是：主承诺、主悬念和关键关系线是否得到有效回收。"
        closing_check = "收尾应保留余味，但不能为了留白再次打散已经完成的收束。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认验收"
    lines = [f"【{scene_label}验收卡】成稿前请用以下标准验收本轮是否真正达标（{combo_text}）"]
    lines.append(f"- 任务命中：{mission_check}")
    lines.append(f"- 变化落地：{change_check}")
    lines.append(f"- 新鲜度：{freshness_check}")
    lines.append(f"- 收束质量：{closing_check}")
    return _compact_prompt_text("\n".join(lines))


def build_story_opening_hook_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        first_strike = "卷首前几章要尽快抛出异常、险情、失衡或难以回避的任务，让读者立刻知道这卷为什么值得追。"
        trouble_seed = "开篇尽量同步埋下一个会持续发酵的麻烦种子，后文要能不断翻面或加压。"
        unresolved_question = "首轮要留下一个具体未决问题，最好与人物选择、关系走向或危险来源直接绑定。"
        avoid_line = "不要先用大段设定、背景回顾或气氛铺陈占满开头，再迟迟不进入真正问题。"
        scene_label = "大纲"
    else:
        first_strike = "开篇前几段尽快给出异常、险情、冲突或打断日常的事件，不要慢热兜圈。"
        trouble_seed = "第一轮动作里要埋下会继续追着人物跑的麻烦种子，而不是一次性小插曲。"
        unresolved_question = "开场后尽快形成一个具体未决问题，让读者明确想知道下一步会发生什么。"
        avoid_line = "不要用天气、环境、回忆或泛情绪独白拖长预热，却迟迟没有真正抓手。"
        scene_label = "章节"

    if normalized_mode == "hook":
        first_strike = "第一击优先落在异常、险情、失衡或强制选择上，先抓住人再补信息。"
        unresolved_question = "未决问题最好带明确倒计时、后果或风险，而不是空泛地卖关子。"
    elif normalized_mode == "emotion":
        trouble_seed = "麻烦种子最好和关系裂缝、误伤余震或压抑失败绑定，让情绪从开头就带刺。"
        avoid_line = "不要只写情绪氛围和内心感受，却没有触发情绪的外部事件。"
    elif normalized_mode == "suspense":
        first_strike = "第一击优先给出异常迹象、线索反常、危险逼近或认知落差。"
        unresolved_question = "未决问题应当具体到谁在做什么、哪里不对、真相缺了哪一块。"
    elif normalized_mode == "relationship":
        trouble_seed = "麻烦种子最好是站位变化、信任裂缝、关系失衡或合作条件改变。"
        unresolved_question = "开头要让读者关心这段关系接下来会靠近、决裂还是暂时停摆。"
    elif normalized_mode == "payoff":
        first_strike = "第一击可以直接掀开旧承诺开始兑现，或让旧伏笔先产生回响和副作用。"
        trouble_seed = "兑现之后要立刻带出新的失衡、代价或连锁反应，不要只给一个爽点就停。"

    if normalized_focus == "advance_plot":
        first_strike = "开场动作要直接推动主线，不要热闹很多却没有实际推进。"
    elif normalized_focus == "deepen_character":
        trouble_seed = "麻烦种子最好能逼出人物软肋、执念或底线，而不是只补背景设定。"
    elif normalized_focus == "escalate_conflict":
        first_strike = "第一击最好就是一次对立碰撞、局势加压或安全区失效。"
        unresolved_question = "未决问题要落在冲突会升级到什么程度、谁先扛不住、谁会失手上。"
    elif normalized_focus == "reveal_mystery":
        first_strike = "开头尽快抛出异常证据、反常细节或新线索，不要把谜团完全藏在后半段。"
    elif normalized_focus == "relationship_shift":
        trouble_seed = "麻烦种子最好让关系一开始就处在新的拉扯位置，而不是老样子慢慢磨。"
    elif normalized_focus == "foreshadow_payoff":
        first_strike = "开场可以先响一下旧伏笔，让读者迅速意识到这次不是无关紧要的新事件。"

    stage_line = ""
    if normalized_stage == "development":
        stage_line = "发展阶段的开篇重点是尽快把本轮主任务、变量和压力源摆上桌，别一直停在准备态。"
    elif normalized_stage == "climax":
        stage_line = "高潮阶段的开篇要延续既有高压，不要重新慢启动或重新铺盘子。"
        avoid_line = "不要在高潮章/卷开头突然切回长铺垫、慢解释或轻松日常，导致气压掉线。"
    elif normalized_stage == "ending":
        stage_line = "结局阶段的开篇优先抓回核心承诺、关键关系或最后代价，不要另起大盘。"
        avoid_line = "不要在结局阶段开头又抛全新主线，把读者注意力从收束目标上拉开。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认抓力"
    lines = [f"【{scene_label}开篇抓力卡】开场请尽快建立抓手与牵引（{combo_text}）"]
    lines.append(f"- 第一击：{first_strike}")
    lines.append(f"- 麻烦种子：{trouble_seed}")
    lines.append(f"- 未决问题：{unresolved_question}")
    if scene != "outline":
        lines.append("- 硬指标：开篇前 20%-25% 内至少落地 1 个抓手（目标 / 异常 / 受阻 / 强制选择），且不能连续两段只做背景预热。")
        lines.append("- 二级硬指标：最好前 120-180 字内同时出现两类抓手（异常 / 任务 / 受阻 / 倒计时 / 强制选择 / 对立问句），并让第一轮动作立刻制造余波。")
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


def build_story_character_arc_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        external_line = "这一轮至少要让核心人物的外在线任务更明确，不只推动事件壳子。"
        internal_line = "安排一次会暴露人物执念、伤口或价值判断的压力测试。"
        relationship_line = "让关键关系在信任、站队或依赖上出现可见变化。"
        arc_landing = "尾段给出人物阶段性变化，让下一轮成长方向更清晰。"
        scene_label = "大纲"
    else:
        external_line = "本章要让人物在外在线上做出能看见后果的动作，而不是被剧情拖着走。"
        internal_line = "本章要逼出一次能暴露人物软肋、执念或底线的反应。"
        relationship_line = "至少让一条关系线发生可见位移，而不只是多说几句情绪台词。"
        arc_landing = "章尾要留下人物状态的新落点，让后续成长有承接。"
        scene_label = "章节"

    if normalized_mode == "hook":
        external_line = "人物外在线最好和迫近危险、未决选择或新任务直接绑定，让他不得不动。"
        arc_landing = "弧光落点要落在人物被推入新处境上，而不只是事件悬空。"
    elif normalized_mode == "emotion":
        internal_line = "内在线重点看人物如何被情绪反噬、误伤他人或压抑失败。"
        relationship_line = "关系线最好呈现安慰失败、靠近受阻或误伤后的余震。"
    elif normalized_mode == "suspense":
        external_line = "人物外在线尽量和追查、判断、求生或拆解异常绑定。"
        internal_line = "通过误判、恐惧和认知落差暴露人物真正的盲区与偏执。"
    elif normalized_mode == "relationship":
        relationship_line = "关系线必须承担主推进，最好出现站队变化、信任重排或亲疏重估。"
        arc_landing = "落点应让人物在关系位置上进入一个再也回不到原点的新阶段。"
    elif normalized_mode == "payoff":
        external_line = "人物外在线要和旧承诺兑现、旧目标回收或能力回报直接挂钩。"
        arc_landing = "落点要让人物因为兑现获得成长回报，或承担兑现带来的新责任。"

    if normalized_focus == "advance_plot":
        external_line = "人物外在线必须和主线推进同频，行动要真的改变局势而非走流程。"
    elif normalized_focus == "deepen_character":
        internal_line = "内在线要让人物在选择里显形，看见他的软肋、执念和价值判断。"
        arc_landing = "落点最好形成一次人物自我认知偏移，而不只是事件结束。"
    elif normalized_focus == "escalate_conflict":
        internal_line = "冲突升级时要逼出人物底线，看看他在更高代价下会怎么变。"
        relationship_line = "更强冲突最好同步改写人物之间的站位与依赖结构。"
    elif normalized_focus == "reveal_mystery":
        external_line = "人物外在线最好围绕调查、判断和选择展开，而不是旁观真相自己掉下来。"
        internal_line = "认知刷新应反照人物偏见、恐惧或执念，而不是只补世界观信息。"
    elif normalized_focus == "relationship_shift":
        relationship_line = "关系线验收重点是：人物之后的说话方式、站位和合作条件是否真的变了。"
    elif normalized_focus == "foreshadow_payoff":
        arc_landing = "人物应因为伏笔兑现进入新的自我认知、责任位置或情感阶段。"

    if normalized_stage == "development":
        external_line = (
            "发展阶段先让人物想要什么、怕什么、要付什么代价变得清楚。"
            if scene == "outline"
            else "发展阶段先把人物眼前要争什么、躲什么、赌什么摆清楚。"
        )
        arc_landing = "落点应把人物推入更难但更清晰的成长压力链。"
    elif normalized_stage == "climax":
        internal_line = "高潮阶段要逼出人物真正底线、真实选择或最不愿面对的自我。"
        relationship_line = "高潮中的关系变化最好是定向性变化，而不是小幅试探。"
    elif normalized_stage == "ending":
        relationship_line = "结局阶段要让关键关系线出现收束、定局或带余温的最终位移。"
        arc_landing = "落点要给人物阶段性定局、余味或代价后的新平衡。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认弧光"
    lines = [f"【{scene_label}角色弧光卡】本轮至少让人物弧光出现以下推进（{combo_text}）"]
    lines.append(f"- 外在线：{external_line}")
    lines.append(f"- 内在线：{internal_line}")
    lines.append(f"- 关系线：{relationship_line}")
    lines.append(f"- 落点：{arc_landing}")
    return _compact_prompt_text("\n".join(lines))


def build_story_cliffhanger_card_block(
    creative_mode: Optional[str],
    story_focus: Optional[str],
    *,
    scene: str,
    plot_stage: Optional[str] = None,
) -> str:
    normalized_mode = normalize_creative_mode(creative_mode)
    normalized_focus = normalize_story_focus(story_focus)
    normalized_stage = normalize_plot_stage(plot_stage)

    if not (normalized_mode or normalized_focus or normalized_stage):
        return ""

    if scene == "outline":
        unresolved_point = "卷尾几章要留一个足够具体的未决点，能自然牵引下一轮主任务，而不是空泛悬着。"
        next_push = "结尾最好把人物逼到新的行动门槛前，让下一轮一开始就有事可做。"
        aftertaste = "尾声要保留情绪余波、关系余震、代价阴影或认知反照。"
        avoid_line = "不要每轮都只靠一句“更大的谜团出现了”来硬卖续读。"
        scene_label = "大纲"
    else:
        unresolved_point = "章尾要留一个具体未决点：一个答案缺口、一个马上要做的选择，或一个刚翻面的新问题。"
        next_push = "结尾最好把人物逼到下一步动作边缘，让读者自然想看下一章。"
        aftertaste = "除了钩子，还要留一点情绪余味、代价回响或关系余震。"
        avoid_line = "不要只靠突然打断、无信息硬切或机械性的“未完待续感”制造悬停。"
        scene_label = "章节"

    if normalized_mode == "hook":
        unresolved_point = "未决点优先是迫近选择、倒计时危险或刚被掀开的麻烦，不要只做语气停顿。"
        next_push = "下一步逼力要明确到人物不得不马上应对，而不是以后再说。"
    elif normalized_mode == "emotion":
        aftertaste = "余味最好落在误伤后的沉默、靠近失败后的反弹，或关系未说破的震荡上。"
        avoid_line = "不要在情绪高点后立刻解释完、说透完，把回响全部冲掉。"
    elif normalized_mode == "suspense":
        unresolved_point = "未决点最好是线索翻面、认知裂缝、危险升级或答案只揭开半层。"
        aftertaste = "余味要让读者感到局势更深、更险，而不是只多了一个名词。"
    elif normalized_mode == "relationship":
        unresolved_point = "未决点最好和立场未定、关系悬空、合作破裂或信任临界绑定。"
        aftertaste = "余味应保留人物之间的温差、敌意、亏欠或迟到的理解。"
    elif normalized_mode == "payoff":
        unresolved_point = "兑现之后要留一个新失衡或新代价，说明故事没有在爽点处直接封口。"
        next_push = "下一步逼力最好来自兑现后的后效，而不是硬塞一个无关新坑。"

    if normalized_focus == "advance_plot":
        next_push = "结尾逼力必须能接到主线下一步，不要只留下气氛而没有行动方向。"
    elif normalized_focus == "deepen_character":
        aftertaste = "余味最好让读者记住人物此刻的新伤口、新认知或新自我怀疑。"
    elif normalized_focus == "escalate_conflict":
        unresolved_point = "未决点应落在冲突升级后的更难位置：谁先出手、谁先失控、谁先付代价。"
        next_push = "下一步逼力要让人物无法轻松退回安全区。"
    elif normalized_focus == "reveal_mystery":
        unresolved_point = "未决点最好是刚拿到半个答案，却暴露出更关键的缺口或反常。"
    elif normalized_focus == "relationship_shift":
        aftertaste = "余味要落在关系新站位上，让读者感到他们再也回不到原来的相处方式。"
    elif normalized_focus == "foreshadow_payoff":
        unresolved_point = "未决点可以是旧伏笔兑现后的新空缺，说明兑现带来了新的问题而非彻底归零。"

    stage_line = ""
    if normalized_stage == "development":
        stage_line = "发展阶段的章尾/卷尾要把下一轮任务说得更具体，别总停在模糊愿景。"
    elif normalized_stage == "climax":
        stage_line = "高潮阶段的结尾要保持冲击余震与决战逼力，不要突然卸压。"
        avoid_line = "不要在高潮结尾处仓促复盘、解释一切或切回轻松缓冲，导致气势塌掉。"
    elif normalized_stage == "ending":
        stage_line = "结局阶段可以减少硬卖关子，更适合保留余波、代价、阴影或尚未完全愈合的裂口。"
        avoid_line = "不要为了续作感硬开全新主线；更适合留下收束后的余味和未尽代价。"

    combo_labels: list[str] = []
    if normalized_mode:
        combo_labels.append(CREATIVE_MODE_SPECS[normalized_mode]["label"])
    if normalized_focus:
        combo_labels.append(STORY_FOCUS_SPECS[normalized_focus]["label"])
    if normalized_stage:
        combo_labels.append(PLOT_STAGE_LABELS[normalized_stage])

    combo_text = " / ".join(combo_labels) if combo_labels else "默认悬停"
    lines = [f"【{scene_label}结尾悬停卡】收尾请留下继续阅读/推进的牵引（{combo_text}）"]
    lines.append(f"- 未决点：{unresolved_point}")
    lines.append(f"- 下一步逼力：{next_push}")
    lines.append(f"- 余味：{aftertaste}")
    if scene != "outline":
        lines.append("- 硬指标：最后一段至少落下 2 类尾钩信号（信息缺口 / 危险逼近 / 身份位移 / 待做选择 / 事态升级），最后一句不要复盘解释。")
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


def build_volume_pacing_block(
    chapter_count: Optional[int],
    *,
    plot_stage: Optional[str] = None,
) -> str:
    total = max(0, int(chapter_count or 0))
    if total <= 0:
        return ""

    normalized_stage = normalize_plot_stage(plot_stage)
    segments = _allocate_volume_segments(total)
    if not segments:
        return ""

    lines = [f"【卷级节奏】若本轮规划 {total} 章，建议整体按以下节奏分段"]
    cursor = 1
    for stage, count in segments:
        start_chapter = cursor
        end_chapter = cursor + count - 1
        cursor = end_chapter + 1
        stage_label = PLOT_STAGE_LABELS.get(stage, stage)
        mission = PLOT_STAGE_MISSIONS.get(stage, "")
        lines.append(f"- 第{start_chapter}-{end_chapter}章：{stage_label}，重点任务是{mission}")

    if normalized_stage:
        lines.append(f"- 当前用户指定重点阶段：{PLOT_STAGE_LABELS.get(normalized_stage, normalized_stage)}，本轮应优先把资源集中到这一段的核心任务。")

    return _compact_prompt_text("\n".join(lines))


def build_story_long_term_goal_block(long_term_goal: Optional[str]) -> str:
    goal_text = _compact_prompt_text(long_term_goal)
    if not goal_text:
        return ""

    lines = [
        "【长线目标锚点】",
        f"- 本书长线目标：{goal_text}",
        "- 本轮输出必须服务这条长线，不要只完成局部热闹。",
        "- 高潮、反转和情绪爆点都要能回扣主线目标、长期代价或最终回报。",
    ]
    return _compact_prompt_text("\n".join(lines))


def build_story_character_focus_anchor_block(
    story_character_focus: Optional[Any],
    *,
    scene: str = "chapter",
) -> str:
    focus_items = _normalize_runtime_prompt_items(story_character_focus, limit=4)
    if not focus_items:
        return ""

    scene_label = "章节" if scene == "chapter" else "大纲"
    joined_focus = " / ".join(focus_items)
    lines = [
        f"【{scene_label}角色焦点锚点】",
        f"- 本轮优先照亮角色：{joined_focus}",
        "- 让这些角色分别承担决定、反应或关系位移，不要只挂名出场。",
        "- 重要情绪变化尽量落在这些角色的选择与后果上，避免镜头平均摊薄。",
    ]
    return _compact_prompt_text("\n".join(lines))


def build_story_foreshadow_payoff_plan_block(
    story_foreshadow_payoff_plan: Optional[Any],
    *,
    scene: str = "chapter",
) -> str:
    payoff_items = _normalize_runtime_prompt_items(story_foreshadow_payoff_plan, limit=3)
    if not payoff_items:
        return ""

    scene_label = "章节" if scene == "chapter" else "大纲"
    lines = [f"【{scene_label}伏笔兑现计划】", "- 本轮优先处理以下伏笔/回报链："]
    lines.extend(f"  - {item}" for item in payoff_items)
    lines.append("- 兑现时要带出新信息、新代价或新失衡，避免只做口头回收。")
    return _compact_prompt_text("\n".join(lines))


def build_story_pacing_budget_block(
    chapter_count: Optional[Any],
    *,
    current_chapter_number: Optional[Any] = None,
    target_word_count: Optional[Any] = None,
    plot_stage: Optional[str] = None,
    scene: str = "chapter",
) -> str:
    total = _coerce_positive_int(chapter_count)
    current = _coerce_positive_int(current_chapter_number)
    target = _coerce_positive_int(target_word_count)
    normalized_stage = normalize_plot_stage(plot_stage)
    scene_label = "章节" if scene == "chapter" else "大纲"

    lines = [f"【{scene_label}节奏预算】"]
    if total and current:
        lines.append(f"- 当前进度：第{current}/{total}章。")
        cursor = 1
        for stage, count in _allocate_volume_segments(total):
            start_chapter = cursor
            end_chapter = cursor + count - 1
            cursor = end_chapter + 1
            if start_chapter <= current <= end_chapter:
                lines.append(
                    f"- 结构位置：当前位于第{start_chapter}-{end_chapter}章的{PLOT_STAGE_LABELS.get(stage, stage)}段，本轮要完成这一段该有的推进。"
                )
                break
    elif total:
        lines.append(f"- 计划体量：约{total}章，推进时先按整卷节奏分配资源，不要只顾单点刺激。")

    if target:
        if scene == "chapter":
            lines.append(f"- 本章目标字数：约{target}字，可在保证节奏完整的前提下浮动 ±20%。")
        else:
            lines.append(f"- 单章体量可参考约{target}字，避免开局章节过短或信息堆积失衡。")

    if normalized_stage:
        lines.append(
            f"- 阶段重点：{PLOT_STAGE_LABELS.get(normalized_stage, normalized_stage)}，优先完成该阶段最关键的任务，不要提前透支后续高潮。"
        )

    if len(lines) == 1:
        return ""

    if scene == "chapter":
        lines.append("- 节奏上要做到：开场尽快立题，中段持续加压，尾段留下动作牵引或情绪余震。")
    else:
        lines.append("- 规划时要兼顾起势、升级、回报与续航，不要把所有强刺激都堆在前几章。")
    return _compact_prompt_text("\n".join(lines))


def build_story_quality_trend_block(
    summary: Optional[Any],
    *,
    scene: str = "chapter",
) -> str:
    if not isinstance(summary, Mapping):
        return ""

    scene_label = "章节" if scene == "chapter" else "大纲"
    header = f"【{scene_label}近期质量趋势】"
    sections: list[tuple[int, str, str]] = []

    def append_section(priority: int, section_key: str, value: Any) -> None:
        cleaned = _compact_prompt_text(value)
        if cleaned:
            sections.append((priority, section_key, cleaned))

    chapter_count = _coerce_positive_int(summary.get("chapter_count"))
    trend_label_map = {
        "rising": "整体质量趋势在回升，本轮可以稳中求进。",
        "stable": "整体质量趋势相对稳定，本轮要优先补短板。",
        "falling": "整体质量趋势在下滑，本轮必须主动修复关键短板。",
    }
    if chapter_count:
        append_section(2, "reference_window", f"- 参考范围：最近 {chapter_count} 章的生成反馈。")

    pacing_score = summary.get("avg_pacing_score")
    if isinstance(pacing_score, (int, float)):
        append_section(2, "avg_pacing_score", f"- 最近节奏稳定度均值：{float(pacing_score):.1f}/10，场景切换与推进要维持连续压强。")

    payoff_rate = summary.get("avg_payoff_chain_rate")
    if isinstance(payoff_rate, (int, float)):
        append_section(2, "avg_payoff_chain_rate", f"- 最近回报兑现均值：{float(payoff_rate):.1f}%，本章至少回收一个既有承诺或伏笔。")

    cliffhanger_rate = summary.get("avg_cliffhanger_rate")
    if isinstance(cliffhanger_rate, (int, float)):
        append_section(2, "avg_cliffhanger_rate", f"- 最近章尾牵引均值：{float(cliffhanger_rate):.1f}%，尾段要留下明确的未决问题、代价或动作牵引。")

    trend_note = trend_label_map.get(str(summary.get("overall_score_trend") or "").strip().lower())
    overall_delta = summary.get("overall_score_delta")
    normalized_trend_note = _trim_prompt_terminal_punctuation(trend_note)
    if normalized_trend_note and isinstance(overall_delta, (int, float)):
        append_section(0, "overall_score_trend", f"- 趋势判断：{normalized_trend_note}（最近综合分变化 {float(overall_delta):+.1f}）。")
    elif normalized_trend_note:
        append_section(0, "overall_score_trend", f"- 趋势判断：{normalized_trend_note}。")

    focus_areas = _normalize_runtime_prompt_items(summary.get("recent_focus_areas"), limit=3)
    if focus_areas:
        append_section(1, "recent_focus_areas", f"- 最近高频修复焦点：{' / '.join(focus_areas)}。")

    volume_goal_completion = summary.get("volume_goal_completion") if isinstance(summary.get("volume_goal_completion"), Mapping) else {}
    volume_completion_rate = volume_goal_completion.get("completion_rate")
    volume_summary = str(volume_goal_completion.get("summary") or "").strip()
    volume_targets = _normalize_runtime_prompt_items(volume_goal_completion.get("repair_targets"), limit=2)
    normalized_volume_targets = _normalize_prompt_sentence_fragments(volume_targets)
    if isinstance(volume_completion_rate, (int, float)):
        append_section(2, "volume_goal_completion_rate", f"- 卷级目标达成率：{float(volume_completion_rate):.1f}%，本章必须对齐当前阶段任务。")
    if volume_summary:
        append_section(0, "volume_goal_completion_summary", f"- 卷级推进判断：{volume_summary}")
    volume_profile_summary = str(volume_goal_completion.get("profile_summary") or "").strip()
    volume_profile_focuses = _normalize_runtime_prompt_items(volume_goal_completion.get("profile_focuses"), limit=3)
    if volume_profile_summary:
        append_section(1, "volume_profile_summary", f"- 当前体裁 / 风格画像：{volume_profile_summary}")
    elif volume_profile_focuses:
        append_section(1, "volume_profile_focuses", f"- 当前体裁 / 风格重心：{' / '.join(volume_profile_focuses)}。")
    if normalized_volume_targets:
        append_section(0, "volume_goal_completion_targets", f"- 本章优先对齐这些卷级任务：{' / '.join(normalized_volume_targets)}。")

    pacing_imbalance = summary.get("pacing_imbalance") if isinstance(summary.get("pacing_imbalance"), Mapping) else {}
    pacing_summary = str(pacing_imbalance.get("summary") or "").strip()
    pacing_targets = _normalize_runtime_prompt_items(pacing_imbalance.get("repair_targets"), limit=2)
    normalized_pacing_targets = _normalize_prompt_sentence_fragments(pacing_targets)
    pacing_signal_lines: list[str] = []
    for signal in pacing_imbalance.get("signals") or []:
        if not isinstance(signal, Mapping):
            continue
        label = str(signal.get("label") or signal.get("key") or "节奏异常").strip()
        if not label:
            continue
        severity = str(signal.get("severity") or "watch").strip().lower()
        severity_label = "预警" if severity == "warning" else "关注"
        signal_summary = _trim_prompt_terminal_punctuation(signal.get("summary"))
        metric = signal.get("metric")
        metric_text = f"，指标 {float(metric):.1f}" if isinstance(metric, (int, float)) else ""
        detail = f"{label}（{severity_label}{metric_text}）"
        if signal_summary:
            detail = f"{detail}：{signal_summary}"
        pacing_signal_lines.append(detail)
    if pacing_summary:
        append_section(0, "pacing_imbalance_summary", f"- 长篇节奏信号：{pacing_summary}")
    if pacing_signal_lines:
        pacing_signal_text = "；".join(pacing_signal_lines)
        append_section(1, "pacing_imbalance_signals", f"- 当前要盯住的长篇节奏异常：{pacing_signal_text}。")
    if normalized_pacing_targets:
        append_section(0, "pacing_imbalance_targets", f"- 本章优先修复这些长篇节奏问题：{' / '.join(normalized_pacing_targets)}。")
        append_section(0, "pacing_guardrail", "- 节奏硬要求：本章必须同时完成“推进一件事 + 回收一件事 + 留下下一步牵引”。")

    foreshadow_payoff_delay = summary.get("foreshadow_payoff_delay") if isinstance(summary.get("foreshadow_payoff_delay"), Mapping) else {}
    delay_index = foreshadow_payoff_delay.get("delay_index")
    foreshadow_summary = str(foreshadow_payoff_delay.get("summary") or "").strip()
    foreshadow_targets = _normalize_runtime_prompt_items(foreshadow_payoff_delay.get("repair_targets"), limit=2)
    normalized_foreshadow_targets = _normalize_prompt_sentence_fragments(foreshadow_targets)
    if isinstance(delay_index, (int, float)):
        append_section(2, "foreshadow_delay_index", f"- 伏笔兑现延迟指数：{float(delay_index):.1f}，越高越说明旧伏笔积压越多。")
    if foreshadow_summary:
        append_section(0, "foreshadow_payoff_summary", f"- 伏笔兑现判断：{foreshadow_summary}")
    if normalized_foreshadow_targets:
        append_section(0, "foreshadow_payoff_targets", f"- 本章优先清偿这些伏笔账：{' / '.join(normalized_foreshadow_targets)}。")

    continuity_preflight = summary.get("continuity_preflight") if isinstance(summary.get("continuity_preflight"), Mapping) else {}
    continuity_summary = str(continuity_preflight.get("summary") or "").strip()
    continuity_targets = _normalize_runtime_prompt_items(continuity_preflight.get("repair_targets"), limit=2)
    normalized_continuity_targets = _normalize_prompt_sentence_fragments(continuity_targets)
    if continuity_summary:
        append_section(0, "continuity_preflight_summary", f"- 连续性预检：{continuity_summary}")
    if normalized_continuity_targets:
        append_section(0, "continuity_preflight_targets", f"- 本章要补齐这些连续性接力：{' / '.join(normalized_continuity_targets)}。")
    if continuity_summary or normalized_continuity_targets:
        append_section(0, "continuity_guardrail", "- 连续性硬要求：至少显式接住 1 个上一章已经建立的人物 / 关系 / 伏笔状态。")

    repair_effectiveness = summary.get("repair_effectiveness") if isinstance(summary.get("repair_effectiveness"), Mapping) else {}
    repair_success_rate = repair_effectiveness.get("success_rate")
    repair_effectiveness_summary = str(repair_effectiveness.get("summary") or "").strip()
    repair_evaluated_pairs = _coerce_positive_int(repair_effectiveness.get("evaluated_pairs"))
    recovered_focuses = _normalize_runtime_prompt_items(repair_effectiveness.get("recovered_focus_areas"), limit=2)
    unresolved_focuses = _normalize_runtime_prompt_items(repair_effectiveness.get("unresolved_focus_areas"), limit=2)
    if isinstance(repair_success_rate, (int, float)):
        pair_text = f"（基于 {repair_evaluated_pairs} 组相邻章节）" if repair_evaluated_pairs else ""
        append_section(2, "repair_effectiveness_rate", f"- 最近修复成效率：{float(repair_success_rate):.1f}%{pair_text}。")
    if repair_effectiveness_summary:
        append_section(0, "repair_effectiveness_summary", f"- 修复效果判断：{repair_effectiveness_summary}")
    if unresolved_focuses:
        append_section(1, "repair_unresolved_focuses", f"- 仍未稳定的修复焦点：{' / '.join(unresolved_focuses)}。")
    elif recovered_focuses:
        append_section(1, "repair_recovered_focuses", f"- 已经开始回收的修复焦点：{' / '.join(recovered_focuses)}。")

    if not sections:
        return ""

    selected_lines = [header]
    total_chars = len(header)
    dropped_optional = False
    max_lines = 18
    max_chars = 1700
    selected_section_keys: list[str] = []
    dropped_section_keys: list[str] = []

    for priority in (0, 1, 2):
        for section_priority, section_key, line in sections:
            if section_priority != priority:
                continue
            line_cost = len(line) + 1
            if priority > 0 and (len(selected_lines) + 1 > max_lines or total_chars + line_cost > max_chars):
                dropped_optional = True
                if section_key not in dropped_section_keys:
                    dropped_section_keys.append(section_key)
                continue
            selected_lines.append(line)
            total_chars += line_cost
            if section_key not in selected_section_keys:
                selected_section_keys.append(section_key)

    final_line = "- 生成时优先修复趋势中持续偏弱的项，同时保留已经稳定成立的强项。"
    if len(selected_lines) + 1 <= max_lines and total_chars + len(final_line) + 1 <= max_chars:
        selected_lines.append(final_line)
        total_chars += len(final_line) + 1
        selected_section_keys.append("final_instruction")

    folded_note = "- 其余次级趋势细项已折叠，优先执行以上关键信号。"
    if dropped_optional and len(selected_lines) + 1 <= max_lines and total_chars + len(folded_note) + 1 <= max_chars:
        selected_lines.append(folded_note)
        total_chars += len(folded_note) + 1
        selected_section_keys.append("folded_optional_note")

    if logger.isEnabledFor(logging.DEBUG):
        logger.debug(
            "story_quality_trend_budget tracking=%s scene=%s total_sections=%s selected_lines=%s selected_chars=%s selected_sections=%s dropped_sections=%s dropped_optional=%s",
            QUALITY_RUNTIME_TRACKING_TAG,
            scene,
            len(sections),
            len(selected_lines),
            total_chars,
            selected_section_keys,
            dropped_section_keys,
            dropped_optional,
        )

    return _compact_prompt_text("\n".join(selected_lines))


def build_story_character_state_ledger_block(
    story_character_state_ledger: Optional[Any],
    *,
    scene: str = "chapter",
) -> str:
    state_items = _normalize_runtime_prompt_items(story_character_state_ledger, limit=4)
    if not state_items:
        return ""

    scene_label = "章节" if scene == "chapter" else "大纲"
    lines = [f"【{scene_label}人物状态账本】", "- 以下状态是本轮必须延续的人物处境、压力或阶段变化："]
    lines.extend(f"  - {item}" for item in state_items)
    lines.append("- 用动作、选择、代价和情绪反应把这些状态写实，不要只在说明句里复述。")
    return _compact_prompt_text("\n".join(lines))


def build_story_relationship_state_ledger_block(
    story_relationship_state_ledger: Optional[Any],
    *,
    scene: str = "chapter",
) -> str:
    relationship_items = _normalize_runtime_prompt_items(story_relationship_state_ledger, limit=4)
    if not relationship_items:
        return ""

    scene_label = "章节" if scene == "chapter" else "大纲"
    lines = [f"【{scene_label}关系状态账本】", "- 以下关系线必须在互动、站队或对白里继续推进："]
    lines.extend(f"  - {item}" for item in relationship_items)
    lines.append("- 至少让其中一条关系出现可见位移，不要只重复旧情绪。")
    return _compact_prompt_text("\n".join(lines))


def build_story_foreshadow_state_ledger_block(
    story_foreshadow_state_ledger: Optional[Any],
    *,
    scene: str = "chapter",
) -> str:
    foreshadow_items = _normalize_runtime_prompt_items(story_foreshadow_state_ledger, limit=4)
    if not foreshadow_items:
        return ""

    scene_label = "章节" if scene == "chapter" else "大纲"
    lines = [f"【{scene_label}伏笔状态账本】", "- 以下伏笔或承诺需要推进、兑现或制造新的回响："]
    lines.extend(f"  - {item}" for item in foreshadow_items)
    lines.append("- 把伏笔状态落在事件结果、信息揭示或代价变化上，不要只口头提醒。")
    return _compact_prompt_text("\n".join(lines))


def build_story_organization_state_ledger_block(
    story_organization_state_ledger: Optional[Any],
    *,
    scene: str = "chapter",
) -> str:
    organization_items = _normalize_runtime_prompt_items(story_organization_state_ledger, limit=4)
    if not organization_items:
        return ""

    scene_label = "章节" if scene == "chapter" else "大纲"
    lines = [f"【{scene_label}组织状态账本】", "- 以下组织或势力状态需要继续影响资源、命令、站队或地盘："]
    lines.extend(f"  - {item}" for item in organization_items)
    lines.append("- 组织变化要落实到人物决策与局势后果，不要只写背景说明。")
    return _compact_prompt_text("\n".join(lines))


def build_story_career_state_ledger_block(
    story_career_state_ledger: Optional[Any],
    *,
    scene: str = "chapter",
) -> str:
    career_items = _normalize_runtime_prompt_items(story_career_state_ledger, limit=4)
    if not career_items:
        return ""

    scene_label = "章节" if scene == "chapter" else "大纲"
    lines = [f"【{scene_label}职业状态账本】", "- 以下职业或能力成长状态要继续体现在技能使用、瓶颈或代价上："]
    lines.extend(f"  - {item}" for item in career_items)
    lines.append("- 职业推进要落到任务结果、能力应用和成长成本，不要只报阶段名。")
    return _compact_prompt_text("\n".join(lines))




