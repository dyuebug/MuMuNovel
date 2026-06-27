"""Shared template contract helpers extracted from story prompt block runtime ownership."""

from __future__ import annotations

import re
from typing import Any


QUALITY_RUNTIME_TRACKING_TAG = "rule_v3_quality_block_20260307"

QUALITY_BLOCK_SECTION_GENERATION = """<quality_contract priority="P0">\n{quality_generation_block}\n{story_quality_hard_guard_block}\n{creative_mode_block}\n{story_focus_block}\n{narrative_blueprint_block}\n{story_creation_brief_block}\n{story_long_term_goal_block}\n{story_pacing_budget_block}\n{story_volume_pacing_block}\n{story_quality_trend_block}\n{story_character_focus_anchor_block}\n{story_foreshadow_payoff_plan_block}\n{story_character_state_ledger_block}\n{story_relationship_state_ledger_block}\n{story_foreshadow_state_ledger_block}\n{story_organization_state_ledger_block}\n{story_career_state_ledger_block}\n{quality_preference_block}\n{story_objective_card_block}\n{story_result_card_block}\n{story_payoff_chain_card_block}\n{story_rule_grounding_card_block}\n{story_information_release_card_block}\n{story_emotion_landing_card_block}\n{story_action_rendering_card_block}\n{story_summary_tone_control_card_block}\n{story_repetition_control_card_block}\n{story_viewpoint_discipline_card_block}\n{story_dialogue_advancement_card_block}\n{story_opening_hook_card_block}\n{story_repair_target_block}\n{story_repair_diagnostic_block}\n{story_execution_checklist_block}\n{story_scene_anchor_card_block}\n{story_scene_density_card_block}\n{story_repetition_risk_block}\n{story_acceptance_card_block}\n{story_cliffhanger_card_block}\n{story_character_arc_card_block}\n{quality_generation_protocol_block}\n{quality_mcp_guard_block}\n{quality_external_assets_block}\n</quality_contract>"""
QUALITY_BLOCK_SECTION_ANALYSIS = """<quality_contract priority="P0">\n{quality_analysis_block}\n{creative_mode_block}\n{story_focus_block}\n{narrative_blueprint_block}\n{story_creation_brief_block}\n{story_long_term_goal_block}\n{story_pacing_budget_block}\n{story_volume_pacing_block}\n{story_quality_trend_block}\n{story_character_focus_anchor_block}\n{story_foreshadow_payoff_plan_block}\n{story_character_state_ledger_block}\n{story_relationship_state_ledger_block}\n{story_foreshadow_state_ledger_block}\n{story_organization_state_ledger_block}\n{story_career_state_ledger_block}\n{quality_preference_block}\n{quality_json_protocol_block}\n{quality_mcp_guard_block}\n{quality_external_assets_block}\n</quality_contract>"""
QUALITY_BLOCK_SECTION_CHECKER = """<quality_contract priority="P0">\n{quality_checker_block}\n{creative_mode_block}\n{story_focus_block}\n{narrative_blueprint_block}\n{story_creation_brief_block}\n{story_long_term_goal_block}\n{story_pacing_budget_block}\n{story_volume_pacing_block}\n{story_quality_trend_block}\n{story_character_focus_anchor_block}\n{story_foreshadow_payoff_plan_block}\n{story_character_state_ledger_block}\n{story_relationship_state_ledger_block}\n{story_foreshadow_state_ledger_block}\n{story_organization_state_ledger_block}\n{story_career_state_ledger_block}\n{quality_preference_block}\n{quality_json_protocol_block}\n{quality_mcp_guard_block}\n{quality_external_assets_block}\n</quality_contract>"""
QUALITY_BLOCK_SECTION_REVISER = """<quality_contract priority="P0">\n{quality_reviser_block}\n{creative_mode_block}\n{story_focus_block}\n{narrative_blueprint_block}\n{story_creation_brief_block}\n{story_long_term_goal_block}\n{story_pacing_budget_block}\n{story_volume_pacing_block}\n{story_quality_trend_block}\n{story_character_focus_anchor_block}\n{story_foreshadow_payoff_plan_block}\n{story_character_state_ledger_block}\n{story_relationship_state_ledger_block}\n{story_foreshadow_state_ledger_block}\n{story_organization_state_ledger_block}\n{story_career_state_ledger_block}\n{quality_preference_block}\n{quality_json_protocol_block}\n{quality_mcp_guard_block}\n{quality_external_assets_block}\n</quality_contract>"""
QUALITY_BLOCK_SECTION_REGENERATION = """<quality_contract priority="P0">\n{quality_regeneration_block}\n{story_quality_hard_guard_block}\n{creative_mode_block}\n{story_focus_block}\n{narrative_blueprint_block}\n{story_creation_brief_block}\n{story_long_term_goal_block}\n{story_pacing_budget_block}\n{story_volume_pacing_block}\n{story_quality_trend_block}\n{story_character_focus_anchor_block}\n{story_foreshadow_payoff_plan_block}\n{story_character_state_ledger_block}\n{story_relationship_state_ledger_block}\n{story_foreshadow_state_ledger_block}\n{story_organization_state_ledger_block}\n{story_career_state_ledger_block}\n{quality_preference_block}\n{story_objective_card_block}\n{story_result_card_block}\n{story_payoff_chain_card_block}\n{story_rule_grounding_card_block}\n{story_information_release_card_block}\n{story_emotion_landing_card_block}\n{story_action_rendering_card_block}\n{story_summary_tone_control_card_block}\n{story_repetition_control_card_block}\n{story_viewpoint_discipline_card_block}\n{story_dialogue_advancement_card_block}\n{story_opening_hook_card_block}\n{story_repair_target_block}\n{story_repair_diagnostic_block}\n{story_execution_checklist_block}\n{story_scene_anchor_card_block}\n{story_scene_density_card_block}\n{story_repetition_risk_block}\n{story_acceptance_card_block}\n{story_cliffhanger_card_block}\n{story_character_arc_card_block}\n{quality_generation_protocol_block}\n{quality_mcp_guard_block}\n{quality_external_assets_block}\n</quality_contract>"""

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


def compact_prompt_text(value: Any) -> str:
    text = str(value or "").strip()
    if not text:
        return ""
    return re.sub(r"\n{3,}", "\n\n", text)
