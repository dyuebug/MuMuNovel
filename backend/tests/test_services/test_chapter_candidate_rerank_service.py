from tests.test_support.chapter_candidate_rerank_test_support import (
    build_candidate_pool_summary,
    build_candidate_selection_metadata,
    build_candidate_retry_strategy_suffix,
    build_targeted_final_repair_suffix,
    build_word_budget_repair_suffix,
    normalize_candidate_quality_gate,
    resolve_candidate_retry_temperature,
    resolve_targeted_final_repair_char_limit,
    resolve_targeted_final_repair_max_tokens,
    resolve_targeted_final_repair_temperature,
    resolve_word_budget_repair_char_limit,
    resolve_word_budget_repair_max_tokens,
    resolve_word_budget_repair_temperature,
    select_targeted_final_repair_seed_candidate,
    should_adopt_targeted_final_repair_candidate,
    should_apply_followup_targeted_final_repair,
    should_apply_targeted_final_repair,
    should_keep_targeted_final_repair_candidate,
    should_keep_word_budget_repair_candidate,
    should_relax_word_budget_repair_limits,
    should_prefer_targeted_final_repair_candidate,
    should_prefer_word_budget_repair_candidate,
    should_apply_word_budget_repair,
    should_generate_additional_candidate,
)


def test_should_not_generate_additional_candidate_for_pure_word_count_deviation():
    candidate = {
        "quality_gate_decision": "manual_review",
        "word_count": 2909,
        "target_word_count": 1200,
        "word_count_fit_score": 0.0,
    }

    assert should_generate_additional_candidate(
        candidate,
        produced_candidates=1,
        max_candidates=2,
    ) is False



def test_should_generate_additional_candidate_when_quality_gate_requests_content_repair():
    candidate = {
        "quality_gate_decision": "auto_repair",
        "word_count": 2909,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": [{"label": "Conflict chain", "focus_area": "conflict"}],
            },
            "active_story_repair_payload": {
                "summary": "Strengthen the scene conflict",
                "repair_targets": ["Escalate opposition"],
            },
        },
    }

    assert should_generate_additional_candidate(
        candidate,
        produced_candidates=1,
        max_candidates=2,
    ) is True


def test_should_not_generate_additional_candidate_when_word_count_is_in_window():
    candidate = {
        "quality_gate_decision": "manual_review",
        "word_count": 1220,
        "target_word_count": 1200,
        "word_count_fit_score": 98.3,
    }

    assert should_generate_additional_candidate(
        candidate,
        produced_candidates=1,
        max_candidates=2,
    ) is False


def test_should_build_retry_strategy_with_word_budget_guard_for_overlong_draft():
    suffix = build_candidate_retry_strategy_suffix(
        quality_gate_plan={},
        quality_metrics={
            "candidate_selection": {
                "word_count": 2909,
                "target_word_count": 1200,
            }
        },
        attempt_index=2,
        source="chapter",
    )

    assert "2909" in suffix
    assert "1080-1350" in suffix
    assert "rewrite to stay within" in suffix


def test_should_lower_retry_temperature_when_previous_draft_ran_long():
    retry_temperature = resolve_candidate_retry_temperature(
        0.8,
        quality_metrics={
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
            },
            "candidate_selection": {
                "word_count": 2909,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={"quality_gate": {"decision": "manual_review"}},
        attempt_index=2,
    )

    assert retry_temperature < 0.8


def test_should_apply_word_budget_repair_when_candidate_remains_far_above_budget():
    candidate = {
        "quality_gate_decision": "manual_review",
        "word_count": 1759,
        "target_word_count": 1200,
    }

    assert should_apply_word_budget_repair(candidate) is True



def test_should_not_apply_word_budget_repair_when_candidate_is_only_slightly_long():
    candidate = {
        "quality_gate_decision": "manual_review",
        "word_count": 1410,
        "target_word_count": 1200,
    }

    assert should_apply_word_budget_repair(candidate) is False



def test_should_apply_word_budget_repair_when_candidate_remains_far_below_budget():
    candidate = {
        "quality_gate_decision": "manual_review",
        "word_count": 900,
        "target_word_count": 1200,
    }

    assert should_apply_word_budget_repair(candidate) is True



def test_should_build_word_budget_repair_suffix_with_hard_cap():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "candidate_selection": {
                "word_count": 1759,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [{"label": "Outline alignment"}],
            }
        },
        target_word_count=1200,
        attempt_index=3,
        source="chapter",
    )

    assert "Word-budget repair pass #3" in suffix
    assert "1080-1350" in suffix
    assert "do not exceed 1350" in suffix



def test_should_build_word_budget_repair_suffix_for_shortfall_expansion():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
            },
            "candidate_selection": {
                "word_count": 900,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={},
        target_word_count=1200,
        attempt_index=2,
        source="chapter",
    )

    assert "landed short" in suffix
    assert "expand with concrete action" in suffix


def test_should_include_opening_and_closing_anchors_in_word_budget_repair_suffix():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1759,
                "target_word_count": 1200,
            }
        },
        quality_gate_plan={},
        current_content=(
            "第一段里，直播画面突然跳出错误编号1774721191，林折意识到今晚不是普通事故。\n\n"
            "中段反复解释平台规则、旧事故背景和旁观者反应。\n\n"
            "结尾里，镜头中的人先一步抬头，对屏幕外准确叫出林折的名字。"
        ),
        target_word_count=1200,
        attempt_index=2,
        source="chapter",
    )

    assert "Preserve this opening anchor beat" in suffix
    assert "错误编号1774721191" in suffix
    assert "Preserve this closing hook beat" in suffix
    assert "准确叫出林折的名字" in suffix



def test_should_lower_word_budget_repair_temperature_for_plot_drive_hook():
    retry_temperature = resolve_word_budget_repair_temperature(
        0.8,
        quality_metrics={
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
            }
        },
    )

    assert retry_temperature < 0.8
    assert retry_temperature <= 0.62



def test_should_resolve_tighter_word_budget_repair_limits_for_overlong_draft():
    max_tokens = resolve_word_budget_repair_max_tokens(
        1200,
        current_word_count=1916,
    )
    char_limit = resolve_word_budget_repair_char_limit(1200)

    assert max_tokens == 607
    assert char_limit == 1386


def test_should_relax_word_budget_repair_limits_for_content_sensitive_focus_areas():
    assert should_relax_word_budget_repair_limits(
        {
            "quality_gate": {
                "failed_metrics": [{"label": "Rule grounding", "focus_area": "rule_grounding"}],
            }
        }
    ) is True


def test_should_keep_tight_word_budget_repair_limits_without_content_focus_area():
    assert should_relax_word_budget_repair_limits(
        {
            "quality_gate": {
                "failed_metrics": [{"label": "Word budget", "focus_area": "word_budget"}],
            }
        }
    ) is False


def test_should_resolve_relaxed_word_budget_limits_for_content_sensitive_repair():
    max_tokens = resolve_word_budget_repair_max_tokens(
        1200,
        current_word_count=1916,
        relax_content_budget=True,
    )
    char_limit = resolve_word_budget_repair_char_limit(
        1200,
        relax_content_budget=True,
    )

    assert max_tokens >= 640
    assert char_limit >= 1420


def test_should_prefer_word_budget_repair_candidate_when_it_recovers_budget_gap():
    selected_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 90.5,
        "word_count": 1916,
        "target_word_count": 1200,
    }
    repair_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 87.8,
        "word_count": 1376,
        "target_word_count": 1200,
    }

    assert should_prefer_word_budget_repair_candidate(selected_candidate, repair_candidate) is True


def test_should_not_prefer_word_budget_repair_candidate_when_quality_drop_is_too_large():
    selected_candidate = {
        "quality_gate_decision": "allow_save",
        "overall_score": 93.2,
        "word_count": 1820,
        "target_word_count": 1200,
    }
    repair_candidate = {
        "quality_gate_decision": "allow_save",
        "overall_score": 84.1,
        "word_count": 1298,
        "target_word_count": 1200,
    }

    assert should_prefer_word_budget_repair_candidate(selected_candidate, repair_candidate) is False


def test_should_prefer_word_budget_repair_candidate_for_severely_overlong_allow_save_draft():
    selected_candidate = {
        "quality_gate_decision": "allow_save",
        "overall_score": 92.0,
        "word_count": 1825,
        "target_word_count": 1200,
    }
    repair_candidate = {
        "quality_gate_decision": "allow_save",
        "overall_score": 84.4,
        "word_count": 1422,
        "target_word_count": 1200,
    }

    assert should_prefer_word_budget_repair_candidate(selected_candidate, repair_candidate) is True


def test_should_prefer_word_budget_repair_candidate_for_severely_overlong_auto_repair_draft_with_fewer_failed_metrics():
    selected_candidate = {
        "quality_gate_decision": "auto_repair",
        "overall_score": 80.5,
        "word_count": 2903,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                    {"label": "Conflict chain", "focus_area": "conflict"},
                ]
            }
        },
    }
    repair_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 68.6,
        "word_count": 1422,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Conflict chain", "focus_area": "conflict"},
                ]
            }
        },
    }

    assert should_prefer_word_budget_repair_candidate(selected_candidate, repair_candidate) is True


def test_should_not_keep_word_budget_repair_candidate_when_it_collapses_and_adds_failures():
    selected_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 90.8,
        "word_count": 1944,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
    }
    repair_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 46.1,
        "word_count": 1422,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "failed_metrics": [
                    {"label": "Conflict chain", "focus_area": "conflict"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ]
            }
        },
    }

    assert should_keep_word_budget_repair_candidate(selected_candidate, repair_candidate) is False


def test_should_apply_targeted_final_repair_for_near_target_manual_review_tail_gaps():
    candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 90.7,
        "word_count": 1398,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 90.7,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Dialogue", "focus_area": "dialogue"},
                ],
            }
        },
    }

    assert should_apply_targeted_final_repair(candidate) is True



def test_should_not_apply_targeted_final_repair_for_structural_gap_or_large_budget_deviation():
    candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 90.7,
        "word_count": 1825,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 90.7,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Conflict chain", "focus_area": "conflict"},
                ],
            }
        },
    }

    assert should_apply_targeted_final_repair(candidate) is False



def test_should_prefer_targeted_final_repair_candidate_when_it_unblocks_manual_review():
    selected_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 90.7,
        "word_count": 1398,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Dialogue", "focus_area": "dialogue"},
                ]
            }
        },
    }
    repair_candidate = {
        "quality_gate_decision": "allow_save",
        "overall_score": 88.5,
        "word_count": 1362,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": []
            }
        },
    }

    assert should_prefer_targeted_final_repair_candidate(selected_candidate, repair_candidate) is True


def test_should_prefer_targeted_final_repair_candidate_for_severely_overlong_auto_repair_winner():
    selected_candidate = {
        "quality_gate_decision": "auto_repair",
        "overall_score": 98.3,
        "word_count": 2023,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "auto_repair",
                "failed_metrics": [],
            }
        },
    }
    repair_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 93.1,
        "word_count": 1434,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ],
            }
        },
    }

    assert should_prefer_targeted_final_repair_candidate(selected_candidate, repair_candidate) is True



def test_should_prefer_targeted_final_repair_candidate_when_same_cliffhanger_gap_scores_jump():
    selected_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 85.9,
        "word_count": 1422,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
    }
    repair_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 88.4,
        "word_count": 1434,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
    }

    assert should_prefer_targeted_final_repair_candidate(selected_candidate, repair_candidate) is True


def test_should_not_adopt_targeted_final_repair_candidate_when_failed_metrics_increase():
    seed_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 93.1,
        "word_count": 1422,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
    }
    repair_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 94.0,
        "word_count": 1433,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ]
            }
        },
    }

    assert should_adopt_targeted_final_repair_candidate(seed_candidate, repair_candidate) is False


def test_should_not_adopt_targeted_final_repair_candidate_when_word_budget_and_score_regress():
    seed_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 92.6,
        "word_count": 1412,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
    }
    repair_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 92.1,
        "word_count": 1478,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
    }

    assert should_adopt_targeted_final_repair_candidate(seed_candidate, repair_candidate) is False


def test_should_not_keep_targeted_final_repair_candidate_when_it_collapses_far_below_floor():
    seed_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 88.7,
        "word_count": 1422,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
    }
    repair_candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 67.6,
        "word_count": 554,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                    {"label": "Outline", "focus_area": "outline"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
    }

    assert should_keep_targeted_final_repair_candidate(seed_candidate, repair_candidate) is False


def test_should_normalize_allow_save_quality_gate_for_severe_word_budget_pressure():
    normalized_quality_gate = normalize_candidate_quality_gate(
        {
            "decision": "allow_save",
            "status": "pass",
        },
        word_count=2023,
        target_word_count=1200,
    )

    assert normalized_quality_gate["decision"] == "auto_repair"
    assert normalized_quality_gate["status"] == "repairable"
    assert normalized_quality_gate["allow_save"] is False
    assert normalized_quality_gate["can_auto_repair"] is True



def test_should_apply_targeted_final_repair_for_near_target_conflict_tail_gaps():
    candidate = {
        "attempt_kind": "word_budget_repair",
        "generation_path": "word_budget_repair",
        "quality_gate_decision": "manual_review",
        "overall_score": 85.2,
        "word_count": 1418,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 85.2,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Conflict chain", "focus_area": "conflict"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ],
            }
        },
    }

    assert should_apply_targeted_final_repair(candidate) is True



def test_should_apply_targeted_final_repair_for_high_score_rule_grounding_only_gap():
    candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 95.3,
        "word_count": 1434,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 95.3,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ],
            }
        },
    }

    assert should_apply_targeted_final_repair(candidate) is True



def test_should_allow_followup_targeted_final_repair_for_rule_grounding_only_gap():
    candidate = {
        "attempt_kind": "targeted_quality_repair",
        "generation_path": "targeted_quality_repair",
        "quality_gate_decision": "manual_review",
        "overall_score": 95.3,
        "word_count": 1434,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 95.3,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ],
            }
        },
    }

    assert should_apply_followup_targeted_final_repair(candidate) is True


def test_should_allow_followup_targeted_final_repair_for_cliffhanger_only_gap():
    candidate = {
        "attempt_kind": "targeted_quality_repair",
        "generation_path": "targeted_quality_repair",
        "quality_gate_decision": "manual_review",
        "overall_score": 90.4,
        "word_count": 1418,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 90.4,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ],
            }
        },
    }

    assert should_apply_followup_targeted_final_repair(candidate) is True


def test_should_allow_followup_targeted_final_repair_for_rule_grounding_and_cliffhanger_gap():
    candidate = {
        "attempt_kind": "targeted_quality_repair",
        "generation_path": "targeted_quality_repair",
        "quality_gate_decision": "manual_review",
        "overall_score": 91.4,
        "word_count": 1426,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 91.4,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ],
            }
        },
    }

    assert should_apply_followup_targeted_final_repair(candidate) is True


def test_should_allow_followup_targeted_final_repair_for_opening_and_rule_grounding_gap():
    candidate = {
        "attempt_kind": "targeted_quality_repair",
        "generation_path": "targeted_quality_repair",
        "quality_gate_decision": "manual_review",
        "overall_score": 90.2,
        "word_count": 1420,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 90.2,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Opening", "focus_area": "opening"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ],
            }
        },
    }

    assert should_apply_followup_targeted_final_repair(candidate) is True


def test_should_allow_followup_targeted_final_repair_for_dialogue_and_cliffhanger_gap():
    candidate = {
        "attempt_kind": "targeted_quality_repair",
        "generation_path": "targeted_quality_repair",
        "quality_gate_decision": "manual_review",
        "overall_score": 90.4,
        "word_count": 1418,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 90.4,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Dialogue", "focus_area": "dialogue"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ],
            }
        },
    }

    assert should_apply_followup_targeted_final_repair(candidate) is True


def test_should_allow_followup_targeted_final_repair_for_opening_conflict_cliffhanger_gap():
    candidate = {
        "attempt_kind": "targeted_quality_repair",
        "generation_path": "targeted_quality_repair",
        "quality_gate_decision": "manual_review",
        "overall_score": 90.6,
        "word_count": 1422,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 90.6,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Opening", "focus_area": "opening"},
                    {"label": "Conflict chain", "focus_area": "conflict"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ],
            }
        },
    }

    assert should_apply_followup_targeted_final_repair(candidate) is True



def test_should_allow_followup_targeted_final_repair_for_opening_rule_grounding_cliffhanger_gap():
    candidate = {
        "attempt_kind": "targeted_quality_repair",
        "generation_path": "targeted_quality_repair",
        "quality_gate_decision": "manual_review",
        "overall_score": 90.5,
        "word_count": 1421,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 90.5,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Opening", "focus_area": "opening"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ],
            }
        },
    }

    assert should_apply_followup_targeted_final_repair(candidate) is True



def test_should_not_apply_targeted_final_repair_for_low_score_rule_grounding_only_gap():
    candidate = {
        "quality_gate_decision": "manual_review",
        "overall_score": 86.4,
        "word_count": 1434,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 86.4,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ],
            }
        },
    }

    assert should_apply_targeted_final_repair(candidate) is False



def test_should_select_word_budget_repair_seed_for_targeted_final_repair_when_winner_is_still_overlong():
    selected_candidate = {
        "candidate_index": 2,
        "attempt_kind": "rerank_candidate",
        "generation_path": "rerank_retry",
        "quality_gate_decision": "manual_review",
        "overall_score": 88.0,
        "word_count": 1957,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 88.0,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Conflict chain", "focus_area": "conflict"},
                ],
            }
        },
    }
    repair_candidate = {
        "candidate_index": 3,
        "attempt_kind": "word_budget_repair",
        "generation_path": "word_budget_repair",
        "quality_gate_decision": "manual_review",
        "overall_score": 79.0,
        "word_count": 1422,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 79.0,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Conflict chain", "focus_area": "conflict"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ],
            }
        },
    }

    seed_candidate = select_targeted_final_repair_seed_candidate(
        selected_candidate,
        [selected_candidate, repair_candidate],
    )

    assert seed_candidate is repair_candidate


def test_should_fallback_to_overlong_cliffhanger_winner_when_no_viable_repair_seed_exists():
    selected_candidate = {
        "candidate_index": 2,
        "attempt_kind": "rerank_candidate",
        "generation_path": "rerank_retry",
        "quality_gate_decision": "manual_review",
        "overall_score": 90.8,
        "word_count": 1944,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 90.8,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ],
            }
        },
    }
    repair_candidate = {
        "candidate_index": 3,
        "attempt_kind": "word_budget_repair",
        "generation_path": "word_budget_repair",
        "quality_gate_decision": "manual_review",
        "overall_score": 46.1,
        "word_count": 1422,
        "target_word_count": 1200,
        "quality_gate_plan": {
            "quality_gate": {
                "decision": "manual_review",
                "overall_score": 46.1,
                "continuity_warning_count": 0,
                "failed_metrics": [
                    {"label": "Conflict chain", "focus_area": "conflict"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ],
            }
        },
    }

    seed_candidate = select_targeted_final_repair_seed_candidate(
        selected_candidate,
        [selected_candidate, repair_candidate],
    )

    assert seed_candidate is selected_candidate


def test_should_add_structural_checklist_to_retry_strategy_for_core_story_failures():
    suffix = build_candidate_retry_strategy_suffix(
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Conflict chain", "focus_area": "conflict"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                    {"label": "Payoff chain", "focus_area": "payoff"},
                ]
            }
        },
        quality_metrics={
            "quality_runtime_context": {
                "character_focus": ["Lin", "Qin"],
                "character_state_ledger": ["Lin: cornered by debt"],
                "organization_state_ledger": ["GrayNet: curfew tightened"],
                "foreshadow_payoff_plan": ["recover the hidden key"],
            },
            "candidate_selection": {
                "word_count": 1493,
                "target_word_count": 1200,
            },
        },
        attempt_index=3,
        source="chapter",
    )

    assert "Hard checklist" in suffix
    assert "Lin / Qin" in suffix
    assert "GrayNet: curfew tightened" in suffix
    assert "受阻" in suffix
    assert "兑现" in suffix



def test_should_add_cliffhanger_and_dialogue_repair_lines_to_retry_strategy():
    suffix = build_candidate_retry_strategy_suffix(
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Dialogue", "focus_area": "dialogue"},
                ]
            }
        },
        quality_metrics={
            "candidate_selection": {
                "word_count": 1493,
                "target_word_count": 1200,
            },
        },
        attempt_index=2,
        source="chapter",
    )

    assert "Rule-grounding repair" in suffix
    assert "Cliffhanger repair" in suffix
    assert "Dialogue repair" in suffix


def test_should_add_joint_pressure_rule_to_retry_strategy_for_conflict_and_rule_grounding():
    suffix = build_candidate_retry_strategy_suffix(
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Conflict chain", "focus_area": "conflict"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ]
            }
        },
        quality_metrics={
            "candidate_selection": {
                "word_count": 1493,
                "target_word_count": 1200,
            },
        },
        attempt_index=2,
        source="chapter",
    )

    assert "Rule-grounding repair" in suffix
    assert "Joint pressure repair" in suffix
    assert "immediate resistance and cost" in suffix


def test_should_add_hard_structural_constraints_to_word_budget_repair_suffix():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "quality_runtime_context": {
                "character_focus": ["Lin", "Qin"],
                "organization_state_ledger": ["GrayNet: curfew tightened"],
                "foreshadow_payoff_plan": ["recover the hidden key"],
            },
            "candidate_selection": {
                "word_count": 1493,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Conflict chain", "focus_area": "conflict"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                    {"label": "Payoff chain", "focus_area": "payoff"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=4,
        source="chapter",
    )

    assert "Word-budget repair pass #4" in suffix
    assert "Hard checklist" in suffix
    assert "正文必须写出" in suffix
    assert "不要输出标题" in suffix
    assert "recover the hidden key" in suffix


def test_should_add_cliffhanger_and_dialogue_hard_rules_to_word_budget_repair_suffix():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1916,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Dialogue", "focus_area": "dialogue"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=3,
        source="chapter",
    )

    assert "Rule-grounding repair" in suffix
    assert "Cliffhanger repair" in suffix
    assert "Dialogue hard rule" in suffix


def test_should_preserve_outline_hook_and_dialogue_when_compressing_word_budget_repair():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1825,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Outline", "focus_area": "outline"},
                    {"label": "Dialogue", "focus_area": "dialogue"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=3,
        source="chapter",
    )

    assert "Compress from the middle first" in suffix
    assert "Preserve the last-two-paragraph skeleton" in suffix
    assert "closing 3-5 lines as protected runway" in suffix
    assert "cut monologue exposition first" in suffix
    assert "Cliffhanger hard rule" in suffix
    assert "Outline hard rule" in suffix
    assert "Dialogue hard rule" in suffix


def test_should_build_targeted_final_repair_suffix_for_tail_hook_and_dialogue_gaps():
    suffix = build_targeted_final_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1398,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Dialogue", "focus_area": "dialogue"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=4,
        source="chapter",
    )

    assert "Targeted quality repair pass #4" in suffix
    assert "final 2-4 paragraphs" in suffix
    assert "cut monologue exposition first" in suffix
    assert "Cliffhanger hard rule" in suffix
    assert "Cliffhanger closing runway" in suffix


def test_should_build_targeted_final_repair_suffix_for_cliffhanger_only_gap():
    suffix = build_targeted_final_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1418,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=4,
        source="chapter",
    )

    assert "Cliffhanger hard rule" in suffix
    assert "Cliffhanger closing runway" in suffix
    assert "Cliffhanger escalation rule" in suffix
    assert "Cliffhanger framing rule" in suffix
    assert "Cliffhanger conversion rule" in suffix


def test_should_build_targeted_final_repair_suffix_for_rule_grounding_and_cliffhanger_gap():
    suffix = build_targeted_final_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1426,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=5,
        source="chapter",
    )

    assert "Joint repair focus" in suffix
    assert "Joint closing hard rule" in suffix


def test_should_build_targeted_final_repair_suffix_for_opening_and_rule_grounding_gap():
    suffix = build_targeted_final_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1420,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Opening", "focus_area": "opening"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=5,
        source="chapter",
    )

    assert "Opening repair focus" in suffix
    assert "Joint repair focus: make the opening anomaly" in suffix
    assert "Joint opening hard rule" in suffix


def test_should_build_targeted_final_repair_suffix_for_dialogue_and_cliffhanger_gap():
    suffix = build_targeted_final_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1418,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Dialogue", "focus_area": "dialogue"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=5,
        source="chapter",
    )

    assert "Dialogue repair focus" in suffix
    assert "Cliffhanger repair focus" in suffix
    assert "Joint repair focus: make one two-sided exchange" in suffix
    assert "Joint dialogue-cliffhanger hard rule" in suffix


def test_should_build_word_budget_repair_suffix_for_opening_and_rule_grounding_gap():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1760,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Opening", "focus_area": "opening"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=4,
        source="chapter",
    )

    assert "Joint compression rule: preserve one sharp opening anomaly or demand" in suffix
    assert "Opening repair" in suffix
    assert "Rule-grounding repair" in suffix


def test_should_build_word_budget_repair_suffix_for_dialogue_and_cliffhanger_gap():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1764,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Dialogue", "focus_area": "dialogue"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=4,
        source="chapter",
    )

    assert "Joint compression rule: preserve one decisive back-and-forth exchange" in suffix
    assert "Dialogue repair" in suffix
    assert "Cliffhanger repair" in suffix


def test_should_build_word_budget_repair_suffix_for_cliffhanger_only_gap():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1762,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=4,
        source="chapter",
    )

    assert "Cliffhanger compression rule" in suffix
    assert "Cliffhanger novelty rule" in suffix
    assert "Cliffhanger repair" in suffix


def test_should_build_targeted_final_repair_suffix_for_opening_conflict_cliffhanger_gap():
    suffix = build_targeted_final_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1422,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Opening", "focus_area": "opening"},
                    {"label": "Conflict chain", "focus_area": "conflict"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=5,
        source="chapter",
    )

    assert "Opening repair focus" in suffix
    assert "Three-beat repair focus" in suffix
    assert "Three-beat hard rule" in suffix


def test_should_build_word_budget_repair_suffix_for_opening_conflict_cliffhanger_gap():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1820,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Opening", "focus_area": "opening"},
                    {"label": "Conflict chain", "focus_area": "conflict"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=4,
        source="chapter",
    )

    assert "Three-beat compression rule" in suffix
    assert "Opening repair" in suffix
    assert "Cliffhanger repair" in suffix


def test_should_build_word_budget_repair_suffix_for_rule_grounding_and_cliffhanger_gap():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1760,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=4,
        source="chapter",
    )

    assert "Joint compression rule" in suffix
    assert "Cliffhanger repair" in suffix
    assert "Rule-grounding repair" in suffix



def test_should_build_targeted_final_repair_suffix_for_opening_rule_grounding_cliffhanger_gap():
    suffix = build_targeted_final_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1421,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Opening", "focus_area": "opening"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=5,
        source="chapter",
    )

    assert "Opening repair focus" in suffix
    assert "Rule-grounding repair focus" in suffix
    assert "Cliffhanger hard rule" in suffix
    assert "Joint repair focus: make the opening anomaly or urgent demand" in suffix
    assert "Joint triad hard rule" in suffix



def test_should_build_word_budget_repair_suffix_for_opening_rule_grounding_cliffhanger_gap():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1816,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Opening", "focus_area": "opening"},
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=4,
        source="chapter",
    )

    assert "Joint compression rule: preserve the causal chain of opening anomaly or task -> grounded rule consequence -> unresolved closing hook" in suffix
    assert "Opening repair" in suffix
    assert "Rule-grounding repair" in suffix
    assert "Cliffhanger repair" in suffix



def test_should_build_targeted_final_repair_suffix_for_rule_grounding_only_gap():
    suffix = build_targeted_final_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1434,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Rule grounding", "focus_area": "rule_grounding"},
                ]
            }
        },
        target_word_count=1200,
        attempt_index=5,
        source="chapter",
    )

    assert "Rule-grounding repair focus" in suffix
    assert "Rule-grounding hard rule" in suffix
    assert "very next move" in suffix



def test_should_resolve_targeted_final_repair_limits_for_near_target_candidate():
    max_tokens = resolve_targeted_final_repair_max_tokens(
        1200,
        current_word_count=1398,
    )
    char_limit = resolve_targeted_final_repair_char_limit(1200)
    temperature = resolve_targeted_final_repair_temperature(
        0.7,
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [
                    {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                    {"label": "Dialogue", "focus_area": "dialogue"},
                ]
            }
        },
    )

    assert max_tokens >= 650
    assert char_limit >= 1410
    assert 0.5 <= temperature <= 0.65


def test_should_add_continuity_constraints_to_retry_strategy_suffix():
    suffix = build_candidate_retry_strategy_suffix(
        quality_gate_plan={
            "active_story_repair_payload": {
                "summary": "Prioritize organization continuity handoff",
                "focus_areas": ["organization_continuity"],
                "repair_targets": ["Carry forward the organization continuity ledger through command, resource, or territory change: GrayNet curfew tightened"],
            }
        },
        quality_metrics={
            "quality_runtime_context": {
                "character_state_ledger": ["Lin: active in this chapter outline"],
                "organization_state_ledger": ["GrayNet: curfew tightened"],
            },
            "candidate_selection": {
                "word_count": 1493,
                "target_word_count": 1200,
            },
        },
        attempt_index=2,
        source="chapter",
    )

    assert "连续性要求" in suffix
    assert "GrayNet: curfew tightened" in suffix
    assert "跨章账本" in suffix


def test_should_add_continuity_constraints_to_word_budget_repair_suffix():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "quality_runtime_context": {
                "character_state_ledger": ["Lin: active in this chapter outline"],
                "organization_state_ledger": ["GrayNet: curfew tightened"],
            },
            "candidate_selection": {
                "word_count": 1759,
                "target_word_count": 1200,
            },
        },
        quality_gate_plan={
            "active_story_repair_payload": {
                "focus_areas": ["organization_continuity"],
                "repair_targets": ["Carry forward the organization continuity ledger through command, resource, or territory change: GrayNet curfew tightened"],
            }
        },
        target_word_count=1200,
        attempt_index=4,
        source="chapter",
    )

    assert "连续性硬约束" in suffix
    assert "GrayNet: curfew tightened" in suffix


def test_should_add_opening_focus_repair_lines_to_retry_suffix():
    suffix = build_candidate_retry_strategy_suffix(
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [{"label": "Opening hook", "focus_area": "opening"}],
            }
        },
        quality_metrics={
            "candidate_selection": {
                "word_count": 1380,
                "target_word_count": 1200,
            }
        },
        attempt_index=2,
        source="chapter",
    )

    assert "Opening repair" in suffix
    assert "120-180 Chinese chars" in suffix


def test_should_add_opening_hard_rule_to_word_budget_repair_suffix():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1759,
                "target_word_count": 1200,
            }
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [{"label": "Opening hook", "focus_area": "opening"}],
            }
        },
        target_word_count=1200,
        attempt_index=3,
        source="chapter",
    )

    assert "Opening hard rule" in suffix
    assert "first two paragraphs" in suffix


def test_should_add_outline_focus_repair_lines_to_retry_suffix():
    suffix = build_candidate_retry_strategy_suffix(
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [{"label": "Outline alignment", "focus_area": "outline"}],
            }
        },
        quality_metrics={
            "candidate_selection": {
                "word_count": 1422,
                "target_word_count": 1200,
            }
        },
        attempt_index=2,
        source="chapter",
    )

    assert "Outline repair" in suffix
    assert "opening anomaly" in suffix


def test_should_add_outline_hard_rule_to_word_budget_repair_suffix():
    suffix = build_word_budget_repair_suffix(
        quality_metrics={
            "candidate_selection": {
                "word_count": 1422,
                "target_word_count": 1200,
            }
        },
        quality_gate_plan={
            "quality_gate": {
                "failed_metrics": [{"label": "Outline alignment", "focus_area": "outline"}],
            }
        },
        target_word_count=1200,
        attempt_index=4,
        source="chapter",
    )

    assert "Outline hard rule" in suffix
    assert "mandatory outline beat" in suffix


def test_should_penalize_selection_score_for_overlong_candidate_and_continuity_warnings():
    baseline = build_candidate_selection_metadata(
        {"overall_score": 90.0, "pacing_score": 8.0},
        word_count=1240,
        target_word_count=1200,
        candidate_index=1,
        candidate_count=2,
        source="chapter",
    )
    penalized = build_candidate_selection_metadata(
        {
            "overall_score": 90.0,
            "pacing_score": 8.0,
            "continuity_preflight": {"warning_count": 2},
        },
        word_count=1905,
        target_word_count=1200,
        candidate_index=2,
        candidate_count=2,
        source="chapter",
    )

    assert penalized["out_of_window_penalty"] > 0
    assert penalized["continuity_warning_count"] == 2
    assert penalized["selection_score"] < baseline["selection_score"]
def test_should_build_candidate_pool_summary_with_winner_marker():
    candidates = [
        {
            "candidate_index": 1,
            "word_count": 1825,
            "overall_score": 90.4,
            "selection_score": 88.72,
            "generation_path": "single_pass",
            "attempt_kind": "initial_candidate",
            "quality_metrics": {
                "quality_gate": {
                    "decision": "auto_repair",
                    "status": "repairable",
                    "failed_metrics": [{"label": "Word budget"}],
                },
                "candidate_selection": {
                    "generation_path": "single_pass",
                    "attempt_kind": "initial_candidate",
                    "word_count": 1825,
                    "target_word_count": 1200,
                    "selection_score": 88.72,
                    "overall_score": 90.4,
                    "quality_gate_decision": "auto_repair",
                    "quality_gate_status": "repairable",
                },
            },
        },
        {
            "candidate_index": 3,
            "word_count": 1388,
            "overall_score": 89.1,
            "selection_score": 97.41,
            "generation_path": "word_budget_repair",
            "attempt_kind": "word_budget_repair",
            "quality_metrics": {
                "quality_gate": {
                    "decision": "allow_save",
                    "status": "pass",
                    "failed_metrics": [],
                },
                "candidate_selection": {
                    "generation_path": "word_budget_repair",
                    "attempt_kind": "word_budget_repair",
                    "word_count": 1388,
                    "target_word_count": 1200,
                    "selection_score": 97.41,
                    "overall_score": 89.1,
                    "quality_gate_decision": "allow_save",
                    "quality_gate_status": "pass",
                    "repair_seed_candidate_index": 1,
                    "repair_seed_generation_path": "single_pass",
                    "repair_seed_attempt_kind": "initial_candidate",
                },
            },
        },
    ]

    summary = build_candidate_pool_summary(
        candidates,
        winner_candidate_index=3,
        repair_seed_candidate_index=1,
    )

    assert len(summary) == 2
    assert summary[0]["candidate_index"] == 1
    assert summary[0]["failed_metrics"] == ["Word budget"]
    assert summary[0]["is_winner"] is False
    assert summary[0]["is_repair_seed"] is True
    assert summary[1]["candidate_index"] == 3
    assert summary[1]["generation_path"] == "word_budget_repair"
    assert summary[1]["quality_gate_decision"] == "allow_save"
    assert summary[1]["repair_seed_candidate_index"] == 1
    assert summary[1]["repair_seed_generation_path"] == "single_pass"
    assert summary[1]["repair_seed_attempt_kind"] == "initial_candidate"
    assert summary[1]["is_winner"] is True
    assert summary[1]["is_repair_seed"] is False
