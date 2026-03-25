import json

from app.services.story_quality_feedback_service import (
    advance_quality_metrics_summary_state,
    build_quality_gate_decision,
    build_quality_metrics_summary,
    build_quality_metrics_summary_from_state,
    build_quality_metrics_summary_state,
    build_story_continuity_preflight,
    build_story_repair_guidance,
    extract_quality_metrics_from_history_payload,
)


def test_should_build_chapter_repair_guidance_from_low_metrics():
    guidance = build_story_repair_guidance(
        {
            "conflict_chain_hit_rate": 48.0,
            "rule_grounding_hit_rate": 72.0,
            "outline_alignment_rate": 54.0,
            "dialogue_naturalness_rate": 83.0,
            "opening_hook_rate": 61.0,
            "payoff_chain_rate": 75.0,
            "cliffhanger_rate": 69.0,
        },
        scope="chapter",
    )

    assert guidance["weakest_metric_key"] == "conflict_chain_hit_rate"
    assert guidance["weakest_metric_label"] == "冲突链推进"
    assert guidance["focus_areas"][:2] == ["conflict", "outline"]
    assert any("冲突" in item for item in guidance["repair_targets"])
    assert any("语气" in item for item in guidance["preserve_strengths"])
    assert guidance["summary"]


def test_should_support_batch_summary_and_pacing_guidance():
    guidance = build_story_repair_guidance(
        {
            "avg_overall_score": 76.0,
            "avg_conflict_chain_hit_rate": 70.0,
            "avg_rule_grounding_hit_rate": 84.0,
            "avg_outline_alignment_rate": 82.0,
            "avg_dialogue_naturalness_rate": 79.0,
            "avg_opening_hook_rate": 76.0,
            "avg_payoff_chain_rate": 80.0,
            "avg_cliffhanger_rate": 78.0,
            "avg_pacing_score": 5.9,
        },
        scope="batch",
    )

    assert guidance["weakest_metric_key"] == "pacing_score"
    assert guidance["focus_areas"][0] == "pacing"
    assert guidance["weakest_metric_value"] == 5.9
    assert guidance["summary"].startswith("这一批章节")


def test_should_build_story_continuity_preflight_and_enrich_guidance():
    runtime_context = {
        "plot_stage": "development",
        "chapter_count": 12,
        "current_chapter_number": 5,
        "character_state_ledger": ["Lin: injured hand still limits movement"],
        "relationship_state_ledger": ["Lin/Su: uneasy alliance under tension"],
        "foreshadow_state_ledger": ["RoyalKey: still missing from the archive"],
    }
    preflight = build_story_continuity_preflight(
        "Lin slipped into the archive alone, his injured hand slowing every move.",
        runtime_context,
    )

    assert preflight["status"] == "warning"
    assert preflight["warning_count"] == 2
    assert any(item["focus_area"] == "relationship_continuity" for item in preflight["warnings"])
    assert any(item["focus_area"] == "foreshadow_continuity" for item in preflight["warnings"])

    guidance = build_story_repair_guidance(
        {
            "overall_score": 86.0,
            "conflict_chain_hit_rate": 84.0,
            "rule_grounding_hit_rate": 85.0,
            "outline_alignment_rate": 83.0,
            "dialogue_naturalness_rate": 82.0,
            "opening_hook_rate": 80.0,
            "payoff_chain_rate": 79.0,
            "cliffhanger_rate": 81.0,
            "quality_runtime_context": runtime_context,
            "continuity_preflight": preflight,
        },
        scope="chapter",
    )

    assert "relationship_continuity" in guidance["focus_areas"]
    assert guidance["repair_targets"]
    assert guidance["summary"]


def test_should_build_pass_quality_gate_from_stable_metrics():
    quality_gate = build_quality_gate_decision(
        {
            "overall_score": 88.0,
            "conflict_chain_hit_rate": 86.0,
            "rule_grounding_hit_rate": 87.0,
            "outline_alignment_rate": 84.0,
            "dialogue_naturalness_rate": 81.0,
            "opening_hook_rate": 80.0,
            "payoff_chain_rate": 78.0,
            "cliffhanger_rate": 83.0,
            "pacing_score": 8.1,
        },
        scope="chapter",
    )

    assert quality_gate["status"] == "pass"
    assert quality_gate["decision"] == "allow_save"
    assert quality_gate["allow_save"] is True
    assert quality_gate["weak_metric_count"] == 0


def test_should_build_repairable_quality_gate_from_single_weak_metric():
    quality_gate = build_quality_gate_decision(
        {
            "avg_overall_score": 76.0,
            "avg_conflict_chain_hit_rate": 70.0,
            "avg_rule_grounding_hit_rate": 84.0,
            "avg_outline_alignment_rate": 82.0,
            "avg_dialogue_naturalness_rate": 79.0,
            "avg_opening_hook_rate": 76.0,
            "avg_payoff_chain_rate": 80.0,
            "avg_cliffhanger_rate": 78.0,
            "avg_pacing_score": 5.9,
        },
        scope="batch",
    )

    assert quality_gate["status"] == "repairable"
    assert quality_gate["decision"] == "auto_repair"
    assert quality_gate["can_auto_repair"] is True
    assert quality_gate["failed_metrics"][0]["key"] == "pacing_score"
    assert quality_gate["recommended_action"] == "bridge_scene"
    assert quality_gate["recommended_action_label"] == "补桥关键场景"



def test_should_build_blocked_quality_gate_from_multiple_weak_metrics():
    quality_gate = build_quality_gate_decision(
        {
            "conflict_chain_hit_rate": 48.0,
            "rule_grounding_hit_rate": 72.0,
            "outline_alignment_rate": 54.0,
            "dialogue_naturalness_rate": 83.0,
            "opening_hook_rate": 61.0,
            "payoff_chain_rate": 75.0,
            "cliffhanger_rate": 69.0,
        },
        scope="chapter",
    )

    assert quality_gate["status"] == "blocked"
    assert quality_gate["decision"] == "manual_review"
    assert quality_gate["requires_manual_review"] is True
    assert quality_gate["weak_metric_count"] == 3
    assert quality_gate["recommended_action"] == "bridge_scene"


def test_should_extract_quality_metrics_from_history_payload_with_guidance():
    generated_content = json.dumps(
        {
            "quality_metrics": {
                "conflict_chain_hit_rate": 55.0,
                "rule_grounding_hit_rate": 74.0,
                "outline_alignment_rate": 63.0,
                "dialogue_naturalness_rate": 82.0,
                "opening_hook_rate": 66.0,
                "payoff_chain_rate": 71.0,
                "cliffhanger_rate": 60.0,
            }
        },
        ensure_ascii=False,
    )

    metrics = extract_quality_metrics_from_history_payload(generated_content, scope="chapter")

    assert metrics is not None
    assert metrics["outline_alignment_rate"] == 63.0
    assert metrics["repair_guidance"]["weakest_metric_key"] == "conflict_chain_hit_rate"
    assert metrics["repair_guidance"]["focus_areas"][0] == "conflict"
    assert metrics["quality_gate"]["status"] == "blocked"
    assert metrics["quality_gate"]["failed_metrics"][0]["label"] == "冲突链推进"


def test_should_build_outline_quality_metrics_summary_from_recent_history():
    summary = build_quality_metrics_summary(
        [
            {
                "overall_score": 72.0,
                "conflict_chain_hit_rate": 58.0,
                "rule_grounding_hit_rate": 76.0,
                "outline_alignment_rate": 61.0,
                "dialogue_naturalness_rate": 80.0,
                "opening_hook_rate": 67.0,
                "payoff_chain_rate": 66.0,
                "cliffhanger_rate": 59.0,
            },
            {
                "overall_score": 75.0,
                "conflict_chain_hit_rate": 62.0,
                "rule_grounding_hit_rate": 79.0,
                "outline_alignment_rate": 64.0,
                "dialogue_naturalness_rate": 82.0,
                "opening_hook_rate": 69.0,
                "payoff_chain_rate": 68.0,
                "cliffhanger_rate": 63.0,
            },
        ],
        scope="outline",
    )

    assert summary is not None
    assert summary["chapter_count"] == 2
    assert summary["avg_outline_alignment_rate"] == 62.5
    assert summary["repair_guidance"]["summary"].startswith("最近章节")
    assert summary["repair_guidance"]["focus_areas"]
    assert summary["quality_gate"]["status"] == "blocked"
    assert summary["quality_gate"]["failed_metrics"][0]["label"] == "冲突链推进"



def test_should_use_opening_stage_thresholds_for_quality_gate():
    quality_gate = build_quality_gate_decision(
        {
            "overall_score": 83.0,
            "conflict_chain_hit_rate": 86.0,
            "rule_grounding_hit_rate": 87.0,
            "outline_alignment_rate": 84.0,
            "dialogue_naturalness_rate": 81.0,
            "opening_hook_rate": 80.0,
            "payoff_chain_rate": 78.0,
            "cliffhanger_rate": 83.0,
            "quality_runtime_context": {
                "plot_stage": "opening",
                "chapter_count": 12,
                "current_chapter_number": 1,
            },
        },
        scope="chapter",
    )

    assert quality_gate["quality_stage"] == "opening"
    assert quality_gate["manual_review_threshold"] == 68.0
    assert quality_gate["allow_save_threshold"] == 80.0
    assert quality_gate["allow_save"] is True



def test_should_raise_ending_payoff_threshold_when_foreshadow_pressure_is_high():
    quality_gate = build_quality_gate_decision(
        {
            "overall_score": 84.0,
            "conflict_chain_hit_rate": 84.0,
            "rule_grounding_hit_rate": 85.0,
            "outline_alignment_rate": 84.0,
            "dialogue_naturalness_rate": 82.0,
            "opening_hook_rate": 78.0,
            "payoff_chain_rate": 74.0,
            "cliffhanger_rate": 83.0,
            "quality_runtime_context": {
                "plot_stage": "ending",
                "chapter_count": 12,
                "current_chapter_number": 11,
                "foreshadow_state_ledger": [
                    "hidden key: still missing from the archive",
                    "banquet trap: not yet triggered in court",
                    "royal seal: ownership has not been confirmed",
                ],
            },
        },
        scope="chapter",
    )

    assert quality_gate["quality_stage"] == "ending"
    assert quality_gate["allow_save_threshold"] == 85.0
    assert quality_gate["quality_runtime_pressure"]["foreshadow_state_count"] == 3
    assert quality_gate["decision"] == "manual_review"
    assert quality_gate["requires_manual_review"] is True



def test_should_aggregate_continuity_preflight_from_history_summary():
    summary = build_quality_metrics_summary(
        [
            {
                "overall_score": 82.0,
                "conflict_chain_hit_rate": 76.0,
                "rule_grounding_hit_rate": 80.0,
                "outline_alignment_rate": 79.0,
                "dialogue_naturalness_rate": 81.0,
                "opening_hook_rate": 75.0,
                "payoff_chain_rate": 74.0,
                "cliffhanger_rate": 78.0,
                "quality_runtime_context": {
                    "plot_stage": "development",
                    "chapter_count": 12,
                    "current_chapter_number": 5,
                },
                "continuity_preflight": {
                    "status": "warning",
                    "warning_count": 1,
                    "missing_item_count": 1,
                    "checked_item_count": 3,
                    "warnings": [
                        {
                            "ledger_label": "Foreshadow continuity ledger",
                            "focus_area": "foreshadow_continuity",
                            "item": "RoyalKey: still missing from the archive",
                        }
                    ],
                    "focus_areas": ["foreshadow_continuity"],
                    "repair_targets": ["Advance the foreshadow ledger toward payoff: RoyalKey: still missing from the archive"],
                    "summary": "Current chapter misses explicit handoff for 1 continuity ledger items. Prioritize Foreshadow continuity ledger.",
                },
            },
            {
                "overall_score": 84.0,
                "conflict_chain_hit_rate": 78.0,
                "rule_grounding_hit_rate": 82.0,
                "outline_alignment_rate": 81.0,
                "dialogue_naturalness_rate": 83.0,
                "opening_hook_rate": 77.0,
                "payoff_chain_rate": 76.0,
                "cliffhanger_rate": 79.0,
                "quality_runtime_context": {
                    "plot_stage": "development",
                    "chapter_count": 12,
                    "current_chapter_number": 6,
                },
                "continuity_preflight": {
                    "status": "warning",
                    "warning_count": 1,
                    "missing_item_count": 1,
                    "checked_item_count": 2,
                    "warnings": [
                        {
                            "ledger_label": "Relationship continuity ledger",
                            "focus_area": "relationship_continuity",
                            "item": "Lin/Su: uneasy alliance under tension",
                        }
                    ],
                    "focus_areas": ["relationship_continuity"],
                    "repair_targets": ["Express the relationship ledger through dialogue, alignment, or exchange: Lin/Su: uneasy alliance under tension"],
                    "summary": "Current chapter misses explicit handoff for 1 continuity ledger items. Prioritize Relationship continuity ledger.",
                },
            },
        ],
        scope="batch",
    )

    assert summary is not None
    assert summary["continuity_preflight"]["warning_count"] == 2
    assert "foreshadow_continuity" in summary["repair_guidance"]["focus_areas"]
    assert "relationship_continuity" in summary["repair_guidance"]["focus_areas"]


def test_should_aggregate_runtime_context_and_trend_from_history():
    summary = build_quality_metrics_summary(
        [
            {
                "overall_score": 72.0,
                "conflict_chain_hit_rate": 58.0,
                "rule_grounding_hit_rate": 76.0,
                "outline_alignment_rate": 61.0,
                "dialogue_naturalness_rate": 80.0,
                "opening_hook_rate": 67.0,
                "payoff_chain_rate": 66.0,
                "cliffhanger_rate": 59.0,
                "quality_runtime_context": {
                    "plot_stage": "development",
                    "chapter_count": 12,
                    "current_chapter_number": 4,
                    "character_state_ledger": ["Lin: distrust remains visible"],
                },
            },
            {
                "overall_score": 76.0,
                "conflict_chain_hit_rate": 64.0,
                "rule_grounding_hit_rate": 79.0,
                "outline_alignment_rate": 67.0,
                "dialogue_naturalness_rate": 82.0,
                "opening_hook_rate": 69.0,
                "payoff_chain_rate": 68.0,
                "cliffhanger_rate": 63.0,
                "quality_runtime_context": {
                    "plot_stage": "development",
                    "chapter_count": 12,
                    "current_chapter_number": 5,
                    "relationship_state_ledger": ["Lin/Su: uneasy alliance under tension"],
                    "foreshadow_state_ledger": ["hidden key: still missing from the archive"],
                },
            },
        ],
        scope="outline",
    )

    assert summary is not None
    assert summary["overall_score_delta"] == 4.0
    assert summary["overall_score_trend"] == "rising"
    assert summary["quality_runtime_context"]["current_chapter_number"] == 5
    assert summary["quality_runtime_context"]["character_state_ledger"] == ["Lin: distrust remains visible"]
    assert summary["quality_runtime_context"]["relationship_state_ledger"] == ["Lin/Su: uneasy alliance under tension"]
    assert summary["quality_runtime_context"]["foreshadow_state_ledger"] == ["hidden key: still missing from the archive"]


def test_should_restore_runtime_snapshot_from_generation_history_payload():
    payload = json.dumps(
        {
            "log_type": "chapter_generation_quality_v1",
            "preview": "chapter preview",
            "quality_metrics": {
                "overall_score": 84.0,
                "conflict_chain_hit_rate": 82.0,
                "rule_grounding_hit_rate": 85.0,
                "outline_alignment_rate": 83.0,
                "dialogue_naturalness_rate": 81.0,
                "opening_hook_rate": 79.0,
                "payoff_chain_rate": 78.0,
                "cliffhanger_rate": 80.0,
            },
            "story_runtime_snapshot": {
                "plot_stage": "development",
                "chapter_count": 12,
                "current_chapter_number": 6,
                "organization_state_ledger": ["ShadowGuild: control tightened around the docks"],
                "career_state_ledger": ["Lin/Strategist: stage 3 with supply-chain pressure"],
            },
        },
        ensure_ascii=False,
    )

    metrics = extract_quality_metrics_from_history_payload(payload, scope="chapter")

    assert metrics is not None
    assert metrics["quality_runtime_context"]["plot_stage"] == "development"
    assert metrics["quality_runtime_context"]["organization_state_ledger"] == [
        "ShadowGuild: control tightened around the docks"
    ]
    assert metrics["quality_runtime_context"]["career_state_ledger"] == [
        "Lin/Strategist: stage 3 with supply-chain pressure"
    ]
    assert metrics["quality_gate"]["quality_runtime_pressure"]["organization_state_count"] == 1
    assert metrics["quality_gate"]["quality_runtime_pressure"]["career_state_count"] == 1


def test_should_collect_recent_quality_gate_trends_from_history():
    summary = build_quality_metrics_summary(
        [
            {
                "overall_score": 66.0,
                "conflict_chain_hit_rate": 48.0,
                "rule_grounding_hit_rate": 52.0,
                "outline_alignment_rate": 50.0,
                "dialogue_naturalness_rate": 75.0,
                "opening_hook_rate": 70.0,
                "payoff_chain_rate": 46.0,
                "cliffhanger_rate": 68.0,
                "pacing_score": 6.1,
            },
            {
                "overall_score": 79.0,
                "conflict_chain_hit_rate": 77.0,
                "rule_grounding_hit_rate": 81.0,
                "outline_alignment_rate": 80.0,
                "dialogue_naturalness_rate": 79.0,
                "opening_hook_rate": 82.0,
                "payoff_chain_rate": 78.0,
                "cliffhanger_rate": 80.0,
                "pacing_score": 7.5,
            },
            {
                "overall_score": 88.0,
                "conflict_chain_hit_rate": 86.0,
                "rule_grounding_hit_rate": 87.0,
                "outline_alignment_rate": 84.0,
                "dialogue_naturalness_rate": 81.0,
                "opening_hook_rate": 80.0,
                "payoff_chain_rate": 78.0,
                "cliffhanger_rate": 83.0,
                "pacing_score": 8.1,
            },
        ],
        scope="batch",
    )

    assert summary is not None
    assert summary["quality_gate_counts"]["blocked"] == 1
    assert summary["quality_gate_counts"]["repairable"] == 1
    assert summary["quality_gate_counts"]["pass"] == 1
    assert summary["recent_manual_review_count"] == 1
    assert summary["recent_auto_repair_count"] == 1
    assert summary["recent_failed_metric_counts"]
    assert summary["recent_failed_metric_counts"][0]["key"] in {"conflict_chain_hit_rate", "payoff_chain_rate", "pacing_score"}


def test_should_build_long_form_pacing_imbalance_from_recent_history():
    summary = build_quality_metrics_summary(
        [
            {
                "overall_score": 76.0,
                "conflict_chain_hit_rate": 61.0,
                "rule_grounding_hit_rate": 79.0,
                "outline_alignment_rate": 63.0,
                "dialogue_naturalness_rate": 78.0,
                "opening_hook_rate": 69.0,
                "payoff_chain_rate": 58.0,
                "cliffhanger_rate": 87.0,
                "pacing_score": 6.7,
            },
            {
                "overall_score": 75.0,
                "conflict_chain_hit_rate": 60.0,
                "rule_grounding_hit_rate": 77.0,
                "outline_alignment_rate": 62.0,
                "dialogue_naturalness_rate": 79.0,
                "opening_hook_rate": 70.0,
                "payoff_chain_rate": 59.0,
                "cliffhanger_rate": 86.0,
                "pacing_score": 6.8,
            },
            {
                "overall_score": 74.0,
                "conflict_chain_hit_rate": 59.0,
                "rule_grounding_hit_rate": 78.0,
                "outline_alignment_rate": 61.0,
                "dialogue_naturalness_rate": 77.0,
                "opening_hook_rate": 68.0,
                "payoff_chain_rate": 57.0,
                "cliffhanger_rate": 85.0,
                "pacing_score": 6.6,
            },
            {
                "overall_score": 75.0,
                "conflict_chain_hit_rate": 62.0,
                "rule_grounding_hit_rate": 80.0,
                "outline_alignment_rate": 64.0,
                "dialogue_naturalness_rate": 78.0,
                "opening_hook_rate": 71.0,
                "payoff_chain_rate": 60.0,
                "cliffhanger_rate": 84.0,
                "pacing_score": 6.9,
            },
            {
                "overall_score": 74.0,
                "conflict_chain_hit_rate": 60.0,
                "rule_grounding_hit_rate": 79.0,
                "outline_alignment_rate": 63.0,
                "dialogue_naturalness_rate": 80.0,
                "opening_hook_rate": 70.0,
                "payoff_chain_rate": 58.0,
                "cliffhanger_rate": 86.0,
                "pacing_score": 6.7,
            },
        ],
        scope="batch",
    )

    assert summary is not None
    pacing_imbalance = summary["pacing_imbalance"]
    assert pacing_imbalance["status"] == "warning"
    assert pacing_imbalance["recent_progression_density"] == 60.5
    assert pacing_imbalance["recent_payoff_rate"] == 58.4
    assert pacing_imbalance["recent_cliffhanger_pull"] == 85.6
    assert pacing_imbalance["recent_tension_variation"] == 1.1
    assert {signal["key"] for signal in pacing_imbalance["signals"]} >= {"middle_drag", "overstretched_suspense", "payoff_fatigue"}
    assert pacing_imbalance["repair_targets"]
    assert pacing_imbalance["summary"]
    assert "pacing" in summary["repair_guidance"]["focus_areas"]
    assert summary["repair_guidance"]["repair_targets"]



def test_should_build_volume_goal_completion_and_foreshadow_payoff_delay_from_recent_history():
    summary = build_quality_metrics_summary(
        [
            {
                "overall_score": 76.0,
                "conflict_chain_hit_rate": 65.0,
                "rule_grounding_hit_rate": 81.0,
                "outline_alignment_rate": 67.0,
                "dialogue_naturalness_rate": 80.0,
                "opening_hook_rate": 72.0,
                "payoff_chain_rate": 60.0,
                "cliffhanger_rate": 82.0,
                "pacing_score": 6.6,
                "quality_runtime_context": {
                    "plot_stage": "development",
                    "chapter_count": 12,
                    "current_chapter_number": 10,
                    "foreshadow_payoff_plan": ["王城密钥", "苏离盟约"],
                    "foreshadow_state_ledger": ["王城密钥仍未现身", "苏离盟约还未兑现", "档案馆真相仍被压住"],
                    "character_state_ledger": ["林砚：必须在终局前拿回主动权"],
                },
            },
            {
                "overall_score": 75.0,
                "conflict_chain_hit_rate": 64.0,
                "rule_grounding_hit_rate": 80.0,
                "outline_alignment_rate": 66.0,
                "dialogue_naturalness_rate": 79.0,
                "opening_hook_rate": 71.0,
                "payoff_chain_rate": 58.0,
                "cliffhanger_rate": 84.0,
                "pacing_score": 6.4,
                "quality_runtime_context": {
                    "plot_stage": "development",
                    "chapter_count": 12,
                    "current_chapter_number": 10,
                    "foreshadow_payoff_plan": ["王城密钥", "苏离盟约"],
                    "foreshadow_state_ledger": ["王城密钥仍未现身", "苏离盟约还未兑现", "档案馆真相仍被压住"],
                    "character_state_ledger": ["林砚：必须在终局前拿回主动权"],
                },
            },
            {
                "overall_score": 74.0,
                "conflict_chain_hit_rate": 63.0,
                "rule_grounding_hit_rate": 79.0,
                "outline_alignment_rate": 65.0,
                "dialogue_naturalness_rate": 78.0,
                "opening_hook_rate": 70.0,
                "payoff_chain_rate": 57.0,
                "cliffhanger_rate": 83.0,
                "pacing_score": 6.5,
                "quality_runtime_context": {
                    "plot_stage": "development",
                    "chapter_count": 12,
                    "current_chapter_number": 10,
                    "foreshadow_payoff_plan": ["王城密钥", "苏离盟约"],
                    "foreshadow_state_ledger": ["王城密钥仍未现身", "苏离盟约还未兑现", "档案馆真相仍被压住"],
                    "character_state_ledger": ["林砚：必须在终局前拿回主动权"],
                },
            },
        ],
        scope="batch",
    )

    assert summary is not None
    assert summary["volume_goal_completion"]["status"] in {"watch", "warning"}
    assert summary["volume_goal_completion"]["expected_stage"] == "ending"
    assert summary["volume_goal_completion"]["current_stage"] == "development"
    assert summary["foreshadow_payoff_delay"]["status"] in {"watch", "warning"}
    assert summary["foreshadow_payoff_delay"]["delay_index"] >= 35.0
    assert summary["foreshadow_payoff_delay"]["repair_targets"]
    assert "payoff" in summary["repair_guidance"]["focus_areas"]
    assert any("伏笔" in item or "兑现" in item for item in summary["repair_guidance"]["repair_targets"])


def test_should_build_quality_metrics_summary_from_reducer_state():
    history = [
        {
            "overall_score": 81.0,
            "conflict_chain_hit_rate": 76.0,
            "rule_grounding_hit_rate": 80.0,
            "outline_alignment_rate": 79.0,
            "dialogue_naturalness_rate": 78.0,
            "opening_hook_rate": 74.0,
            "payoff_chain_rate": 72.0,
            "cliffhanger_rate": 77.0,
            "pacing_score": 7.2,
        },
        {
            "overall_score": 86.0,
            "conflict_chain_hit_rate": 82.0,
            "rule_grounding_hit_rate": 84.0,
            "outline_alignment_rate": 83.0,
            "dialogue_naturalness_rate": 80.0,
            "opening_hook_rate": 79.0,
            "payoff_chain_rate": 75.0,
            "cliffhanger_rate": 81.0,
            "pacing_score": 8.0,
        },
    ]

    summary = build_quality_metrics_summary(history, scope="batch")
    state = build_quality_metrics_summary_state(history, scope="batch")

    assert state is not None
    assert build_quality_metrics_summary_from_state(state, scope="batch") == summary


def test_should_advance_quality_metrics_summary_state_incrementally_with_drop():
    history = [
        {
            "overall_score": 79.0,
            "conflict_chain_hit_rate": 72.0,
            "rule_grounding_hit_rate": 77.0,
            "outline_alignment_rate": 74.0,
            "dialogue_naturalness_rate": 76.0,
            "opening_hook_rate": 73.0,
            "payoff_chain_rate": 70.0,
            "cliffhanger_rate": 75.0,
            "pacing_score": 6.8,
        },
        {
            "overall_score": 84.0,
            "conflict_chain_hit_rate": 80.0,
            "rule_grounding_hit_rate": 82.0,
            "outline_alignment_rate": 81.0,
            "dialogue_naturalness_rate": 79.0,
            "opening_hook_rate": 78.0,
            "payoff_chain_rate": 74.0,
            "cliffhanger_rate": 80.0,
            "pacing_score": 7.6,
        },
    ]
    next_event = {
        "overall_score": 88.0,
        "conflict_chain_hit_rate": 85.0,
        "rule_grounding_hit_rate": 86.0,
        "outline_alignment_rate": 87.0,
        "dialogue_naturalness_rate": 82.0,
        "opening_hook_rate": 81.0,
        "payoff_chain_rate": 79.0,
        "cliffhanger_rate": 84.0,
        "pacing_score": 8.3,
    }

    state = build_quality_metrics_summary_state(history, scope="batch")
    trimmed_history = [history[1], next_event]
    advanced_state = advance_quality_metrics_summary_state(
        state,
        appended_event=next_event,
        current_history=trimmed_history,
        dropped_event=history[0],
        scope="batch",
    )

    assert advanced_state is not None
    assert build_quality_metrics_summary_from_state(advanced_state, scope="batch") == build_quality_metrics_summary(
        trimmed_history,
        scope="batch",
    )

