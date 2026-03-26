"""提示词管理服务"""
from typing import Dict, Any, Mapping, Optional, Tuple, Sequence
import json
import re

from app.logger import get_logger
from app.services.novel_quality_profile_service import novel_quality_profile_service

try:
    from app.services.mcp_tools_loader import (
        MCP_CANON_PRIORITY_RULE,
        MCP_SOURCE_DISCLOSURE_RULE,
    )
except Exception:
    MCP_CANON_PRIORITY_RULE = "项目 canon（既有设定、角色关系、本章大纲）优先级高于一切外部参考。"
    MCP_SOURCE_DISCLOSURE_RULE = "最终输出禁止暴露 MCP、工具名、检索过程或来源站点。"


logger = get_logger(__name__)


QUALITY_RUNTIME_TRACKING_TAG = "rule_v3_quality_block_20260307"
QUALITY_TEMPLATE_MARKER_PATTERN = re.compile(r"^<prompt_template_key value=\"(?P<key>[A-Z0-9_]+)\" />\n?", re.MULTILINE)

QUALITY_BLOCK_SECTION_GENERATION = """<quality_contract priority=\"P0\">\n{quality_generation_block}\n{creative_mode_block}\n{story_focus_block}\n{narrative_blueprint_block}\n{story_creation_brief_block}\n{story_long_term_goal_block}\n{story_pacing_budget_block}\n{story_volume_pacing_block}\n{story_quality_trend_block}\n{story_character_focus_anchor_block}\n{story_foreshadow_payoff_plan_block}\n{story_character_state_ledger_block}\n{story_relationship_state_ledger_block}\n{story_foreshadow_state_ledger_block}\n{story_organization_state_ledger_block}\n{story_career_state_ledger_block}\n{quality_preference_block}\n{story_objective_card_block}\n{story_result_card_block}\n{story_payoff_chain_card_block}\n{story_rule_grounding_card_block}\n{story_information_release_card_block}\n{story_emotion_landing_card_block}\n{story_action_rendering_card_block}\n{story_summary_tone_control_card_block}\n{story_repetition_control_card_block}\n{story_viewpoint_discipline_card_block}\n{story_dialogue_advancement_card_block}\n{story_opening_hook_card_block}\n{story_repair_target_block}\n{story_repair_diagnostic_block}\n{story_execution_checklist_block}\n{story_scene_anchor_card_block}\n{story_scene_density_card_block}\n{story_repetition_risk_block}\n{story_acceptance_card_block}\n{story_cliffhanger_card_block}\n{story_character_arc_card_block}\n{quality_generation_protocol_block}\n{quality_mcp_guard_block}\n{quality_external_assets_block}\n</quality_contract>"""
QUALITY_BLOCK_SECTION_ANALYSIS = """<quality_contract priority=\"P0\">\n{quality_analysis_block}\n{creative_mode_block}\n{story_focus_block}\n{narrative_blueprint_block}\n{story_creation_brief_block}\n{story_long_term_goal_block}\n{story_pacing_budget_block}\n{story_volume_pacing_block}\n{story_quality_trend_block}\n{story_character_focus_anchor_block}\n{story_foreshadow_payoff_plan_block}\n{story_character_state_ledger_block}\n{story_relationship_state_ledger_block}\n{story_foreshadow_state_ledger_block}\n{story_organization_state_ledger_block}\n{story_career_state_ledger_block}\n{quality_preference_block}\n{quality_json_protocol_block}\n{quality_mcp_guard_block}\n{quality_external_assets_block}\n</quality_contract>"""
QUALITY_BLOCK_SECTION_CHECKER = """<quality_contract priority=\"P0\">\n{quality_checker_block}\n{creative_mode_block}\n{story_focus_block}\n{narrative_blueprint_block}\n{story_creation_brief_block}\n{story_long_term_goal_block}\n{story_pacing_budget_block}\n{story_volume_pacing_block}\n{story_quality_trend_block}\n{story_character_focus_anchor_block}\n{story_foreshadow_payoff_plan_block}\n{story_character_state_ledger_block}\n{story_relationship_state_ledger_block}\n{story_foreshadow_state_ledger_block}\n{story_organization_state_ledger_block}\n{story_career_state_ledger_block}\n{quality_preference_block}\n{quality_json_protocol_block}\n{quality_mcp_guard_block}\n{quality_external_assets_block}\n</quality_contract>"""
QUALITY_BLOCK_SECTION_REVISER = """<quality_contract priority=\"P0\">\n{quality_reviser_block}\n{creative_mode_block}\n{story_focus_block}\n{narrative_blueprint_block}\n{story_creation_brief_block}\n{story_long_term_goal_block}\n{story_pacing_budget_block}\n{story_volume_pacing_block}\n{story_quality_trend_block}\n{story_character_focus_anchor_block}\n{story_foreshadow_payoff_plan_block}\n{story_character_state_ledger_block}\n{story_relationship_state_ledger_block}\n{story_foreshadow_state_ledger_block}\n{story_organization_state_ledger_block}\n{story_career_state_ledger_block}\n{quality_preference_block}\n{quality_json_protocol_block}\n{quality_mcp_guard_block}\n{quality_external_assets_block}\n</quality_contract>"""
QUALITY_BLOCK_SECTION_REGENERATION = """<quality_contract priority=\"P0\">\n{quality_regeneration_block}\n{creative_mode_block}\n{story_focus_block}\n{narrative_blueprint_block}\n{story_creation_brief_block}\n{story_long_term_goal_block}\n{story_pacing_budget_block}\n{story_volume_pacing_block}\n{story_quality_trend_block}\n{story_character_focus_anchor_block}\n{story_foreshadow_payoff_plan_block}\n{story_character_state_ledger_block}\n{story_relationship_state_ledger_block}\n{story_foreshadow_state_ledger_block}\n{story_organization_state_ledger_block}\n{story_career_state_ledger_block}\n{quality_preference_block}\n{story_objective_card_block}\n{story_result_card_block}\n{story_payoff_chain_card_block}\n{story_rule_grounding_card_block}\n{story_information_release_card_block}\n{story_emotion_landing_card_block}\n{story_action_rendering_card_block}\n{story_summary_tone_control_card_block}\n{story_repetition_control_card_block}\n{story_viewpoint_discipline_card_block}\n{story_dialogue_advancement_card_block}\n{story_opening_hook_card_block}\n{story_repair_target_block}\n{story_repair_diagnostic_block}\n{story_execution_checklist_block}\n{story_scene_anchor_card_block}\n{story_scene_density_card_block}\n{story_repetition_risk_block}\n{story_acceptance_card_block}\n{story_cliffhanger_card_block}\n{story_character_arc_card_block}\n{quality_generation_protocol_block}\n{quality_mcp_guard_block}\n{quality_external_assets_block}\n</quality_contract>"""

QUALITY_TEMPLATE_INSERTIONS = {
    "CHAPTER_GENERATION_ONE_TO_MANY": QUALITY_BLOCK_SECTION_GENERATION,
    "CHAPTER_GENERATION_ONE_TO_MANY_NEXT": QUALITY_BLOCK_SECTION_GENERATION,
    "CHAPTER_GENERATION_ONE_TO_ONE": QUALITY_BLOCK_SECTION_GENERATION,
    "CHAPTER_GENERATION_ONE_TO_ONE_NEXT": QUALITY_BLOCK_SECTION_GENERATION,
    "PLOT_ANALYSIS": QUALITY_BLOCK_SECTION_ANALYSIS,
    "CHAPTER_TEXT_CHECKER": QUALITY_BLOCK_SECTION_CHECKER,
    "CHAPTER_TEXT_REVISER": QUALITY_BLOCK_SECTION_REVISER,
    "CHAPTER_REGENERATION_SYSTEM": QUALITY_BLOCK_SECTION_REGENERATION,
}

CREATIVE_STORY_ENGINE_GUIDE = """【故事发动机优先】
- 先把人物眼前最想要什么、阻力从哪来、做错要付什么代价写清
- 先让读者看到变化、压力和反应，再补背景解释
- 每段内容至少服务推进情节、暴露关系、制造记忆点中的一项
- 能用场景、动作、对白表达，就不要改写成抽象总结
- 控制重复，不要全员同一种语气、全章同一种节拍、全篇同一种钩子"""

CREATIVE_LOW_AI_GUARD = """【自然表达护栏】
- 少用“总之/综上/值得注意的是/在这个过程中”等模板连接词
- 少讲作者结论，多给角色动作、情绪回弹和现场压力
- 允许少量不规整句式，别把文字修成统一说明书口吻
- 保留人物自己的说话习惯，不把所有角色抹平成同一个声音"""

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


def normalize_creative_mode(mode: Optional[str]) -> Optional[str]:
    cleaned = str(mode or "").strip()
    if not cleaned:
        return None
    return CREATIVE_MODE_ALIASES.get(cleaned) or CREATIVE_MODE_ALIASES.get(cleaned.lower())


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


def normalize_story_focus(value: Optional[str]) -> Optional[str]:
    cleaned = str(value or "").strip()
    if not cleaned:
        return None
    return STORY_FOCUS_ALIASES.get(cleaned) or STORY_FOCUS_ALIASES.get(cleaned.lower())


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


def normalize_quality_preset(value: Optional[str]) -> Optional[str]:
    cleaned = str(value or "").strip()
    if not cleaned:
        return None
    return QUALITY_PREFERENCE_ALIASES.get(cleaned) or QUALITY_PREFERENCE_ALIASES.get(cleaned.lower())


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


def normalize_plot_stage(value: Optional[str]) -> Optional[str]:
    cleaned = str(value or "").strip()
    if not cleaned:
        return None
    return PLOT_STAGE_ALIASES.get(cleaned) or PLOT_STAGE_ALIASES.get(cleaned.lower())


def _dedupe_prompt_items(items: list[str]) -> list[str]:
    result: list[str] = []
    seen: set[str] = set()
    for item in items:
        text = str(item or "").strip()
        if not text or text in seen:
            continue
        seen.add(text)
        result.append(text)
    return result


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
        rule_landing = "这一轮至少让一个核心规则、行业逻辑或力量边界在情节里真正起作用，不只停在设定表。"
        trigger_condition = "提前想清楚规则是被什么动作、条件、身份、地点或代价触发的。"
        cost_limit = "规则一旦出手，要带出门槛、冷却、风险、限制或现实成本，别只剩万能效果。"
        scene_manifestation = "安排规则在关键场景里留下清晰反馈：局势变了、关系变了、后续选择变了。"
        avoid_line = "不要先用整段说明把规则讲透，到了情节里却几乎用不上。"
        scene_label = "大纲"
    else:
        rule_landing = "本章至少让一个设定规则通过人物动作、现场反馈和后果被看见，而不是靠讲解出现。"
        trigger_condition = "写清这条规则是怎么被触发的，谁触发、在什么条件下触发、为什么现在触发。"
        cost_limit = "规则生效后要有边界、耗损、反噬、误差或现实牵连，避免像开挂按钮。"
        scene_manifestation = "规则必须改写当下场面：让人受限、受益、失手、暴露、受伤或改变判断。"
        avoid_line = "不要把设定写成只存在于说明文字里的背景板，也不要一触发就万能解决所有问题。"
        scene_label = "章节"

    if normalized_mode == "hook":
        rule_landing = "设定最好一上来就制造麻烦、压力或危险，让规则本身成为抓手。"
    elif normalized_mode == "emotion":
        scene_manifestation = "规则落地最好能压到情绪与关系，让人物因为规则约束、代价或失手而受伤。"
    elif normalized_mode == "suspense":
        trigger_condition = "规则触发最好带出异常征兆、反常反馈或认知缺口，让读者感觉哪里不对。"
        cost_limit = "边界与代价不要一次讲完，先给足够可感的反常，再逐步揭开机制。"
    elif normalized_mode == "relationship":
        rule_landing = "设定最好落在身份、契约、门第、组织纪律或社会规则上，直接影响人物站位。"
    elif normalized_mode == "payoff":
        scene_manifestation = "优先让前文埋过的规则真正兑现，展示它终于生效时的爽点与后效。"

    if normalized_focus == "advance_plot":
        scene_manifestation = "规则生效后必须推动主线，不要只是展示世界观却不改局势。"
    elif normalized_focus == "deepen_character":
        trigger_condition = "最好通过人物主动触发、拒绝触发或误用规则，暴露他的价值判断与软肋。"
    elif normalized_focus == "escalate_conflict":
        cost_limit = "规则的代价、限制或反噬要把冲突抬高，而不是轻松替角色解围。"
    elif normalized_focus == "reveal_mystery":
        rule_landing = "规则落地应顺带暴露机制缺口、异常样本或隐藏条件，让谜团推进。"
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
    lines.append("- 修复必须落到具体事件、动作和后果，不要只加解释或换说法。")
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
    if stage_line:
        lines.append(f"- 阶段提醒：{stage_line}")
    lines.append(f"- 避免：{avoid_line}")
    return _compact_prompt_text("\n".join(lines))


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
    return text.rstrip("\u3002\uff01\uff1f!?\uff1b;,.\uff0c\u3001 ")


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
        text = str(raw or "").strip()
        if not text:
            continue
        text = re.sub(r"^[-•*·\d\.\)\s]+", "", text).strip()
        if not text or text.startswith("【"):
            continue
        normalized.append(text)

    return _dedupe_prompt_items(normalized)[:limit]




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


def _resolve_quality_focus_protected_blocks(summary: Optional[Any]) -> tuple[str, ...]:
    if not isinstance(summary, Mapping):
        return ()

    focus_tags = _normalize_quality_focus_tags(summary.get("recent_focus_areas"))
    continuity_preflight = summary.get("continuity_preflight") if isinstance(summary.get("continuity_preflight"), Mapping) else {}
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


def build_story_quality_trend_block(
    summary: Optional[Any],
    *,
    scene: str = "chapter",
) -> str:
    if not isinstance(summary, Mapping):
        return ""

    scene_label = "\u7ae0\u8282" if scene == "chapter" else "\u5927\u7eb2"
    header = f"\u3010{scene_label}\u8fd1\u671f\u8d28\u91cf\u8d8b\u52bf\u3011"
    sections: list[tuple[int, str, str]] = []

    def append_section(priority: int, section_key: str, value: Any) -> None:
        cleaned = _compact_prompt_text(value)
        if cleaned:
            sections.append((priority, section_key, cleaned))

    chapter_count = _coerce_positive_int(summary.get("chapter_count"))
    trend_label_map = {
        "rising": "\u6574\u4f53\u8d28\u91cf\u8d8b\u52bf\u5728\u56de\u5347\uff0c\u672c\u8f6e\u53ef\u4ee5\u7a33\u4e2d\u6c42\u8fdb\u3002",
        "stable": "\u6574\u4f53\u8d28\u91cf\u8d8b\u52bf\u76f8\u5bf9\u7a33\u5b9a\uff0c\u672c\u8f6e\u8981\u4f18\u5148\u8865\u77ed\u677f\u3002",
        "falling": "\u6574\u4f53\u8d28\u91cf\u8d8b\u52bf\u5728\u4e0b\u6ed1\uff0c\u672c\u8f6e\u5fc5\u987b\u4e3b\u52a8\u4fee\u590d\u5173\u952e\u77ed\u677f\u3002",
    }
    if chapter_count:
        append_section(2, "reference_window", f"- \u53c2\u8003\u8303\u56f4\uff1a\u6700\u8fd1 {chapter_count} \u7ae0\u7684\u751f\u6210\u53cd\u9988\u3002")

    pacing_score = summary.get("avg_pacing_score")
    if isinstance(pacing_score, (int, float)):
        append_section(2, "avg_pacing_score", f"- \u6700\u8fd1\u8282\u594f\u7a33\u5b9a\u5ea6\u5747\u503c\uff1a{float(pacing_score):.1f}/10\uff0c\u573a\u666f\u5207\u6362\u4e0e\u63a8\u8fdb\u8981\u7ef4\u6301\u8fde\u7eed\u538b\u5f3a\u3002")

    payoff_rate = summary.get("avg_payoff_chain_rate")
    if isinstance(payoff_rate, (int, float)):
        append_section(2, "avg_payoff_chain_rate", f"- \u6700\u8fd1\u56de\u62a5\u5151\u73b0\u5747\u503c\uff1a{float(payoff_rate):.1f}%\uff0c\u672c\u7ae0\u81f3\u5c11\u56de\u6536\u4e00\u4e2a\u65e2\u6709\u627f\u8bfa\u6216\u4f0f\u7b14\u3002")

    cliffhanger_rate = summary.get("avg_cliffhanger_rate")
    if isinstance(cliffhanger_rate, (int, float)):
        append_section(2, "avg_cliffhanger_rate", f"- \u6700\u8fd1\u7ae0\u5c3e\u7275\u5f15\u5747\u503c\uff1a{float(cliffhanger_rate):.1f}%\uff0c\u5c3e\u6bb5\u8981\u7559\u4e0b\u660e\u786e\u7684\u672a\u51b3\u95ee\u9898\u3001\u4ee3\u4ef7\u6216\u52a8\u4f5c\u7275\u5f15\u3002")

    trend_note = trend_label_map.get(str(summary.get("overall_score_trend") or "").strip().lower())
    overall_delta = summary.get("overall_score_delta")
    normalized_trend_note = _trim_prompt_terminal_punctuation(trend_note)
    if normalized_trend_note and isinstance(overall_delta, (int, float)):
        append_section(0, "overall_score_trend", f"- \u8d8b\u52bf\u5224\u65ad\uff1a{normalized_trend_note}\uff08\u6700\u8fd1\u7efc\u5408\u5206\u53d8\u5316 {float(overall_delta):+.1f}\uff09\u3002")
    elif normalized_trend_note:
        append_section(0, "overall_score_trend", f"- \u8d8b\u52bf\u5224\u65ad\uff1a{normalized_trend_note}\u3002")

    focus_areas = _normalize_runtime_prompt_items(summary.get("recent_focus_areas"), limit=3)
    if focus_areas:
        append_section(1, "recent_focus_areas", f"- \u6700\u8fd1\u9ad8\u9891\u4fee\u590d\u7126\u70b9\uff1a{' / '.join(focus_areas)}\u3002")

    volume_goal_completion = summary.get("volume_goal_completion") if isinstance(summary.get("volume_goal_completion"), Mapping) else {}
    volume_completion_rate = volume_goal_completion.get("completion_rate")
    volume_summary = str(volume_goal_completion.get("summary") or "").strip()
    volume_targets = _normalize_runtime_prompt_items(volume_goal_completion.get("repair_targets"), limit=2)
    normalized_volume_targets = _normalize_prompt_sentence_fragments(volume_targets)
    if isinstance(volume_completion_rate, (int, float)):
        append_section(2, "volume_goal_completion_rate", f"- \u5377\u7ea7\u76ee\u6807\u8fbe\u6210\u7387\uff1a{float(volume_completion_rate):.1f}%\uff0c\u672c\u7ae0\u5fc5\u987b\u5bf9\u9f50\u5f53\u524d\u9636\u6bb5\u4efb\u52a1\u3002")
    if volume_summary:
        append_section(0, "volume_goal_completion_summary", f"- \u5377\u7ea7\u63a8\u8fdb\u5224\u65ad\uff1a{volume_summary}")
    volume_profile_summary = str(volume_goal_completion.get("profile_summary") or "").strip()
    volume_profile_focuses = _normalize_runtime_prompt_items(volume_goal_completion.get("profile_focuses"), limit=3)
    if volume_profile_summary:
        append_section(1, "volume_profile_summary", f"- \u5f53\u524d\u4f53\u88c1 / \u98ce\u683c\u753b\u50cf\uff1a{volume_profile_summary}")
    elif volume_profile_focuses:
        append_section(1, "volume_profile_focuses", f"- \u5f53\u524d\u4f53\u88c1 / \u98ce\u683c\u91cd\u5fc3\uff1a{' / '.join(volume_profile_focuses)}\u3002")
    if normalized_volume_targets:
        append_section(0, "volume_goal_completion_targets", f"- \u672c\u7ae0\u4f18\u5148\u5bf9\u9f50\u8fd9\u4e9b\u5377\u7ea7\u4efb\u52a1\uff1a{' / '.join(normalized_volume_targets)}\u3002")

    pacing_imbalance = summary.get("pacing_imbalance") if isinstance(summary.get("pacing_imbalance"), Mapping) else {}
    pacing_summary = str(pacing_imbalance.get("summary") or "").strip()
    pacing_targets = _normalize_runtime_prompt_items(pacing_imbalance.get("repair_targets"), limit=2)
    normalized_pacing_targets = _normalize_prompt_sentence_fragments(pacing_targets)
    pacing_signal_lines: list[str] = []
    for signal in pacing_imbalance.get("signals") or []:
        if not isinstance(signal, Mapping):
            continue
        label = str(signal.get("label") or signal.get("key") or "\u8282\u594f\u5f02\u5e38").strip()
        if not label:
            continue
        severity = str(signal.get("severity") or "watch").strip().lower()
        severity_label = "\u9884\u8b66" if severity == "warning" else "\u5173\u6ce8"
        signal_summary = _trim_prompt_terminal_punctuation(signal.get("summary"))
        metric = signal.get("metric")
        metric_text = f"\uff0c\u6307\u6807 {float(metric):.1f}" if isinstance(metric, (int, float)) else ""
        detail = f"{label}\uff08{severity_label}{metric_text}\uff09"
        if signal_summary:
            detail = f"{detail}\uff1a{signal_summary}"
        pacing_signal_lines.append(detail)
    if pacing_summary:
        append_section(0, "pacing_imbalance_summary", f"- \u957f\u7bc7\u8282\u594f\u4fe1\u53f7\uff1a{pacing_summary}")
    if pacing_signal_lines:
        append_section(1, "pacing_imbalance_signals", f"- \u5f53\u524d\u8981\u76ef\u4f4f\u7684\u957f\u7bc7\u8282\u594f\u5f02\u5e38\uff1a{'\uff1b'.join(pacing_signal_lines)}\u3002")
    if normalized_pacing_targets:
        append_section(0, "pacing_imbalance_targets", f"- \u672c\u7ae0\u4f18\u5148\u4fee\u590d\u8fd9\u4e9b\u957f\u7bc7\u8282\u594f\u95ee\u9898\uff1a{' / '.join(normalized_pacing_targets)}\u3002")
        append_section(0, "pacing_guardrail", "- \u8282\u594f\u786c\u8981\u6c42\uff1a\u672c\u7ae0\u5fc5\u987b\u540c\u65f6\u5b8c\u6210\u201c\u63a8\u8fdb\u4e00\u4ef6\u4e8b + \u56de\u6536\u4e00\u4ef6\u4e8b + \u7559\u4e0b\u4e0b\u4e00\u6b65\u7275\u5f15\u201d\u3002")

    foreshadow_payoff_delay = summary.get("foreshadow_payoff_delay") if isinstance(summary.get("foreshadow_payoff_delay"), Mapping) else {}
    delay_index = foreshadow_payoff_delay.get("delay_index")
    foreshadow_summary = str(foreshadow_payoff_delay.get("summary") or "").strip()
    foreshadow_targets = _normalize_runtime_prompt_items(foreshadow_payoff_delay.get("repair_targets"), limit=2)
    normalized_foreshadow_targets = _normalize_prompt_sentence_fragments(foreshadow_targets)
    if isinstance(delay_index, (int, float)):
        append_section(2, "foreshadow_delay_index", f"- \u4f0f\u7b14\u5151\u73b0\u5ef6\u8fdf\u6307\u6570\uff1a{float(delay_index):.1f}\uff0c\u8d8a\u9ad8\u8d8a\u8bf4\u660e\u65e7\u4f0f\u7b14\u79ef\u538b\u8d8a\u591a\u3002")
    if foreshadow_summary:
        append_section(0, "foreshadow_payoff_summary", f"- \u4f0f\u7b14\u5151\u73b0\u5224\u65ad\uff1a{foreshadow_summary}")
    if normalized_foreshadow_targets:
        append_section(0, "foreshadow_payoff_targets", f"- \u672c\u7ae0\u4f18\u5148\u6e05\u507f\u8fd9\u4e9b\u4f0f\u7b14\u8d26\uff1a{' / '.join(normalized_foreshadow_targets)}\u3002")

    continuity_preflight = summary.get("continuity_preflight") if isinstance(summary.get("continuity_preflight"), Mapping) else {}
    continuity_summary = str(continuity_preflight.get("summary") or "").strip()
    continuity_targets = _normalize_runtime_prompt_items(continuity_preflight.get("repair_targets"), limit=2)
    normalized_continuity_targets = _normalize_prompt_sentence_fragments(continuity_targets)
    if continuity_summary:
        append_section(0, "continuity_preflight_summary", f"- \u8fde\u7eed\u6027\u9884\u68c0\uff1a{continuity_summary}")
    if normalized_continuity_targets:
        append_section(0, "continuity_preflight_targets", f"- \u672c\u7ae0\u8981\u8865\u9f50\u8fd9\u4e9b\u8fde\u7eed\u6027\u63a5\u529b\uff1a{' / '.join(normalized_continuity_targets)}\u3002")
    if continuity_summary or normalized_continuity_targets:
        append_section(0, "continuity_guardrail", "- \u8fde\u7eed\u6027\u786c\u8981\u6c42\uff1a\u81f3\u5c11\u663e\u5f0f\u63a5\u4f4f 1 \u4e2a\u4e0a\u4e00\u7ae0\u5df2\u7ecf\u5efa\u7acb\u7684\u4eba\u7269 / \u5173\u7cfb / \u4f0f\u7b14\u72b6\u6001\u3002")

    repair_effectiveness = summary.get("repair_effectiveness") if isinstance(summary.get("repair_effectiveness"), Mapping) else {}
    repair_success_rate = repair_effectiveness.get("success_rate")
    repair_effectiveness_summary = str(repair_effectiveness.get("summary") or "").strip()
    repair_evaluated_pairs = _coerce_positive_int(repair_effectiveness.get("evaluated_pairs"))
    recovered_focuses = _normalize_runtime_prompt_items(repair_effectiveness.get("recovered_focus_areas"), limit=2)
    unresolved_focuses = _normalize_runtime_prompt_items(repair_effectiveness.get("unresolved_focus_areas"), limit=2)
    if isinstance(repair_success_rate, (int, float)):
        pair_text = f"\uff08\u57fa\u4e8e {repair_evaluated_pairs} \u7ec4\u76f8\u90bb\u7ae0\u8282\uff09" if repair_evaluated_pairs else ""
        append_section(2, "repair_effectiveness_rate", f"- \u6700\u8fd1\u4fee\u590d\u6210\u6548\u7387\uff1a{float(repair_success_rate):.1f}%{pair_text}\u3002")
    if repair_effectiveness_summary:
        append_section(0, "repair_effectiveness_summary", f"- \u4fee\u590d\u6548\u679c\u5224\u65ad\uff1a{repair_effectiveness_summary}")
    if unresolved_focuses:
        append_section(1, "repair_unresolved_focuses", f"- \u4ecd\u672a\u7a33\u5b9a\u7684\u4fee\u590d\u7126\u70b9\uff1a{' / '.join(unresolved_focuses)}\u3002")
    elif recovered_focuses:
        append_section(1, "repair_recovered_focuses", f"- \u5df2\u7ecf\u5f00\u59cb\u56de\u6536\u7684\u4fee\u590d\u7126\u70b9\uff1a{' / '.join(recovered_focuses)}\u3002")

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

    final_line = "- \u751f\u6210\u65f6\u4f18\u5148\u4fee\u590d\u8d8b\u52bf\u4e2d\u6301\u7eed\u504f\u5f31\u7684\u9879\uff0c\u540c\u65f6\u4fdd\u7559\u5df2\u7ecf\u7a33\u5b9a\u6210\u7acb\u7684\u5f3a\u9879\u3002"
    if len(selected_lines) + 1 <= max_lines and total_chars + len(final_line) + 1 <= max_chars:
        selected_lines.append(final_line)
        total_chars += len(final_line) + 1
        selected_section_keys.append("final_instruction")

    folded_note = "- \u5176\u4f59\u6b21\u7ea7\u8d8b\u52bf\u7ec6\u9879\u5df2\u6298\u53e0\uff0c\u4f18\u5148\u6267\u884c\u4ee5\u4e0a\u5173\u952e\u4fe1\u53f7\u3002"
    if dropped_optional and len(selected_lines) + 1 <= max_lines and total_chars + len(folded_note) + 1 <= max_chars:
        selected_lines.append(folded_note)
        total_chars += len(folded_note) + 1
        selected_section_keys.append("folded_optional_note")

    if logger.isEnabledFor(10):
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


def _compact_prompt_text(value: Any) -> str:
    text = str(value or "").strip()
    if not text:
        return ""
    return re.sub(r"\n{3,}", "\n\n", text)


def _append_prompt_block(template: str, block: str, *, after_tag: Optional[str] = None) -> str:
    cleaned_block = _compact_prompt_text(block)
    if not cleaned_block:
        return template
    if cleaned_block in template:
        return template
    if after_tag and after_tag in template:
        return template.replace(after_tag, f"{after_tag}\n\n{cleaned_block}", 1)
    return f"{template.rstrip()}\n\n{cleaned_block}".strip()


def _extract_template_key_marker(template: str) -> Tuple[Optional[str], str]:
    if not template:
        return None, template
    match = QUALITY_TEMPLATE_MARKER_PATTERN.match(template)
    if not match:
        return None, template
    return match.group("key"), template[match.end():]


class WritingStyleManager:
    """写作风格管理器"""
    
    @staticmethod
    def apply_style_to_prompt(base_prompt: str, style_content: str) -> str:
        """
        将写作风格应用到基础提示词中
        
        Args:
            base_prompt: 基础提示词
            style_content: 风格要求内容
            
        Returns:
            组合后的提示词
        """
        style_profile = "default"
        normalized = (style_content or "").lower()
        if "连载感" in normalized:
            style_profile = "low_ai_serial"
        elif "生活化" in normalized:
            style_profile = "low_ai_life"
        elif "都市金融" in normalized or ("金融" in normalized and "商战" in normalized):
            style_profile = "urban_finance"
        elif "技术流修仙" in normalized or ("技术流" in normalized and "修仙" in normalized):
            style_profile = "tech_xianxia"
        elif "轻松幽默" in normalized or "幽默" in normalized:
            style_profile = "light_humor"
        elif "朴实年代" in normalized or "年代风" in normalized:
            style_profile = "era_plain"

        common_guard = (
            "写作执行要点："
            "你正在写长篇小说中段，不是开书导语，也不是全书终章。"
            "用中文母语者的自然表达写作，长短句穿插，读起来顺口。"
            "对话要像真人交流，少讲道理，多给反应和潜台词。"
            "出现设定术语时，尽量在场景中补一句通俗解释。"
            "比喻要克制：能直接写动作、表情、声音和即时结果，就不要先写抽象比喻。"
            "慎用高频定式句法，如“像……一样”“仿佛”“不是……而是……”“下一秒”“那一瞬”“忽然”。"
            "疼痛、恐惧和异常优先写身体反应、动作受阻、物件变化和现场声响，不要每次都靠意象包裹。"
            "允许保留少量朴素、直接、甚至略笨的过渡句，不要把每句话都打磨成有设计感的好句子。"
            "结尾禁止总结型/预告型/感悟型收束，优先停在动作、对话或突发事件上。"
            "直接输出章节正文，不要加章节标题和额外说明。"
        )

        serial_guard = (
            "连载强化要点："
            "保持追更节奏，中段给小波折，章末留自然未完感。"
            "人物情绪要有层次，不要开口就结论化表态。"
            "让配角有主动选择，避免只当信息传声筒。"
            "同一自然段尽量不要连续堆叠两个以上“像……”比喻；危险感先靠事件和反馈建立。"
        )

        life_guard = (
            "生活化强化要点："
            "优先用动作、表情和场景噪声传递情绪，别把解释写满。"
            "允许少量口语毛边，避免句句工整。"
            "少写漂亮空话和修辞连发，保留日常说话的停顿、改口和没那么圆的句子。"
        )

        urban_finance_guard = (
            "都市金融强化要点："
            "专业术语要落地到利益得失，避免术语堆砌。"
            "谈判和博弈要体现信息差与筹码变化，突出人物选择代价。"
        )

        tech_xianxia_guard = (
            "技术流修仙强化要点："
            "规则推演要清楚，但每段都要有行动反馈，避免连续讲义化解释。"
            "术法/阵法/功法术语出现后，尽量用角色互动补一句人话解释。"
        )

        light_humor_guard = (
            "轻松幽默强化要点："
            "笑点要服务剧情推进，不做连续段子堆叠。"
            "人物互怼要有立场差异，避免全员同口吻抖机灵。"
        )

        era_plain_guard = (
            "朴实年代强化要点："
            "时代细节要自然入戏，优先写可见的生活动作与人际压力。"
            "语言克制朴素，避免现代网络梗和悬浮金句。"
        )

        profile_guard = ""
        if style_profile == "low_ai_serial":
            profile_guard = serial_guard
        elif style_profile == "low_ai_life":
            profile_guard = life_guard
        elif style_profile == "urban_finance":
            profile_guard = urban_finance_guard
        elif style_profile == "tech_xianxia":
            profile_guard = tech_xianxia_guard
        elif style_profile == "light_humor":
            profile_guard = light_humor_guard
        elif style_profile == "era_plain":
            profile_guard = era_plain_guard

        # 在基础提示词末尾追加风格要求与执行护栏
        return f"{base_prompt}\n\n{style_content}\n\n{common_guard}\n{profile_guard}".strip()


class PromptService:
    """提示词模板管理"""
    
    # ========== V2版本提示词模板（RTCO框架）==========
    
    # 世界构建提示词 V2（RTCO框架）
    WORLD_BUILDING = """<system>
你是小说世界观策划搭档，擅长为{genre}类型作品搭出真实、可写、可推进剧情的世界底盘。
</system>

<task>
【设计任务】
为小说《{title}》构建完整的世界观设定。

【核心要求】
- 主题契合：世界观必须支撑主题"{theme}"
- 简介匹配：为简介中的情节提供合理背景
- 类型适配：符合{genre}类型的特征
- 规模适当：根据题材选择合适的设定尺度
- 表达自然：避免百科腔和教科书腔，尽量写成创作可直接使用的描述
</task>

<input priority="P0">
【项目信息】
书名：{title}
类型：{genre}
主题：{theme}
简介：{description}
</input>

<guidelines priority="P1">
【类型指导原则】

**现代都市/言情/青春**：
- 时间：当代社会（2020年代）或近未来（2030-2050年）
- 避免：大崩解、纪元、末日等宏大概念
- 重点：具体城市环境、职场文化、社会现状

**历史/古代**：
- 时间：明确的历史朝代或虚构古代
- 重点：时代特征、礼教制度、阶级分化

**玄幻/仙侠/修真**：
- 时间：修炼文明的特定时期
- 重点：修炼规则、灵气环境、门派势力

**科幻**：
- 时间：未来明确时期（如2150年、星际时代初期）
- 重点：科技水平、社会形态、文明转折

**奇幻/魔法**：
- 时间：魔法文明的特定阶段
- 重点：魔法体系、种族关系、大陆格局

**设定尺度控制**：
- 现代都市：聚焦某个城市、行业、阶层
- 校园青春：学校环境、学生生活、成长困境
- 职场言情：公司文化、行业特点、职业压力
- 史诗题材：才需要宏大的世界观架构
</guidelines>

<output priority="P0">
【输出格式】
生成包含以下四个字段的JSON对象，每个字段300-500字：

1. **time_period**（时间背景与社会状态）
   - 根据类型设定合适规模的时间背景
   - 现代题材：具体社会特征（如：2024年北京，互联网行业高速发展）
   - 历史题材：明确朝代和阶段（如：明朝嘉靖年间，海禁政策下的沿海）
   - 幻想题材：文明发展阶段，具体而非空泛
   - 阐明时代核心矛盾和社会焦虑

2. **location**（空间环境与地理特征）
   - 故事主要发生的空间环境
   - 现代题材：具体城市名或类型
   - 环境如何影响居民生存方式
   - 标志性场景描述

3. **atmosphere**（感官体验与情感基调）
   - 身临其境的感官细节（视觉、听觉、嗅觉）
   - 美学风格和色彩基调
   - 居民心理状态和情绪氛围
   - 与主题情感呼应

4. **rules**（世界规则与社会结构）
   - 世界运行的核心法则
   - 现代题材：社会规则、行业潜规则、人际法则
   - 幻想题材:力量体系、社会等级、资源分配
   - 权力结构和利益格局
   - 社会禁忌及后果

【格式规范】
- 纯JSON输出，以{{开始、}}结束
- 无markdown标记、代码块符号
- 字段值为完整段落文本
- 不使用特殊符号包裹内容
- 提供充实原创内容

【JSON示例】
{{
  "time_period": "时间背景与社会状态的详细描述（300-500字）",
  "location": "空间环境与地理特征的详细描述（300-500字）",
  "atmosphere": "感官体验与情感基调的详细描述（300-500字）",
  "rules": "世界规则与社会结构的详细描述（300-500字）"
}}
</output>

<constraints>
【必须遵守】
✅ 简介契合：为简介情节提供合理背景
✅ 类型适配：符合{genre}的特征
✅ 主题贴合：支撑主题"{theme}"
✅ 具象化：用具体细节而非空洞概念
✅ 逻辑自洽：所有设定相互支撑
✅ 叙述可写：给出的设定能直接转成场景、冲突和角色行动

【禁止事项】
❌ 生成与类型不匹配的设定
❌ 为小规模题材使用宏大世界观
❌ 使用模板化、空泛的表达
❌ 输出markdown或代码块标记
❌ 大量使用“首先/其次/最后”式说明文结构
</constraints>"""

    # 批量角色生成提示词 V2（RTCO框架）
    CHARACTERS_BATCH_GENERATION = """<system>
你是小说角色与势力设定搭档，擅长按{genre}题材做出能推动剧情的人物和组织。
</system>

<task>
【生成任务】
生成{count}个角色和组织实体。

【数量要求 - 严格遵守】
数组中必须精确包含{count}个对象，不多不少。

【实体类型分配】
- 至少1个主角（protagonist）
- 多个配角（supporting）
- 可包含反派（antagonist）
- 可包含1-2个高影响力组织（power_level: 70-95）

【写法要求】
- 设定信息要具体可用，避免空泛评价词
- 名字、称谓和关系表达贴近中文网文阅读习惯
- 角色与组织要为后续冲突和推进服务，不做摆设
</task>

<worldview priority="P0">
【世界观信息】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
世界规则：{rules}

主题：{theme}
类型：{genre}
</worldview>

<requirements priority="P1">
【特殊要求】
{requirements}
</requirements>

<output priority="P0">
【输出格式】
返回纯JSON数组，每个对象包含：

**角色对象**：
{{
  "name": "角色姓名",
  "age": 25,
  "gender": "男/女/其他",
  "is_organization": false,
  "role_type": "protagonist/supporting/antagonist",
  "personality": "性格特点（100-200字）：核心性格、优缺点、特殊习惯",
  "background": "背景故事（100-200字）：家庭背景、成长经历、重要转折",
  "appearance": "外貌描述（50-100字）：身高、体型、面容、着装风格",
  "traits": ["特长1", "特长2", "特长3"],
  "relationships_array": [
    {{
      "target_character_name": "已生成的角色名称",
      "relationship_type": "关系类型",
      "intimacy_level": 75,
      "description": "关系描述"
    }}
  ],
  "organization_memberships": [
    {{
      "organization_name": "已生成的组织名称",
      "position": "职位",
      "rank": 5,
      "loyalty": 80
    }}
  ]
}}

**组织对象**：
{{
  "name": "组织名称",
  "is_organization": true,
  "role_type": "supporting",
  "personality": "组织特性（100-200字）：运作方式、核心理念、行事风格",
  "background": "组织背景（100-200字）：建立历史、发展历程、重要事件",
  "appearance": "外在表现（50-100字）：总部位置、标志性建筑",
  "organization_type": "组织类型",
  "organization_purpose": "组织目的",
  "organization_members": ["成员1", "成员2"],
  "power_level": 85,
  "location": "所在地或主要活动区域",
  "motto": "组织格言、口号或宗旨",
  "color": "代表颜色",
  "traits": []
}}

【关系类型参考】
- 家族：父亲、母亲、兄弟、姐妹、子女、配偶、恋人
- 社交：师父、徒弟、朋友、同学、同事、邻居、知己
- 职业：上司、下属、合作伙伴
- 敌对：敌人、仇人、竞争对手、宿敌

【数值范围】
- intimacy_level：-100到100（负值表示敌对）
- loyalty：0到100
- rank：0到10（职位等级）
- power_level：70到95（组织影响力）
</output>

<constraints>
【必须遵守】
✅ 数量精确：数组必须包含{count}个对象
✅ 符合世界观：角色设定与世界观一致
✅ 有深度：性格和背景要立体
✅ 关系网络：角色间形成合理关系
✅ 组织合理：组织是推动剧情的关键力量

【关系约束】
✅ relationships_array只能引用本批次已出现的角色
✅ organization_memberships只能引用本批次的组织
✅ 第一个角色的relationships_array必须为空[]
✅ 禁止幻觉：不引用不存在的角色或组织

【格式约束】
✅ 纯JSON数组输出，无markdown标记
✅ 内容描述中严禁使用特殊符号（引号、方括号、书名号等）
✅ 专有名词直接书写，不使用符号包裹

【禁止事项】
❌ 生成数量不符（多于或少于{count}个）
❌ 引用不存在的角色或组织
❌ 生成低影响力的无关紧要组织
❌ 使用markdown或代码块标记
❌ 在描述中使用特殊符号
❌ 用“总之/综上/值得注意的是”这类模板化总结句灌水
</constraints>"""

    # 大纲生成提示词 V2（RTCO框架）
    OUTLINE_CREATE = """<system>
你是小说大纲搭档，擅长为{genre}类型作品设计有钩子的开篇。
</system>

<task>
【创作任务】
为小说《{title}》生成开篇{chapter_count}章的大纲。

【重要说明】
这是项目初始化的开头部分，不是完整大纲：
- 完成开局设定和世界观展示
- 引入主要角色，建立初始关系
- 埋下核心矛盾和悬念钩子
- 为后续剧情发展打下基础
- 不需要完整闭环，为续写留空间

【风格要求】
- 用中文小说读者熟悉的叙事表达，不写公文式说明
- 每章summary尽量“可视化”：有场景、有动作、有冲突触发点
- 避免空泛评价词，优先给可落地的情节信息
- 开篇前150字尽量进入异常变化、硬任务压力或正面冲突，不平铺背景
- 黄金三章分工（若包含第1-3章）：第1章“钩子+主角处境+首个冲突”，第2章“升级变化+必要设定+后续期待”，第3章“首次小高潮+阶段爽点+章尾钩子”
- 每章至少配置1个小爽点（反转/收获/打脸/突破其一）和1个章尾钩子，爽点尽量体现“铺垫→爆发→反馈”
- 每章summary尽量隐含“开场钩子→对抗推进→小爆发→章尾卡点”四拍结构，至少落地其中三拍
- 前10万字对应阶段避免大段背景直给，设定信息跟随冲突推进逐步释放
- 每章至少体现一次“目标受阻→角色决策→即时后果”链条
- 每章至少安排一个可直接写对白的交锋场面（两人及以上，立场不完全一致）
- 世界规则要落地到事件层：至少1个关键事件必须体现规则如何限制或放大角色选择
- 章尾钩子尽量轮换类型（信息缺口/危险临门/身份反转/选择悬而未决），避免连续同型卡点
- 本模板只负责产出章节规划内容，不输出调度执行说明或方案对比结论
</task>

<project priority="P0">
【项目信息】
书名：{title}
主题：{theme}
类型：{genre}
开篇章节数：{chapter_count}
叙事视角：{narrative_perspective}
</project>

<worldview priority="P1">
【世界观】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
世界规则：{rules}
</worldview>

<characters priority="P1">
【角色信息】
{characters_info}
</characters>

<mcp_context priority="P2">
{mcp_references}
</mcp_context>

<requirements priority="P1">
【其他要求】
{requirements}
</requirements>

<fusion_contract priority="P0">
【第三版融合约束（调度边界）】
- 你是内容生成者，不是调度器、评审员或流程记录员
- 禁止输出“执行X.X”“调用Agent”“方案对比/总结”这类调度话术
- 禁止输出流程日志、步骤说明、元叙事解释
- 当信息不足时，先给最小可执行方案：角色目标→阻力→选择→即时后果
</fusion_contract>

<output priority="P0">
【输出格式】
返回包含{chapter_count}个章节对象的JSON数组：

[
  {{
   "chapter_number": 1,
   "title": "章节标题",
   "summary": "章节概要（500-1000字）：主要情节、角色互动、关键事件、冲突与转折",
   "scenes": ["场景1描述", "场景2描述", "场景3描述"],
   "characters": [
     {{"name": "角色名1", "type": "character"}},
     {{"name": "组织/势力名1", "type": "organization"}}
   ],
   "key_points": ["情节要点1", "情节要点2"],
   "emotion": "本章情感基调",
   "goal": "本章叙事目标"
 }},
 {{
   "chapter_number": 2,
   "title": "章节标题",
   "summary": "章节概要...",
   "scenes": ["场景1", "场景2"],
   "characters": [
     {{"name": "角色名2", "type": "character"}},
     {{"name": "组织名2", "type": "organization"}}
   ],
   "key_points": ["要点1", "要点2"],
   "emotion": "情感基调",
   "goal": "叙事目标"
 }}
]

【characters字段说明】
- type为"character"表示个人角色，type为"organization"表示组织/势力/门派/帮派等
- 必须区分角色和组织，不要把组织当作角色

【格式规范】
- 纯JSON数组输出，无markdown标记
- 内容描述中严禁使用特殊符号
- 专有名词直接书写
- 字段结构与已有章节完全一致
</output>

<constraints>
【开篇大纲要求】
✅ 开局设定：前几章完成世界观呈现、主角登场、初始状态
✅ 矛盾引入：引出核心冲突，但不急于展开
✅ 角色亮相：主要角色依次登场，展示性格和关系
✅ 节奏控制：开篇不宜过快，给读者适应时间
✅ 悬念设置：埋下伏笔和钩子，为续写预留空间
✅ 视角统一：采用{narrative_perspective}视角
✅ 留白艺术：结尾不收束过紧，留发展空间
✅ 冲突有压强：每章都有清晰阻力，不让情节平推
✅ 规则有作用：关键事件中体现世界规则对行动成本/风险的影响
✅ 黄金三章：若本次覆盖第1-3章，分别承担“钩子/升级/高潮”职责
✅ 爽点密度：每章至少1个小爽点，且有明确触发与反馈
✅ 章尾卡点：每章结尾给出可追读的动作指向或悬念钩子

【必须遵守】
✅ 数量精确：数组包含{chapter_count}个章节对象
✅ 符合类型：情节符合{genre}类型特征
✅ 主题贴合：体现主题"{theme}"
✅ 开篇定位：是开局而非完整故事
✅ 描述详细：每个summary 500-1000字
✅ summary可落地：读完能直接用于后续写章
✅ 调度边界：只输出章节规划内容，不输出调度说明或执行日志
✅ 缺口降级：信息不足时仍保持“目标→阻力→选择→后果”链条完整
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出markdown或代码块标记
❌ 在描述中使用特殊符号
❌ 试图在开篇完结故事
❌ 节奏过快，信息过载
❌ 连续大段背景直给，导致冲突滞后
❌ 使用“总之/综上/值得注意的是/在这个过程中”等模板连接词
❌ 章节只有信息介绍，没有目标冲突和决策后果
❌ 使用调度器口吻（如“执行1.1/调用Agent/方案对比”）
❌ 把输出写成流程文档或复盘报告
</constraints>"""

    # 拆书导入-反向项目提炼
    BOOK_IMPORT_REVERSE_PROJECT_SUGGESTION = """<system>
你是资深网文策划编辑，擅长从小说正文中反向提炼项目立项信息。
</system>

<task>
基于给定正文片段，提炼项目建议信息。

输出必须是 JSON 对象，且仅包含以下字段：
- title: 书名
- description: 项目简介
- theme: 核心主题
- genre: 小说类型
- narrative_perspective: 叙事视角（第一人称 / 第三人称 / 全知视角）
- target_words: 目标总字数（整数）
</task>

<input>
书名：{title}
正文片段：
{sampled_text}
</input>

<constraints>
- 仅输出 JSON
- 不要输出 markdown 或解释
- 信息必须尽量基于正文，不要无依据扩写
</constraints>"""

    # 拆书导入-反向章节大纲
    BOOK_IMPORT_REVERSE_OUTLINES = """<system>
你是资深网文总编与剧情策划，擅长基于已完成章节反向提炼标准化章节大纲。
</system>

<task>
根据给定章节正文，输出与系统章节大纲结构兼容的 JSON 数组。

每个元素必须包含：
- chapter_number
- title
- summary
- scenes
- characters
- key_points
- emotion
- goal
</task>

<input>
书名：{title}
类型：{genre}
主题：{theme}
叙事视角：{narrative_perspective}
章节范围：第{start_chapter}章 - 第{end_chapter}章
正文：
{chapters_text}
</input>

<constraints>
- 仅输出 JSON 数组
- 数组长度必须等于 {expected_count}
- 不要输出 markdown 或解释
</constraints>"""
    
    # 大纲续写提示词 V2（RTCO框架 - 简化版）
    OUTLINE_CONTINUE = """<system>
你是小说续写大纲搭档，擅长在不跑偏的前提下推进{genre}类型剧情。
</system>

<task>
【续写任务】
基于已有{current_chapter_count}章内容，续写第{start_chapter}章到第{end_chapter}章的大纲（共{chapter_count}章）。

【当前情节阶段】
{plot_stage_instruction}

【故事发展方向】
{story_direction}

【风格要求】
- 续写语言保持中文小说叙事习惯，不写说明书式大纲
- 每章summary优先写“发生了什么”，而不是“要表达什么”
- 同时给出冲突推进与情绪变化，避免流水账
- 续写章节开场尽量在150字内切入动作、异常变化或高压任务，不平铺回顾
- 每章至少配置1个小爽点（反转/收获/打脸/突破其一）和1个章尾钩子，爽点尽量体现“铺垫→爆发→反馈”
- 每章summary尽量形成“开场钩子→对抗推进→小爆发→章尾卡点”四拍中的至少三拍
- 前10万字对应阶段避免大段背景直给，设定信息跟随冲突推进逐步释放
- 每章至少体现一次“目标受阻→角色决策→即时后果”链条
- 每章至少安排一个可直接写对白的交锋场面（两人及以上，立场不完全一致）
- 世界规则要落地到事件层：至少1个关键事件必须体现规则如何限制或放大角色选择
- 章尾钩子尽量轮换类型（信息缺口/危险临门/身份反转/选择悬而未决），避免连续同型卡点
- 本模板只负责产出章节规划内容，不输出调度执行说明或方案对比结论
</task>

<project priority="P0">
【项目信息】
书名：{title}
主题：{theme}
类型：{genre}
叙事视角：{narrative_perspective}
</project>

<worldview priority="P1">
【世界观】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
世界规则：{rules}
</worldview>

<previous_context priority="P0">
{recent_outlines}
</previous_context>

<characters priority="P0">
【所有角色信息】
{characters_info}
</characters>

<user_input priority="P0">
【用户输入】
续写章节数：{chapter_count}章
情节阶段：{plot_stage_instruction}
故事方向：{story_direction}
其他要求：{requirements}
</user_input>

<fusion_contract priority="P0">
【第三版融合约束（大纲续写）】
- 你是内容生成者，不是调度器、评审员或流程记录员
- 禁止输出“执行X.X”“调用Agent”“方案对比/总结”这类调度话术
- 禁止输出流程日志、步骤说明、元叙事解释
- 当信息不足时，先给最小可执行方案：角色目标→阻力→选择→即时后果
</fusion_contract>

<mcp_context priority="P2">
【MCP工具参考】
{mcp_references}

【MCP使用原则】
- 只吸收与当前章节规划直接相关的事实、灵感或行业认知，不照搬来源原句
- MCP参考仅作补充，不覆盖项目信息、既定设定、角色关系和大纲边界
- 若MCP内容与项目上下文冲突，以项目上下文为准
- 不要在输出里暴露“根据MCP/工具显示”等来源说明
</mcp_context>

<output priority="P0">
【输出格式】
返回第{start_chapter}到第{end_chapter}章的JSON数组（共{chapter_count}个对象）：

[
  {{
   "chapter_number": {start_chapter},
   "title": "章节标题",
   "summary": "章节概要（500-1000字）：主要情节、角色互动、关键事件、冲突与转折",
   "scenes": ["场景1描述", "场景2描述", "场景3描述"],
   "characters": [
     {{"name": "角色名1", "type": "character"}},
     {{"name": "组织/势力名1", "type": "organization"}}
   ],
   "key_points": ["情节要点1", "情节要点2"],
   "emotion": "本章情感基调",
   "goal": "本章叙事目标"
 }},
 {{
   "chapter_number": {start_chapter} + 1,
   "title": "章节标题",
   "summary": "章节概要...",
   "scenes": ["场景1", "场景2"],
   "characters": [
     {{"name": "角色名2", "type": "character"}},
     {{"name": "组织名2", "type": "organization"}}
   ],
   "key_points": ["要点1", "要点2"],
   "emotion": "情感基调",
   "goal": "叙事目标"
 }}
]

【characters字段说明】
- type为"character"表示个人角色，type为"organization"表示组织/势力/门派/帮派等
- 必须区分角色和组织，不要把组织当作角色

【格式规范】
- 纯JSON数组输出，无markdown标记
- 内容描述中严禁使用特殊符号
- 专有名词直接书写
- 字段结构与已有章节完全一致
</output>

<constraints>
【续写要求】
✅ 剧情连贯：与前文自然衔接，保持连贯性
✅ 角色发展：遵循角色成长轨迹，充分利用角色信息
✅ 情节阶段：遵循{plot_stage_instruction}的要求
✅ 风格一致：保持与已有章节相同风格和详细程度
✅ 大纲详细：充分解析最近10章大纲的structure字段信息
✅ 冲突升级：每章有新的阻力或风险上升，不做平铺推进
✅ 规则落地：关键事件体现世界规则的约束、代价或红利
✅ 爽点密度：每章至少1个小爽点，避免平推叙事
✅ 章尾卡点：每章结尾保留追读钩子

【必须遵守】
✅ 数量精确：数组包含{chapter_count}个章节
✅ 编号正确：从第{start_chapter}章开始
✅ 描述详细：每个summary 500-1000字
✅ 承上启下：自然衔接前文
✅ 情节可执行：summary能直接转成章节正文
✅ 调度边界：只输出章节规划内容，不输出调度说明或执行日志
✅ 缺口降级：信息不足时仍保持“目标→阻力→选择→后果”链条完整
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出markdown或代码块标记
❌ 在描述中使用特殊符号
❌ 与前文矛盾或脱节
❌ 忽略已有角色发展
❌ 忽略最近大纲中的情节线索
❌ 连续大段背景直给，导致推进迟滞
❌ 使用“总之/综上/值得注意的是/在这个过程中”等模板连接词
❌ 把世界规则当背景板，不作用于角色选择与冲突结果
❌ 使用调度器口吻（如“执行1.1/调用Agent/方案对比”）
❌ 把输出写成流程文档或复盘报告
</constraints>"""
    
    # 章节生成 - 1-N模式（第1章）
    CHAPTER_GENERATION_ONE_TO_MANY = """<system>
你正在创作《{project_title}》，请按{genre}网文读者习惯写作，语言自然、利落、有画面。
你写的是长篇小说的中段内容，必须承前启后，禁止写成导语或终章总结。
</system>

<task>
【创作任务】
撰写第{chapter_number}章《{chapter_title}》的完整正文。

【基本要求】
- 目标字数：{target_word_count}字（允许±200字浮动）
- 叙事视角：{narrative_perspective}

【小说风格要求】
- 叙述贴近中文网文阅读习惯，少官话、少说明腔
- 句式有长短变化，避免连续同构句
- 情绪优先通过动作、对话和场景细节呈现
- 比喻要省着用：同一自然段尽量不连续堆叠两个以上“像……/仿佛/像……一样”，能直写就直写
- 对话要有人物性格，不要写成解释型对白
- 对话口吻要像真人交流：短句优先，允许打断、停顿、潜台词，避免“完整书面句”连发
- 信息密度要可读：单段连续出现2个及以上术语时，需在后续短对话或互动中补一句通俗解释
- 情节推进尽量遵循“动作→反馈→后果”链条，关键段落避免只做概述
- 单章任务要聚焦：优先只承担1个主功能（推进主线/揭示信息/关系转折/兑现回报其一），副功能最多1个，避免平均铺线
- 视角要守规矩：严格贴合{narrative_perspective}，非必要不切入他人内心，不用上帝视角替人物下结论
- 场景切换要有锚点：切换时间、地点或行动阶段时，用简短动作或环境变化交代原因，避免镜头漂移
- 每章至少出现一次“目标受阻→角色选择→代价/新麻烦”戏剧链条
- 世界设定必须入戏：至少1个关键情节点体现世界规则如何影响行动结果
- 角色刻画要立体：主角和核心配角都要呈现外在行为与内在动机/情绪落差
- 配角需有记忆点：若本章有足够戏份的核心配角，优先让其做出一次影响局势的独立选择；动机可通过动作、对白或即时反应侧写，不强求硬解释
- 快节奏开场：前300字内给出冲突触发、异常变化或任务阻力，不做长前情导入
- 场景书写尽量遵循“环境+动作+心理+对话”组合，避免纯说明段
- 疼痛、惊惧和异常优先写动作后果、身体反应和现场变化，不要只靠抽象意象撑气氛
- 优先提供1个可感知回报（反转/收获/打脸/突破/认知刷新其一）；若本章承担压抑或过桥功能，可改为“局势升级/关系位移/真相推进”的有效回报
- 章尾留钩子：结尾给出未完成动作、新风险或悬念问题，支撑追读
- 第1章职责明确：完成“钩子+主角处境+首个冲突”三件事
- 仅输出正文内容，不输出流程说明、调度说明、创作解释
</task>

<outline priority="P0">
【本章大纲 - 作为主线】
{chapter_outline}
</outline>

<worldview priority="P1">
【世界观锚点 - 必须落地到事件】
时间背景：{world_time_period}
地理位置：{world_location}
氛围基调：{world_atmosphere}
世界规则：{world_rules}
</worldview>

<characters priority="P1">
【本章角色 - 保持人设稳定】
{characters_info}

⚠️ 角色互动须知：
- 角色之间的对话和行为必须符合其关系设定（如师徒、敌对等）
- 涉及组织的情节须体现角色在组织中的身份和职位
- 角色的能力表现须符合其职业和阶段设定
</characters>

<careers priority="P2">
【本章职业】
{chapter_careers}
</careers>

<foreshadow_reminders priority="P2">
【🎯 伏笔提醒】
{foreshadow_reminders}
</foreshadow_reminders>

<memory priority="P2">
【相关记忆】
{relevant_memories}
</memory>

<fusion_contract priority="P0">
【第三版融合约束（正文生成）】
- 信息冲突时按优先级处理：本章大纲 > 上下文锚点 > 相关记忆 > 风格补充
- 严禁输出“我将/我认为/本章将会”等过程化说明
- 若信息不足，使用最小动作闭环补足，不写空泛总结句
</fusion_contract>

<constraints>
【必须遵守】
✅ 按大纲推进情节
✅ 保持角色性格、说话方式一致
✅ 角色互动须符合关系设定（师徒、朋友、敌对等）
✅ 组织相关情节须体现成员身份和职位层级
✅ 字数控制在目标范围内
✅ 如有伏笔提醒，请在本章中适当埋入或回收相应伏笔
✅ 文风自然：用词贴近日常中文表达，不写模板腔
✅ 比喻不过载：能直接写动作与结果就不要先写“像……”，单段强比喻尽量控制在1个
✅ 句法去模板：慎用“下一秒/那一瞬/忽然/不是……而是……”等固定推进句，避免连续出现
✅ 表达有起伏：长短句交替，段落节奏有变化
✅ 术语可理解：单段连续出现2个及以上术语时，需在3句内补一句生活化解释（可用角色问答完成）
✅ 剧情可感知：关键情节至少写出动作、人物反应和即时结果三要素
✅ 单章聚焦：优先完成1个主任务，可兼带1个副任务，避免把推进、解释、回收、抒情平均摊开
✅ 视角稳定：严格贴合{narrative_perspective}，非必要不切入他人内心，不替角色提前下全知结论
✅ 场景有锚点：切换时间、地点或行动阶段时，要有简短承接动作或环境变化
✅ 快节奏开场：前300字内出现冲突触发或异常变化
✅ 章内有节拍：正文尽量形成“开场钩子→对抗推进→小爆发→余波/钩子”四拍，至少落地其中三拍
✅ 戏剧有压强：至少出现一次“目标受阻→角色选择→代价/新麻烦”
✅ 世界规则入戏：至少1个关键情节点体现规则如何限制、放大或扭转角色选择
✅ 配角有自主性：若核心配角戏份充足，应让其做出独立选择并影响局势，不只附和主角
✅ 章尾有钩子：章节最后保留追读牵引点，不平收
✅ 钩子有变化：优先在“信息缺口/危险临门/身份反转/选择悬而未决”中轮换，避免连续同型卡点
✅ 对话有人味：至少1段双向博弈式对白，体现人物立场差异而非轮流讲解设定
✅ 人物不扁平：主角与至少1名核心配角都要有细节化表现（语气、动作习惯、选择代价）
✅ 回报可感知：优先给出1个有效回报（反转/收获/打脸/突破/认知刷新其一）；压抑段可用局势升级、关系位移或真相推进替代
✅ 调度隔离：正文中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：若信息缺口明显，优先保住“目标→阻力→选择→后果”而非堆设定
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出章节标题、序号等元信息
❌ 避免使用“总之”“综上所述”等总结腔
❌ 在结尾处使用开放式反问
❌ 用“他明白了/他意识到/命运将会”等感悟或预告句收尾
❌ 用全知视角总结人物命运或下章走向
❌ 无缘由切入多个角色内心，或替人物提前解释真实想法，造成视角漂移
❌ 添加作者注释或创作说明
❌ 角色行为超出其职业阶段的能力范围
❌ 堆叠“与此同时、值得注意的是、在这个过程中”等模板连接词
❌ 连续使用说明式句子（如“这意味着…”“他知道…”）
❌ 高密度重复“像……/仿佛/像……一样”比喻，把疼痛、危险和异常都写成同一类修辞
❌ 高频使用“下一秒/那一瞬/忽然/不是……而是……”等定式句法，导致全文腔调过稳
❌ 句句都像金句、漂亮句，没有朴素过渡和直写动作
❌ 连续抛术语但不解释，造成阅读门槛陡增
❌ 整段只讲设定或背景，没有可视化动作推进
❌ 场景切换不交代时间、地点或触发动作，导致读者失去空间感
❌ 超过两段连续背景说明且无冲突推进
❌ 把角色写成只负责报信息的工具人
❌ 一章同时塞满推进、解释、回收、抒情、立人等多重任务，导致主任务失焦
❌ 配角只有附和与执行，没有独立选择或情绪立场
❌ 大段解释型对白（角色轮流讲设定、讲道理）
❌ 对话全部语法完整且长度整齐，读起来像书面报告
❌ 世界设定只停留在名词陈列，不影响事件后果
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”等调度文本
❌ 在正文里插入创作复盘、自评或面向用户的说明句
</constraints>

<output>
【输出规范】
直接输出小说正文内容，从故事场景或动作开始。
不要加前言、后记或解释说明。

现在开始创作：
</output>"""

    # 章节生成 - 1-1模式（第1章）
    CHAPTER_GENERATION_ONE_TO_ONE = """<system>
你正在创作《{project_title}》，请按{genre}网文读者习惯写作，语言自然、利落、有画面。
你写的是长篇小说的中段内容，必须承前启后，禁止写成导语或终章总结。
</system>

<task priority="P0">
【创作任务】
撰写第{chapter_number}章《{chapter_title}》的完整正文。

【基本要求】
- 目标字数：{target_word_count}字（允许±200字浮动）
- 叙事视角：{narrative_perspective}

【小说风格要求】
- 叙述贴近中文网文阅读习惯，少官话、少说明腔
- 句式有长短变化，避免连续同构句
- 情绪优先通过动作、对话和场景细节呈现
- 比喻要省着用：同一自然段尽量不连续堆叠两个以上“像……/仿佛/像……一样”，能直写就直写
- 对话要有人物性格，不要写成解释型对白
- 对话口吻要像真人交流：短句优先，允许打断、停顿、潜台词，避免“完整书面句”连发
- 信息密度要可读：单段连续出现2个及以上术语时，需在后续短对话或互动中补一句通俗解释
- 情节推进尽量遵循“动作→反馈→后果”链条，关键段落避免只做概述
- 单章任务要聚焦：优先只承担1个主功能（推进主线/揭示信息/关系转折/兑现回报其一），副功能最多1个，避免平均铺线
- 视角要守规矩：严格贴合{narrative_perspective}，非必要不切入他人内心，不用上帝视角替人物下结论
- 场景切换要有锚点：切换时间、地点或行动阶段时，用简短动作或环境变化交代原因，避免镜头漂移
- 每章至少出现一次“目标受阻→角色选择→代价/新麻烦”戏剧链条
- 世界设定必须入戏：至少1个关键情节点体现世界规则如何影响行动结果
- 角色刻画要立体：主角和核心配角都要呈现外在行为与内在动机/情绪落差
- 配角需有记忆点：若本章有足够戏份的核心配角，优先让其做出一次影响局势的独立选择；动机可通过动作、对白或即时反应侧写，不强求硬解释
- 快节奏开场：前300字内给出冲突触发、异常变化或任务阻力，不做长前情导入
- 场景书写尽量遵循“环境+动作+心理+对话”组合，避免纯说明段
- 疼痛、惊惧和异常优先写动作后果、身体反应和现场变化，不要只靠抽象意象撑气氛
- 优先提供1个可感知回报（反转/收获/打脸/突破/认知刷新其一）；若本章承担压抑或过桥功能，可改为“局势升级/关系位移/真相推进”的有效回报
- 章尾留钩子：结尾给出未完成动作、新风险或悬念问题，支撑追读
- 第1章职责明确：完成“钩子+主角处境+首个冲突”三件事
- 仅输出正文内容，不输出流程说明、调度说明、创作解释
</task>

<outline priority="P0">
【本章大纲】
{chapter_outline}
</outline>

<worldview priority="P1">
【世界观锚点 - 必须落地到事件】
时间背景：{world_time_period}
地理位置：{world_location}
氛围基调：{world_atmosphere}
世界规则：{world_rules}
</worldview>

<characters priority="P1">
【本章角色】
{characters_info}
</characters>

<careers priority="P2">
【本章职业】
{chapter_careers}
</careers>

<foreshadow_reminders priority="P2">
【🎯 伏笔提醒】
{foreshadow_reminders}
</foreshadow_reminders>

<memory priority="P2">
【相关记忆】
{relevant_memories}
</memory>

<fusion_contract priority="P0">
【第三版融合约束（正文生成）】
- 信息冲突时按优先级处理：本章大纲 > 上下文锚点 > 相关记忆 > 风格补充
- 严禁输出“我将/我认为/本章将会”等过程化说明
- 若信息不足，使用最小动作闭环补足，不写空泛总结句
</fusion_contract>

<constraints>
【必须遵守】
✅ 按大纲推进情节
✅ 保持角色性格、说话方式一致
✅ 字数尽量贴近目标字数（允许小幅波动）
✅ 如有伏笔提醒，请在本章中适当埋入或回收相应伏笔
✅ 文风自然：用词贴近日常中文表达，不写模板腔
✅ 比喻不过载：能直接写动作与结果就不要先写“像……”，单段强比喻尽量控制在1个
✅ 句法去模板：慎用“下一秒/那一瞬/忽然/不是……而是……”等固定推进句，避免连续出现
✅ 表达有起伏：长短句交替，段落节奏有变化
✅ 术语可理解：单段连续出现2个及以上术语时，需在3句内补一句生活化解释（可用角色问答完成）
✅ 剧情可感知：关键情节至少写出动作、人物反应和即时结果三要素
✅ 单章聚焦：优先完成1个主任务，可兼带1个副任务，避免把推进、解释、回收、抒情平均摊开
✅ 视角稳定：严格贴合{narrative_perspective}，非必要不切入他人内心，不替角色提前下全知结论
✅ 场景有锚点：切换时间、地点或行动阶段时，要有简短承接动作或环境变化
✅ 快节奏开场：前300字内出现冲突触发或异常变化
✅ 章内有节拍：正文尽量形成“开场钩子→对抗推进→小爆发→余波/钩子”四拍，至少落地其中三拍
✅ 戏剧有压强：至少出现一次“目标受阻→角色选择→代价/新麻烦”
✅ 世界规则入戏：至少1个关键情节点体现规则如何限制、放大或扭转角色选择
✅ 配角有自主性：若核心配角戏份充足，应让其做出独立选择并影响局势，不只附和主角
✅ 章尾有钩子：章节最后保留追读牵引点，不平收
✅ 钩子有变化：优先在“信息缺口/危险临门/身份反转/选择悬而未决”中轮换，避免连续同型卡点
✅ 对话有人味：至少1段双向博弈式对白，体现人物立场差异而非轮流讲解设定
✅ 人物不扁平：主角与至少1名核心配角都要有细节化表现（语气、动作习惯、选择代价）
✅ 回报可感知：优先给出1个有效回报（反转/收获/打脸/突破/认知刷新其一）；压抑段可用局势升级、关系位移或真相推进替代
✅ 调度隔离：正文中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：若信息缺口明显，优先保住“目标→阻力→选择→后果”而非堆设定
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出章节标题、序号等元信息
❌ 避免使用“总之”“综上所述”等总结腔
❌ 用“他明白了/他意识到/命运将会”等感悟或预告句收尾
❌ 用全知视角总结人物命运或下章走向
❌ 无缘由切入多个角色内心，或替人物提前解释真实想法，造成视角漂移
❌ 添加作者注释或创作说明
❌ 不要明显超出目标字数
❌ 堆叠“与此同时、值得注意的是、在这个过程中”等模板连接词
❌ 连续使用说明式句子（如“这意味着…”“他知道…”）
❌ 高密度重复“像……/仿佛/像……一样”比喻，把疼痛、危险和异常都写成同一类修辞
❌ 高频使用“下一秒/那一瞬/忽然/不是……而是……”等定式句法，导致全文腔调过稳
❌ 句句都像金句、漂亮句，没有朴素过渡和直写动作
❌ 连续抛术语但不解释，造成阅读门槛陡增
❌ 整段只讲设定或背景，没有可视化动作推进
❌ 场景切换不交代时间、地点或触发动作，导致读者失去空间感
❌ 超过两段连续背景说明且无冲突推进
❌ 把角色写成只负责报信息的工具人
❌ 一章同时塞满推进、解释、回收、抒情、立人等多重任务，导致主任务失焦
❌ 配角只有附和与执行，没有独立选择或情绪立场
❌ 大段解释型对白（角色轮流讲设定、讲道理）
❌ 对话全部语法完整且长度整齐，读起来像书面报告
❌ 世界设定只停留在名词陈列，不影响事件后果
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”等调度文本
❌ 在正文里插入创作复盘、自评或面向用户的说明句
</constraints>

<output>
【输出规范】
直接输出小说正文内容，从故事场景或动作开始。
不要加前言、后记或解释说明。

现在开始创作：
</output>"""

    # 章节生成 - 1-1模式（第2章及以后）
    CHAPTER_GENERATION_ONE_TO_ONE_NEXT = """<system>
你正在创作《{project_title}》，请按{genre}网文读者习惯写作，语言自然、利落、有画面。
你写的是长篇小说的中段内容，必须承前启后，禁止写成导语或终章总结。
</system>

<task priority="P0">
【创作任务】
撰写第{chapter_number}章《{chapter_title}》的完整正文。

【基本要求】
- 目标字数：{target_word_count}字（允许±200字浮动）
- 叙事视角：{narrative_perspective}

【小说风格要求】
- 叙述贴近中文网文阅读习惯，少官话、少说明腔
- 句式有长短变化，避免连续同构句
- 情绪优先通过动作、对话和场景细节呈现
- 比喻要省着用：同一自然段尽量不连续堆叠两个以上“像……/仿佛/像……一样”，能直写就直写
- 对话要有人物性格，不要写成解释型对白
- 对话口吻要像真人交流：短句优先，允许打断、停顿、潜台词，避免“完整书面句”连发
- 信息密度要可读：单段连续出现2个及以上术语时，需在后续短对话或互动中补一句通俗解释
- 情节推进尽量遵循“动作→反馈→后果”链条，关键段落避免只做概述
- 单章任务要聚焦：优先只承担1个主功能（推进主线/揭示信息/关系转折/兑现回报其一），副功能最多1个，避免平均铺线
- 视角要守规矩：严格贴合{narrative_perspective}，非必要不切入他人内心，不用上帝视角替人物下结论
- 场景切换要有锚点：切换时间、地点或行动阶段时，用简短动作或环境变化交代原因，避免镜头漂移
- 每章至少出现一次“目标受阻→角色选择→代价/新麻烦”戏剧链条
- 世界设定必须入戏：至少1个关键情节点体现世界规则如何影响行动结果
- 角色刻画要立体：主角和核心配角都要呈现外在行为与内在动机/情绪落差
- 配角需有记忆点：若本章有足够戏份的核心配角，优先让其做出一次影响局势的独立选择；动机可通过动作、对白或即时反应侧写，不强求硬解释
- 快节奏开场：前300字内给出冲突触发、异常变化或任务阻力，不做长前情导入
- 场景书写尽量遵循“环境+动作+心理+对话”组合，避免纯说明段
- 疼痛、惊惧和异常优先写动作后果、身体反应和现场变化，不要只靠抽象意象撑气氛
- 优先提供1个可感知回报（反转/收获/打脸/突破/认知刷新其一）；若本章承担压抑或过桥功能，可改为“局势升级/关系位移/真相推进”的有效回报
- 章尾留钩子：结尾给出未完成动作、新风险或悬念问题，支撑追读
- 若chapter_number为2或3，尽量遵循黄金三章分工：第2章“升级+信息+期待”，第3章“小高潮+爽点+钩子”
- 仅输出正文内容，不输出流程说明、调度说明、创作解释
</task>

<outline priority="P0">
【本章大纲】
{chapter_outline}
</outline>

<worldview priority="P1">
【世界观锚点 - 必须落地到事件】
时间背景：{world_time_period}
地理位置：{world_location}
氛围基调：{world_atmosphere}
世界规则：{world_rules}
</worldview>

<previous_chapter_summary priority="P1">
【上一章剧情概要】
{previous_chapter_summary}
</previous_chapter_summary>

<previous_chapter priority="P1">
【上一章末尾500字内容】
{previous_chapter_content}
</previous_chapter>

<characters priority="P1">
【本章角色】
{characters_info}
</characters>

<careers priority="P2">
【本章职业】
{chapter_careers}
</careers>

<foreshadow_reminders priority="P2">
【🎯 伏笔提醒】
{foreshadow_reminders}
</foreshadow_reminders>

<memory priority="P2">
【相关记忆】
{relevant_memories}
</memory>

<fusion_contract priority="P0">
【第三版融合约束（正文续写）】
- 信息冲突时按优先级处理：本章大纲 > 上章末尾内容 > 上章剧情概要 > 相关记忆
- 严禁输出“我将/我认为/本章将会”等过程化说明
- 若信息不足，使用最小动作闭环补足，不写空泛总结句
</fusion_contract>

<constraints>
【必须遵守】
✅ 按大纲推进情节
✅ 自然承接上一章末尾内容，保持连贯性
✅ 保持角色性格、说话方式一致
✅ 字数尽量贴近目标字数（允许小幅波动）
✅ 如有伏笔提醒，请在本章中适当埋入或回收相应伏笔
✅ 文风自然：用词贴近日常中文表达，不写模板腔
✅ 比喻不过载：能直接写动作与结果就不要先写“像……”，单段强比喻尽量控制在1个
✅ 句法去模板：慎用“下一秒/那一瞬/忽然/不是……而是……”等固定推进句，避免连续出现
✅ 表达有起伏：长短句交替，段落节奏有变化
✅ 术语可理解：单段连续出现2个及以上术语时，需在3句内补一句生活化解释（可用角色问答完成）
✅ 剧情可感知：关键情节至少写出动作、人物反应和即时结果三要素
✅ 单章聚焦：优先完成1个主任务，可兼带1个副任务，避免把推进、解释、回收、抒情平均摊开
✅ 视角稳定：严格贴合{narrative_perspective}，非必要不切入他人内心，不替角色提前下全知结论
✅ 场景有锚点：切换时间、地点或行动阶段时，要有简短承接动作或环境变化
✅ 快节奏开场：前300字内出现冲突触发或异常变化
✅ 章内有节拍：正文尽量形成“开场钩子→对抗推进→小爆发→余波/钩子”四拍，至少落地其中三拍
✅ 戏剧有压强：至少出现一次“目标受阻→角色选择→代价/新麻烦”
✅ 世界规则入戏：至少1个关键情节点体现规则如何限制、放大或扭转角色选择
✅ 配角有自主性：若核心配角戏份充足，应让其做出独立选择并影响局势，不只附和主角
✅ 章尾有钩子：章节最后保留追读牵引点，不平收
✅ 钩子有变化：优先在“信息缺口/危险临门/身份反转/选择悬而未决”中轮换，避免连续同型卡点
✅ 对话有人味：至少1段双向博弈式对白，体现人物立场差异而非轮流讲解设定
✅ 人物不扁平：主角与至少1名核心配角都要有细节化表现（语气、动作习惯、选择代价）
✅ 回报可感知：优先给出1个有效回报（反转/收获/打脸/突破/认知刷新其一）；压抑段可用局势升级、关系位移或真相推进替代
✅ 调度隔离：正文中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：若信息缺口明显，优先保住“目标→阻力→选择→后果”而非堆设定
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出章节标题、序号等元信息
❌ 避免使用“总之”“综上所述”等总结腔
❌ 在结尾处使用开放式反问
❌ 用“他明白了/他意识到/命运将会”等感悟或预告句收尾
❌ 用全知视角总结人物命运或下章走向
❌ 无缘由切入多个角色内心，或替人物提前解释真实想法，造成视角漂移
❌ 添加作者注释或创作说明
❌ 重复上一章已发生的事件
❌ 不要明显超出目标字数
❌ 堆叠“与此同时、值得注意的是、在这个过程中”等模板连接词
❌ 连续使用说明式句子（如“这意味着…”“他知道…”）
❌ 高密度重复“像……/仿佛/像……一样”比喻，把疼痛、危险和异常都写成同一类修辞
❌ 高频使用“下一秒/那一瞬/忽然/不是……而是……”等定式句法，导致全文腔调过稳
❌ 句句都像金句、漂亮句，没有朴素过渡和直写动作
❌ 连续抛术语但不解释，造成阅读门槛陡增
❌ 整段只讲设定或背景，没有可视化动作推进
❌ 场景切换不交代时间、地点或触发动作，导致读者失去空间感
❌ 超过两段连续背景说明且无冲突推进
❌ 把角色写成只负责报信息的工具人
❌ 一章同时塞满推进、解释、回收、抒情、立人等多重任务，导致主任务失焦
❌ 配角只有附和与执行，没有独立选择或情绪立场
❌ 大段解释型对白（角色轮流讲设定、讲道理）
❌ 对话全部语法完整且长度整齐，读起来像书面报告
❌ 世界设定只停留在名词陈列，不影响事件后果
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”等调度文本
❌ 在正文里插入创作复盘、自评或面向用户的说明句
</constraints>

<output>
【输出规范】
直接输出小说正文内容，从故事场景或动作开始。
不要加前言、后记或解释说明。

现在开始创作：
</output>"""

    # 章节生成 - 1-N模式（第2章及以后）
    CHAPTER_GENERATION_ONE_TO_MANY_NEXT = """<system>
你正在创作《{project_title}》，请按{genre}网文读者习惯写作，语言自然、利落、有画面。
你写的是长篇小说的中段内容，必须承前启后，禁止写成导语或终章总结。
</system>

<task>
【创作任务】
撰写第{chapter_number}章《{chapter_title}》的完整正文。

【基本要求】
- 目标字数：{target_word_count}字（允许±200字浮动）
- 叙事视角：{narrative_perspective}

【小说风格要求】
- 叙述贴近中文网文阅读习惯，少官话、少说明腔
- 句式有长短变化，避免连续同构句
- 情绪优先通过动作、对话和场景细节呈现
- 比喻要省着用：同一自然段尽量不连续堆叠两个以上“像……/仿佛/像……一样”，能直写就直写
- 对话要有人物性格，不要写成解释型对白
- 对话口吻要像真人交流：短句优先，允许打断、停顿、潜台词，避免“完整书面句”连发
- 信息密度要可读：单段连续出现2个及以上术语时，需在后续短对话或互动中补一句通俗解释
- 情节推进尽量遵循“动作→反馈→后果”链条，关键段落避免只做概述
- 单章任务要聚焦：优先只承担1个主功能（推进主线/揭示信息/关系转折/兑现回报其一），副功能最多1个，避免平均铺线
- 视角要守规矩：严格贴合{narrative_perspective}，非必要不切入他人内心，不用上帝视角替人物下结论
- 场景切换要有锚点：切换时间、地点或行动阶段时，用简短动作或环境变化交代原因，避免镜头漂移
- 每章至少出现一次“目标受阻→角色选择→代价/新麻烦”戏剧链条
- 世界设定必须入戏：至少1个关键情节点体现世界规则如何影响行动结果
- 角色刻画要立体：主角和核心配角都要呈现外在行为与内在动机/情绪落差
- 配角需有记忆点：若本章有足够戏份的核心配角，优先让其做出一次影响局势的独立选择；动机可通过动作、对白或即时反应侧写，不强求硬解释
- 快节奏开场：前300字内给出冲突触发、异常变化或任务阻力，不做长前情导入
- 场景书写尽量遵循“环境+动作+心理+对话”组合，避免纯说明段
- 疼痛、惊惧和异常优先写动作后果、身体反应和现场变化，不要只靠抽象意象撑气氛
- 优先提供1个可感知回报（反转/收获/打脸/突破/认知刷新其一）；若本章承担压抑或过桥功能，可改为“局势升级/关系位移/真相推进”的有效回报
- 章尾留钩子：结尾给出未完成动作、新风险或悬念问题，支撑追读
- 若chapter_number为2或3，尽量遵循黄金三章分工：第2章“升级+信息+期待”，第3章“小高潮+爽点+钩子”
- 仅输出正文内容，不输出流程说明、调度说明、创作解释
</task>

<outline priority="P0">
【本章大纲 - 作为主线】
{chapter_outline}
</outline>

<worldview priority="P1">
【世界观锚点 - 必须落地到事件】
时间背景：{world_time_period}
地理位置：{world_location}
氛围基调：{world_atmosphere}
世界规则：{world_rules}
</worldview>

<recent_context priority="P1">
【最近章节规划 - 故事脉络参考】
{recent_chapters_context}
</recent_context>

<continuation priority="P0">
【衔接锚点 - 用来接续】
上一章结尾：
「{continuation_point}」

【上一章已完成剧情（避免重复）】
{previous_chapter_summary}

注意：
1. 上述"已完成剧情"和"衔接锚点"是**已经写过的**内容
2. 本章必须推进到**新的情节点**，不要重复叙述已经发生的事件
3. 如果锚点是对话结束，请描写对话后的动作或场景转换，不要重复对话
4. 如果锚点是场景描写，请直接开始人物行动，不要重复描写环境
</continuation>

<characters priority="P1">
【本章角色 - 保持人设稳定】
{characters_info}

⚠️ 角色互动须知：
- 角色之间的对话和行为必须符合其关系设定（如师徒、敌对等）
- 涉及组织的情节须体现角色在组织中的身份和职位
- 角色的能力表现须符合其职业和阶段设定
</characters>

<careers priority="P2">
【本章职业】
{chapter_careers}
</careers>

<foreshadow_reminders priority="P1">
【🎯 伏笔提醒 - 需关注】
{foreshadow_reminders}
</foreshadow_reminders>

<memory priority="P2">
【相关记忆 - 参考】
{relevant_memories}
</memory>

<fusion_contract priority="P0">
【第三版融合约束（正文续写）】
- 信息冲突时按优先级处理：本章大纲 > 上章已完成剧情 > 衔接锚点 > 最近章节规划 > 相关记忆
- 严禁输出“我将/我认为/本章将会”等过程化说明
- 若信息不足，使用最小动作闭环补足，不写空泛总结句
</fusion_contract>

<constraints>
【必须遵守】
✅ 按大纲推进情节
✅ 自然承接上一章结尾，不重复已发生事件
✅ 保持角色性格、说话方式一致
✅ 角色互动须符合关系设定（师徒、朋友、敌对等）
✅ 组织相关情节须体现成员身份和职位层级
✅ 字数控制在目标范围内
✅ 如有伏笔提醒，请在本章中适当埋入或回收相应伏笔
✅ 文风自然：用词贴近日常中文表达，不写模板腔
✅ 比喻不过载：能直接写动作与结果就不要先写“像……”，单段强比喻尽量控制在1个
✅ 句法去模板：慎用“下一秒/那一瞬/忽然/不是……而是……”等固定推进句，避免连续出现
✅ 表达有起伏：长短句交替，段落节奏有变化
✅ 术语可理解：单段连续出现2个及以上术语时，需在3句内补一句生活化解释（可用角色问答完成）
✅ 剧情可感知：关键情节至少写出动作、人物反应和即时结果三要素
✅ 单章聚焦：优先完成1个主任务，可兼带1个副任务，避免把推进、解释、回收、抒情平均摊开
✅ 视角稳定：严格贴合{narrative_perspective}，非必要不切入他人内心，不替角色提前下全知结论
✅ 场景有锚点：切换时间、地点或行动阶段时，要有简短承接动作或环境变化
✅ 快节奏开场：前300字内出现冲突触发或异常变化
✅ 章内有节拍：正文尽量形成“开场钩子→对抗推进→小爆发→余波/钩子”四拍，至少落地其中三拍
✅ 戏剧有压强：至少出现一次“目标受阻→角色选择→代价/新麻烦”
✅ 世界规则入戏：至少1个关键情节点体现规则如何限制、放大或扭转角色选择
✅ 配角有自主性：若核心配角戏份充足，应让其做出独立选择并影响局势，不只附和主角
✅ 章尾有钩子：章节最后保留追读牵引点，不平收
✅ 钩子有变化：优先在“信息缺口/危险临门/身份反转/选择悬而未决”中轮换，避免连续同型卡点
✅ 对话有人味：至少1段双向博弈式对白，体现人物立场差异而非轮流讲解设定
✅ 人物不扁平：主角与至少1名核心配角都要有细节化表现（语气、动作习惯、选择代价）
✅ 回报可感知：优先给出1个有效回报（反转/收获/打脸/突破/认知刷新其一）；压抑段可用局势升级、关系位移或真相推进替代

【🔴 反重复特别指令】
✅ 检查本章开篇是否与"衔接锚点"内容重复
✅ 检查本章情节是否与"上一章已完成剧情"重复
✅ 确保本章推进到了大纲中规划的新事件
✅ 调度隔离：正文中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：若信息缺口明显，优先保住“目标→阻力→选择→后果”而非堆设定
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出章节标题、序号等元信息
❌ 避免使用“总之”“综上所述”等总结腔
❌ 在结尾处使用开放式反问
❌ 用“他明白了/他意识到/命运将会”等感悟或预告句收尾
❌ 用全知视角总结人物命运或下章走向
❌ 无缘由切入多个角色内心，或替人物提前解释真实想法，造成视角漂移
❌ 添加作者注释或创作说明
❌ 重复叙述上一章已发生的事件（包括环境描写、心理活动）
❌ 在开篇使用"接上回"、"书接上文"等套话
❌ 角色行为超出其职业阶段的能力范围
❌ 堆叠“与此同时、值得注意的是、在这个过程中”等模板连接词
❌ 连续使用说明式句子（如“这意味着…”“他知道…”）
❌ 高密度重复“像……/仿佛/像……一样”比喻，把疼痛、危险和异常都写成同一类修辞
❌ 高频使用“下一秒/那一瞬/忽然/不是……而是……”等定式句法，导致全文腔调过稳
❌ 句句都像金句、漂亮句，没有朴素过渡和直写动作
❌ 连续抛术语但不解释，造成阅读门槛陡增
❌ 整段只讲设定或背景，没有可视化动作推进
❌ 场景切换不交代时间、地点或触发动作，导致读者失去空间感
❌ 超过两段连续背景说明且无冲突推进
❌ 把角色写成只负责报信息的工具人
❌ 一章同时塞满推进、解释、回收、抒情、立人等多重任务，导致主任务失焦
❌ 配角只有附和与执行，没有独立选择或情绪立场
❌ 大段解释型对白（角色轮流讲设定、讲道理）
❌ 对话全部语法完整且长度整齐，读起来像书面报告
❌ 世界设定只停留在名词陈列，不影响事件后果
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”等调度文本
❌ 在正文里插入创作复盘、自评或面向用户的说明句
</constraints>

<output>
【输出规范】
直接输出小说正文内容，从故事场景或动作开始。
不要加前言、后记或解释说明。

现在开始创作：
</output>"""

    # 单个角色生成提示词 V2（RTCO框架）
    SINGLE_CHARACTER_GENERATION = """<system>
你是小说角色设定搭档，擅长做出真实、有戏、能推动剧情的人物。
</system>

<task>
【设计任务】
根据用户需求和项目上下文，创建一个完整的角色设定。

【写法要求】
- 先解决“这个人能在故事里干什么”，再补性格与背景
- 避免模板化夸赞，优先给可落地的行为特征
- 描述贴近中文小说阅读习惯，不写教科书腔
- 角色要有可直接入戏的冲突入口，避免只有资料感没有戏剧性
</task>

<context priority="P0">
【项目上下文】
{project_context}

【用户需求】
{user_input}
</context>

<fusion_contract priority="P0">
【第三版融合约束（角色生成）】
- 只输出JSON角色卡，不输出“执行X.X/调用Agent/方案A-B/复盘”等流程文本
- 信息冲突时按优先级处理：用户需求 > 项目上下文硬设定 > 风格补充
- 角色设定必须服务剧情冲突：明确其目标、阻力来源、可承担代价
- 信息不足时保守生成：先给最小可执行角色框架，不杜撰高风险设定
</fusion_contract>

<output priority="P0">
【输出格式】
生成完整的角色卡片JSON对象：

{{
  "name": "角色姓名（如用户未提供则生成符合世界观的名字）",
  "age": "年龄（具体数字或年龄段）",
  "gender": "男/女/其他",
  "appearance": "外貌描述（100-150字）：身高体型、面容特征、着装风格",
  "personality": "性格特点（150-200字）：核心性格特质、优缺点、特殊习惯",
  "background": "背景故事（200-300字）：家庭背景、成长经历、重要转折、与主题关联",
  "traits": ["特长1", "特长2", "特长3"],
  "relationships_text": "人际关系的自然语言描述",
  "relationships": [
    {{
      "target_character_name": "已存在的角色名称",
      "relationship_type": "关系类型",
      "intimacy_level": 75,
      "description": "关系的详细描述",
      "started_at": "关系开始的故事时间点（可选）"
    }}
  ],
  "organization_memberships": [
    {{
      "organization_name": "已存在的组织名称",
      "position": "职位名称",
      "rank": 8,
      "loyalty": 80,
      "joined_at": "加入时间（可选）",
      "status": "active"
    }}
  ],
  "career_info": {{
    "main_career_name": "从可用主职业列表中选择的职业名称",
    "main_career_stage": 5,
    "sub_careers": [
      {{
        "career_name": "从可用副职业列表中选择的职业名称",
        "stage": 3
      }}
    ]
  }}
}}

【职业信息说明】
如果项目上下文包含职业列表：
- 主职业：从"可用主职业"列表中选择最符合角色的职业
- 主职业阶段：根据角色实力设定合理阶段（1到max_stage）
- 副职业：可选择0-2个副职业
- ⚠️ 填写职业名称而非ID，系统会自动匹配
- 职业选择必须与角色背景、能力和定位高度契合

【关系类型参考】
- 家族：父亲、母亲、兄弟、姐妹、子女、配偶、恋人
- 社交：师父、徒弟、朋友、同学、同事、邻居、知己
- 职业：上司、下属、合作伙伴
- 敌对：敌人、仇人、竞争对手、宿敌

【数值范围】
- intimacy_level：-100到100（负值表示敌对）
- loyalty：0到100
- rank：0到10
</output>

<constraints>
【必须遵守】
✅ 符合世界观：角色设定与项目世界观一致
✅ 主题关联：背景故事与项目主题关联
✅ 立体饱满：性格复杂有矛盾性，不脸谱化
✅ 为故事服务：设定要推动剧情发展
✅ 职业匹配：职业选择与角色高度契合
✅ 冲突可用：背景或关系中至少落地一个“目标受阻→选择→后果/代价”的事件线索
✅ 入场即有戏：角色设定里尽量包含一个可直接写成首次登场场面的冲突触发点或反常举动

【角色定位要求】
✅ 主角：有成长空间和目标动机
✅ 反派：有合理动机，不脸谱化
✅ 配角：有独特性，不是工具人

【关系约束】
✅ relationships只引用已存在的角色
✅ organization_memberships只引用已存在的组织
✅ 无关系或组织时对应数组为空[]

【格式约束】
✅ 纯JSON对象输出，无markdown标记
✅ 内容描述中严禁使用特殊符号
✅ 专有名词直接书写
✅ 模板追踪标签：rule_v3_fusion_20260305

【禁止事项】
❌ 输出markdown或代码块标记
❌ 在描述中使用特殊符号（引号、方括号等）
❌ 引用不存在的角色或组织
❌ 脸谱化的角色设定
❌ 使用“总之/综上/值得注意的是”这类模板连接句
❌ 把角色写成“无成本开挂”或“只负责报信息”的工具人
❌ 输出流程说明、调度术语或自我评注
</constraints>"""

    # 单个组织生成提示词 V2（RTCO框架）
    SINGLE_ORGANIZATION_GENERATION = """<system>
你是小说组织设定搭档，擅长创建有目标、有规则、有戏剧价值的势力。
</system>

<task>
【设计任务】
根据用户需求和项目上下文，创建一个完整的组织/势力设定。

【写法要求】
- 组织设定要能直接用于剧情，不做纯背景板
- 描述优先具体行动逻辑，少抽象口号
- 语言自然，避免模板腔
- 组织要让读者一眼看出“会制造什么麻烦/机会”，不能只有设定感
</task>

<context priority="P0">
【项目上下文】
{project_context}

【用户需求】
{user_input}
</context>

<fusion_contract priority="P0">
【第三版融合约束（组织生成）】
- 只输出JSON组织设定，不输出“执行X.X/调用Agent/方案A-B/复盘”等流程文本
- 信息冲突时按优先级处理：用户需求 > 项目上下文硬设定 > 风格补充
- 组织必须具备冲突功能：明确其目标、手段、约束与代价
- 信息不足时保守生成：优先给最小可执行组织框架，不杜撰跨设定能力
</fusion_contract>

<output priority="P0">
【输出格式】
生成完整的组织设定JSON对象：

{{
  "name": "组织名称（如用户未提供则生成符合世界观的名称）",
  "is_organization": true,
  "organization_type": "组织类型（帮派/公司/门派/学院/政府机构/宗教组织等）",
  "personality": "组织特性（150-200字）：核心理念、行事风格、文化价值观、运作方式",
  "background": "组织背景（200-300字）：建立历史、发展历程、重要事件、当前地位",
  "appearance": "外在表现（100-150字）：总部位置、标志性建筑、组织标志、制服等",
  "organization_purpose": "组织目的和宗旨：明确目标、长期愿景、行动准则",
  "power_level": 75,
  "location": "所在地点：主要活动区域、势力范围",
  "motto": "组织格言或口号",
  "traits": ["特征1", "特征2", "特征3"],
  "color": "组织代表颜色（如：深红色、金色、黑色等）",
  "organization_members": ["重要成员1", "重要成员2", "重要成员3"]
}}

【字段说明】
- power_level：0-100的整数，表示在世界中的影响力
- organization_members：组织内重要成员名字列表（可关联已有角色）
- 成立时间：在background中描述
</output>

<constraints>
【必须遵守】
✅ 符合世界观：组织设定与项目世界观一致
✅ 主题关联：背景与项目主题关联
✅ 推动剧情：组织能推动故事发展
✅ 有层级结构：内部有明确的层级和结构
✅ 势力互动：与其他势力有互动关系
✅ 代价机制：组织行动至少体现一种成本（资源、声望、规则惩罚或内部风险）
✅ 首次出场可写：设定里尽量隐含一个适合首次亮相的动作场景、冲突场面或公开事件

【组织定位要求】
✅ 有存在必要性：不是可有可无的背景板
✅ 目标合理：不过于理想化或脸谱化
✅ 具体细节：描述详细具体，避免空泛

【格式约束】
✅ 纯JSON对象输出，无markdown标记
✅ 内容描述中严禁使用特殊符号
✅ 专有名词直接书写
✅ 模板追踪标签：rule_v3_fusion_20260305

【禁止事项】
❌ 输出markdown或代码块标记
❌ 在描述中使用特殊符号（引号、方括号等）
❌ 过于理想化或脸谱化的设定
❌ 空泛的描述
❌ 使用“总之/综上/值得注意的是”这类模板连接句
❌ 把组织写成“全能无代价”的万能设定
❌ 输出流程说明、调度术语或自我评注
</constraints>"""

    # 情节分析提示词 V2（RTCO框架 + 伏笔ID追踪）
    PLOT_ANALYSIS = """<system>
你是小说剧情复盘搭档，擅长把章节里的钩子、冲突和伏笔拆清楚。
</system>

<task>
【分析任务】
全面分析第{chapter_number}章《{title}》的剧情要素、钩子、伏笔、冲突和角色发展。

【表达要求】
- 分析结论要具体，避免空泛评价
- 术语可用，但描述保持自然中文表达
- 只输出结构化分析结果，不输出流程说明、调度话术或自我评注

【🔴 伏笔追踪任务（重要）】
系统已提供【已埋入伏笔列表】，当你识别到章节中有回收伏笔时：
1. 必须从列表中找出对应的伏笔ID
2. 在 foreshadows 数组中使用 reference_foreshadow_id 字段关联
3. 如果无法确定是哪个伏笔，reference_foreshadow_id 填 null
</task>

<chapter priority="P0">
【章节信息】
章节：第{chapter_number}章
标题：{title}
字数：{word_count}字

【章节内容】
{content}
</chapter>

<existing_foreshadows priority="P1">
【已埋入伏笔列表 - 用于回收匹配】
以下是本项目中已埋入但尚未回收的伏笔，分析时如发现章节内容回收了某个伏笔，请使用对应的ID：

{existing_foreshadows}
</existing_foreshadows>

<characters priority="P1">
【项目角色信息 - 用于角色状态分析】
以下是项目中已有的角色列表，分析 character_states 和 relationship_changes 时请使用这些角色的准确名称：

{characters_info}
</characters>

<analysis_framework priority="P0">
【分析维度】

**1. 剧情钩子 (Hooks)**
识别吸引读者的关键元素：
- 悬念钩子：未解之谜、疑问、谜团
- 情感钩子：引发共鸣的情感点
- 冲突钩子：矛盾对抗、紧张局势
- 认知钩子：颠覆认知的信息

额外要求：
- 必须重点检查开头300字内是否存在有效开场钩子，并在hooks里至少标记1项 opening 相关钩子（若确实没有，再如实说明）
- 必须检查结尾是否存在章尾牵引点，并在hooks里至少标记1项 ending 相关钩子（若确实没有，再如实说明）

每个钩子需要：
- 类型分类
- 具体内容描述
- 强度评分(1-10)
- 出现位置(开头/中段/结尾)
- **关键词**：【必填】从原文逐字复制8-25字的文本片段，用于精确定位

**2. 伏笔分析 (Foreshadowing) - 🔴 支持ID追踪**
- 埋下的新伏笔：内容、预期作用、隐藏程度(1-10)
- 回收的旧伏笔：【必须】从已埋入伏笔列表中匹配ID
- 伏笔质量：巧妙性和合理性
- **关键词**：【必填】从原文逐字复制8-25字

每个伏笔需要：
- **title**：简洁标题（10-20字，概括伏笔核心）
  - ⚠️ 回收伏笔时，标题应与原伏笔标题保持一致，不要添加"回收"等后缀
  - 例如：原伏笔标题是"绿头发的视觉符号"，回收时标题仍为"绿头发的视觉符号"，而非"绿头发的视觉符号回收"
- **content**：详细描述伏笔内容和预期作用
- **type**：planted（埋下）或 resolved（回收）
- **strength**：强度1-10（对读者的吸引力）
- **subtlety**：隐藏度1-10（越高越隐蔽）
- **reference_chapter**：回收时引用的原埋入章节号，埋下时为null
- **reference_foreshadow_id**：【回收时必填】被回收伏笔的ID（从已埋入伏笔列表中选择），埋下时为null
  - 🔴 重要：回收伏笔时，必须从【已埋入伏笔列表】中找到对应的伏笔ID并填写
  - 如果列表中有标注【ID: xxx】的伏笔，回收时必须使用该ID
  - 如果无法确定是哪个伏笔，才填写null（但应尽量避免）
- **keyword**：【必填】从原文逐字复制8-25字的定位文本
- **category**：分类（identity=身世/mystery=悬念/item=物品/relationship=关系/event=事件/ability=能力/prophecy=预言）
- **is_long_term**：是否长线伏笔（跨10章以上回收为true）
- **related_characters**：涉及的角色名列表
- **estimated_resolve_chapter**：【必填】预估回收章节号（埋下时必须预估，回收时为当前章节）

**3. 冲突分析 (Conflict)**
- 冲突类型：人与人/人与己/人与环境/人与社会
- 冲突各方及立场
- 冲突强度(1-10)
- 解决进度(0-100%)

**3b. 追更节拍检查 (Serial Rhythm)**
- 开场是否在前300字内进入异常、任务压力、危险或正面冲突
- 本章是否存在至少一个“小爽点链条”：铺垫→爆发→反馈
- 章尾是否形成有效卡点：信息缺口/危险临门/身份反转/选择未决
- 若缺失，必须在suggestions中给出对应修正建议

**4. 情感曲线 (Emotional Arc)**
- 主导情绪（最多10字）
- 情感强度(1-10)
- 情绪变化轨迹

**5. 角色状态追踪 (Character Development)**
对每个出场角色分析：
- 心理状态变化(前→后)
- 关系变化
- 关键行动和决策
- 成长或退步
- **💀 存活状态（重要）**：
  - survival_status: 角色当前存活状态
  - 可选值：active(正常)/deceased(死亡)/missing(失踪)/retired(退场)
  - 默认为null（表示无变化），仅当章节中角色明确死亡、失踪或永久退场时才填写
  - 死亡/失踪需要有明确的剧情依据，不可臆测
- ** 职业变化（可选）**：
  - 仅当章节明确描述职业进展时填写
  - main_career_stage_change: 整数(+1晋升/-1退步/0无变化)
  - sub_career_changes: 副职业变化数组
  - new_careers: 新获得职业
  - career_breakthrough: 突破过程描述
- **🏛️ 组织变化（可选）**：
  - 仅当章节明确描述角色与组织关系变化时填写
  - organization_changes: 组织变动数组
  - 每项包含：organization_name(组织名)、change_type(加入joined/离开left/晋升promoted/降级demoted/开除expelled/叛变betrayed)、new_position(新职位，可选)、loyalty_change(忠诚度变化描述，可选)、description(变化描述)

**5b. 组织状态追踪 (Organization Status) - 可选**
仅当章节涉及组织势力变化时填写，分析出场组织的状态变化：
- 组织名称
- 势力等级变化(power_change: 整数，+N增强/-N削弱/0无变化)
- 据点变化(new_location: 新据点，可选)
- 宗旨/目标变化(new_purpose: 新目标，可选)
- 组织状态描述(status_description: 当前状态概述)
- 关键事件(key_event: 触发变化的事件)
- **💀 组织存续状态（重要）**：
  - is_destroyed: 组织是否被覆灭（true/false，默认false）
  - 仅当章节明确描述组织被彻底消灭、瓦解、灭亡时设为true

**6. 关键情节点 (Plot Points)**
列出3-5个核心情节点：
- 情节内容
- 类型(revelation/conflict/resolution/transition)
- 重要性(0.0-1.0)
- 对故事的影响
- **关键词**：【必填】从原文逐字复制8-25字

**7. 场景与节奏**
- 主要场景
- 叙事节奏(快/中/慢)
- 对话与描写比例

**8. 质量评分（支持小数，严格区分度）**
评分范围：1.0-10.0，支持一位小数（如 6.5、7.8）
每个维度必须根据以下标准严格评分，避免所有内容都打中等分数：

**节奏把控 (pacing)**：
- 1.0-3.9（差）：节奏混乱，该快不快该慢不慢；场景切换生硬；大段无意义描写拖沓
- 4.0-5.9（中下）：节奏基本可读但有明显问题；部分场景过于冗长或仓促
- 6.0-7.9（中上）：节奏整体流畅，偶有小问题；张弛有度但不够精妙
- 8.0-9.4（优秀）：节奏把控精准，高潮迭起；场景切换自然，详略得当
- 9.5-10.0（完美）：节奏大师级，每个段落都恰到好处

**吸引力 (engagement)**：
- 1.0-3.9（差）：内容乏味，缺乏钩子；读者难以继续阅读
- 4.0-5.9（中下）：有基本情节但缺乏亮点；钩子设置生硬或缺失
- 6.0-7.9（中上）：有一定吸引力，钩子有效但不够巧妙
- 8.0-9.4（优秀）：引人入胜，钩子设置精妙；让人欲罢不能
- 9.5-10.0（完美）：极具吸引力，每个段落都有阅读动力

**连贯性 (coherence)**：
- 1.0-3.9（差）：逻辑混乱，前后矛盾；角色行为不合理
- 4.0-5.9（中下）：基本连贯但有明显漏洞；部分情节衔接生硬
- 6.0-7.9（中上）：整体连贯，偶有小瑕疵；角色行为基本合理
- 8.0-9.4（优秀）：逻辑严密，衔接自然；角色行为高度一致
- 9.5-10.0（完美）：无懈可击的连贯性

**整体质量 (overall)**：
- 计算公式：(pacing + engagement + coherence) / 3，保留一位小数
- 可根据综合印象±0.5调整，必须与各项分数保持一致性

**9. 改进建议（与分数关联）**
建议数量必须与整体质量分数关联：
- overall < 4.0：必须提供4-5条具体改进建议，指出严重问题
- overall 4.0-5.9：必须提供3-4条改进建议，指出主要问题
- overall 6.0-7.9：提供1-2条优化建议，指出可提升之处
- overall ≥ 8.0：提供0-1条锦上添花的建议

每条建议必须：
- 指出具体问题位置或类型
- 说明为什么是问题
- 给出明确的改进方向
</analysis_framework>

<fusion_contract priority="P0">
【第三版融合约束（剧情分析）】
- 只输出JSON分析结果，不输出“执行X.X/调用Agent/方案对比/复盘”类流程文本
- 信息冲突时按优先级处理：章节内容 > 已埋入伏笔列表 > 角色信息 > 评分经验规则
- 证据不足时使用保守填法：字段置null/空数组，不臆造不存在的剧情事实
</fusion_contract>

<output priority="P0">
【输出格式】
返回纯JSON对象（无markdown标记）：

{{
  "hooks": [
    {{
      "type": "悬念",
      "content": "具体描述",
      "strength": 8,
      "position": "中段",
      "keyword": "从原文逐字复制的8-25字文本"
    }}
  ],
  "serial_rhythm": {{
    "opening_hook": {{
      "present": true,
      "strength": 8,
      "type": "异常变化|危险临门|高压任务|正面冲突|信息反差|无",
      "keyword": "从原文逐字复制的8-25字文本或null",
      "assessment": "开场是否足够快、是否能拉住读者"
    }},
    "payoff_chain": {{
      "present": true,
      "strength": 7,
      "stages": ["铺垫", "爆发", "反馈"],
      "keyword": "从原文逐字复制的8-25字文本或null",
      "assessment": "本章小爽点是否完整、是否有效"
    }},
    "ending_cliffhanger": {{
      "present": true,
      "strength": 8,
      "type": "信息缺口|危险临门|身份反转|选择未决|无",
      "keyword": "从原文逐字复制的8-25字文本或null",
      "assessment": "结尾是否具备追读牵引"
    }}
  }},
  "foreshadows": [
    {{
      "title": "伏笔简洁标题",
      "content": "伏笔详细内容和预期作用",
      "type": "planted",
      "strength": 7,
      "subtlety": 8,
      "reference_chapter": null,
      "reference_foreshadow_id": null,
      "keyword": "从原文逐字复制的8-25字文本",
      "category": "mystery",
      "is_long_term": false,
      "related_characters": ["角色A", "角色B"],
      "estimated_resolve_chapter": 15
    }},
    {{
      "title": "回收的伏笔标题",
      "content": "伏笔如何被回收的描述",
      "type": "resolved",
      "strength": 8,
      "subtlety": 6,
      "reference_chapter": 5,
      "reference_foreshadow_id": "abc123-已埋入伏笔的ID",
      "keyword": "从原文逐字复制的8-25字文本",
      "category": "mystery",
      "is_long_term": false,
      "related_characters": ["角色A"],
      "estimated_resolve_chapter": 10
    }}
  ],
  "conflict": {{
    "types": ["人与人", "人与己"],
    "parties": ["主角-复仇", "反派-维护现状"],
    "level": 8,
    "description": "冲突描述",
    "resolution_progress": 0.3
  }},
  "emotional_arc": {{
    "primary_emotion": "紧张焦虑",
    "intensity": 8,
    "curve": "平静→紧张→高潮→释放",
    "secondary_emotions": ["期待", "焦虑"]
  }},
  "character_states": [
    {{
      "character_name": "张三",
      "survival_status": null,
      "state_before": "犹豫",
      "state_after": "坚定",
      "psychological_change": "心理变化描述",
      "key_event": "触发事件",
      "relationship_changes": {{"李四": "关系改善"}},
      "career_changes": {{
        "main_career_stage_change": 1,
        "sub_career_changes": [{{"career_name": "炼丹", "stage_change": 1}}],
        "new_careers": [],
        "career_breakthrough": "突破描述"
      }},
      "organization_changes": [
        {{
          "organization_name": "某门派",
          "change_type": "promoted",
          "new_position": "长老",
          "loyalty_change": "忠诚度提升",
          "description": "因立下大功被提拔为长老"
        }}
      ]
    }}
  ],
  "plot_points": [
    {{
      "content": "情节点描述",
      "type": "revelation",
      "importance": 0.9,
      "impact": "推动故事发展",
      "keyword": "从原文逐字复制的8-25字文本"
    }}
  ],
  "scenes": [
    {{
      "location": "地点",
      "atmosphere": "氛围",
      "duration": "时长估计"
    }}
  ],
  "organization_states": [
    {{
      "organization_name": "某门派",
      "power_change": -10,
      "new_location": null,
      "new_purpose": null,
      "status_description": "因内乱势力受损，但核心力量未动摇",
      "key_event": "长老叛变导致分支瓦解",
      "is_destroyed": false
    }}
  ],
  "pacing": "varied",
  "dialogue_ratio": 0.4,
  "description_ratio": 0.3,
  "scores": {{
    "pacing": 6.5,
    "engagement": 5.8,
    "coherence": 7.2,
    "overall": 6.5,
    "score_justification": "节奏整体流畅但中段略显拖沓；钩子设置有效但不够巧妙；逻辑连贯无明显漏洞"
  }},
  "plot_stage": "发展",
  "suggestions": [
    "【节奏问题】第三场景的心理描写过长（约500字），建议精简至200字以内，保留核心情感即可",
    "【吸引力不足】章节中段缺乏有效钩子，建议在主角发现线索后增加一个小悬念"
  ]
}}
</output>

<constraints>
【必须遵守】
✅ keyword字段必填：钩子、伏笔、情节点的keyword不能为空
✅ 逐字复制：keyword必须从原文复制，长度8-25字
✅ 精确定位：keyword能在原文中精确找到
✅ serial_rhythm必填：必须判断 opening_hook、payoff_chain、ending_cliffhanger 三项是否存在
✅ 若 serial_rhythm 某项 present=true，则对应 keyword 不能为空且必须逐字复制
✅ 职业变化可选：仅当章节明确描述时填写
✅ 组织变化可选：仅当章节明确描述角色与组织关系变动时填写（character_states中的organization_changes）
✅ 组织状态可选：仅当章节明确描述组织势力/据点/目标变化时填写（organization_states顶级字段）
✅ 存活状态谨慎：survival_status仅当章节有明确死亡/失踪/退场描写时填写，默认null
✅ 组织覆灭谨慎：is_destroyed仅当组织被彻底消灭时设true，组织受损不算覆灭
✅ 【伏笔ID追踪】回收伏笔时，必须从【已埋入伏笔列表】中查找匹配的ID填入 reference_foreshadow_id

【评分约束 - 严格执行】
✅ 严格按评分标准打分，支持小数（如6.5、7.2、8.3）
✅ 不要默认给7.0-8.0分，差的内容必须给低分（1.0-5.0），好的内容才给高分（8.0-10.0）
✅ score_justification必填：简要说明各项评分的依据
✅ 建议数量必须与overall分数关联：
   - overall≤4.0 → 4-5条建议
   - overall 4.0-6.0 → 3-4条建议
   - overall 6.0-8.0 → 1-2条建议
   - overall≥8.0 → 0-1条建议
✅ 每条建议必须标注问题类型（如【节奏问题】【描写不足】等）
✅ 调度隔离：结果中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：证据不足时可返回null或空数组，不得杜撰事实
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ keyword使用概括或改写的文字
❌ 输出markdown标记
❌ 遗漏必填的keyword字段
❌ 无根据地添加职业变化
❌ 无根据地添加组织变化或组织状态变化
❌ 无确切剧情依据地标记角色死亡或组织覆灭
❌ 所有章节都打7-8分的"安全分"
❌ 高分章节给大量建议，或低分章节不给建议
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”“复盘结论”等流程文本
</constraints>"""

    # 正文质量检查提示词（第三版融合）
    CHAPTER_TEXT_CHECKER = """<system>
你是小说正文质量检查专家，负责找出可导致阅读体验下降的关键问题，并给出可执行修正建议。
</system>

<task>
【检查任务】
对第{chapter_number}章《{chapter_title}》进行结构化质检。

【检查重点】
1. 设定一致性：世界规则、角色能力边界、关系逻辑是否冲突
2. 连贯性：上下文衔接是否自然，是否有重复叙述或断裂
3. 叙事有效性：是否存在大段解释腔、模板腔、空泛总结
4. 对话质量：人物声线是否区分，是否出现轮流讲设定
5. 结尾质量：是否出现感悟式/预告式/全知式收束
6. 可读性：术语密度过高是否缺乏人话解释
7. 开场抓力：前300字是否进入异常、危险、任务或正面冲突
8. 爽点链条：本章是否存在“铺垫→爆发→反馈”的最小爽点闭环
9. 章尾牵引：结尾是否形成信息缺口、危险临门、身份反转或选择未决
10. 模板腔与AI味：是否高频出现“像……/仿佛/像……一样/下一秒/那一瞬/忽然/不是……而是……”等定式句法，导致修辞堆叠、句句发力或真人感下降
</task>

<context priority="P0">
【章节正文】
{chapter_content}
</context>

<anchors priority="P1">
【本章大纲锚点】
{chapter_outline}

【角色信息】
{characters_info}

【世界规则】
{world_rules}
</anchors>

<fusion_contract priority="P0">
【第三版融合约束（正文检查）】
- 只输出JSON，不输出执行步骤、调度术语、自我评注
- 信息冲突时按优先级：章节正文 > 大纲锚点 > 角色信息 > 世界规则
- 证据不足时保持保守：不要杜撰问题，不要强行判错
</fusion_contract>

<output>
【输出格式】
返回纯JSON对象（无markdown）：

{{
  "overall_assessment": "优秀|良好|一般|较差|存在严重问题",
  "severity_counts": {{
    "critical": 0,
    "major": 0,
    "minor": 0
  }},
  "issues": [
    {{
      "severity": "critical|major|minor",
      "category": "设定冲突|逻辑连贯|角色失真|文风表达|对话质量|结尾处理|术语可读性|开场抓力|爽点链条|章尾牵引|模板腔AI味",
      "location": "位置描述（段落/片段）",
      "evidence": "原文证据（可截断）",
      "impact": "问题影响",
      "suggestion": "可直接执行的修正建议"
    }}
  ],
  "priority_actions": [
    "优先修改项1",
    "优先修改项2"
  ],
  "revision_suggestions": [
    "建议1",
    "建议2"
  ],
  "serial_rhythm_assessment": {{
    "opening_hook_ok": true,
    "payoff_chain_ok": true,
    "ending_cliffhanger_ok": false
  }}
}}
</output>

<constraints>
【必须遵守】
✅ issues最多输出8条，按严重度降序排列
✅ revision_suggestions最多输出8条，必须是可执行建议
✅ severity_counts必须与issues中的严重度统计一致
✅ location必须可定位，避免“某处/部分地方”这类模糊描述
✅ suggestion必须能直接用于改写，避免空话
✅ 若判定存在模板腔AI味，需指出触发它的具体句式、比喻或套话，而不是泛泛评价“像AI”
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出markdown、代码块、流程日志
❌ 输出“执行X.X/调用Agent/方案A-B/复盘”等调度文本
❌ 没有证据时硬判错误
❌ 输出与正文无关的泛化建议
</constraints>"""

    # 正文自动修订提示词（第三版融合，生成修订草案，不直接落库正文）
    CHAPTER_TEXT_REVISER = """<system>
你是小说正文修订专家。请根据质检报告优先修复高优先问题（critical / major），保持原文风格与剧情主线不变。
</system>

<task>
【修订任务】
对第{chapter_number}章《{chapter_title}》生成一版“可直接替换”的修订草案。

【修订原则】
1. 先修复所有高优先问题（critical / major）；若同时存在 critical 与 major，先处理 critical
2. 最小改动优先：能改一句不改一段，避免剧情跑偏
3. 不新增片段外重大事件，不改角色核心关系和能力边界
4. 维持原文风格、人称和叙事节奏
5. 严禁流程化元文本与说明腔
6. 若存在模板腔AI味，优先删除重复比喻、固定推进句和过分工整的漂亮句，改成动作句、反应句与朴素过渡句
</task>

<context priority="P0">
【原章节正文】
{chapter_content}

【高优先问题清单（优先修复）】
{critical_issues_text}
</context>

<checker priority="P1">
【完整质检结果（JSON）】
{checker_result_json}
</checker>

<fusion_contract priority="P0">
【第三版融合约束（正文修订）】
- 只输出JSON结果，不输出执行步骤、调度术语、自我评注
- 信息冲突时按优先级：原章节正文 > 高优先问题清单 > 完整质检结果
- 若某问题证据不足，标记为未解决，不强行改写
</fusion_contract>

<output>
【输出格式】
返回纯JSON对象（无markdown）：

{{
  "revised_text": "修订后的完整正文",
  "applied_issues": [
    "已修复问题1",
    "已修复问题2"
  ],
  "unresolved_issues": [
    "未解决问题及原因"
  ],
  "change_summary": "一句话说明本次修订重点"
}}
</output>

<constraints>
【必须遵守】
✅ revised_text必须是完整正文，不得夹带说明文字
✅ applied_issues/unresolved_issues 均为字符串数组，最多各8条
✅ unresolved_issues 只记录无法安全修复的问题，不得空泛
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出markdown、代码块、流程日志
❌ 输出“执行X.X/调用Agent/方案A-B/复盘”等调度文本
❌ 改写为导语、总结、预告式文案
❌ 新增原文不存在的重大剧情转折
</constraints>"""

    # 大纲单批次展开提示词 V2（RTCO框架）
    OUTLINE_EXPAND_SINGLE = """<system>
你是小说大纲展开搭档，擅长把一个大纲节点拆成可直接动笔的章节规划。
</system>

<task>
【展开任务】
将第{outline_order_index}节大纲《{outline_title}》展开为{target_chapter_count}个章节的详细规划。

【展开策略】
{strategy_instruction}

【风格要求】
- `plot_summary` 按“谁在什么场景做了什么、引发什么后果”来写
- 叙述使用中文网文常见表达，避免说明书腔
- 每章都要有可写性，不能只写抽象目标
- 每章至少体现一次“目标受阻→角色决策→即时后果”链条
- 每章至少安排一个可直接写对白的交锋场面（两人及以上，立场不完全一致）
- 关键事件要落地世界规则：体现规则如何限制、放大或扭转角色选择
- 只输出章节规划JSON，不输出流程说明、调度说明、创作解释
</task>

<project priority="P1">
【项目信息】
小说名称：{project_title}
类型：{project_genre}
主题：{project_theme}
叙事视角：{project_narrative_perspective}

【世界观背景】
时间背景：{project_world_time_period}
地理位置：{project_world_location}
氛围基调：{project_world_atmosphere}
</project>

<characters priority="P1">
【角色信息】
{characters_info}
</characters>

<outline_node priority="P0">
【当前大纲节点 - 展开对象】
序号：第 {outline_order_index} 节
标题：{outline_title}
内容：{outline_content}
</outline_node>

<context priority="P2">
【上下文参考】
{context_info}
</context>

<fusion_contract priority="P0">
【第三版融合约束（大纲展开-单批次）】
- 只输出JSON章节规划，不输出“执行X.X/调用Agent/方案对比/复盘”等流程文本
- 信息冲突时按优先级处理：当前大纲节点 > 项目上下文 > 角色信息 > 其余参考
- 信息不足时先保住“目标→阻力→选择→后果”最小链，不跨到后续大纲
</fusion_contract>

<output priority="P0">
【输出格式】
返回{target_chapter_count}个章节规划的JSON数组：

[
  {{
    "sub_index": 1,
    "title": "章节标题（体现核心冲突或情感）",
    "plot_summary": "剧情摘要（200-300字）：详细描述该章发生的事件，仅限当前大纲内容",
    "key_events": ["关键事件1", "关键事件2", "关键事件3"],
    "character_focus": ["角色A", "角色B"],
    "emotional_tone": "情感基调（如：紧张、温馨、悲伤）",
    "narrative_goal": "叙事目标（该章要达成的叙事效果）",
    "conflict_type": "冲突类型（如：内心挣扎、人际冲突）",
    "estimated_words": 3000{scene_field}
  }}
]

【格式规范】
- 纯JSON数组输出，无其他文字
- 内容描述中严禁使用特殊符号
</output>

<constraints>
【⚠️ 内容边界约束 - 必须严格遵守】
✅ 只能展开当前大纲节点的内容
✅ 深化当前大纲，而非跨越到后续
✅ 放慢叙事节奏，充分体验当前阶段

❌ 绝对不能推进到后续大纲内容
❌ 不要让剧情快速推进
❌ 不要提前展开【后一节】的内容

【展开原则】
✅ 将单一事件拆解为多个细节丰富的章节
✅ 深入挖掘情感、心理、环境、对话
✅ 每章是当前大纲内容的不同侧面或阶段
✅ `plot_summary` 要具体到行动与结果，保证可直接转写正文
✅ 每章要有清晰的阻力来源和代价，避免“顺风推进”

【🔴 相邻章节差异化约束（防止重复）】
✅ 每章有独特的开场方式（不同场景、时间点、角色状态）
✅ 每章有独特的结束方式（不同悬念、转折、情感收尾）
✅ key_events在相邻章节间绝不重叠
✅ plot_summary描述该章独特内容，不与其他章雷同
✅ 同一事件的不同阶段要明确区分"前、中、后"

【章节间要求】
✅ 衔接自然流畅（每章从不同起点开始）
✅ 剧情递进合理（但不超出当前大纲边界）
✅ 节奏张弛有度
✅ 每章有明确且独特的叙事价值
✅ 最后一章结束时恰好完成当前大纲内容
✅ 关键事件无重叠：检查相邻章节key_events
✅ 调度边界：只输出章节规划内容，不输出调度说明或执行日志
✅ 缺口降级：信息不足时仍保持“目标→阻力→选择→后果”链条完整
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出非JSON格式
❌ 剧情越界到后续大纲
❌ 相邻章节内容重复
❌ 关键事件雷同
❌ 使用“总之/综上/值得注意的是/在这个过程中”等模板连接词
❌ 只写设定介绍，不写角色决策与后果
❌ 使用调度器口吻（如“执行1.1/调用Agent/方案对比”）
❌ 把输出写成流程文档或复盘报告
</constraints>"""

    # 大纲分批展开提示词 V2（RTCO框架）
    OUTLINE_EXPAND_MULTI = """<system>
你是小说大纲分批展开搭档，擅长在不跑偏的前提下持续细化章节。
</system>

<task>
【展开任务】
继续展开第{outline_order_index}节大纲《{outline_title}》，生成第{start_index}-{end_index}节（共{target_chapter_count}个章节）的详细规划。

【分批说明】
- 这是整个展开的一部分
- 必须与前面已生成的章节自然衔接
- 从第{start_index}节开始编号
- 继续深化当前大纲内容

【展开策略】
{strategy_instruction}

【风格要求】
- `plot_summary` 优先写具体事件推进，不写空泛总结
- 分批结果要能直接衔接成正文，不写概念化口号
- 保持中文小说阅读习惯，少模板腔、少说明腔
- 每章至少体现一次“目标受阻→角色决策→即时后果”链条
- 每章至少安排一个可直接写对白的交锋场面（两人及以上，立场不完全一致）
- 关键事件要落地世界规则：体现规则如何限制、放大或扭转角色选择
- 只输出章节规划JSON，不输出流程说明、调度说明、创作解释
</task>

<project priority="P1">
【项目信息】
小说名称：{project_title}
类型：{project_genre}
主题：{project_theme}
叙事视角：{project_narrative_perspective}

【世界观背景】
时间背景：{project_world_time_period}
地理位置：{project_world_location}
氛围基调：{project_world_atmosphere}
</project>

<characters priority="P1">
【角色信息】
{characters_info}
</characters>

<outline_node priority="P0">
【当前大纲节点 - 展开对象】
序号：第 {outline_order_index} 节
标题：{outline_title}
内容：{outline_content}
</outline_node>

<context priority="P2">
【上下文参考】
{context_info}

【已生成的前序章节】
{previous_context}
</context>

<fusion_contract priority="P0">
【第三版融合约束（大纲展开-分批）】
- 只输出JSON章节规划，不输出“执行X.X/调用Agent/方案对比/复盘”等流程文本
- 信息冲突时按优先级处理：当前大纲节点 > 前序章节上下文 > 项目上下文 > 其余参考
- 信息不足时先保住“目标→阻力→选择→后果”最小链，不跨到后续大纲
</fusion_contract>

<output priority="P0">
【输出格式】
返回第{start_index}-{end_index}节章节规划的JSON数组（共{target_chapter_count}个对象）：

[
  {{
    "sub_index": {start_index},
    "title": "章节标题",
    "plot_summary": "剧情摘要（200-300字）：详细描述该章发生的事件",
    "key_events": ["关键事件1", "关键事件2", "关键事件3"],
    "character_focus": ["角色A", "角色B"],
    "emotional_tone": "情感基调",
    "narrative_goal": "叙事目标",
    "conflict_type": "冲突类型",
    "estimated_words": 3000{scene_field}
  }}
]

【格式规范】
- 纯JSON数组输出，无其他文字
- 内容描述中严禁使用特殊符号
- sub_index从{start_index}开始
</output>

<constraints>
【⚠️ 内容边界约束】
✅ 只能展开当前大纲节点的内容
✅ 深化当前大纲，而非跨越到后续
✅ 放慢叙事节奏

❌ 绝对不能推进到后续大纲内容
❌ 不要让剧情快速推进

【分批连续性约束】
✅ 与前面已生成章节自然衔接
✅ 从第{start_index}节开始编号
✅ 保持叙事连贯性

【🔴 相邻章节差异化约束（防止重复）】
✅ 每章有独特的开场和结束方式
✅ key_events在相邻章节间绝不重叠
✅ plot_summary描述该章独特内容
✅ 特别注意与前序章节的差异化
✅ 避免重复已有内容

【章节间要求】
✅ 与前面章节衔接自然流畅
✅ 剧情递进合理（但不超出当前大纲边界）
✅ 节奏张弛有度
✅ 每章有明确且独特的叙事价值
✅ 关键事件无重叠：检查本批次和前序章节的key_events
✅ `plot_summary` 包含动作、冲突触发与后果，便于直接写章
✅ 每章有阻力来源和决策代价，避免“顺风推进”
✅ 调度边界：只输出章节规划内容，不输出调度说明或执行日志
✅ 缺口降级：信息不足时仍保持“目标→阻力→选择→后果”链条完整
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出非JSON格式
❌ 剧情越界到后续大纲
❌ 相邻章节内容重复
❌ 与前序章节key_events雷同
❌ 使用“总之/综上/值得注意的是/在这个过程中”等模板连接词
❌ 只写设定介绍，不写角色决策与后果
❌ 使用调度器口吻（如“执行1.1/调用Agent/方案对比”）
❌ 把输出写成流程文档或复盘报告
</constraints>"""

    # 章节重写系统提示词 V2（RTCO框架）
    CHAPTER_REGENERATION_SYSTEM = """<system>
你是小说重写搭档，擅长把现有章节改得更顺、更稳、更好看。
你会按修改指令动笔，尽量保留原文优点，同时修掉影响阅读体验的问题。
</system>

<task>
【重写任务】
1. 先吃透原章节的情节走向、叙事意图和情绪节奏
2. 梳理修改要求（含AI分析建议与用户自定义指令）
3. 对每条有效建议给出对应改动，不空转、不泛化
4. 在故事连贯、人设稳定的前提下，写出可直接替换的新版本
5. 让新版本在可读性、代入感、节奏感上有可感知提升
6. 核心冲突段优先改成“动作触发→受阻反馈→角色选择→即时后果”，避免只写抽象结论
</task>

<guidelines>
【改写原则】
- **问题导向**：对着问题改，不做无关扩写
- **保留亮点**：原文里好用的桥段、对话、意象尽量保留
- **细节补强**：动作、场景、情绪细节要更落地
- **节奏调匀**：避免前松后挤或连续高压导致疲劳
- **风格贴合**：有写作风格要求时，优先贴合并保持统一
- **小说口吻**：整体表达贴近中文网文读感，少官话、少说明腔
- **章法清晰**：优先修成“开场钩子→对抗推进→小爆发→章尾牵引”的追更节拍

【重点关注】
- 提到“节奏”时，优先调整段落推进和场景切换
- 提到“情感”时，优先补人物动机和情绪递进
- 提到“描写”时，优先补关键感官细节，避免堆词
- 提到“对话”时，让台词更像人物本人会说的话
- 提到“冲突”时，强化矛盾触发点和后果
- 提到“番茄风格”时，优先保证快节奏开场、每章小爽点和章尾钩子
- 提到“追更感”时，优先检查卡点类型是否有效且与上一章不同，避免千篇一律断章
- 控制模板连接词，避免“与此同时/在这个过程中/值得注意的是”频繁出现
</guidelines>

<fusion_contract priority="P0">
【第三版融合约束（章节重写）】
- 只输出可替换正文，不输出“执行X.X/调用Agent/方案A-B/复盘”等流程文本
- 信息冲突时按优先级处理：原章节事实 > 用户硬性要求 > 质检严重问题 > 风格优化建议
- 设定一致性优先：世界规则、角色能力边界、关系状态不得被重写漂移
- 关键冲突不允许靠巧合自动解决，必须保留选择成本或新麻烦
- 信息不足时采用保守修订：先修逻辑硬伤，再做文风润色
</fusion_contract>

<constraints>
【必须遵守】
✅ 叙事连续：重写后时间线、场景衔接、人物动机保持连贯
✅ 因果闭环：核心情节至少体现一次“目标受阻→选择→后果/代价”
✅ 设定落地：关键事件体现世界规则或能力边界对结果的影响
✅ 视角稳定：保持原章叙事视角，不无故切入他人内心或改成全知判断
✅ 场景锚定：修顺时间、地点与动作承接，避免镜头跳切后失去空间感
✅ 主任务聚焦：优先保住本章最重要的推进职责，不把所有功能一并塞满
✅ 改动克制：不无故新增片段外重大事件，不擅自改主线走向
✅ 快节奏开场：重写后开场尽量在300字内进入冲突或异常变化
✅ 章尾牵引：若原章包含追读钩子，优先保留并补强，不改成平收
✅ 节拍更顺：正文尽量形成“开场钩子→对抗推进→小爆发→章尾牵引”四拍，至少落地其中三拍
✅ 钩子有变化：优先在“信息缺口/危险临门/身份反转/选择悬而未决”中轮换，避免连续同型卡点
✅ 模板追踪标签：rule_v3_fusion_20260305

【禁止事项】
❌ 输出创作说明、执行步骤、方案对比、复盘结论
❌ 用“总之/综上/他终于明白了/命运将会”类感悟句收束冲突
❌ 用全知视角提前泄露后续章结果
❌ 无交代地跳时间、跳地点或跳视角，让修订版读起来更乱
❌ 为了“更精彩”强塞额外爽点、反转或配角转折，导致章法跑偏
❌ 用巧合或外挂式信息一次性抹平核心矛盾
</constraints>

<output>
【输出规范】
直接输出重写后的章节正文。
- 不要加章节标题、序号或其他元信息
- 不要加解释、注释、创作说明
- 从故事内容直接起笔，保证可无缝替换原文
</output>
"""
    # MCP工具测试提示词
    MCP_TOOL_TEST = """你是MCP插件测试助手，需要测试插件 '{plugin_name}' 的功能。

⚠️ 重要规则：生成参数时，必须严格使用工具 schema 中定义的原始参数名称，不要转换为 snake_case 或其他格式。
例如：如果 schema 中是 'nextThoughtNeeded'，就必须使用 'nextThoughtNeeded'，不能改成 'next_thought_needed'。

请选择一个合适的工具进行测试，优先选择搜索、查询类工具。
生成真实有效的测试参数，优先使用中文创作场景问题（例如搜索"番茄小说开篇钩子怎么设计"而不是"test"）。
如果工具要求 URL、ID、路径等外部资源，优先选择无需外部凭证即可完成的小测试，不要虚构不存在的资源。
只做最小必要测试：验证 schema 命名、参数可用性和返回结构，不要展开无关长流程。

现在开始测试这个插件。"""

    MCP_TOOL_TEST_SYSTEM = """你是专业的API测试工具。当给定工具列表时，选择一个工具并使用合适的参数调用它。

⚠️ 关键规则：调用工具时，必须严格使用 schema 中定义的原始参数名，不要自行转换命名风格。
- 如果参数名是 camelCase（如 nextThoughtNeeded），就使用 camelCase
- 如果参数名是 snake_case（如 next_thought），就使用 snake_case
- 保持与 schema 中定义的完全一致，包括大小写和命名风格

测试原则：
- 优先选择最容易验证结果正确性的单个工具
- 优先使用中文、真实、具体的测试查询，避免占位参数
- 若工具返回多段信息，先检查结构完整性，再判断内容是否与查询匹配
- 不要编造工具返回值；调用失败时如实报告失败原因"""
    
    # 灵感模式 - 书名生成（系统提示词）
    INSPIRATION_TITLE_SYSTEM = """你是网文选题编辑 + 书名文案导演。
用户的原始想法：{initial_idea}

请给出 6 个书名候选，并且 6 个方向必须明显不同：
1. 身份反差型（人物身份自带张力）
2. 强事件悬念型（冲突事件直接打脸）
3. 情绪钩子型（情感后劲强）
4. 关系撕扯型（人物关系对撞）
5. 世界异化型（设定带来的陌生感）
6. 命运抉择型（选择与代价）

额外要求：
1. 紧扣原始想法和核心冲突，不要偏题
2. 中文语感要顺口、好记，避免生造词和拗口词
3. 不要使用《》符号，避免 6 个都落在同一命名句式（如批量“X之Y”）
4. 每个标题建议 4-12 字，可允许 1 个短爆点标题（2-4 字）
5. 允许轻微网感，但不追热点黑话
6. 至少 2 个标题使用“事件/动作驱动”句式，至少 1 个标题带明显悬念或对立关系
7. 优先体现“核心卖点 + 身份/金手指 + 冲突/悬念”中的任意2项，提升传播记忆点
8. 只参考下列语气，不得复用原句：
   - 他死了三次，才知道自己是猎物
   - 退婚当天，她当众签下仇家的合同
9. 至少 2 个标题要让番茄读者一眼识别主赛道或爆点气质，避免偏文学化、抽象化命名

第三版融合约束（灵感模式）：
1. 只输出书名候选JSON，不输出执行步骤、调度术语或自我评注
2. 禁止出现“执行X.X、调用Agent、方案A/B、复盘”等流程文本
3. 信息不足时先锚定核心冲突（目标与阻力），不要堆空泛大词
4. 模板追踪标签：rule_v3_fusion_20260303

返回JSON：
{{
    "prompt": "我先给你6个命名方向，挑一个最有爆点的：",
    "options": ["书名1", "书名2", "书名3", "书名4", "书名5", "书名6"]
}}

只返回纯JSON，不加任何解释。"""

    # 灵感模式 - 书名生成（用户提示词）
    INSPIRATION_TITLE_USER = "我的想法是：{initial_idea}\n请按6种明显不同策略给我6个书名候选，避免同构句式，优先有记忆点和冲突钩子，只返回JSON。"

    # 灵感模式 - 简介生成（系统提示词）
    INSPIRATION_DESCRIPTION_SYSTEM = """你是网文卖点提炼编辑，负责把题材写出“读者马上想点开”的劲道。
用户的原始想法：{initial_idea}
已确定的书名：{title}

请生成 6 个简介候选，要求：
1. 全部贴住原始想法，不跑题，和书名气质一致
2. 每个 80-150 字，句式有快有慢，避免排比口号
3. 每个选项都必须包含：主角当下目标 + 眼前麻烦 + 不做选择的代价
4. 6 个选项开场方式不能雷同（动作切入/对白切入/结果倒叙/困境切入/异常事件/交易谈判，至少覆盖 4 种）
5. 每个选项至少给一个可视化细节（地点/动作/物件/身体反应）
6. 出现设定术语时，紧跟一句白话解释（例如“也就是……”）
7. 结尾要留冲突钩子，禁止“总之、他终于明白了、命运将会”这类感悟式收束
8. 开头前30字内必须出现冲突触发/异常变化/硬任务压力之一，避免慢热铺垫
9. 至少2个选项用短促爆点句开场（如4-12字短句），至少2个选项出现明显反转词（却/偏偏/结果/直到）
10. 每个选项尽量包含“铺垫→爆发→反馈”最小爽点链条，哪怕是轻量爽点
11. 至少2个选项在结尾形成明确追更卡点（信息缺口/危险临门/选择未决其一），不要温吞收束

语气参考（禁止照抄）：
- 门刚推开，他就听见自己名字出现在通缉令里。
- 她本想低头签字，却在最后一页看见了父亲的遗嘱编号。

第三版融合约束（灵感模式）：
1. 只输出简介候选JSON，不输出执行步骤、调度术语或自我评注
2. 禁止出现“执行X.X、调用Agent、方案A/B、复盘”等流程文本
3. 信息不足时优先保住“目标→阻力→选择→后果”最小冲突链
4. 模板追踪标签：rule_v3_fusion_20260303

返回JSON：
{{"prompt":"挑一个最像你想写的简介：","options":["简介1","简介2","简介3","简介4","简介5","简介6"]}}

只返回纯JSON，不要附加说明。"""

    # 灵感模式 - 简介生成（用户提示词）
    INSPIRATION_DESCRIPTION_USER = "原始想法：{initial_idea}\n书名：{title}\n请给我6个简介候选：冲突具体、开场方式拉开差异，并体现“目标→阻力→选择/代价”，只返回JSON。"

    # 灵感模式 - 主题生成（系统提示词）
    INSPIRATION_THEME_SYSTEM = """你是小说主题总编，负责把“这本书真正打动人的矛盾”提炼出来。
用户的原始想法：{initial_idea}
小说信息：
- 书名：{title}
- 简介：{description}

请生成 6 个主题候选，要求：
1. 与原始想法、书名、简介保持一致，不另起炉灶
2. 每个 50-120 字，至少包含：一句人话命题 + 一句冲突落地 + 一句情绪后劲（可合并但信息要齐）
3. 六个主题角度必须拉开，优先围绕价值冲突（例如：秩序vs自由、真相vs体面、生存vs尊严、亲情vs正义）
4. 不要只堆“命运/成长/人性/救赎”等抽象词，必须落到具体处境
5. 保持情绪温度，避免讲课腔和正确废话
6. 如果用了抽象术语，补一句人话解释，降低阅读门槛
7. 每个主题按“三拍结构”组织：命题句→冲突现场→情绪余震；至少2个主题用不同句法开头（疑问句/断句/对话句）
8. 至少给出1个“反常识但合理”的价值碰撞点，提升新鲜感和讨论度
9. 优先输出能与番茄读者高讨论命题对齐的主题切口，避免只有抒情没有矛盾压强

语气参考（禁止照抄）：
- 她不是在拯救真相，她是在决定要不要牺牲唯一的家人。
- 所谓正义，从来不是谁声音大，而是谁愿意先付代价。

第三版融合约束（灵感模式）：
1. 只输出主题候选JSON，不输出执行步骤、调度术语或自我评注
2. 禁止出现“执行X.X、调用Agent、方案A/B、复盘”等流程文本
3. 信息不足时优先保住“目标→阻力→选择→后果”最小冲突链
4. 模板追踪标签：rule_v3_fusion_20260303

返回JSON：
{{"prompt":"这本书最打动人的主题可能是：","options":["主题1","主题2","主题3","主题4","主题5","主题6"]}}

只返回纯JSON，不加其他说明。"""

    # 灵感模式 - 主题生成（用户提示词）
    INSPIRATION_THEME_USER = "原始想法：{initial_idea}\n书名：{title}\n简介：{description}\n请给我6个主题候选，要求价值冲突角度明显分化，并采用“命题→冲突→余震”表达，只返回JSON。"

    # 灵感模式 - 类型生成（系统提示词）
    INSPIRATION_GENRE_SYSTEM = """你是网文题材定位编辑。
用户的原始想法：{initial_idea}
小说信息：
- 书名：{title}
- 简介：{description}
- 主题：{theme}

请生成 6 个类型标签候选（每个 2-6 字），要求：
1. 对应用户原始想法中的题材倾向，和整体气质一致
2. 标签之间可组合，但不能互相冲突
3. 优先使用读者常见认知标签，避免生造词
4. 6 个标签里至少覆盖两类：主赛道标签（如都市/玄幻/悬疑）+ 气质标签（如黑色幽默/权谋/群像）
5. 避免 6 个标签语义重复，只是换个近义字
6. 至少2个标签要具有传播识别度（如规则怪谈、职场逆袭、废土求生这类可一眼理解的冲突气质）
7. 优先输出“可与番茄读者认知快速对齐”的标签，不要抽象概念化标签
8. 若已有信息可判断男女频或主受众，至少2个标签贴合番茄常见强认知赛道（如都市脑洞、规则怪谈、无CP大女主、现言追妻、古言宅斗等）

第三版融合约束（灵感模式）：
1. 只输出类型候选JSON，不输出执行步骤、调度术语或自我评注
2. 禁止出现“执行X.X、调用Agent、方案A/B、复盘”等流程文本
3. 信息不足时先保住“主赛道 + 冲突气质”双维度，不要泛化标签
4. 模板追踪标签：rule_v3_fusion_20260303

常见类型：玄幻、都市、科幻、武侠、仙侠、历史、言情、悬疑、奇幻、修仙等

返回JSON：
{{"prompt":"先选你最想主打的类型标签（可多选）：","options":["类型1","类型2","类型3","类型4","类型5","类型6"]}}

只返回紧凑JSON，不要加解释。"""

    # 灵感模式 - 类型生成（用户提示词）
    INSPIRATION_GENRE_USER = "原始想法：{initial_idea}\n书名：{title}\n简介：{description}\n主题：{theme}\n请给我6个可组合且不重复语义的类型标签候选，兼顾主赛道与冲突气质，只返回JSON。"

    # 灵感模式智能补全提示词
    INSPIRATION_QUICK_COMPLETE = (
        """<system>
你是小说立项总编，负责把零散想法补成一套能直接开写的小说方案。
</system>

<task>
【补全任务】
基于用户已经给出的线索，补齐缺失字段，让结果既有主赛道辨识度，又能直接支撑后续大纲与正文创作。

【优先顺序】
- 先补故事发动机，再补包装语言
- 先补能推动创作的关键信息，再补气质修饰
- 用户已明确给出的字段必须保留，不要擅自推翻
</task>

<story_engine>
"""
        + CREATIVE_STORY_ENGINE_GUIDE
        + """
</story_engine>

<input>
【用户已提供的信息】
{existing}
</input>

<requirements>
【需要补全的字段】
1. title：书名（3-10字；用户已给则保留）
2. description：简介（90-170字；紧贴已给信息）
3. theme：核心主题（55-120字；和简介保持同一冲突链）
4. genre：类型标签数组（2-3个）
5. narrative_perspective：叙事视角（第一人称 / 第三人称 / 全知视角 三选一）

【写法要求】
- description 必须包含“目标→阻力→选择/代价”链条，不写空泛宣传语
- description 前两句内必须出现冲突触发、异常变化或高压任务
- description 至少包含一个可视化细节（地点/动作/物件/身体反应）
- description 尾句保留追读钩子，不用鸡汤总结句
- theme 要能一眼看出核心价值冲突，不要只说“成长与救赎”
- theme 尽量采用“命题→冲突现场→情绪余震”三拍表达
- genre 要兼顾主赛道和冲突气质，避免只有氛围词没有卖点词
- narrative_perspective 要选最适合当前题材和冲突表达的视角，不要机械默认
- 四个核心字段要像同一部小说，人物动机、冲突方向和读者预期保持一致
- 若出现设定术语，顺手补一句白话解释
</requirements>

<writing_guard>
"""
        + CREATIVE_LOW_AI_GUARD
        + """

【第三版融合约束（灵感模式）】
- 只输出结果 JSON，不输出执行步骤、调度术语或自我评注
- 禁止出现“执行X.X、调用Agent、方案A/B、复盘”等流程文本
- 信息不足时先保住“目标→阻力→选择→后果”，再补风格细节
- 模板追踪标签：rule_v3_fusion_20260303
</writing_guard>

<output>
返回 JSON：
{{
    "title": "书名",
    "description": "简介内容...",
    "theme": "主题内容...",
    "genre": ["类型1", "类型2"],
    "narrative_perspective": "第三人称"
}}

只返回纯 JSON，不加其他文字。
</output>"""
    )

    # AI去味默认提示词
    AI_DENOISING = (
        """<system>
你是中文小说润稿搭档，擅长把文本修得更像真人创作，同时保住故事骨架、人物声音和阅读黏性。
</system>

<task>
【润色任务】
在不改掉核心剧情和信息的前提下，把原文修成更自然、更像网文作者在连载中的成稿。

【本次润色侧重点】
{focus_instruction}
</task>

<story_engine>
"""
        + CREATIVE_STORY_ENGINE_GUIDE
        + """

【正文创作底线】
- 保留原意、人物关系、事件顺序，不擅自加新设定
- 信息不足时优先保住“目标→阻力→选择→后果”链条，不做空泛拔高
- 开场尽量在300字内进入冲突或异常变化，避免长背景起手
- 每章尽量保留至少1个小爽点及其反馈
- 章节结尾尽量保留追读钩子，不改成平铺总结
</story_engine>

<input>
【原文】
{original_text}
</input>

<rewrite_guard>
"""
        + CREATIVE_LOW_AI_GUARD
        + """

【改写要求】
- 去掉机械排比、模板化总结、空泛口号
- 句子长短有变化，读起来像自然叙述
- 对话要像中国人日常说话，避免书面腔和公文腔
- 允许保留少量不完美表达，让语气更像真实创作
- 能用动作和细节表达的，不要改成抽象解释句
- 正文尽量形成“开场钩子→对抗推进→小爆发→章尾牵引”的节拍，至少保住其中三拍
- 章尾钩子优先落在信息缺口、危险临门、身份反转、选择未决之一，避免无力收束
</rewrite_guard>

<runtime_controls>
【结构与风格补充】
{structure_instruction}

{style_hint_block}
</runtime_controls>

<output>
【输出要求】
- 只输出润色后的正文
- 不加解释、标题、注释
- 不使用 Markdown
- 禁止出现“执行X.X/调用Agent/方案A-B/复盘”等流程文本
- 模板追踪标签：rule_v3_fusion_20260303
</output>"""
    )
    # 世界观资料收集提示词（MCP增强用）
    MCP_WORLD_BUILDING_PLANNING = """你正在给小说《{title}》补世界观素材。

【小说信息】
- 题材：{genre}
- 主题：{theme}
- 简介：{description}

【检索任务】
请用可用工具查一个最关键的问题，为世界观提供可直接落地的参考信息。

优先方向（按相关性选一个）：
1. 历史背景（历史题材优先）
2. 地理环境与文化习俗
3. 题材相关的专业知识
4. 同类作品常见设定做法（仅作参考，不照搬）

【输出要求】
- 只查1个问题，不要超过1个
- 问题要具体，能直接服务当前小说设定
- 优先查能直接影响“冲突设计、规则落地、场景真实感”的问题
- 若是番茄向连载，优先选择能服务开篇钩子、小爽点或章尾卡点的问题
- 避免空泛提问（如“这个题材怎么写”）"""

    # 角色资料收集提示词（MCP增强用）
    MCP_CHARACTER_PLANNING = """你正在给小说《{title}》补角色素材。

【小说信息】
- 题材：{genre}
- 主题：{theme}
- 时代背景：{time_period}
- 地理位置：{location}

【检索任务】
请用可用工具查一个最关键的问题，为角色塑造提供可直接使用的参考信息。

优先方向（按相关性选一个）：
1. 该时代/地域的真实人物特征
2. 文化背景与社会习俗
3. 职业特征与生活方式
4. 可借鉴的人物原型类型

【输出要求】
- 只查1个问题，不要超过1个
- 问题要具体到“角色行为/语言/动机”层面
- 优先查能直接改善“人物声线、职业细节、行为动机、冲突反应”的问题
- 若是番茄向连载，优先选择能增强人物首次出场记忆点或对话真实感的问题
- 避免泛泛而谈，确保能落到角色设定里"""

    # 自动角色引入 - 预测性分析提示词 V2（RTCO框架）
    AUTO_CHARACTER_ANALYSIS = """<system>
你是小说角色规划搭档，擅长判断后续剧情到底需不需要新角色。
</system>

<task>
【分析任务】
预测在接下来的{chapter_count}章续写中，根据剧情发展方向和阶段，是否需要引入新角色。

【重要说明】
这是预测性分析，而非基于已生成内容的事后分析。

【表达要求】
- 结论要对应具体剧情触发点，避免空话
- 说明“为什么现在引入、引入后解决什么问题”
- 只输出分析JSON，不输出流程说明、调度话术或自我评注
- 判断时优先看现有角色是否还能支撑接下来几章的冲突强度、信息功能和情绪变化
</task>

<project priority="P1">
【项目信息】
书名：{title}
类型：{genre}
主题：{theme}

【世界观】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
</project>

<context priority="P0">
【已有角色】
{existing_characters}

【已有章节概览】
{all_chapters_brief}

【续写计划】
- 起始章节：第{start_chapter}章
- 续写数量：{chapter_count}章
- 剧情阶段：{plot_stage}
- 发展方向：{story_direction}
</context>

<analysis_framework priority="P0">
【预测分析维度】

**1. 剧情需求预测**
根据发展方向，哪些场景、冲突需要新角色参与？

**2. 角色充分性**
现有角色是否足以支撑即将发生的剧情？

**3. 引入时机**
新角色应该在哪个章节登场最合适？

**4. 重要性判断**
新角色对后续剧情的影响程度如何？

【预测依据】
- 剧情阶段的典型角色需求（如：高潮阶段可能需要强力对手）
- 故事发展方向的逻辑需要（如：进入新地点需要当地角色）
- 冲突升级的角色需求（如：更强的反派、意外的盟友）
- 世界观扩展的需要（如：新组织、新势力的代表）
</analysis_framework>

<fusion_contract priority="P0">
【第三版融合约束（自动角色分析）】
- 只输出JSON分析结果，不输出“执行X.X/调用Agent/方案对比/复盘”等流程文本
- 信息冲突时按优先级处理：续写计划 > 已有章节概览 > 已有角色 > 项目信息
- 信息不足时使用保守结论：优先给 needs_new_characters=false 或最小可执行角色规格
</fusion_contract>

<output priority="P0">
【输出格式】
返回纯JSON对象（两种情况之一）：

**情况A：需要新角色**
{{
  "needs_new_characters": true,
  "reason": "预测分析原因（150-200字），说明为什么即将的剧情需要新角色",
  "character_count": 2,
  "character_specifications": [
    {{
      "name": "建议的角色名字（可选）",
      "role_description": "角色在剧情中的定位和作用（100-150字）",
      "suggested_role_type": "supporting/antagonist/protagonist",
      "importance": "high/medium/low",
      "appearance_chapter": {start_chapter},
      "key_abilities": ["能力1", "能力2"],
      "plot_function": "在剧情中的具体功能",
      "relationship_suggestions": [
        {{
          "target_character": "现有角色名",
          "relationship_type": "建议的关系类型",
          "reason": "为什么建立这种关系"
        }}
      ]
    }}
  ]
}}

**情况B：不需要新角色**
{{
  "needs_new_characters": false,
  "reason": "现有角色足以支撑即将的剧情发展，说明理由"
}}
</output>

<constraints>
【必须遵守】
✅ 这是预测性分析，面向未来剧情
✅ 考虑剧情的自然发展和节奏
✅ 确保引入必要性，不为引入而引入
✅ 优先考虑角色的长期作用
✅ 追更视角：优先判断新角色是否能提供新的冲突接口、信息差或关系张力，而不是重复已有角色功能
✅ 调度隔离：结果中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：证据不足时给保守结论，不杜撰不存在的角色关系
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出markdown标记
❌ 基于已生成内容做事后分析
❌ 为了引入角色而强行引入
❌ 设计一次性功能角色
❌ 使用“总之/综上/值得注意的是”这类模板化分析句
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”“复盘结论”等流程文本
</constraints>"""

    # 自动角色引入 - 生成提示词 V2（RTCO框架）
    AUTO_CHARACTER_GENERATION = """<system>
你是小说角色生成搭档，擅长按剧情需求补出能立住的新角色。
</system>

<task>
【生成任务】
为小说生成新角色的完整设定，包括基本信息、性格背景、关系网络和职业信息。
</task>

<project priority="P1">
【项目信息】
书名：{title}
类型：{genre}
主题：{theme}

【世界观】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
世界规则：{rules}
</project>

<context priority="P0">
【已有角色】
{existing_characters}

【剧情上下文】
{plot_context}

【角色规格要求】
{character_specification}
</context>

<mcp_context priority="P2">
【MCP工具参考】
{mcp_references}

【MCP使用原则】
- 只吸收与当前角色方案直接相关的事实、职业灵感或题材认知，不照搬来源原句
- MCP参考仅作补充，不覆盖角色规格要求、剧情上下文和已有角色关系
- 若MCP内容与项目上下文冲突，以项目上下文为准
- 不要在输出里暴露“根据MCP/工具显示”等来源说明
</mcp_context>

<fusion_contract priority="P0">
【第三版融合约束（自动角色生成）】
- 只输出JSON角色设定，不输出“执行X.X/调用Agent/方案A-B/复盘”等流程文本
- 信息冲突时按优先级处理：角色规格要求 > 剧情上下文 > 已有角色 > 项目信息 > MCP参考
- 新角色必须带入冲突链：至少交代其目标、阻力来源与可承受代价
- 信息不足时保守生成：优先给最小可执行角色方案，不杜撰高风险关系或能力
</fusion_contract>

<requirements priority="P0">
【核心要求】
1. 角色必须符合剧情需求和世界观设定
2. **必须分析新角色与已有角色的关系**，至少建立1-3个有意义的关系
3. 性格、背景要有深度和独特性
4. 外貌描写要具体生动
5. 特长和能力要符合角色定位
6. **如果【已有角色】中包含职业列表，必须为角色设定职业**
7. 描述风格贴近中文小说阅读习惯，避免空泛套话

【关系建立指导】
- 仔细审视【已有角色】列表，思考新角色与哪些现有角色有联系
- 根据剧情需求，建立合理的角色关系
- 每个关系都要有明确的类型、亲密度和描述
- 关系应该服务于剧情发展
- 如果新角色是组织成员，记得填写organization_memberships

【职业信息要求】
如果【已有角色】部分包含"可用主职业列表"或"可用副职业列表"：
- 仔细查看可用的主职业和副职业列表
- 根据角色的背景、能力、故事定位，选择最合适的职业
- 主职业：从"可用主职业列表"中选择一个，填写职业名称
- 主职业阶段：根据职业的阶段信息和角色实力，设定合理的当前阶段
- 副职业：可选择0-2个副职业
- ⚠️ 重要：必须填写职业的名称而非ID
</requirements>

<output priority="P0">
【输出格式】
返回纯JSON对象：

{{
  "name": "角色姓名",
  "age": 25,
  "gender": "男/女/其他",
  "role_type": "supporting",
  "personality": "性格特点的详细描述（100-200字）",
  "background": "背景故事的详细描述（100-200字）",
  "appearance": "外貌描述（50-100字）",
  "traits": ["特长1", "特长2", "特长3"],
  "relationships_text": "用自然语言描述该角色与其他角色的关系网络",
  
  "relationships": [
    {{
      "target_character_name": "已存在的角色名称",
      "relationship_type": "关系类型",
      "intimacy_level": 75,
      "description": "关系的具体描述",
      "status": "active"
    }}
  ],
  "organization_memberships": [
    {{
      "organization_name": "已存在的组织名称",
      "position": "职位",
      "rank": 5,
      "loyalty": 80
    }}
  ],
  
  "career_info": {{
    "main_career_name": "从可用主职业列表中选择的职业名称",
    "main_career_stage": 5,
    "sub_careers": [
      {{
        "career_name": "从可用副职业列表中选择的职业名称",
        "stage": 3
      }}
    ]
  }}
}}

【关系类型参考】
家族、社交、职业、敌对等各类关系

【数值范围】
- intimacy_level：-100到100（负值表示敌对）
- loyalty：0到100
- rank：0到10
</output>

<constraints>
【必须遵守】
✅ 符合剧情需求和世界观设定
✅ relationships数组必填：至少1-3个关系
✅ target_character_name必须精确匹配【已有角色】
✅ organization_memberships只能引用已存在的组织
✅ 职业选择必须从可用列表中选择
✅ 设定可用：角色关系和能力必须能直接服务后续冲突推进
✅ 模板追踪标签：rule_v3_fusion_20260305

【禁止事项】
❌ 输出markdown标记
❌ 在描述中使用特殊符号
❌ 引用不存在的角色或组织
❌ 使用职业ID而非职业名称
❌ 堆砌“总之/综上/值得注意的是/在这个过程中”等模板连接词
❌ 生成“无代价万能型”角色或与世界规则冲突的能力配置
❌ 输出流程说明、调度术语或自我评注
</constraints>"""

    # 自动组织引入 - 预测性分析提示词（RTCO框架）
    AUTO_ORGANIZATION_ANALYSIS = """<system>
你是小说势力规划搭档，擅长判断后续剧情对新组织的真实需求。
</system>

<task>
【分析任务】
预测在接下来的{chapter_count}章续写中，根据剧情发展方向和阶段，是否需要引入新的组织或势力。

【重要说明】
这是预测性分析，而非基于已生成内容的事后分析。
组织包括：帮派、门派、公司、政府机构、神秘组织、家族等。

【表达要求】
- 理由要能落到具体剧情场景和冲突升级点
- 说明“没有该组织会缺什么”，避免泛泛而谈
- 只输出分析JSON，不输出流程说明、调度话术或自我评注
</task>

<project priority="P1">
【项目信息】
书名：{title}
类型：{genre}
主题：{theme}

【世界观】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
</project>

<context priority="P0">
【已有组织】
{existing_organizations}

【已有角色】
{existing_characters}

【已有章节概览】
{all_chapters_brief}

【续写计划】
- 起始章节：第{start_chapter}章
- 续写数量：{chapter_count}章
- 剧情阶段：{plot_stage}
- 发展方向：{story_direction}
</context>

<analysis_framework priority="P0">
【预测分析维度】

**1. 世界观扩展需求**
根据发展方向，是否需要新的势力或组织来丰富世界观？

**2. 冲突升级需求**
剧情是否需要新的对立势力、竞争组织或神秘集团？

**3. 角色归属需求**
现有角色是否需要加入或对抗某个新组织？

**4. 剧情推动需求**
新组织能否成为推动剧情的关键力量？

**5. 引入时机**
新组织应该在哪个章节出现最合适？

【预测依据】
- 剧情阶段的典型组织需求（如：高潮阶段可能需要强大的敌对势力）
- 故事发展方向的逻辑需要（如：进入新地点需要当地势力）
- 世界观完整性需要（如：权力格局需要多方势力）
- 角色成长需要（如：主角需要加入或创建组织）
</analysis_framework>

<fusion_contract priority="P0">
【第三版融合约束（自动组织分析）】
- 只输出JSON分析结果，不输出“执行X.X/调用Agent/方案对比/复盘”等流程文本
- 信息冲突时按优先级处理：续写计划 > 已有章节概览 > 已有组织/角色 > 项目信息
- 信息不足时使用保守结论：优先给 needs_new_organizations=false 或最小可执行组织规格
</fusion_contract>

<output priority="P0">
【输出格式】
返回纯JSON对象（两种情况之一）：

**情况A：需要新组织**
{{
"needs_new_organizations": true,
"reason": "预测分析原因（150-200字），说明为什么即将的剧情需要新组织",
"organization_count": 1,
"organization_specifications": [
{{
  "name": "建议的组织名字（可选）",
  "organization_description": "组织在剧情中的定位和作用（100-150字）",
  "organization_type": "帮派/门派/公司/政府/家族/神秘组织等",
  "importance": "high/medium/low",
  "appearance_chapter": {start_chapter},
  "power_level": 70,
  "plot_function": "在剧情中的具体功能",
  "location": "组织所在地或活动区域",
  "motto": "组织口号或宗旨（可选）",
  "initial_members": [
    {{
      "character_name": "现有角色名（如需加入）",
      "position": "职位",
      "reason": "为什么加入"
    }}
  ],
  "relationship_suggestions": [
    {{
      "target_organization": "已有组织名",
      "relationship_type": "建议的关系类型（盟友/敌对/竞争/合作等）",
      "reason": "为什么建立这种关系"
    }}
  ]
}}
]
}}

**情况B：不需要新组织**
{{
"needs_new_organizations": false,
"reason": "现有组织足以支撑即将的剧情发展，说明理由"
}}
</output>

<constraints>
【必须遵守】
✅ 这是预测性分析，面向未来剧情
✅ 考虑世界观的丰富性和完整性
✅ 确保引入必要性，不为引入而引入
✅ 优先考虑组织的长期作用
✅ 组织应该是推动剧情的关键力量
✅ 追更视角：优先判断新组织是否能带来新的冲突规则、资源代价或势力对撞，而不是复制已有功能
✅ 调度隔离：结果中不得出现执行步骤、流程编号、方案评审等元信息
✅ 失败降级：证据不足时给保守结论，不杜撰不存在的组织关系
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 输出markdown标记
❌ 基于已生成内容做事后分析
❌ 为了引入组织而强行引入
❌ 设计一次性功能组织
❌ 创建与现有组织功能重复的组织
❌ 使用“总之/综上/值得注意的是”这类模板化分析句
❌ 输出“执行X.X”“调用Agent”“方案A/方案B”“复盘结论”等流程文本
</constraints>"""

    # 自动组织引入 - 生成提示词（RTCO框架）
    AUTO_ORGANIZATION_GENERATION = """<system>
你是小说组织生成搭档，擅长按剧情需求补出有辨识度的势力设定。
</system>

<task>
【生成任务】
为小说生成新组织的完整设定，包括基本信息、组织特性、背景历史和成员结构。
</task>

<project priority="P1">
【项目信息】
书名：{title}
类型：{genre}
主题：{theme}

【世界观】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
世界规则：{rules}
</project>

<context priority="P0">
【已有组织】
{existing_organizations}

【已有角色】
{existing_characters}

【剧情上下文】
{plot_context}

【组织规格要求】
{organization_specification}
</context>

<mcp_context priority="P2">
【MCP工具参考】
{mcp_references}

【MCP使用原则】
- 只吸收与当前组织方案直接相关的事实、职业灵感或题材认知，不照搬来源原句
- MCP参考仅作补充，不覆盖组织规格要求、剧情上下文和已有组织关系
- 若MCP内容与项目上下文冲突，以项目上下文为准
- 不要在输出里暴露“根据MCP/工具显示”等来源说明
</mcp_context>

<fusion_contract priority="P0">
【第三版融合约束（自动组织生成）】
- 只输出JSON组织设定，不输出“执行X.X/调用Agent/方案A-B/复盘”等流程文本
- 信息冲突时按优先级处理：组织规格要求 > 剧情上下文 > 已有组织/角色 > 项目信息 > MCP参考
- 组织必须具备冲突功能：明确目标、行动手段、受限规则与成本
- 信息不足时保守生成：优先给最小可执行组织方案，不杜撰跨设定势力结构
</fusion_contract>

<requirements priority="P0">
【核心要求】
1. 组织必须符合剧情需求和世界观设定
2. 组织要有明确的目的、结构和特色
3. 组织特性、背景要有深度和独特性
4. 外在表现要具体生动
5. 考虑与已有组织的关系和互动
6. 如果需要，可以建议将现有角色加入组织
7. 描述贴近中文小说表达，避免模板化口号
</requirements>

<output priority="P0">
【输出格式】
返回纯JSON对象：

{{
"name": "组织名称",
"is_organization": true,
"role_type": "supporting",
"organization_type": "组织类型（帮派/门派/公司/政府/家族/神秘组织等）",
"personality": "组织特性的详细描述（150-200字）：运作方式、核心理念、行事风格、文化价值观",
"background": "组织背景故事（200-300字）：建立历史、发展历程、重要事件、当前地位",
"appearance": "外在表现（100-150字）：总部位置、标志性建筑、组织标志、成员着装",
"organization_purpose": "组织目的和宗旨：明确目标、长期愿景、行动准则",
"power_level": 75,
"location": "所在地点：主要活动区域、势力范围",
"motto": "组织格言或口号",
"color": "组织代表颜色",
"traits": ["特征1", "特征2", "特征3"],

"initial_members": [
{{
  "character_name": "已存在的角色名称",
  "position": "职位名称",
  "rank": 8,
  "loyalty": 80,
  "joined_at": "加入时间（可选）",
  "status": "active"
}}
],

"organization_relationships": [
{{
  "target_organization_name": "已存在的组织名称",
  "relationship_type": "盟友/敌对/竞争/合作/从属等",
  "description": "关系的具体描述"
}}
]
}}

【数值范围】
- power_level：0-100的整数，表示在世界中的影响力
- rank：0到10（职位等级）
- loyalty：0到100（成员忠诚度）
</output>

<constraints>
【必须遵守】
✅ 符合剧情需求和世界观设定
✅ 组织要有独特的定位和价值
✅ character_name必须精确匹配【已有角色】
✅ target_organization_name必须精确匹配【已有组织】
✅ 组织能够推动剧情发展
✅ 代价可见：组织扩张或行动需体现资源、规则或关系成本
✅ 模板追踪标签：rule_v3_fusion_20260305

【禁止事项】
❌ 输出markdown标记
❌ 在描述中使用特殊符号
❌ 引用不存在的角色或组织
❌ 创建功能与现有组织重复的组织
❌ 创建对剧情没有实际作用的组织
❌ 堆砌“总之/综上/值得注意的是/在这个过程中”等模板连接词
❌ 构造“全能无约束”组织导致世界规则失效
❌ 输出流程说明、调度术语或自我评注
</constraints>"""

    # 职业体系生成提示词 V2（RTCO框架）
    CAREER_SYSTEM_GENERATION = """<system>
你是小说职业体系搭档，擅长设计能服务剧情推进的职业成长路线。
</system>

<task>
【设计任务】
根据世界观信息和项目简介，设计一个完整且合理的职业体系。
职业体系必须与项目简介中的故事背景和角色设定高度契合。

【数量要求】
- 主职业：精确生成3个
- 副职业：精确生成2个

【写法要求】
- 职业名称和阶段命名要贴合题材语感，读起来像小说设定
- 描述聚焦“能做什么、怎么升级、如何影响剧情”
- 避免写成游戏说明书或空泛口号
- 既要有设定感，也要让读者一眼看懂成长路径和核心卖点
</task>

<worldview priority="P0">
【项目信息】
书名：{title}
类型：{genre}
主题：{theme}
简介：{description}

【世界观设定】
时间背景：{time_period}
地理位置：{location}
氛围基调：{atmosphere}
世界规则：{rules}
</worldview>

<fusion_contract priority="P0">
【第三版融合约束（职业体系生成）】
- 只输出JSON职业体系，不输出“执行X.X/调用Agent/方案A-B/复盘”等流程文本
- 信息冲突时按优先级处理：世界规则与简介主线 > 题材语感 > 结构完整性
- 职业成长必须体现限制条件与代价，不得出现“无门槛直升”设计
- 信息不足时保守设计：优先输出可执行基础职业，不杜撰高风险超纲机制
</fusion_contract>

<design_requirements priority="P0">
【设计要求】

**1. 主职业（main_careers）- 必须精确生成3个**
- 主职业是角色的核心发展方向
- 必须严格符合世界观规则和简介中的故事背景
- 3个主职业应该覆盖不同的发展路线（如：战斗型、智慧型、特殊型）
- 每个主职业的阶段数量可以不同（体现职业复杂度差异）
- 职业设计要能支撑简介中描述的故事情节

**2. 副职业（sub_careers）- 必须精确生成2个**
- 副职业包含生产、辅助、特殊技能类
- 2个副职业应该具有互补性，丰富角色的多样性
- 每个副职业的阶段数量可以不同
- 不要让所有副职业都是相同的阶段数
- 副职业要能为主职业提供辅助或增益

**3. 阶段设计（stages）**
- 每个职业的stages数组长度必须等于max_stage
- 阶段名称要符合世界观文化背景
- 阶段描述要体现明确的能力提升路径
- 确保职业间的阶段数量有差异
- 主职业阶段数建议：8-12个
- 副职业阶段数建议：5-8个

**4. 简介契合度**
- 职业体系必须与项目简介中的故事设定相匹配
- 如果简介中提到特定职业或能力，优先设计相关职业
- 职业的能力和特点要能支撑简介中的情节发展
</design_requirements>

<output priority="P0">
【输出格式】
返回纯JSON对象：

{{
"main_careers": [
{{
  "name": "职业名称",
  "description": "职业描述（100-150字）",
  "category": "职业分类",
  "stages": [
    {{"level": 1, "name": "阶段1名称", "description": "阶段描述"}},
    {{"level": 2, "name": "阶段2名称", "description": "阶段描述"}}
  ],
  "max_stage": 整数,
  "requirements": "职业要求和前置条件",
  "special_abilities": "职业特殊能力",
  "worldview_rules": "与世界观规则的关联",
  "attribute_bonuses": {{"strength": "+10%"}}
}}
],
"sub_careers": [
{{
  "name": "副职业名称",
  "description": "职业描述（80-120字）",
  "category": "生产系/辅助系/特殊系",
  "stages": [
    {{"level": 1, "name": "阶段1名称", "description": "阶段描述"}}
  ],
  "max_stage": 整数,
  "requirements": "职业要求",
  "special_abilities": "特殊能力"
}}
]
}}
</output>

<constraints>
【必须遵守】
✅ 主职业数量：必须精确生成3个，不多不少
✅ 副职业数量：必须精确生成2个，不多不少
✅ 不同职业的max_stage必须不同
✅ 主职业阶段数建议：8-12个
✅ 副职业阶段数建议：5-8个
✅ stages数组长度必须等于max_stage
✅ 确保职业体系与世界观高度契合
✅ 职业设计必须支撑项目简介中的故事情节
✅ 每个职业都要体现成长门槛与使用代价（资源、风险或限制条件）
✅ 赛道可读：职业名字、能力和阶段描述要让对应题材读者快速理解，不写过度抽象术语
✅ 模板追踪标签：rule_v3_fusion_20260305

【禁止事项】
❌ 生成超过3个主职业或少于3个主职业
❌ 生成超过2个副职业或少于2个副职业
❌ 所有职业使用相同的阶段数
❌ 输出markdown标记
❌ 职业设计与世界观或简介脱节
❌ 忽略简介中提到的职业或能力设定
❌ 使用“总之/综上/值得注意的是”这类模板化总结句
❌ 设计“无成本、无约束、无副作用”的万能成长路线
❌ 输出流程说明、调度术语或自我评注
</constraints>"""

    # 局部重写提示词（RTCO框架）
    PARTIAL_REGENERATE = """<system>
你是小说局部改写搭档，擅长按用户要求改写指定段落，并与前后文自然咬合。
</system>

<task>
【改写任务】
根据用户的修改要求，重写下面选中的文本段落。

【重要要求】
1. 只输出重写后的内容，不要包含任何解释、前缀或后缀
2. 保持与前后文的自然衔接和语气连贯
3. 优先满足用户的修改要求
4. 保持整体叙事风格的一致性
5. 用词贴近中文小说阅读习惯，避免模板腔和说明腔
6. 若选中片段位于冲突段或章末，优先保住小爆发和追读牵引，不改成温吞复述
</task>

<context priority="P0">
【前文参考】（用于衔接，勿重复）
{context_before}

【需要重写的原文】（共{original_word_count}字）
{selected_text}

【后文参考】（用于衔接，勿重复）
{context_after}
</context>

<user_requirements priority="P0">
【用户修改要求】
{user_instructions}

【字数要求】
{length_requirement}
</user_requirements>

<style priority="P1">
【写作风格】
{style_content}
</style>

<fusion_contract priority="P0">
【第三版融合约束（人工修订）】
- 只改写选中片段，不扩写到片段外剧情任务
- 保持事实连续性：时间、地点、角色关系、能力边界不可无故漂移
- 用户要求与上下文冲突时，采用最小改动满足核心诉求，避免破坏后文衔接
- 核心冲突段保持局部因果链：目标受阻→角色选择→后果/代价
- 若改写片段位于章末，优先保留或增强追读钩子，避免结尾泄力
- 禁止输出修改说明、评语、对比表、步骤日志
</fusion_contract>

<output>
【输出规范】
直接输出重写后的内容，从故事内容开始写。
- 不要输出任何解释或说明文字
- 不要输出"重写后："等前缀
- 不要输出引号包裹内容
- 确保输出内容可以直接替换原文

请直接输出重写后的内容：
</output>

<constraints>
【必须遵守】
✅ 前后衔接：输出内容必须与前文自然衔接，与后文平滑过渡
✅ 风格一致：保持与原文相同的叙事风格、语气和人称
✅ 要求优先：严格执行用户的修改要求
✅ 字数控制：遵循字数要求
✅ 语言自然：长短句交替，避免连续同构句
✅ 对话自然：人物台词符合角色身份和说话习惯
✅ 事实稳定：不破坏既有时间线、关系线、能力线
✅ 改动边界：默认仅重写选中片段，不跨段扩散改写
✅ 片段目标明确：改写段至少有“动作推进或冲突推进”其一
✅ 节拍不断：若原片段承担开场、爆发或章尾功能，改写后继续承担同类叙事职责
✅ 模板追踪标签：rule_v3_fusion_20260303

【禁止事项】
❌ 重复前文内容
❌ 重复后文内容
❌ 添加任何元信息或说明
❌ 改变叙事人称或视角
❌ 偏离用户的修改要求
❌ 堆叠“与此同时、值得注意的是、在这个过程中”等模板连接词
❌ 输出“修改说明/改写思路/版本对比”等非正文文本
❌ 擅自新增片段外关键事件，导致后文失配
❌ 用巧合或突兀外挂在本段一次性解决核心冲突
</constraints>"""

    @classmethod
    def _prepare_template_content(cls, template_key: Optional[str], template: Optional[str]) -> Optional[str]:
        if not template:
            return template
        marker = f'<prompt_template_key value="{template_key}" />\n' if template_key else ""
        prepared = template
        if marker and not prepared.startswith(marker):
            prepared = f"{marker}{prepared}"
        return prepared

    @staticmethod
    def _augment_template_parameters(template_key: Optional[str], parameters: list) -> list:
        augmented = list(parameters or [])
        if template_key not in QUALITY_TEMPLATE_INSERTIONS:
            return augmented

        for item in [
            "genre",
            "style_name",
            "style_preset_id",
            "style_content",
            "creative_mode",
            "creative_mode_block",
            "story_focus",
            "story_focus_block",
            "story_creation_brief",
            "story_creation_brief_block",
            "quality_metrics_summary",
            "story_quality_trend_block",
            "story_repair_summary",
            "story_repair_targets",
            "story_preserve_strengths",
            "story_repair_target_block",
            "story_repair_diagnostic_block",
            "external_assets",
            "reference_assets",
            "quality_generation_block",
            "quality_analysis_block",
            "quality_checker_block",
            "quality_reviser_block",
            "quality_regeneration_block",
            "quality_generation_protocol_block",
            "quality_json_protocol_block",
            "quality_mcp_guard_block",
            "mcp_guard",
            "quality_external_assets_block",
            "mcp_references",
        ]:
            if item not in augmented:
                augmented.append(item)
        return augmented

    @staticmethod
    def _build_quality_profile_context(**kwargs) -> Dict[str, Any]:
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

    @staticmethod
    def _resolve_quality_optional_block_budget(
        template_key: Optional[str],
        template_insertion: Optional[str],
        plot_stage: Optional[str],
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
        resolved_budget = QUALITY_OPTIONAL_CARD_BLOCK_BUDGETS.get(
            normalized_stage,
            QUALITY_OPTIONAL_CARD_DEFAULT_BUDGET,
        )
        if template_key == "CHAPTER_REGENERATION_SYSTEM":
            return min(resolved_budget, QUALITY_REGENERATION_OPTIONAL_CARD_BUDGET)
        return resolved_budget

    @staticmethod
    def _resolve_quality_optional_block_drop_order(
        plot_stage: Optional[str],
        protected_blocks: Sequence[str] = (),
    ) -> tuple[str, ...]:
        normalized_stage = normalize_plot_stage(plot_stage) or "development"
        base_order = QUALITY_OPTIONAL_CARD_DROP_ORDER.get(
            normalized_stage,
            QUALITY_OPTIONAL_CARD_DROP_ORDER["development"],
        )
        protected_set = {key for key in protected_blocks if key}
        if not protected_set:
            return base_order
        return tuple(
            [key for key in base_order if key not in protected_set]
            + [key for key in base_order if key in protected_set]
        )

    @classmethod
    def _apply_quality_optional_block_budget(
        cls,
        blocks: Dict[str, str],
        *,
        template_key: Optional[str],
        template_insertion: Optional[str],
        plot_stage: Optional[str],
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
        budget = cls._resolve_quality_optional_block_budget(
            template_key,
            template_insertion,
            plot_stage,
            budget_override=budget_override,
            continuity_density=continuity_density,
        )
        if budget is None:
            return blocks

        placeholders = {
            match.group(1)
            for match in re.finditer(r"\{([A-Za-z0-9_]+)\}", template_insertion or "")
        }
        protected_blocks = _resolve_quality_focus_protected_blocks(quality_metrics_summary)
        protected_set = {key for key in protected_blocks if key}
        drop_order = cls._resolve_quality_optional_block_drop_order(
            plot_stage,
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

    @classmethod
    def _build_quality_runtime_blocks(cls, template_key: Optional[str], **kwargs) -> Dict[str, str]:
        profile = cls._build_quality_profile_context(**kwargs)
        prompt_blocks = profile.get("prompt_blocks") or {}
        quality_metrics_summary = kwargs.get("quality_metrics_summary") if isinstance(kwargs.get("quality_metrics_summary"), Mapping) else {}
        continuity_preflight = (
            quality_metrics_summary.get("continuity_preflight")
            if isinstance(quality_metrics_summary.get("continuity_preflight"), Mapping)
            else None
        )

        generation_block = _compact_prompt_text(prompt_blocks.get("generation"))
        checker_block = _compact_prompt_text(prompt_blocks.get("checker"))
        reviser_block = _compact_prompt_text(prompt_blocks.get("reviser"))
        mcp_guard_block = _compact_prompt_text(
            kwargs.get("mcp_guard") or kwargs.get("quality_mcp_guard") or prompt_blocks.get("mcp_guard")
        )
        external_assets_block = _compact_prompt_text(prompt_blocks.get("external_assets"))
        mcp_references = _compact_prompt_text(
            kwargs.get("mcp_references") or kwargs.get("quality_mcp_references")
        )
        creative_mode_block = _compact_prompt_text(
            kwargs.get("creative_mode_block") or build_creative_mode_block(kwargs.get("creative_mode"), scene="chapter")
        )
        story_focus_block = _compact_prompt_text(
            kwargs.get("story_focus_block") or build_story_focus_block(kwargs.get("story_focus"), scene="chapter")
        )
        narrative_blueprint_block = _compact_prompt_text(
            kwargs.get("narrative_blueprint_block")
            or build_narrative_blueprint_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_creation_brief_block = _compact_prompt_text(
            kwargs.get("story_creation_brief_block")
            or build_story_creation_brief_block(kwargs.get("story_creation_brief"))
        )
        story_long_term_goal_block = _compact_prompt_text(
            kwargs.get("story_long_term_goal_block")
            or build_story_long_term_goal_block(kwargs.get("story_long_term_goal"))
        )
        story_pacing_budget_block = _compact_prompt_text(
            kwargs.get("story_pacing_budget_block")
            or build_story_pacing_budget_block(
                kwargs.get("chapter_count"),
                current_chapter_number=kwargs.get("current_chapter_number"),
                target_word_count=kwargs.get("target_word_count"),
                plot_stage=kwargs.get("plot_stage"),
                scene="chapter",
            )
        )
        story_volume_pacing_block = _compact_prompt_text(
            kwargs.get("story_volume_pacing_block")
            or build_volume_pacing_block(
                kwargs.get("chapter_count"),
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_quality_trend_block = _compact_prompt_text(
            kwargs.get("story_quality_trend_block")
            or build_story_quality_trend_block(
                quality_metrics_summary,
                scene="chapter",
            )
        )
        story_character_focus_anchor_block = _compact_prompt_text(
            kwargs.get("story_character_focus_anchor_block")
            or build_story_character_focus_anchor_block(
                kwargs.get("story_character_focus"),
                scene="chapter",
            )
        )
        story_foreshadow_payoff_plan_block = _compact_prompt_text(
            kwargs.get("story_foreshadow_payoff_plan_block")
            or build_story_foreshadow_payoff_plan_block(
                kwargs.get("story_foreshadow_payoff_plan"),
                scene="chapter",
            )
        )
        story_character_state_ledger_block = _compact_prompt_text(
            kwargs.get("story_character_state_ledger_block")
            or build_story_character_state_ledger_block(
                kwargs.get("story_character_state_ledger"),
                scene="chapter",
            )
        )
        story_relationship_state_ledger_block = _compact_prompt_text(
            kwargs.get("story_relationship_state_ledger_block")
            or build_story_relationship_state_ledger_block(
                kwargs.get("story_relationship_state_ledger"),
                scene="chapter",
            )
        )
        story_foreshadow_state_ledger_block = _compact_prompt_text(
            kwargs.get("story_foreshadow_state_ledger_block")
            or build_story_foreshadow_state_ledger_block(
                kwargs.get("story_foreshadow_state_ledger"),
                scene="chapter",
            )
        )
        story_organization_state_ledger_block = _compact_prompt_text(
            kwargs.get("story_organization_state_ledger_block")
            or build_story_organization_state_ledger_block(
                kwargs.get("story_organization_state_ledger"),
                scene="chapter",
            )
        )
        story_career_state_ledger_block = _compact_prompt_text(
            kwargs.get("story_career_state_ledger_block")
            or build_story_career_state_ledger_block(
                kwargs.get("story_career_state_ledger"),
                scene="chapter",
            )
        )
        quality_preference_block = _compact_prompt_text(
            kwargs.get("quality_preference_block")
            or build_quality_preference_block(
                kwargs.get("quality_preset"),
                kwargs.get("quality_notes"),
                scene="chapter",
            )
        )
        story_objective_card_block = _compact_prompt_text(
            kwargs.get("story_objective_card_block")
            or build_story_objective_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_result_card_block = _compact_prompt_text(
            kwargs.get("story_result_card_block")
            or build_story_result_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_payoff_chain_card_block = _compact_prompt_text(
            kwargs.get("story_payoff_chain_card_block")
            or build_story_payoff_chain_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_rule_grounding_card_block = _compact_prompt_text(
            kwargs.get("story_rule_grounding_card_block")
            or build_story_rule_grounding_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_information_release_card_block = _compact_prompt_text(
            kwargs.get("story_information_release_card_block")
            or build_story_information_release_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_emotion_landing_card_block = _compact_prompt_text(
            kwargs.get("story_emotion_landing_card_block")
            or build_story_emotion_landing_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_action_rendering_card_block = _compact_prompt_text(
            kwargs.get("story_action_rendering_card_block")
            or build_story_action_rendering_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_summary_tone_control_card_block = _compact_prompt_text(
            kwargs.get("story_summary_tone_control_card_block")
            or build_story_summary_tone_control_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_repetition_control_card_block = _compact_prompt_text(
            kwargs.get("story_repetition_control_card_block")
            or build_story_repetition_control_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_viewpoint_discipline_card_block = _compact_prompt_text(
            kwargs.get("story_viewpoint_discipline_card_block")
            or build_story_viewpoint_discipline_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_dialogue_advancement_card_block = _compact_prompt_text(
            kwargs.get("story_dialogue_advancement_card_block")
            or build_story_dialogue_advancement_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_opening_hook_card_block = _compact_prompt_text(
            kwargs.get("story_opening_hook_card_block")
            or build_story_opening_hook_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_repair_target_block = _compact_prompt_text(
            kwargs.get("story_repair_target_block")
            or build_story_repair_target_block(
                kwargs.get("story_repair_summary"),
                kwargs.get("story_repair_targets"),
                kwargs.get("story_preserve_strengths"),
            )
        )
        story_repair_diagnostic_block = _compact_prompt_text(
            kwargs.get("story_repair_diagnostic_block")
        )
        story_execution_checklist_block = _compact_prompt_text(
            kwargs.get("story_execution_checklist_block")
            or build_story_execution_checklist_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
                continuity_preflight=continuity_preflight,
            )
        )
        story_scene_anchor_card_block = _compact_prompt_text(
            kwargs.get("story_scene_anchor_card_block")
            or build_story_scene_anchor_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_scene_density_card_block = _compact_prompt_text(
            kwargs.get("story_scene_density_card_block")
            or build_story_scene_density_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_repetition_risk_block = _compact_prompt_text(
            kwargs.get("story_repetition_risk_block")
            or build_story_repetition_risk_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_acceptance_card_block = _compact_prompt_text(
            kwargs.get("story_acceptance_card_block")
            or build_story_acceptance_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_cliffhanger_card_block = _compact_prompt_text(
            kwargs.get("story_cliffhanger_card_block")
            or build_story_cliffhanger_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )
        story_character_arc_card_block = _compact_prompt_text(
            kwargs.get("story_character_arc_card_block")
            or build_story_character_arc_card_block(
                kwargs.get("creative_mode"),
                kwargs.get("story_focus"),
                scene="chapter",
                plot_stage=kwargs.get("plot_stage"),
            )
        )

        quality_generation_protocol_block = _compact_prompt_text(
            "\n".join(
                [
                    "【统一协议护栏】",
                    f"- 质量块追踪标签：{QUALITY_RUNTIME_TRACKING_TAG}",
                    "- 统一吸收第三版规则摘要，不在各链路重复手写散落逻辑。",
                    "- runtime 质量块只补充规则来源，不覆盖用户模板主体与业务上下文。",
                    f"- {MCP_CANON_PRIORITY_RULE}",
                    f"- {MCP_SOURCE_DISCLOSURE_RULE}",
                    "- 禁止输出流程化元文本、调度说明、自我评注与来源暴露。",
                ]
            )
        )
        quality_json_protocol_block = _compact_prompt_text(
            "\n".join(
                [
                    "【统一JSON协议护栏】",
                    f"- 质量块追踪标签：{QUALITY_RUNTIME_TRACKING_TAG}",
                    "- 维持纯 JSON 输出，不追加 markdown、解释说明、流程文本或来源披露。",
                    f"- {MCP_CANON_PRIORITY_RULE}",
                    f"- {MCP_SOURCE_DISCLOSURE_RULE}",
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

        template_insertion = QUALITY_TEMPLATE_INSERTIONS.get(template_key or "")
        blocks = cls._apply_quality_optional_block_budget(
            blocks,
            template_key=template_key,
            template_insertion=template_insertion,
            plot_stage=kwargs.get("plot_stage"),
            budget_override=kwargs.get("quality_optional_block_budget"),
            quality_metrics_summary=kwargs.get("quality_metrics_summary"),
        )
        if template_insertion:
            blocks["quality_contract_block"] = PromptService.format_prompt(template_insertion, **blocks)
        else:
            blocks["quality_contract_block"] = ""
        return blocks

    @classmethod
    def _inject_quality_contract(cls, template: str, template_key: Optional[str], **kwargs) -> str:
        blocks = cls._build_quality_runtime_blocks(template_key, **kwargs)
        injected = _append_prompt_block(template, blocks.get("quality_contract_block"), after_tag="</fusion_contract>")
        injected = _append_prompt_block(injected, blocks.get("quality_mcp_references_block"), after_tag="</fusion_contract>")
        return injected

    @staticmethod
    def format_prompt(template: str, **kwargs) -> str:
        """
        格式化提示词模板

        Args:
            template: 提示词模板
            **kwargs: 模板参数

        Returns:
            格式化后的提示词
        """
        template_key = kwargs.pop("_template_key", None)
        extracted_template_key = None
        try:
            extracted_template_key, prepared_template = _extract_template_key_marker(template)
            if not template_key:
                template_key = extracted_template_key
            prepared_template = PromptService._prepare_template_content(template_key, prepared_template)
            rendered = prepared_template.format(**kwargs)
            return PromptService._inject_quality_contract(rendered, template_key, **kwargs)
        except KeyError as e:
            raise ValueError(f"缺少必需的参数: {e}")
    

    @classmethod
    async def get_chapter_regeneration_prompt(cls, chapter_number: int, title: str, word_count: int, content: str,
                                        modification_instructions: str, project_context: Dict[str, Any],
                                        style_content: str, target_word_count: int,
                                        user_id: str = None, db = None) -> str:
        """
        获取章节重写提示词（支持用户自定义）
        
        Args:
            chapter_number: 章节序号
            title: 章节标题
            word_count: 原始字数
            content: 原始内容
            modification_instructions: 修改指令
            project_context: 项目上下文
            style_content: 写作风格
            target_word_count: 目标字数
            user_id: 用户ID（可选，用于获取自定义模板）
            db: 数据库会话（可选，用于查询自定义模板）
            
        Returns:
            完整的章节重写提示词
        """
        # 获取系统提示词模板（支持用户自定义）
        if user_id and db:
            system_template = await cls.get_template("CHAPTER_REGENERATION_SYSTEM", user_id, db)
        else:
            system_template = cls._prepare_template_content("CHAPTER_REGENERATION_SYSTEM", cls.CHAPTER_REGENERATION_SYSTEM)
        
        prompt_parts = [system_template]
        
        # 原始章节信息
        prompt_parts.append(f"""## 📖 原始章节信息

**章节**：第{chapter_number}章
**标题**：{title}
**字数**：{word_count}字

**原始内容**：
{content}

---
""")
        
        # 修改指令
        prompt_parts.append(modification_instructions)
        prompt_parts.append("\n---\n")
        
        # 项目背景信息
        prompt_parts.append(f"""## 🌍 项目背景信息

**小说标题**：{project_context.get('project_title', '未知')}
**题材**：{project_context.get('genre', '未设定')}
**主题**：{project_context.get('theme', '未设定')}
**叙事视角**：{project_context.get('narrative_perspective', '第三人称')}
**世界观设定**：
- 时代背景：{project_context.get('time_period', '未设定')}
- 地理位置：{project_context.get('location', '未设定')}
- 氛围基调：{project_context.get('atmosphere', '未设定')}

---
""")
        
        # 角色信息
        if project_context.get('characters_info'):
            prompt_parts.append(f"""## 👥 角色信息

{project_context['characters_info']}

---
""")
        
        # 章节大纲
        if project_context.get('chapter_outline'):
            prompt_parts.append(f"""## 📝 本章大纲

{project_context['chapter_outline']}

---
""")
        
        # 前置章节上下文
        if project_context.get('previous_context'):
            prompt_parts.append(f"""## 📚 前置章节上下文

{project_context['previous_context']}

---
""")
        
        # 写作风格要求
        if style_content:
            prompt_parts.append(f"""## 🎨 写作风格要求

{style_content}

请在重新创作时贴合上述写作风格。

---
""")
        
        # 创作要求
        prompt_parts.append(f"""## ✨ 创作要求

1. **解决问题**：针对上述修改指令中提到的所有问题进行改进
2. **保持连贯**：确保与前后章节的情节、人物、风格保持一致
3. **提升质量**：在节奏、情感、描写等方面明显优于原版
4. **保留精华**：保持原章节中优秀的部分和关键情节
5. **字数控制**：目标字数约{target_word_count}字（可适当浮动±20%）
{f'6. **风格一致**：按上述写作风格创作，语气保持自然' if style_content else ''}

---

## 🎬 开始创作

请现在开始创作改进后的新版本章节内容。

**重要提示**：
- 直接输出章节正文内容，从故事内容开始写
- **不要**输出章节标题（如"第X章"、"第X章：XXX"等）
- **不要**输出任何额外的说明、注释或元数据
- 只需要纯粹的故事正文内容

现在开始：
""")
        
        prompt_text = "\n".join(prompt_parts)
        quality_kwargs = project_context.get("prompt_quality_kwargs") or {
            "genre": project_context.get("genre"),
            "style_name": project_context.get("style_name"),
            "style_preset_id": project_context.get("style_preset_id"),
            "style_content": style_content,
            "external_assets": project_context.get("external_assets"),
            "reference_assets": project_context.get("reference_assets"),
            "mcp_references": project_context.get("mcp_references"),
            "mcp_guard": project_context.get("mcp_guard"),
        }
        return cls._inject_quality_contract(
            prompt_text,
            "CHAPTER_REGENERATION_SYSTEM",
            **quality_kwargs,
        )

    @classmethod
    async def get_mcp_tool_test_prompts(
        cls,
        plugin_name: str,
        user_id: str = None,
        db = None
    ) -> Dict[str, str]:
        """
        获取MCP工具测试的提示词（支持自定义）
        
        Args:
            plugin_name: 插件名称
            user_id: 用户ID（可选）
            db: 数据库会话（可选）
            
        Returns:
            包含user和system提示词的字典
        """
        # 获取用户自定义或系统默认的user提示词
        if user_id and db:
            user_template = await cls.get_template("MCP_TOOL_TEST", user_id, db)
        else:
            user_template = cls._prepare_template_content("MCP_TOOL_TEST", cls.MCP_TOOL_TEST)

        # 获取用户自定义或系统默认的system提示词
        if user_id and db:
            system_template = await cls.get_template("MCP_TOOL_TEST_SYSTEM", user_id, db)
        else:
            system_template = cls._prepare_template_content("MCP_TOOL_TEST_SYSTEM", cls.MCP_TOOL_TEST_SYSTEM)
        
        return {
            "user": cls.format_prompt(user_template, plugin_name=plugin_name, _template_key="MCP_TOOL_TEST"),
            "system": system_template
        }

    # ========== 自定义提示词支持 ==========
    
    @classmethod
    async def get_template_with_fallback(cls,
                                        template_key: str,
                                        user_id: str = None,
                                        db = None) -> str:
        """
        获取提示词模板（优先用户自定义，支持降级）
        
        Args:
            template_key: 模板键名
            user_id: 用户ID（可选，如果不提供则直接返回系统默认）
            db: 数据库会话（可选）
            
        Returns:
            提示词模板内容
        """
        # 如果没有提供user_id或db，直接返回系统默认
        if not user_id or not db:
            return getattr(cls, template_key, None)
        
        # 尝试获取用户自定义模板
        return await cls.get_template(template_key, user_id, db)
    
    @classmethod
    async def get_template(cls,
                          template_key: str,
                          user_id: str,
                          db) -> str:
        """
        获取提示词模板（优先用户自定义）
        
        Args:
            template_key: 模板键名
            user_id: 用户ID
            db: 数据库会话
            
        Returns:
            提示词模板内容
        """
        from sqlalchemy import select
        from app.models.prompt_template import PromptTemplate
        from app.services.prompt_template_sync_service import sync_managed_template_if_legacy

        # Resolve current system template metadata once and reuse it.
        template_content = getattr(cls, template_key, None)
        template_content = cls._prepare_template_content(template_key, template_content)
        template_info = cls.get_system_template_info(template_key)

        # Sync managed templates only when user's copy still matches known legacy defaults.
        try:
            await sync_managed_template_if_legacy(
                db=db,
                user_id=user_id,
                template_key=template_key,
                system_template_content=template_content,
                system_template_info=template_info,
            )
        except Exception as sync_error:
            logger.warning(
                "Managed template sync failed, fallback to normal flow: user_id=%s, template_key=%s, error=%s",
                user_id,
                template_key,
                sync_error,
            )
        
        # 1. 尝试从数据库获取用户自定义模板
        result = await db.execute(
            select(PromptTemplate).where(
                PromptTemplate.user_id == user_id,
                PromptTemplate.template_key == template_key,
                PromptTemplate.is_active == True
            )
        )
        custom_template = result.scalar_one_or_none()
        
        if custom_template:
            logger.info(f"✅ 使用用户自定义提示词: user_id={user_id}, template_key={template_key}, template_name={custom_template.template_name}")
            return cls._prepare_template_content(template_key, custom_template.template_content)
        
        # 2. 降级到系统默认模板
        logger.info(f"⚪ 使用系统默认提示词: user_id={user_id}, template_key={template_key} (未找到自定义模板)")
        
        if template_content is None:
            logger.warning(f"⚠️ 未找到系统默认模板: {template_key}")
        
        return template_content
    
    @classmethod
    def get_all_system_templates(cls) -> list:
        """
        获取所有系统默认模板的信息
        
        Returns:
            系统模板列表
        """
        templates = []
        
        # 定义所有模板及其元信息
        template_definitions = {
            "WORLD_BUILDING": {
                "name": "世界构建",
                "category": "世界构建",
                "description": "用于生成小说世界观设定，包括时间背景、地理位置、氛围基调和世界规则",
                "parameters": ["title", "theme", "genre", "description"]
            },
            "CHARACTERS_BATCH_GENERATION": {
                "name": "批量角色生成",
                "category": "角色生成",
                "description": "批量生成多个角色和组织，建立角色关系网络",
                "parameters": ["count", "time_period", "location", "atmosphere", "rules", "theme", "genre", "requirements"]
            },
            "SINGLE_CHARACTER_GENERATION": {
                "name": "单个角色生成",
                "category": "角色生成",
                "description": "生成单个角色的详细设定",
                "parameters": ["project_context", "user_input"]
            },
            "SINGLE_ORGANIZATION_GENERATION": {
                "name": "组织生成",
                "category": "角色生成",
                "description": "生成组织/势力的详细设定",
                "parameters": ["project_context", "user_input"]
            },
            "OUTLINE_CREATE": {
                "name": "大纲生成",
                "category": "大纲生成",
                "description": "根据项目信息生成完整的章节大纲",
                "parameters": ["title", "theme", "genre", "chapter_count", "narrative_perspective", "target_words", 
                             "time_period", "location", "atmosphere", "rules", "characters_info", "requirements", "mcp_references"]
            },
            "BOOK_IMPORT_REVERSE_PROJECT_SUGGESTION": {
                "name": "拆书导入-反向项目提炼",
                "category": "拆书导入",
                "description": "基于正文片段反向提炼项目简介、主题、类型、视角与目标字数",
                "parameters": ["title", "sampled_text"]
            },
            "BOOK_IMPORT_REVERSE_OUTLINES": {
                "name": "拆书导入-反向章节大纲",
                "category": "拆书导入",
                "description": "基于章节正文反向提炼章节大纲结构",
                "parameters": [
                    "title", "genre", "theme", "narrative_perspective",
                    "start_chapter", "end_chapter", "expected_count", "chapters_text"
                ]
            },
            "OUTLINE_CONTINUE": {
                "name": "大纲续写",
                "category": "大纲生成",
                "description": "基于已有章节续写大纲",
                "parameters": ["title", "theme", "genre", "narrative_perspective", "chapter_count", "time_period", 
                             "location", "atmosphere", "rules", "characters_info", "current_chapter_count", 
                             "all_chapters_brief", "recent_plot", "memory_context", "mcp_references", 
                             "plot_stage_instruction", "start_chapter", "end_chapter", "story_direction", "requirements"]
            },
            "CHAPTER_GENERATION_ONE_TO_MANY": {
                "name": "章节创作-1-N模式（第1章）",
                "category": "章节创作",
                "description": "1-N模式：根据大纲创作章节内容（用于第1章，无前置章节）",
                "parameters": ["project_title", "genre", "chapter_number", "chapter_title", "chapter_outline",
                             "target_word_count", "narrative_perspective", "characters_info"]
            },
            "CHAPTER_GENERATION_ONE_TO_MANY_NEXT": {
                "name": "章节创作-1-N模式（第2章及以后）",
                "category": "章节创作",
                "description": "1-N模式：基于前置章节内容创作新章节（用于第2章及以后）",
                "parameters": ["project_title", "genre", "chapter_number", "chapter_title", "chapter_outline",
                             "target_word_count", "narrative_perspective", "characters_info", "continuation_point",
                             "foreshadow_reminders", "relevant_memories", "story_skeleton", "previous_chapter_summary"]
            },
            "CHAPTER_GENERATION_ONE_TO_ONE": {
                "name": "章节创作-1-1模式（第1章）",
                "category": "章节创作",
                "description": "1-1模式：章节创作（用于第1章，无前置章节）",
                "parameters": ["project_title", "genre", "chapter_number", "chapter_title", "chapter_outline",
                             "target_word_count", "narrative_perspective", "characters_info", "chapter_careers"]
            },
            "CHAPTER_GENERATION_ONE_TO_ONE_NEXT": {
                "name": "章节创作-1-1模式（第2章及以后）",
                "category": "章节创作",
                "description": "1-1模式：基于上一章内容创作新章节（用于第2章及以后）",
                "parameters": ["project_title", "genre", "chapter_number", "chapter_title", "chapter_outline",
                             "target_word_count", "narrative_perspective", "previous_chapter_content",
                             "characters_info", "chapter_careers", "foreshadow_reminders", "relevant_memories"]
            },
            "CHAPTER_REGENERATION_SYSTEM": {
                "name": "章节重写系统提示",
                "category": "章节重写",
                "description": "用于章节重写的系统提示词",
                "parameters": ["chapter_number", "title", "word_count", "content", "modification_instructions",
                             "project_context", "style_content", "target_word_count"]
            },
            "PARTIAL_REGENERATE": {
                "name": "局部重写",
                "category": "章节重写",
                "description": "根据用户修改要求重写选中的段落内容",
                "parameters": ["context_before", "original_word_count", "selected_text", "context_after",
                             "user_instructions", "length_requirement", "style_content"]
            },
            "PLOT_ANALYSIS": {
                "name": "情节分析",
                "category": "情节分析",
                "description": "深度分析章节的剧情、钩子、伏笔等",
                "parameters": ["chapter_number", "title", "content", "word_count"]
            },
            "CHAPTER_TEXT_CHECKER": {
                "name": "正文质量检查",
                "category": "情节分析",
                "description": "对章节正文进行结构化质量检查并输出可执行修订建议",
                "parameters": ["chapter_number", "chapter_title", "chapter_content", "chapter_outline", "characters_info", "world_rules"]
            },
            "CHAPTER_TEXT_REVISER": {
                "name": "正文自动修订",
                "category": "章节重写",
                "description": "根据质检结果自动生成修订草案（优先修复严重问题）",
                "parameters": ["chapter_number", "chapter_title", "chapter_content", "critical_issues_text", "checker_result_json"]
            },
            "OUTLINE_EXPAND_SINGLE": {
                "name": "大纲单批次展开",
                "category": "情节展开",
                "description": "将大纲节点展开为详细章节规划（单批次）",
                "parameters": ["project_title", "project_genre", "project_theme", "project_narrative_perspective", 
                             "project_world_time_period", "project_world_location", "project_world_atmosphere", 
                             "characters_info", "outline_order_index", "outline_title", "outline_content", 
                             "context_info", "strategy_instruction", "target_chapter_count", "scene_instruction", "scene_field"]
            },
            "OUTLINE_EXPAND_MULTI": {
                "name": "大纲分批展开",
                "category": "情节展开",
                "description": "将大纲节点展开为详细章节规划（分批）",
                "parameters": ["project_title", "project_genre", "project_theme", "project_narrative_perspective", 
                             "project_world_time_period", "project_world_location", "project_world_atmosphere", 
                             "characters_info", "outline_order_index", "outline_title", "outline_content", 
                             "context_info", "previous_context", "strategy_instruction", "start_index", 
                             "end_index", "target_chapter_count", "scene_instruction", "scene_field"]
            },
            "MCP_TOOL_TEST": {
                "name": "MCP工具测试(用户提示词)",
                "category": "MCP测试",
                "description": "用于测试MCP插件功能的用户提示词",
                "parameters": ["plugin_name"]
            },
            "MCP_TOOL_TEST_SYSTEM": {
                "name": "MCP工具测试(系统提示词)",
                "category": "MCP测试",
                "description": "用于测试MCP插件功能的系统提示词",
                "parameters": []
            },
            "MCP_WORLD_BUILDING_PLANNING": {
                "name": "MCP世界观规划",
                "category": "MCP增强",
                "description": "使用MCP工具搜索资料辅助世界观设计",
                "parameters": ["title", "genre", "theme", "description"]
            },
            "MCP_CHARACTER_PLANNING": {
                "name": "MCP角色规划",
                "category": "MCP增强",
                "description": "使用MCP工具搜索资料辅助角色设计",
                "parameters": ["title", "genre", "theme", "time_period", "location"]
            },
            "AUTO_CHARACTER_ANALYSIS": {
                "name": "自动角色分析",
                "category": "自动角色引入",
                "description": "分析新生成的大纲，判断是否需要引入新角色",
                "parameters": ["title", "genre", "theme", "time_period", "location", "atmosphere",
                             "existing_characters", "new_outlines", "start_chapter", "end_chapter"]
            },
            "AUTO_CHARACTER_GENERATION": {
                "name": "自动角色生成",
                "category": "自动角色引入",
                "description": "根据剧情需求自动生成新角色的完整设定",
                "parameters": ["title", "genre", "theme", "time_period", "location", "atmosphere", "rules",
                             "existing_characters", "plot_context", "character_specification", "mcp_references"]
            },
            "AUTO_ORGANIZATION_ANALYSIS": {
                "name": "自动组织分析",
                "category": "自动组织引入",
                "description": "分析新生成的大纲，判断是否需要引入新组织",
                "parameters": ["title", "genre", "theme", "time_period", "location", "atmosphere",
                             "existing_organizations", "existing_characters", "all_chapters_brief", "start_chapter", "chapter_count", "plot_stage", "story_direction"]
            },
            "AUTO_ORGANIZATION_GENERATION": {
                "name": "自动组织生成",
                "category": "自动组织引入",
                "description": "根据剧情需求自动生成新组织的完整设定",
                "parameters": ["title", "genre", "theme", "time_period", "location", "atmosphere", "rules",
                             "existing_organizations", "existing_characters", "plot_context", "organization_specification", "mcp_references"]
            },
            "CAREER_SYSTEM_GENERATION": {
                "name": "职业体系生成",
                "category": "世界构建",
                "description": "根据世界观和项目简介自动生成完整的职业体系，包括主职业和副职业",
                "parameters": ["title", "genre", "theme", "description", "time_period", "location", "atmosphere", "rules"]
            },
            "INSPIRATION_TITLE_SYSTEM": {
                "name": "灵感模式-书名生成(系统提示词)",
                "category": "灵感模式",
                "description": "根据用户的原始想法生成6个书名建议的系统提示词",
                "parameters": ["initial_idea"]
            },
            "INSPIRATION_TITLE_USER": {
                "name": "灵感模式-书名生成(用户提示词)",
                "category": "灵感模式",
                "description": "根据用户的原始想法生成6个书名建议的用户提示词",
                "parameters": ["initial_idea"]
            },
            "INSPIRATION_DESCRIPTION_SYSTEM": {
                "name": "灵感模式-简介生成(系统提示词)",
                "category": "灵感模式",
                "description": "根据用户想法和书名生成6个简介选项的系统提示词",
                "parameters": ["initial_idea", "title"]
            },
            "INSPIRATION_DESCRIPTION_USER": {
                "name": "灵感模式-简介生成(用户提示词)",
                "category": "灵感模式",
                "description": "根据用户想法和书名生成6个简介选项的用户提示词",
                "parameters": ["initial_idea", "title"]
            },
            "INSPIRATION_THEME_SYSTEM": {
                "name": "灵感模式-主题生成(系统提示词)",
                "category": "灵感模式",
                "description": "根据书名和简介生成6个深刻的主题选项的系统提示词",
                "parameters": ["initial_idea", "title", "description"]
            },
            "INSPIRATION_THEME_USER": {
                "name": "灵感模式-主题生成(用户提示词)",
                "category": "灵感模式",
                "description": "根据书名和简介生成6个深刻的主题选项的用户提示词",
                "parameters": ["initial_idea", "title", "description"]
            },
            "INSPIRATION_GENRE_SYSTEM": {
                "name": "灵感模式-类型生成(系统提示词)",
                "category": "灵感模式",
                "description": "根据小说信息生成6个合适的类型标签的系统提示词",
                "parameters": ["initial_idea", "title", "description", "theme"]
            },
            "INSPIRATION_GENRE_USER": {
                "name": "灵感模式-类型生成(用户提示词)",
                "category": "灵感模式",
                "description": "根据小说信息生成6个合适的类型标签的用户提示词",
                "parameters": ["initial_idea", "title", "description", "theme"]
            },
            "INSPIRATION_QUICK_COMPLETE": {
                "name": "灵感模式-智能补全",
                "category": "灵感模式",
                "description": "根据用户提供的部分信息智能补全完整的小说方案",
                "parameters": ["existing"]
            },
            "AI_DENOISING": {
                "name": "AI去味",
                "category": "文本润色",
                "description": "将文本改写为更自然的中文表达，降低模板腔和AI腔",
                "parameters": ["original_text", "focus_instruction", "structure_instruction", "style_hint_block"]
            }
        }
        
        for key, info in template_definitions.items():
            template_content = getattr(cls, key, None)
            if template_content:
                templates.append({
                    "template_key": key,
                    "template_name": info["name"],
                    "category": info["category"],
                    "description": info["description"],
                    "parameters": cls._augment_template_parameters(key, info["parameters"]),
                    "content": cls._prepare_template_content(key, template_content)
                })
        
        return templates
    
    @classmethod
    def get_system_template_info(cls, template_key: str) -> dict:
        """
        获取指定系统模板的信息
        
        Args:
            template_key: 模板键名
            
        Returns:
            模板信息字典
        """
        all_templates = cls.get_all_system_templates()
        for template in all_templates:
            if template["template_key"] == template_key:
                return template
        return None

# ========== 全局实例 ==========
prompt_service = PromptService()
