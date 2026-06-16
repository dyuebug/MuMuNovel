// Rust owner for the rerank-heavy formula group originally mapped from
// Python chapter_candidate_rerank_service.py. Generation, finalize, repair,
// and default executor dependency owners consume these formulas directly.

use std::collections::HashSet;

use serde_json::{json, Map, Number, Value};

const STRUCTURAL_REPAIR_FOCUS_AREAS: &[&str] = &["conflict", "rule_grounding", "payoff"];
const CONTENT_SENSITIVE_REPAIR_FOCUS_AREAS: &[&str] = &[
    "conflict",
    "rule_grounding",
    "payoff",
    "cliffhanger",
    "dialogue",
    "outline",
    "opening",
];
const TARGETED_FINAL_REPAIR_FOCUS_AREAS: &[&str] = &[
    "cliffhanger",
    "dialogue",
    "outline",
    "rule_grounding",
    "conflict",
    "opening",
];

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CandidateSelectionMetadataInput {
    pub(crate) quality_metrics: Option<Value>,
    pub(crate) word_count: i64,
    pub(crate) target_word_count: i64,
    pub(crate) candidate_index: i64,
    pub(crate) candidate_count: i64,
    pub(crate) source: String,
    pub(crate) quality_gate_plan: Option<Value>,
    pub(crate) generation_path: Option<String>,
    pub(crate) attempt_kind: Option<String>,
    pub(crate) rerank_used: Option<bool>,
    pub(crate) word_budget_repair_used: Option<bool>,
    pub(crate) winner_candidate_index: Option<i64>,
    pub(crate) repair_seed_candidate_index: Option<i64>,
    pub(crate) repair_seed_generation_path: Option<String>,
    pub(crate) repair_seed_attempt_kind: Option<String>,
}

pub(crate) fn normalize_candidate_quality_gate_plan(
    quality_gate_plan: Value,
    word_count: i64,
    target_word_count: i64,
    quality_metrics: Value,
) -> Value {
    let mut normalized_plan = object_from_value(quality_gate_plan);
    let raw_gate = normalized_plan
        .get("quality_gate")
        .and_then(Value::as_object)
        .cloned()
        .or_else(|| {
            quality_metrics
                .as_object()
                .and_then(|metrics| metrics.get("quality_gate"))
                .and_then(Value::as_object)
                .cloned()
        });

    let normalized_gate = normalize_candidate_quality_gate(
        raw_gate.unwrap_or_default(),
        word_count,
        target_word_count,
    );
    if !normalized_gate.is_empty() {
        normalized_plan.insert("quality_gate".to_string(), Value::Object(normalized_gate));
    }
    Value::Object(normalized_plan)
}

pub(crate) fn resolve_word_budget_repair_char_limit(
    target_word_count: i64,
    relax_content_budget: bool,
) -> Option<i64> {
    let safe_target_word_count = target_word_count.max(200);
    let (_, target_upper_bound) = resolve_target_word_bounds(safe_target_word_count);
    let buffer_chars = if relax_content_budget {
        (safe_target_word_count * 6 / 100).clamp(40, 120)
    } else {
        (safe_target_word_count * 3 / 100).clamp(24, 48)
    };
    Some(target_upper_bound + buffer_chars)
}

pub(crate) fn resolve_word_budget_repair_max_tokens(
    target_word_count: i64,
    current_word_count: i64,
    relax_content_budget: bool,
) -> i64 {
    let safe_target_word_count = target_word_count.max(200);
    let (target_lower_bound, target_upper_bound) =
        resolve_target_word_bounds(safe_target_word_count);
    let current_word_count = current_word_count.max(0);
    let calculated_max_tokens = if current_word_count > target_upper_bound {
        if relax_content_budget {
            target_upper_bound * 48 / 100
        } else {
            target_upper_bound * 45 / 100
        }
    } else if current_word_count > 0 && current_word_count < target_lower_bound {
        target_upper_bound * 60 / 100
    } else {
        safe_target_word_count * 52 / 100
    };
    calculated_max_tokens.clamp(520, 7200)
}

pub(crate) fn should_relax_word_budget_repair_limits(quality_gate_plan: Option<Value>) -> bool {
    let (_, focus_areas) = extract_failed_metric_labels_and_focus_areas(quality_gate_plan.as_ref());
    focus_areas
        .iter()
        .any(|focus_area| CONTENT_SENSITIVE_REPAIR_FOCUS_AREAS.contains(&focus_area.as_str()))
}

pub(crate) fn build_candidate_retry_prompt_suffix(
    quality_gate_plan: Option<Value>,
    attempt_index: i64,
) -> Option<String> {
    let plan = quality_gate_plan.as_ref().and_then(Value::as_object)?;
    let quality_gate = plan.get("quality_gate").and_then(Value::as_object);
    let payload = active_story_repair_payload_object_from_plan(Some(plan));

    let summary = payload
        .and_then(|payload| safe_text(payload.get("summary")))
        .or_else(|| safe_text(plan.get("message")))
        .unwrap_or_default();
    let repair_targets =
        normalize_items(payload.and_then(|payload| payload.get("repair_targets")), 3);
    let preserve_strengths = normalize_items(
        payload.and_then(|payload| payload.get("preserve_strengths")),
        2,
    );
    let failed_metric_labels = quality_gate
        .map(|gate| {
            array_items(gate.get("failed_metrics"))
                .into_iter()
                .filter_map(|item| {
                    item.as_object()
                        .and_then(|item| safe_text(item.get("label")))
                })
                .take(3)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let recommended_action = quality_gate
        .and_then(|gate| {
            safe_text(
                gate.get("recommended_action_label")
                    .or_else(|| gate.get("recommended_action")),
            )
        })
        .unwrap_or_default();

    let mut lines = vec![
        format!("Revision attempt #{attempt_index}"),
        "- Keep the narrative voice, continuity, and established facts intact.".to_string(),
        "- Repair the weak spots identified by the quality gate before finalizing.".to_string(),
    ];
    push_line(&mut lines, "Focus summary", &summary);
    push_joined_line(&mut lines, "Failed metrics", &failed_metric_labels);
    push_joined_line(&mut lines, "Repair targets", &repair_targets);
    push_joined_line(&mut lines, "Preserve strengths", &preserve_strengths);
    push_line(&mut lines, "Recommended action", &recommended_action);
    Some(lines.join("\n"))
}

pub(crate) fn build_candidate_retry_strategy_suffix(
    quality_gate_plan: Option<Value>,
    quality_metrics: Option<Value>,
    attempt_index: i64,
    source: String,
) -> Option<String> {
    let runtime_context = extract_runtime_context(quality_metrics.as_ref());
    let candidate_selection = extract_candidate_selection(quality_metrics.as_ref());
    let (failed_metric_labels, failed_focus_areas) =
        extract_failed_metric_labels_and_focus_areas(quality_gate_plan.as_ref());
    let structural_focus_areas = failed_focus_areas
        .iter()
        .filter(|focus_area| STRUCTURAL_REPAIR_FOCUS_AREAS.contains(&focus_area.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    let mut lines = vec![
        format!("Alternative candidate strategy #{attempt_index}"),
        "- Recast the same chapter intent with a visibly different scene progression, not just local word swaps.".to_string(),
        format!(
            "- Keep the same target outcome for this {source} draft while varying scene sequencing and emphasis."
        ),
    ];
    if !failed_metric_labels.is_empty() {
        lines.push(format!(
            "- Counter the weak metrics through scene design: {}",
            failed_metric_labels
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(" / ")
        ));
    }
    lines.extend(build_focus_strategy_lines(&runtime_context));
    lines.extend(build_quality_gate_focus_repair_lines(
        &failed_focus_areas,
        false,
    ));
    lines.extend(build_structural_repair_lines(
        &structural_focus_areas,
        &runtime_context,
        false,
    ));
    if structural_focus_areas.contains(&"conflict".to_string())
        && structural_focus_areas.contains(&"rule_grounding".to_string())
    {
        lines.push("- Joint pressure repair: make the visible blocker come from an active rule, platform check, organization restriction, countdown, or resource constraint, so each push forward triggers immediate resistance and cost on-page.".to_string());
    }
    lines.extend(build_continuity_repair_lines(
        quality_gate_plan.as_ref(),
        &runtime_context,
        false,
    ));

    let current_word_count = map_i64(&candidate_selection, "word_count");
    let target_word_count = map_i64(&candidate_selection, "target_word_count")
        .max(map_i64(&runtime_context, "target_word_count"));
    if target_word_count > 0 {
        let (target_lower_bound, target_upper_bound) =
            resolve_target_word_bounds(target_word_count);
        if current_word_count > target_upper_bound {
            lines.push(format!(
                "- The previous draft ran long at about {current_word_count} chars; rewrite to stay within {target_lower_bound}-{target_upper_bound} Chinese characters."
            ));
            lines.push("- Compress by merging repeated beats, removing recap/exposition, and ending immediately once the hook lands.".to_string());
        } else if current_word_count > 0 && current_word_count < target_lower_bound {
            lines.push(format!(
                "- The previous draft landed short at about {current_word_count} chars; expand to roughly {target_lower_bound}-{target_upper_bound} Chinese characters through concrete action and consequence."
            ));
        }
    }
    Some(lines.join("\n"))
}

pub(crate) fn resolve_candidate_retry_temperature(
    base_temperature: f64,
    quality_metrics: Option<Value>,
    quality_gate_plan: Option<Value>,
    attempt_index: i64,
) -> Option<f64> {
    let runtime_context = extract_runtime_context(quality_metrics.as_ref());
    let candidate_selection = extract_candidate_selection(quality_metrics.as_ref());
    let quality_preset = map_text(&runtime_context, "quality_preset");
    let creative_mode = map_text(&runtime_context, "creative_mode");
    let decision = quality_gate_plan
        .as_ref()
        .and_then(|plan| plan.get("quality_gate"))
        .and_then(Value::as_object)
        .and_then(|gate| safe_text(gate.get("decision")))
        .unwrap_or_default();

    let mut temperature = if base_temperature.is_finite() && base_temperature != 0.0 {
        base_temperature
    } else {
        0.8
    };
    match quality_preset.as_str() {
        "clean_prose" => temperature -= 0.08,
        "immersive" | "emotion_drama" => temperature += 0.05,
        "plot_drive" => temperature += 0.02,
        _ => {}
    }
    match creative_mode.as_str() {
        "hook" | "suspense" | "relationship" | "emotion" => temperature += 0.04,
        "payoff" => temperature += 0.02,
        _ => {}
    }
    match decision.as_str() {
        "manual_review" => temperature += 0.03,
        "allow_save" => temperature -= 0.02,
        _ => {}
    }

    let current_word_count = map_i64(&candidate_selection, "word_count");
    let target_word_count = map_i64(&candidate_selection, "target_word_count")
        .max(map_i64(&runtime_context, "target_word_count"));
    if target_word_count > 0 {
        let (target_lower_bound, target_upper_bound) =
            resolve_target_word_bounds(target_word_count);
        if current_word_count > target_upper_bound {
            temperature -= 0.12;
        } else if current_word_count > 0 && current_word_count < target_lower_bound {
            temperature += 0.02;
        }
    }

    temperature -= (attempt_index - 2).max(0) as f64 * 0.05;
    Some(round_to(temperature.clamp(0.45, 1.05), 2))
}

pub(crate) fn build_candidate_selection_metadata(input: CandidateSelectionMetadataInput) -> Value {
    let metrics = object_from_option(input.quality_metrics);
    let existing_selection_metadata = metrics
        .get("candidate_selection")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut quality_gate = input
        .quality_gate_plan
        .as_ref()
        .and_then(|plan| plan.get("quality_gate"))
        .and_then(Value::as_object)
        .cloned()
        .or_else(|| {
            metrics
                .get("quality_gate")
                .and_then(Value::as_object)
                .cloned()
        })
        .unwrap_or_default();

    let decision =
        safe_text(quality_gate.get("decision")).unwrap_or_else(|| "allow_save".to_string());
    let status = safe_text(quality_gate.get("status")).unwrap_or_else(|| "pass".to_string());
    if !quality_gate.contains_key("decision") {
        quality_gate.insert("decision".to_string(), Value::String(decision.clone()));
    }

    let overall_score = value_to_f64(metrics.get("overall_score")).unwrap_or(0.0);
    let pacing_score = value_to_f64(metrics.get("pacing_score")).unwrap_or(0.0);
    let continuity_warning_count = metrics
        .get("continuity_preflight")
        .and_then(Value::as_object)
        .and_then(|preflight| preflight.get("warning_count"))
        .and_then(value_to_i64)
        .unwrap_or(0);
    let target_word_count = input.target_word_count.max(1);
    let word_count = input.word_count.max(0);
    let word_count_delta = (word_count - target_word_count).abs();
    let word_count_fit_ratio = (1.0 - word_count_delta as f64 / target_word_count as f64).max(0.0);
    let word_count_fit_score = round_to(word_count_fit_ratio * 100.0, 1);
    let (target_lower_bound, target_upper_bound) = resolve_target_word_bounds(target_word_count);
    let out_of_window_chars = if word_count > target_upper_bound {
        word_count - target_upper_bound
    } else if word_count > 0 && word_count < target_lower_bound {
        target_lower_bound - word_count
    } else {
        0
    };
    let out_of_window_penalty = round_to(
        out_of_window_chars as f64 / target_word_count as f64 * 24.0,
        2,
    );
    let decision_priority = quality_gate_decision_priority(&decision);
    let decision_bonus = match decision.as_str() {
        "allow_save" => 18.0,
        "auto_repair" => 4.0,
        "manual_review" => -18.0,
        _ => 0.0,
    };
    let selection_score = round_to(
        overall_score
            + decision_bonus
            + word_count_fit_score * 0.12
            + (pacing_score - 7.0).max(0.0) * 1.5
            - continuity_warning_count as f64 * 4.0
            - out_of_window_penalty,
        2,
    );

    let mut metadata = Map::new();
    insert_i64(&mut metadata, "candidate_index", input.candidate_index);
    insert_i64(&mut metadata, "candidate_count", input.candidate_count);
    insert_string(&mut metadata, "source", input.source);
    insert_f64(&mut metadata, "selection_score", selection_score);
    insert_f64(&mut metadata, "overall_score", round_to(overall_score, 1));
    insert_string(&mut metadata, "quality_gate_decision", decision);
    insert_string(&mut metadata, "quality_gate_status", status);
    insert_i64(&mut metadata, "quality_gate_priority", decision_priority);
    insert_i64(&mut metadata, "word_count", word_count);
    insert_i64(&mut metadata, "target_word_count", target_word_count);
    insert_f64(&mut metadata, "word_count_fit_score", word_count_fit_score);
    insert_i64(&mut metadata, "word_count_delta", word_count_delta);
    insert_f64(
        &mut metadata,
        "out_of_window_penalty",
        out_of_window_penalty,
    );
    insert_i64(
        &mut metadata,
        "continuity_warning_count",
        continuity_warning_count,
    );
    insert_optional_string(&mut metadata, "generation_path", input.generation_path);
    insert_optional_string(&mut metadata, "attempt_kind", input.attempt_kind);
    if let Some(rerank_used) = input.rerank_used {
        metadata.insert("rerank_used".to_string(), Value::Bool(rerank_used));
    }
    if let Some(word_budget_repair_used) = input.word_budget_repair_used {
        metadata.insert(
            "word_budget_repair_used".to_string(),
            Value::Bool(word_budget_repair_used),
        );
    }
    if let Some(winner_candidate_index) = input.winner_candidate_index {
        insert_i64(
            &mut metadata,
            "winner_candidate_index",
            winner_candidate_index.max(1),
        );
    }

    let repair_seed_candidate_index = input.repair_seed_candidate_index.or_else(|| {
        existing_selection_metadata
            .get("repair_seed_candidate_index")
            .and_then(value_to_i64)
            .map(|value| value.max(1))
    });
    let repair_seed_generation_path = input.repair_seed_generation_path.or_else(|| {
        existing_selection_metadata
            .get("repair_seed_generation_path")
            .and_then(|value| safe_text(Some(value)))
    });
    let repair_seed_attempt_kind = input.repair_seed_attempt_kind.or_else(|| {
        existing_selection_metadata
            .get("repair_seed_attempt_kind")
            .and_then(|value| safe_text(Some(value)))
    });
    if let Some(value) = repair_seed_candidate_index {
        insert_i64(&mut metadata, "repair_seed_candidate_index", value.max(1));
    }
    insert_optional_string(
        &mut metadata,
        "repair_seed_generation_path",
        repair_seed_generation_path,
    );
    insert_optional_string(
        &mut metadata,
        "repair_seed_attempt_kind",
        repair_seed_attempt_kind,
    );
    Value::Object(metadata)
}

pub(crate) fn attach_candidate_selection_metadata(
    quality_metrics: Value,
    selection_metadata: Value,
) -> Value {
    let mut metrics = object_from_value(quality_metrics);
    metrics.insert(
        "candidate_selection".to_string(),
        Value::Object(object_from_value(selection_metadata)),
    );
    Value::Object(metrics)
}

pub(crate) fn build_candidate_pool_summary(
    candidates: Vec<Value>,
    winner_candidate_index: Option<i64>,
    repair_seed_candidate_index: Option<i64>,
) -> Value {
    let winner_candidate_index = winner_candidate_index.unwrap_or(0).max(0);
    let repair_seed_candidate_index = repair_seed_candidate_index.unwrap_or(0).max(0);
    let mut summary = candidates
        .into_iter()
        .filter_map(|candidate| {
            let candidate = candidate.as_object()?.clone();
            let metrics = candidate
                .get("quality_metrics")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let selection = metrics
                .get("candidate_selection")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let gate = metrics
                .get("quality_gate")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let failed_metrics = array_items(gate.get("failed_metrics"))
                .into_iter()
                .filter_map(|item| {
                    item.as_object()
                        .and_then(|item| safe_text(item.get("label").or_else(|| item.get("key"))))
                        .or_else(|| safe_text(Some(item)))
                })
                .map(Value::String)
                .collect::<Vec<_>>();
            let candidate_index = map_i64(&candidate, "candidate_index").max(0);
            Some(json!({
                "candidate_index": candidate_index,
                "generation_path": map_text(&selection, "generation_path").if_empty(map_text(&candidate, "generation_path")),
                "attempt_kind": map_text(&selection, "attempt_kind").if_empty(map_text(&candidate, "attempt_kind")),
                "quality_gate_decision": map_text(&selection, "quality_gate_decision").if_empty(map_text(&gate, "decision")),
                "quality_gate_status": map_text(&selection, "quality_gate_status").if_empty(map_text(&gate, "status")),
                "word_count": map_i64(&selection, "word_count").max(map_i64(&candidate, "word_count")).max(0),
                "target_word_count": map_i64(&selection, "target_word_count").max(0),
                "overall_score": round_to(map_f64(&selection, "overall_score").max(map_f64(&candidate, "overall_score")), 1),
                "selection_score": round_to(map_f64(&selection, "selection_score").max(map_f64(&candidate, "selection_score")), 2),
                "repair_seed_candidate_index": map_i64(&selection, "repair_seed_candidate_index").max(0),
                "repair_seed_generation_path": map_text(&selection, "repair_seed_generation_path"),
                "repair_seed_attempt_kind": map_text(&selection, "repair_seed_attempt_kind"),
                "failed_metrics": failed_metrics,
                "is_winner": candidate_index == winner_candidate_index,
                "is_repair_seed": candidate_index == repair_seed_candidate_index,
            }))
        })
        .collect::<Vec<_>>();
    summary.sort_by_key(|item| {
        item.get("candidate_index")
            .and_then(value_to_i64)
            .unwrap_or(0)
    });
    Value::Array(summary)
}

pub(crate) fn select_best_generation_candidate(candidates: Vec<Value>) -> Option<Value> {
    let mut normalized = candidates
        .into_iter()
        .filter_map(|candidate| candidate.as_object().cloned().map(Value::Object))
        .collect::<Vec<_>>();
    let pool_size = normalized.len() as i64;
    normalized.sort_by(|left, right| {
        candidate_rank_key(right)
            .partial_cmp(&candidate_rank_key(left))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut winner = object_from_value(normalized.into_iter().next()?);
    insert_i64(&mut winner, "rerank_pool_size", pool_size.max(1));
    Some(Value::Object(winner))
}

pub(crate) fn should_apply_word_budget_repair(candidate: Value) -> bool {
    let Some(candidate) = candidate.as_object() else {
        return false;
    };
    let target_word_count = map_i64(candidate, "target_word_count");
    let current_word_count = map_i64(candidate, "word_count");
    if target_word_count <= 0 || current_word_count <= 0 {
        return false;
    }
    let (target_lower_bound, target_upper_bound) = resolve_target_word_bounds(target_word_count);
    let severe_upper_bound = (target_upper_bound + 120).max(target_upper_bound * 110 / 100);
    let severe_lower_bound = 200.max((target_lower_bound - 120).min(target_lower_bound * 90 / 100));
    current_word_count > severe_upper_bound
        || (current_word_count > 0 && current_word_count < severe_lower_bound)
}

pub(crate) fn should_prefer_word_budget_repair_candidate(
    selected_candidate: Value,
    repair_candidate: Value,
) -> bool {
    let Some(repair) = repair_candidate.as_object() else {
        return false;
    };
    let Some(selected) = selected_candidate.as_object() else {
        return true;
    };
    let target_word_count =
        map_i64(selected, "target_word_count").max(map_i64(repair, "target_word_count"));
    if target_word_count <= 0 {
        return false;
    }
    let selected_word_count = map_i64(selected, "word_count");
    let repair_word_count = map_i64(repair, "word_count");
    let selected_delta = (selected_word_count - target_word_count).abs();
    let repair_delta = (repair_word_count - target_word_count).abs();
    if repair_delta >= selected_delta {
        return false;
    }

    let selected_priority =
        quality_gate_decision_priority(&map_text(selected, "quality_gate_decision"));
    let repair_priority =
        quality_gate_decision_priority(&map_text(repair, "quality_gate_decision"));
    let selected_in_window = is_candidate_word_count_in_target_window(selected);
    let repair_in_window = is_candidate_word_count_in_target_window(repair);
    let (_, target_upper_bound) = resolve_target_word_bounds(target_word_count);
    let severe_upper_bound = (target_upper_bound + 120).max(target_upper_bound * 110 / 100);
    let repair_soft_upper_bound = target_upper_bound + 80.max(target_word_count * 6 / 100);
    let quality_drop =
        (map_f64(selected, "overall_score") - map_f64(repair, "overall_score")).max(0.0);
    let selected_failed_count = extract_failed_metric_count(selected);
    let repair_failed_count = extract_failed_metric_count(repair);
    let delta_improvement = selected_delta - repair_delta;
    let substantial_improvement = delta_improvement >= 120.max(target_word_count * 10 / 100);
    let decisive_improvement = delta_improvement >= 240.max(target_word_count * 20 / 100);

    if repair_in_window && !selected_in_window {
        return quality_drop <= 8.0;
    }
    if selected_word_count > severe_upper_bound
        && repair_word_count > 0
        && repair_word_count <= repair_soft_upper_bound
        && substantial_improvement
    {
        return if repair_failed_count <= selected_failed_count {
            quality_drop <= 14.0
        } else {
            quality_drop <= 8.0
        };
    }
    if repair_priority < selected_priority {
        return false;
    }
    if repair_priority > selected_priority {
        return quality_drop <= 8.0;
    }
    if should_apply_word_budget_repair(Value::Object(selected.clone())) && substantial_improvement {
        return quality_drop <= 6.0;
    }
    decisive_improvement && quality_drop <= 3.5
}

pub(crate) fn should_keep_word_budget_repair_candidate(
    selected_candidate: Value,
    repair_candidate: Value,
) -> bool {
    let Some(repair) = repair_candidate.as_object() else {
        return false;
    };
    let Some(selected) = selected_candidate.as_object() else {
        return true;
    };
    if should_prefer_word_budget_repair_candidate(
        Value::Object(selected.clone()),
        Value::Object(repair.clone()),
    ) {
        return true;
    }
    let target_word_count =
        map_i64(selected, "target_word_count").max(map_i64(repair, "target_word_count"));
    if target_word_count <= 0 {
        return true;
    }
    let selected_word_count = map_i64(selected, "word_count");
    let repair_word_count = map_i64(repair, "word_count");
    let (target_lower_bound, target_upper_bound) = resolve_target_word_bounds(target_word_count);
    let repair_hard_lower_bound = (target_lower_bound - 60.max(target_word_count * 5 / 100))
        .max(200)
        .max(
            selected_word_count
                .min(target_upper_bound)
                .max(target_lower_bound)
                * 72
                / 100,
        );
    if repair_word_count > 0 && repair_word_count < repair_hard_lower_bound {
        return false;
    }
    let selected_failed_count = extract_failed_metric_count(selected);
    let repair_failed_count = extract_failed_metric_count(repair);
    let selected_score = map_f64(selected, "overall_score");
    let repair_score = map_f64(repair, "overall_score");
    if repair_failed_count > selected_failed_count + 1 && repair_score + 10.0 < selected_score {
        return false;
    }
    if selected_failed_count <= 1
        && repair_failed_count >= selected_failed_count + 2
        && repair_score + 10.0 < selected_score
    {
        return false;
    }
    repair_score + 24.0 >= selected_score
}

pub(crate) fn resolve_targeted_final_repair_char_limit(target_word_count: i64) -> Option<i64> {
    let safe_target_word_count = target_word_count.max(200);
    let (_, target_upper_bound) = resolve_target_word_bounds(safe_target_word_count);
    Some(target_upper_bound + (safe_target_word_count * 7 / 100).clamp(80, 140))
}

pub(crate) fn resolve_targeted_final_repair_max_tokens(
    target_word_count: i64,
    current_word_count: i64,
) -> i64 {
    let safe_target_word_count = target_word_count.max(200);
    let (_, target_upper_bound) = resolve_target_word_bounds(safe_target_word_count);
    let base_limit = target_upper_bound.max(current_word_count.max(0));
    (base_limit * 50 / 100).clamp(520, 6400)
}

pub(crate) fn should_apply_targeted_final_repair(candidate: Value) -> bool {
    candidate
        .as_object()
        .map(|candidate| can_seed_targeted_final_repair_candidate(candidate, false))
        .unwrap_or(false)
}

pub(crate) fn should_apply_followup_targeted_final_repair(candidate: Value) -> bool {
    let Some(candidate) = candidate.as_object() else {
        return false;
    };
    if map_text(candidate, "attempt_kind") != "targeted_quality_repair" {
        return false;
    }
    let gate = extract_quality_gate_payload(candidate);
    let overall_score = map_f64(candidate, "overall_score").max(map_f64(&gate, "overall_score"));
    let focus_areas = extract_failed_focus_areas_from_candidate(candidate);
    let failed_count = extract_failed_metric_count(candidate);
    can_seed_targeted_final_repair_candidate(candidate, false)
        && (is_rule_grounding_only_final_polish_candidate(
            candidate,
            overall_score,
            &focus_areas,
            failed_count,
            false,
        ) || is_opening_rule_grounding_final_polish_candidate(
            candidate,
            overall_score,
            &focus_areas,
            failed_count,
            false,
        ) || is_opening_rule_grounding_cliffhanger_final_polish_candidate(
            candidate,
            overall_score,
            &focus_areas,
            failed_count,
            false,
        ) || is_dialogue_cliffhanger_final_polish_candidate(
            candidate,
            overall_score,
            &focus_areas,
            failed_count,
            false,
        ) || is_cliffhanger_only_final_polish_candidate(
            candidate,
            overall_score,
            &focus_areas,
            failed_count,
            false,
        ) || is_rule_grounding_cliffhanger_final_polish_candidate(
            candidate,
            overall_score,
            &focus_areas,
            failed_count,
            false,
        ) || is_opening_conflict_cliffhanger_final_polish_candidate(
            candidate,
            overall_score,
            &focus_areas,
            failed_count,
            false,
        ))
}

pub(crate) fn build_targeted_final_repair_suffix(
    quality_metrics: Option<Value>,
    quality_gate_plan: Option<Value>,
    target_word_count: i64,
    attempt_index: i64,
    source: String,
) -> Option<String> {
    if target_word_count <= 0 {
        return None;
    }
    let candidate_selection = extract_candidate_selection(quality_metrics.as_ref());
    let (failed_metric_labels, failed_focus_areas) =
        extract_failed_metric_labels_and_focus_areas(quality_gate_plan.as_ref());
    let focus_set = focus_set_for(&failed_focus_areas, TARGETED_FINAL_REPAIR_FOCUS_AREAS);
    if focus_set.is_empty() {
        return None;
    }
    let current_word_count = map_i64(&candidate_selection, "word_count");
    let (target_lower_bound, target_upper_bound) = resolve_target_word_bounds(target_word_count);
    let polish_upper_bound = target_upper_bound + (target_word_count * 7 / 100).clamp(80, 140);
    let mut lines = vec![
        format!("Targeted quality repair pass #{attempt_index}"),
        format!("- Rewrite the same {source} draft into {target_lower_bound}-{polish_upper_bound} Chinese characters; stay close to the current length while fixing only the weak quality gaps."),
        "- Preserve the same scene order, revealed facts, character decisions, and already-landed rule payoffs.".to_string(),
        "- Do not reopen the whole chapter; keep the opening and middle beats stable, and spend most revisions on the final 2-4 paragraphs plus any weak dialogue exchange.".to_string(),
        "- Replace soft summary, afterthought explanation, and reflective cooldown lines with concrete signal, interruption, pressure, or decision fallout.".to_string(),
    ];
    if current_word_count > 0 {
        lines.push(format!(
            "- The current draft is about {current_word_count} chars; keep the revision tight and avoid regrowing the chapter."
        ));
    }
    push_joined_line(
        &mut lines,
        "Repair these weak metrics without changing the chapter mission",
        &failed_metric_labels
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>(),
    );
    add_targeted_focus_lines(&mut lines, &focus_set);
    Some(lines.join("\n"))
}

pub(crate) fn resolve_targeted_final_repair_temperature(
    base_temperature: f64,
    quality_gate_plan: Option<Value>,
) -> f64 {
    let (_, failed_focus_areas) =
        extract_failed_metric_labels_and_focus_areas(quality_gate_plan.as_ref());
    let focus_set = focus_set_for(&failed_focus_areas, TARGETED_FINAL_REPAIR_FOCUS_AREAS);
    let mut temperature = safe_float(base_temperature).unwrap_or(0.8).min(0.62);
    if focus_set.contains("dialogue") {
        temperature += 0.02;
    }
    if focus_set.contains("cliffhanger") {
        temperature += 0.01;
    }
    round_to(temperature.clamp(0.5, 0.65), 2)
}

pub(crate) fn should_prefer_targeted_final_repair_candidate(
    selected_candidate: Value,
    repair_candidate: Value,
) -> bool {
    let Some(repair) = repair_candidate.as_object() else {
        return false;
    };
    let Some(selected) = selected_candidate.as_object() else {
        return true;
    };
    let selected_priority =
        quality_gate_decision_priority(&map_text(selected, "quality_gate_decision"));
    let repair_priority =
        quality_gate_decision_priority(&map_text(repair, "quality_gate_decision"));
    let target_word_count =
        map_i64(selected, "target_word_count").max(map_i64(repair, "target_word_count"));
    let selected_word_count = map_i64(selected, "word_count");
    let repair_word_count = map_i64(repair, "word_count");
    let quality_drop =
        (map_f64(selected, "overall_score") - map_f64(repair, "overall_score")).max(0.0);
    let selected_failed_count = extract_failed_metric_count(selected);
    let repair_failed_count = extract_failed_metric_count(repair);
    let selected_in_window = is_candidate_word_count_in_target_window(selected);
    let repair_in_window = is_candidate_word_count_in_target_window(repair);
    let selected_delta = (selected_word_count - target_word_count).abs();
    let repair_delta = (repair_word_count - target_word_count).abs();
    let substantial_improvement = target_word_count > 0
        && selected_delta - repair_delta >= 120.max(target_word_count * 10 / 100);
    let (_, target_upper_bound) = if target_word_count > 0 {
        resolve_target_word_bounds(target_word_count)
    } else {
        (0, 0)
    };
    let severe_upper_bound = if target_upper_bound > 0 {
        (target_upper_bound + 120).max(target_upper_bound * 110 / 100)
    } else {
        0
    };
    let repair_soft_upper_bound = if target_upper_bound > 0 {
        target_upper_bound + 100.max(target_word_count * 8 / 100)
    } else {
        0
    };

    if repair_priority > selected_priority {
        return quality_drop <= 6.0;
    }
    if repair_in_window && !selected_in_window {
        return quality_drop <= 6.0;
    }
    if severe_upper_bound > 0
        && selected_word_count > severe_upper_bound
        && repair_word_count > 0
        && repair_word_count <= repair_soft_upper_bound
        && substantial_improvement
    {
        return quality_drop <= 6.0;
    }
    if repair_priority < selected_priority {
        return false;
    }
    if repair_failed_count < selected_failed_count {
        return quality_drop <= 4.5;
    }

    let selected_focus = extract_failed_focus_areas_from_candidate(selected);
    let repair_focus = extract_failed_focus_areas_from_candidate(repair);
    let same_focus_profile = !repair_focus.is_empty() && selected_focus == repair_focus;
    let selected_near_target_ceiling = repair_soft_upper_bound > 0
        && selected_word_count > 0
        && selected_word_count <= repair_soft_upper_bound;
    let repair_near_target_ceiling = repair_soft_upper_bound > 0
        && repair_word_count > 0
        && repair_word_count <= repair_soft_upper_bound;
    repair_failed_count == selected_failed_count
        && selected_near_target_ceiling
        && repair_near_target_ceiling
        && same_focus_profile
        && repair_focus.iter().any(|focus| focus == "cliffhanger")
        && map_f64(repair, "overall_score") >= map_f64(selected, "overall_score") + 1.5
        && repair_delta <= selected_delta + 40
}

pub(crate) fn should_adopt_targeted_final_repair_candidate(
    seed_candidate: Value,
    repair_candidate: Value,
) -> bool {
    let Some(repair) = repair_candidate.as_object() else {
        return false;
    };
    let Some(seed) = seed_candidate.as_object() else {
        return true;
    };
    let seed_failed_count = extract_failed_metric_count(seed);
    let repair_failed_count = extract_failed_metric_count(repair);
    if repair_failed_count > seed_failed_count {
        return false;
    }
    let seed_score = map_f64(seed, "overall_score");
    let repair_score = map_f64(repair, "overall_score");
    let target_word_count =
        map_i64(seed, "target_word_count").max(map_i64(repair, "target_word_count"));
    let seed_delta = (map_i64(seed, "word_count") - target_word_count).abs();
    let repair_delta = (map_i64(repair, "word_count") - target_word_count).abs();
    let seed_in_window = is_candidate_word_count_in_target_window(seed);
    let repair_in_window = is_candidate_word_count_in_target_window(repair);
    if seed_in_window && !repair_in_window && repair_failed_count >= seed_failed_count {
        return false;
    }
    if repair_failed_count == seed_failed_count {
        if repair_delta > seed_delta {
            return false;
        }
        if repair_score + 0.3 < seed_score {
            return false;
        }
    } else if repair_score + 4.0 < seed_score && repair_delta >= seed_delta {
        return false;
    }
    true
}

pub(crate) fn should_keep_targeted_final_repair_candidate(
    seed_candidate: Value,
    repair_candidate: Value,
) -> bool {
    let Some(repair) = repair_candidate.as_object() else {
        return false;
    };
    let Some(seed) = seed_candidate.as_object() else {
        return true;
    };
    let target_word_count =
        map_i64(seed, "target_word_count").max(map_i64(repair, "target_word_count"));
    if target_word_count <= 0 {
        return true;
    }
    let (target_lower_bound, target_upper_bound) = resolve_target_word_bounds(target_word_count);
    let repair_hard_lower_bound = (target_lower_bound - 60.max(target_word_count * 5 / 100))
        .max(200)
        .max(
            map_i64(seed, "word_count")
                .min(target_upper_bound)
                .max(target_lower_bound)
                * 72
                / 100,
        );
    let repair_word_count = map_i64(repair, "word_count");
    if repair_word_count > 0 && repair_word_count < repair_hard_lower_bound {
        return false;
    }
    let seed_failed_count = extract_failed_metric_count(seed);
    let repair_failed_count = extract_failed_metric_count(repair);
    if repair_failed_count > seed_failed_count + 1 {
        return false;
    }
    let seed_score = map_f64(seed, "overall_score");
    let repair_score = map_f64(repair, "overall_score");
    !(repair_score + 10.0 < seed_score && repair_failed_count >= seed_failed_count)
}

pub(crate) fn select_targeted_final_repair_seed_candidate(
    selected_candidate: Value,
    candidates: Vec<Value>,
) -> Option<Value> {
    let selected = selected_candidate.as_object();
    let mut eligible = Vec::<Value>::new();
    if let Some(selected) = selected {
        if can_seed_targeted_final_repair_candidate(selected, false)
            || can_seed_targeted_final_repair_candidate(selected, true)
        {
            eligible.push(Value::Object(selected.clone()));
        }
    }
    let selected_index = selected
        .map(|selected| map_i64(selected, "candidate_index"))
        .unwrap_or(0);
    for candidate in candidates {
        let Some(candidate_map) = candidate.as_object() else {
            continue;
        };
        if selected_index > 0 && map_i64(candidate_map, "candidate_index") == selected_index {
            continue;
        }
        if can_seed_targeted_final_repair_candidate(candidate_map, true) {
            eligible.push(Value::Object(candidate_map.clone()));
        }
    }
    eligible.sort_by(|left, right| {
        targeted_seed_rank_key(right)
            .partial_cmp(&targeted_seed_rank_key(left))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    eligible.into_iter().next().or_else(|| {
        selected_candidate
            .as_object()
            .filter(|candidate| can_fallback_seed_targeted_final_repair_candidate(candidate))
            .cloned()
            .map(Value::Object)
    })
}

pub(crate) fn build_word_budget_repair_suffix(
    quality_metrics: Option<Value>,
    quality_gate_plan: Option<Value>,
    current_content: Option<String>,
    target_word_count: i64,
    attempt_index: i64,
    source: String,
) -> Option<String> {
    if target_word_count <= 0 {
        return None;
    }
    let runtime_context = extract_runtime_context(quality_metrics.as_ref());
    let candidate_selection = extract_candidate_selection(quality_metrics.as_ref());
    let (failed_metric_labels, failed_focus_areas) =
        extract_failed_metric_labels_and_focus_areas(quality_gate_plan.as_ref());
    let structural_focus_areas = failed_focus_areas
        .iter()
        .filter(|focus_area| STRUCTURAL_REPAIR_FOCUS_AREAS.contains(&focus_area.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let current_word_count = map_i64(&candidate_selection, "word_count");
    let (target_lower_bound, target_upper_bound) = resolve_target_word_bounds(target_word_count);
    let (opening_anchor, closing_anchor) =
        extract_edge_anchors(current_content.as_deref().unwrap_or(""));
    let mut lines = vec![
        format!("Word-budget repair pass #{attempt_index}"),
        format!("- Rewrite the same {source} draft from scratch into {target_lower_bound}-{target_upper_bound} Chinese characters; do not exceed {target_upper_bound}."),
        "- Preserve the same POV, continuity, established facts, and chapter mission.".to_string(),
        "- Protect the first-paragraph incident hook and the final unresolved hook; cut the middle before weakening either edge.".to_string(),
        "- Keep only the beats that directly advance conflict, rule payoff, outline progression, and the closing hook.".to_string(),
        "- Remove recap, repeated explanation, and side detours; convert exposition into action, dialogue, and consequence.".to_string(),
        "- Hard constraint: output continuous in-scene chapter prose only; no title, outline bullets, bracket notes, or meta commentary.".to_string(),
    ];
    push_line(
        &mut lines,
        "Preserve this opening anchor beat in equivalent dramatic form",
        &opening_anchor,
    );
    push_line(
        &mut lines,
        "Preserve this closing hook beat in equivalent dramatic form",
        &closing_anchor,
    );
    if current_word_count > target_upper_bound {
        lines.push(format!(
            "- The previous draft ran to about {current_word_count} chars; compress structure aggressively and merge overlapping beats."
        ));
    } else if current_word_count > 0 && current_word_count < target_lower_bound {
        lines.push(format!(
            "- The previous draft landed short at about {current_word_count} chars; expand with concrete action, consequence, and one stronger closing turn while staying inside the target range."
        ));
    }
    push_joined_line(
        &mut lines,
        "Repair the weak metrics while compressing",
        &failed_metric_labels
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>(),
    );
    lines.extend(build_quality_gate_focus_repair_lines(
        &failed_focus_areas,
        true,
    ));
    lines.extend(build_structural_repair_lines(
        &structural_focus_areas,
        &runtime_context,
        true,
    ));
    lines.extend(build_continuity_repair_lines(
        quality_gate_plan.as_ref(),
        &runtime_context,
        true,
    ));
    lines.extend(build_focus_strategy_lines(&runtime_context));
    Some(lines.join("\n"))
}

pub(crate) fn resolve_word_budget_repair_temperature(
    base_temperature: f64,
    quality_metrics: Option<Value>,
) -> f64 {
    let runtime_context = extract_runtime_context(quality_metrics.as_ref());
    let quality_preset = map_text(&runtime_context, "quality_preset");
    let creative_mode = map_text(&runtime_context, "creative_mode");
    let mut temperature = safe_float(base_temperature).unwrap_or(0.8).min(0.62);
    if quality_preset == "plot_drive" {
        temperature -= 0.06;
    } else if quality_preset == "clean_prose" {
        temperature -= 0.08;
    }
    if matches!(creative_mode.as_str(), "hook" | "suspense" | "payoff") {
        temperature -= 0.04;
    }
    round_to(temperature.clamp(0.42, 0.62), 2)
}

pub(crate) fn should_generate_additional_candidate(
    candidate: Value,
    produced_candidates: usize,
    max_candidates: i64,
) -> bool {
    if produced_candidates >= max_candidates.max(1) as usize {
        return false;
    }
    let Some(candidate) = candidate.as_object() else {
        return false;
    };
    let has_pressure = candidate_has_explicit_quality_repair_pressure(candidate);
    let decision = map_text(candidate, "quality_gate_decision");
    if decision == "auto_repair" {
        return has_pressure;
    }
    let target_word_count = map_i64(candidate, "target_word_count");
    let current_word_count = map_i64(candidate, "word_count");
    if target_word_count > 0 {
        let (target_lower_bound, target_upper_bound) =
            resolve_target_word_bounds(target_word_count);
        if current_word_count > target_upper_bound
            || (current_word_count > 0 && current_word_count < target_lower_bound)
        {
            return has_pressure;
        }
    }
    false
}

fn normalize_candidate_quality_gate(
    mut gate: Map<String, Value>,
    word_count: i64,
    target_word_count: i64,
) -> Map<String, Value> {
    let decision = safe_text(gate.get("decision"))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "allow_save".to_string());
    let (pressure, reason) = resolve_severe_word_budget_pressure(word_count, target_word_count);
    if pressure && decision == "allow_save" {
        insert_string(&mut gate, "decision", "auto_repair".to_string());
        insert_string(&mut gate, "status", "repairable".to_string());
        if safe_text(gate.get("label")).unwrap_or_default().is_empty() {
            insert_string(&mut gate, "label", "Needs repair".to_string());
        }
        if safe_text(gate.get("reason")).unwrap_or_default().is_empty() {
            insert_string(&mut gate, "reason", reason);
        }
        if safe_text(gate.get("summary"))
            .unwrap_or_default()
            .is_empty()
        {
            insert_string(
                &mut gate,
                "summary",
                "The draft still needs a targeted revision before it should be saved.".to_string(),
            );
        }
        gate.insert("allow_save".to_string(), Value::Bool(false));
        gate.insert("can_auto_repair".to_string(), Value::Bool(true));
        gate.insert("requires_manual_review".to_string(), Value::Bool(false));
    }
    gate
}

fn active_story_repair_payload_object_from_plan<'a>(
    plan: Option<&'a Map<String, Value>>,
) -> Option<&'a Map<String, Value>> {
    plan.and_then(|plan| {
        plan.get("active_story_repair_payload")
            .and_then(Value::as_object)
    })
}

fn resolve_target_word_bounds(target_word_count: i64) -> (i64, i64) {
    let safe_target_word_count = target_word_count.max(200);
    let lower_bound = (safe_target_word_count - 120)
        .min(safe_target_word_count * 90 / 100)
        .max(200);
    let upper_bound = (safe_target_word_count + 150)
        .min(safe_target_word_count * 115 / 100)
        .max(lower_bound + 80);
    (lower_bound, upper_bound)
}

fn resolve_severe_word_budget_pressure(word_count: i64, target_word_count: i64) -> (bool, String) {
    let target_word_count = target_word_count.max(0);
    let word_count = word_count.max(0);
    if target_word_count <= 0 || word_count <= 0 {
        return (false, String::new());
    }
    let (lower_bound, upper_bound) = resolve_target_word_bounds(target_word_count);
    let severe_upper_bound = (upper_bound + 120).max(upper_bound * 110 / 100);
    let severe_lower_bound = 200.max((lower_bound - 120).min(lower_bound * 90 / 100));
    let pressure =
        word_count > severe_upper_bound || (word_count > 0 && word_count < severe_lower_bound);
    if !pressure {
        return (false, String::new());
    }
    (
        true,
        format!(
            "Word count deviates too far from the target window (current {word_count}, target {target_word_count}, ideal range {lower_bound}-{upper_bound})."
        ),
    )
}

fn extract_failed_metric_labels_and_focus_areas(
    plan: Option<&Value>,
) -> (Vec<String>, Vec<String>) {
    let Some(plan) = plan.and_then(Value::as_object) else {
        return (Vec::new(), Vec::new());
    };
    let mut labels = Vec::<String>::new();
    let mut focus_areas = Vec::<String>::new();

    if let Some(gate) = plan.get("quality_gate").and_then(Value::as_object) {
        for item in array_items(gate.get("failed_metrics")) {
            let Some(item) = item.as_object() else {
                continue;
            };
            if let Some(label) = safe_text(item.get("label")) {
                if !labels.contains(&label) {
                    labels.push(label);
                }
            }
            if let Some(focus_area) = normalize_focus_area(
                item.get("focus_area")
                    .or_else(|| item.get("key"))
                    .or_else(|| item.get("label")),
            ) {
                if !focus_areas.contains(&focus_area) {
                    focus_areas.push(focus_area);
                }
            }
            if labels.len() >= 4 && focus_areas.len() >= 3 {
                break;
            }
        }
    }
    if let Some(payload) = active_story_repair_payload_object_from_plan(Some(plan)) {
        for item in normalize_items(payload.get("focus_areas"), 4) {
            if let Some(focus_area) = normalize_focus_area(Some(&Value::String(item))) {
                if !focus_areas.contains(&focus_area) {
                    focus_areas.push(focus_area);
                }
            }
        }
    }
    (
        labels.into_iter().take(4).collect(),
        focus_areas.into_iter().take(4).collect(),
    )
}

fn infer_focus_area_from_text(value: &str) -> Option<String> {
    let normalized = value.to_lowercase();
    let hints = [
        (
            "conflict",
            ["conflict", "冲突", "对抗", "受阻", "阻力", "升级", "张力"].as_slice(),
        ),
        (
            "rule_grounding",
            [
                "rule", "ground", "规则", "设定", "限制", "约束", "机制", "法则",
            ]
            .as_slice(),
        ),
        (
            "payoff",
            ["payoff", "兑现", "回收", "伏笔", "承诺", "反馈", "闭环"].as_slice(),
        ),
    ];
    hints.iter().find_map(|(focus, values)| {
        values
            .iter()
            .any(|hint| normalized.contains(hint))
            .then(|| focus.to_string())
    })
}

fn normalize_focus_area(raw_value: Option<&Value>) -> Option<String> {
    let mut normalized = raw_value
        .and_then(|value| safe_text(Some(value)))
        .map(|value| value.to_lowercase())?;
    if normalized.is_empty() {
        return None;
    }
    if !STRUCTURAL_REPAIR_FOCUS_AREAS.contains(&normalized.as_str()) {
        normalized = infer_focus_area_from_text(&normalized).unwrap_or(normalized);
    }
    (!normalized.is_empty()).then_some(normalized)
}

fn extract_runtime_context(metrics: Option<&Value>) -> Map<String, Value> {
    metrics
        .and_then(Value::as_object)
        .and_then(|metrics| metrics.get("quality_runtime_context"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn extract_candidate_selection(metrics: Option<&Value>) -> Map<String, Value> {
    metrics
        .and_then(Value::as_object)
        .and_then(|metrics| metrics.get("candidate_selection"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn candidate_has_explicit_quality_repair_pressure(candidate: &Map<String, Value>) -> bool {
    let Some(plan) = candidate
        .get("quality_gate_plan")
        .and_then(Value::as_object)
    else {
        return false;
    };
    if let Some(gate) = plan.get("quality_gate").and_then(Value::as_object) {
        for item in array_items(gate.get("failed_metrics")) {
            let Some(item) = item.as_object() else {
                continue;
            };
            if safe_text(item.get("label")).is_some()
                || safe_text(item.get("key")).is_some()
                || safe_text(item.get("focus_area")).is_some()
            {
                return true;
            }
        }
    }
    if let Some(payload) = active_story_repair_payload_object_from_plan(Some(plan)) {
        if safe_text(payload.get("summary")).is_some()
            || !normalize_items(payload.get("repair_targets"), 1).is_empty()
            || !normalize_items(payload.get("focus_areas"), 1).is_empty()
        {
            return true;
        }
    }
    false
}

fn is_candidate_word_count_in_target_window(candidate: &Map<String, Value>) -> bool {
    let target_word_count = map_i64(candidate, "target_word_count");
    let word_count = map_i64(candidate, "word_count");
    if target_word_count <= 0 || word_count <= 0 {
        return false;
    }
    let (lower, upper) = resolve_target_word_bounds(target_word_count);
    lower <= word_count && word_count <= upper
}

fn extract_quality_gate_payload(candidate: &Map<String, Value>) -> Map<String, Value> {
    candidate
        .get("quality_gate_plan")
        .and_then(Value::as_object)
        .and_then(|plan| plan.get("quality_gate"))
        .and_then(Value::as_object)
        .cloned()
        .or_else(|| {
            candidate
                .get("quality_metrics")
                .and_then(Value::as_object)
                .and_then(|metrics| metrics.get("quality_gate"))
                .and_then(Value::as_object)
                .cloned()
        })
        .unwrap_or_default()
}

fn extract_failed_focus_areas_from_candidate(candidate: &Map<String, Value>) -> Vec<String> {
    let gate = extract_quality_gate_payload(candidate);
    let mut focus_areas = Vec::new();
    for item in array_items(gate.get("failed_metrics")) {
        let Some(item) = item.as_object() else {
            continue;
        };
        let mut normalized = safe_text(
            item.get("focus_area")
                .or_else(|| item.get("key"))
                .or_else(|| item.get("label")),
        )
        .unwrap_or_default()
        .to_lowercase();
        if !normalized.is_empty() && !STRUCTURAL_REPAIR_FOCUS_AREAS.contains(&normalized.as_str()) {
            normalized = infer_focus_area_from_text(&normalized).unwrap_or(normalized);
        }
        if !normalized.is_empty() && !focus_areas.contains(&normalized) {
            focus_areas.push(normalized);
        }
    }
    focus_areas
}

fn extract_failed_metric_count(candidate: &Map<String, Value>) -> i64 {
    let gate = extract_quality_gate_payload(candidate);
    array_items(gate.get("failed_metrics"))
        .into_iter()
        .filter(|item| item.as_object().is_some())
        .count() as i64
}

fn is_word_budget_repair_candidate(candidate: &Map<String, Value>) -> bool {
    map_text(candidate, "attempt_kind") == "word_budget_repair"
        || map_text(candidate, "generation_path") == "word_budget_repair"
}

fn can_seed_targeted_final_repair_candidate(candidate: &Map<String, Value>, relaxed: bool) -> bool {
    let gate = extract_quality_gate_payload(candidate);
    if map_text(&gate, "decision").if_empty(map_text(candidate, "quality_gate_decision"))
        != "manual_review"
    {
        return false;
    }
    let target_word_count = map_i64(candidate, "target_word_count");
    let word_count = map_i64(candidate, "word_count");
    if target_word_count <= 0 || word_count <= 0 {
        return false;
    }
    let (lower, upper) = resolve_target_word_bounds(target_word_count);
    let polish_upper = upper + (target_word_count * 7 / 100).clamp(80, 140);
    let relaxed_upper = polish_upper + (target_word_count * 12 / 100).clamp(80, 200);
    if word_count < lower || word_count > if relaxed { relaxed_upper } else { polish_upper } {
        return false;
    }
    if map_i64(&gate, "continuity_warning_count") > 1 {
        return false;
    }
    let overall_score = map_f64(candidate, "overall_score").max(map_f64(&gate, "overall_score"));
    let score_floor = if relaxed {
        if is_word_budget_repair_candidate(candidate) {
            76.0
        } else {
            80.0
        }
    } else {
        84.0
    };
    if overall_score < score_floor {
        return false;
    }
    let focus_areas = extract_failed_focus_areas_from_candidate(candidate);
    if focus_areas.is_empty()
        || !focus_areas
            .iter()
            .all(|focus| TARGETED_FINAL_REPAIR_FOCUS_AREAS.contains(&focus.as_str()))
    {
        return false;
    }
    let failed_count = extract_failed_metric_count(candidate);
    if !focus_areas.iter().any(|focus| focus == "cliffhanger") {
        return is_rule_grounding_only_final_polish_candidate(
            candidate,
            overall_score,
            &focus_areas,
            failed_count,
            relaxed,
        ) || is_opening_rule_grounding_final_polish_candidate(
            candidate,
            overall_score,
            &focus_areas,
            failed_count,
            relaxed,
        );
    }
    let max_failed_count = if relaxed && is_word_budget_repair_candidate(candidate) {
        4
    } else {
        3
    };
    failed_count >= 1 && failed_count <= max_failed_count
}

fn can_fallback_seed_targeted_final_repair_candidate(candidate: &Map<String, Value>) -> bool {
    if is_word_budget_repair_candidate(candidate) {
        return false;
    }
    let gate = extract_quality_gate_payload(candidate);
    if map_text(&gate, "decision").if_empty(map_text(candidate, "quality_gate_decision"))
        != "manual_review"
    {
        return false;
    }
    let target_word_count = map_i64(candidate, "target_word_count");
    let word_count = map_i64(candidate, "word_count");
    if target_word_count <= 0 || word_count <= 0 {
        return false;
    }
    let focus_areas = extract_failed_focus_areas_from_candidate(candidate);
    let failed_count = extract_failed_metric_count(candidate);
    let overall_score = map_f64(candidate, "overall_score").max(map_f64(&gate, "overall_score"));
    if map_i64(&gate, "continuity_warning_count") > 1 {
        return false;
    }
    if !is_cliffhanger_only_final_polish_candidate(
        candidate,
        overall_score,
        &focus_areas,
        failed_count,
        false,
    ) {
        return false;
    }
    let (_, upper) = resolve_target_word_bounds(target_word_count);
    let polish_upper = upper + (target_word_count * 7 / 100).clamp(80, 140);
    let fallback_upper = upper + 600.max(target_word_count * 50 / 100);
    polish_upper < word_count && word_count <= fallback_upper
}

fn is_rule_grounding_only_final_polish_candidate(
    candidate: &Map<String, Value>,
    overall_score: f64,
    focus_areas: &[String],
    failed_count: i64,
    relaxed: bool,
) -> bool {
    focus_matches(focus_areas, &["rule_grounding"])
        && failed_count == 1
        && overall_score
            >= if relaxed {
                if is_word_budget_repair_candidate(candidate) {
                    86.0
                } else {
                    88.0
                }
            } else {
                90.0
            }
}

fn is_cliffhanger_only_final_polish_candidate(
    candidate: &Map<String, Value>,
    overall_score: f64,
    focus_areas: &[String],
    failed_count: i64,
    relaxed: bool,
) -> bool {
    focus_matches(focus_areas, &["cliffhanger"])
        && failed_count == 1
        && overall_score
            >= if relaxed {
                if is_word_budget_repair_candidate(candidate) {
                    85.0
                } else {
                    87.0
                }
            } else {
                89.0
            }
}

fn is_rule_grounding_cliffhanger_final_polish_candidate(
    candidate: &Map<String, Value>,
    overall_score: f64,
    focus_areas: &[String],
    failed_count: i64,
    relaxed: bool,
) -> bool {
    focus_matches(focus_areas, &["rule_grounding", "cliffhanger"])
        && failed_count == 2
        && overall_score
            >= if relaxed {
                if is_word_budget_repair_candidate(candidate) {
                    87.0
                } else {
                    89.0
                }
            } else {
                91.0
            }
}

fn is_opening_rule_grounding_final_polish_candidate(
    candidate: &Map<String, Value>,
    overall_score: f64,
    focus_areas: &[String],
    failed_count: i64,
    relaxed: bool,
) -> bool {
    focus_matches(focus_areas, &["opening", "rule_grounding"])
        && failed_count == 2
        && overall_score
            >= if relaxed {
                if is_word_budget_repair_candidate(candidate) {
                    86.0
                } else {
                    88.0
                }
            } else {
                90.0
            }
}

fn is_opening_rule_grounding_cliffhanger_final_polish_candidate(
    candidate: &Map<String, Value>,
    overall_score: f64,
    focus_areas: &[String],
    failed_count: i64,
    relaxed: bool,
) -> bool {
    focus_matches(focus_areas, &["opening", "rule_grounding", "cliffhanger"])
        && failed_count == 3
        && overall_score
            >= if relaxed {
                if is_word_budget_repair_candidate(candidate) {
                    86.0
                } else {
                    88.0
                }
            } else {
                90.0
            }
}

fn is_dialogue_cliffhanger_final_polish_candidate(
    candidate: &Map<String, Value>,
    overall_score: f64,
    focus_areas: &[String],
    failed_count: i64,
    relaxed: bool,
) -> bool {
    focus_matches(focus_areas, &["dialogue", "cliffhanger"])
        && failed_count == 2
        && overall_score
            >= if relaxed {
                if is_word_budget_repair_candidate(candidate) {
                    85.0
                } else {
                    87.0
                }
            } else {
                89.0
            }
}

fn is_opening_conflict_cliffhanger_final_polish_candidate(
    candidate: &Map<String, Value>,
    overall_score: f64,
    focus_areas: &[String],
    failed_count: i64,
    relaxed: bool,
) -> bool {
    focus_matches(focus_areas, &["opening", "conflict", "cliffhanger"])
        && failed_count == 3
        && overall_score
            >= if relaxed {
                if is_word_budget_repair_candidate(candidate) {
                    86.0
                } else {
                    88.0
                }
            } else {
                90.0
            }
}

fn focus_matches(focus_areas: &[String], expected: &[&str]) -> bool {
    let left = focus_areas
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let right = expected.iter().copied().collect::<HashSet<_>>();
    left == right
}

fn targeted_seed_rank_key(candidate: &Value) -> (i64, i64, i64, i64, i64) {
    let Some(candidate) = candidate.as_object() else {
        return (0, 0, 0, 0, 0);
    };
    let target = map_i64(candidate, "target_word_count");
    let word_count = map_i64(candidate, "word_count");
    (
        i64::from(is_candidate_word_count_in_target_window(candidate)),
        i64::from(is_word_budget_repair_candidate(candidate)),
        -(word_count - target).abs(),
        -extract_failed_metric_count(candidate),
        (map_f64(candidate, "overall_score") * 100.0).round() as i64,
    )
}

fn candidate_rank_key(candidate: &Value) -> (i64, i64, i64, i64, i64) {
    let Some(candidate) = candidate.as_object() else {
        return (0, 0, 0, 0, 0);
    };
    (
        map_i64(candidate, "quality_gate_priority"),
        (map_f64(candidate, "selection_score") * 100.0).round() as i64,
        (map_f64(candidate, "overall_score") * 100.0).round() as i64,
        (map_f64(candidate, "word_count_fit_score") * 100.0).round() as i64,
        -map_i64(candidate, "candidate_index"),
    )
}

fn build_focus_strategy_lines(runtime_context: &Map<String, Value>) -> Vec<String> {
    let story_focus = map_text(runtime_context, "story_focus");
    let creative_mode = map_text(runtime_context, "creative_mode");
    let quality_preset = map_text(runtime_context, "quality_preset");
    let mut lines = Vec::new();
    match story_focus.as_str() {
        "advance_plot" => lines.push("- Rebuild the scene around visible objectives, resistance, and a changed situation.".to_string()),
        "deepen_character" => lines.push("- Surface the protagonist's tradeoff through decisive action and emotional aftershock.".to_string()),
        "escalate_conflict" => lines.push("- Make the opposition push back harder and force a more costly next move.".to_string()),
        "reveal_mystery" => lines.push("- Introduce one concrete clue while preserving a sharper unanswered question.".to_string()),
        "relationship_shift" => lines.push("- Let dialogue and power balance visibly shift the relationship by scene end.".to_string()),
        "foreshadow_payoff" => lines.push("- Cash out at least one prior setup with a visible consequence on the page.".to_string()),
        _ => {}
    }
    match creative_mode.as_str() {
        "hook" | "suspense" => lines.push("- Finish on a tighter question, approaching risk, or decision under pressure.".to_string()),
        "emotion" => lines.push("- Strengthen nonverbal reactions, hesitation, and emotional recoil instead of explanation.".to_string()),
        "relationship" => lines.push("- Increase push-pull dialogue and protect each character's distinct voice.".to_string()),
        "payoff" => lines.push("- Emphasize setup -> action -> feedback so the scene lands with payoff, not summary.".to_string()),
        _ => {}
    }
    match quality_preset.as_str() {
        "plot_drive" => lines
            .push("- Prefer sharper action-counteraction beats over extra exposition.".to_string()),
        "immersive" => lines.push(
            "- Add sensory anchors at the decisive beats without stalling the scene.".to_string(),
        ),
        "emotion_drama" => lines.push(
            "- Sharpen subtext, contradiction, and relational tension in dialogue.".to_string(),
        ),
        "clean_prose" => lines.push(
            "- Cut repeated explanation and keep sentence rhythm cleaner and tighter.".to_string(),
        ),
        _ => {}
    }
    lines
}

fn build_quality_gate_focus_repair_lines(focus_areas: &[String], hard_mode: bool) -> Vec<String> {
    let set = focus_set_for(focus_areas, TARGETED_FINAL_REPAIR_FOCUS_AREAS);
    let mut lines = Vec::new();
    if set.contains("outline") {
        lines.push("- Outline repair: stay on the promised outline rail and explicitly land the chapter's required beats in-scene.".to_string());
        if hard_mode {
            lines.push("- Outline hard rule: cover every mandatory outline beat in compressed form before the final paragraph.".to_string());
        }
    }
    if set.contains("opening") {
        lines.push("- Opening repair: within the first 120-180 Chinese chars, surface at least two on-page hooks.".to_string());
    }
    if set.contains("rule_grounding") {
        lines.push("- Rule-grounding repair: convert at least one active rule, countdown, platform mechanism, or organization constraint into an on-page obstacle, cost, or action result.".to_string());
    }
    if set.contains("cliffhanger") {
        lines.push("- Cliffhanger repair: the final paragraph must end on a fresh imbalance, pending choice, approaching danger, identity shift, or new access signal.".to_string());
    }
    if set.contains("dialogue") {
        lines.push("- Dialogue repair: keep at least one back-and-forth exchange with stance collision or subtext pressure.".to_string());
    }
    lines
}

fn build_structural_repair_lines(
    focus_areas: &[String],
    runtime_context: &Map<String, Value>,
    hard_mode: bool,
) -> Vec<String> {
    let focus_areas = focus_areas
        .iter()
        .filter(|focus| STRUCTURAL_REPAIR_FOCUS_AREAS.contains(&focus.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if focus_areas.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![
        "- Hard checklist: objective -> resistance -> forced choice -> consequence -> payoff/hook."
            .to_string(),
    ];
    if hard_mode {
        lines.push(
            "- 中文硬约束：正文必须写出“目标/受阻 → 被迫投择 → 代价/后果 → 阶段性兑现或悬念钩子”。"
                .to_string(),
        );
    }
    if focus_areas.contains(&"conflict".to_string()) {
        lines.push("- 必须至少出现 1 次明确受阻与升级。".to_string());
    }
    if focus_areas.contains(&"rule_grounding".to_string()) {
        lines.push("- 必须至少出现 1 次规则/限制改变行动结果。".to_string());
    }
    if focus_areas.contains(&"payoff".to_string()) {
        lines.push("- 必须至少出现 1 次兑现/回收。".to_string());
    }
    let character_focus = normalize_items(
        runtime_context
            .get("character_focus")
            .or_else(|| runtime_context.get("story_character_focus")),
        3,
    );
    push_joined_line(&mut lines, "关键动作优先落在这些角色身上", &character_focus);
    lines
}

fn build_continuity_repair_lines(
    plan: Option<&Value>,
    runtime_context: &Map<String, Value>,
    hard_mode: bool,
) -> Vec<String> {
    let Some(plan) = plan.and_then(Value::as_object) else {
        return Vec::new();
    };
    let gate = plan
        .get("quality_gate")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let payload = active_story_repair_payload_object_from_plan(Some(plan))
        .cloned()
        .unwrap_or_default();
    let mut has_pressure = array_items(gate.get("failed_metrics"))
        .into_iter()
        .any(|item| {
            item.as_object().is_some_and(|item| {
                ["focus_area", "key", "label", "repair_target"]
                    .iter()
                    .any(|key| {
                        item.get(*key)
                            .and_then(|value| safe_text(Some(value)))
                            .is_some_and(|text| is_continuity_focus_area(&text))
                    })
            })
        });
    if !has_pressure {
        has_pressure = normalize_items(payload.get("focus_areas"), 4)
            .into_iter()
            .chain(normalize_items(payload.get("repair_targets"), 4))
            .chain(safe_text(payload.get("summary")).into_iter())
            .any(|item| is_continuity_focus_area(&item));
    }
    if !has_pressure {
        return Vec::new();
    }
    let mut lines = vec![if hard_mode {
        "- 中文连续性硬约束：至少显式接住 1-2 项跨章账本，把它写成动作、站位变化、资源调度、关系反馈或组织指令。".to_string()
    } else {
        "- 中文连续性要求：优先把跨章账本改写成现场动作、关系反馈或组织变化。".to_string()
    }];
    let targets = normalize_items(payload.get("repair_targets"), 3);
    push_joined_line(&mut lines, "优先补齐这些连续性接力点", &targets);
    let ledgers = [
        "character_state_ledger",
        "relationship_state_ledger",
        "organization_state_ledger",
    ]
    .iter()
    .filter_map(|key| {
        normalize_items(runtime_context.get(*key), 1)
            .into_iter()
            .next()
    })
    .collect::<Vec<_>>();
    push_joined_line(&mut lines, "本轮至少落地其中一项连续性账本", &ledgers);
    lines
}

fn add_targeted_focus_lines(lines: &mut Vec<String>, focus_set: &HashSet<String>) {
    if focus_set.contains("opening") {
        lines.push("- Opening repair focus: the first 120-180 chars must present a live anomaly, concrete objective, obstruction, warning, or forced choice on-page.".to_string());
    }
    if focus_set.contains("outline") {
        lines.push("- Outline preservation rule: keep every already-landed required beat on-page; tighten wording instead of deleting the promised chapter turn.".to_string());
    }
    if focus_set.contains("dialogue") {
        lines.push("- Dialogue repair focus: keep at least one two-sided exchange with stance collision, interruption, or subtext pressure; cut monologue exposition first.".to_string());
    }
    if focus_set.contains("conflict") {
        lines.push("- Conflict repair focus: preserve a visible blocker, counter-move, or leverage swing on-page.".to_string());
    }
    if focus_set.contains("rule_grounding") {
        lines.push("- Rule-grounding repair focus: keep at least one active rule, platform check, timer, or organization constraint on-page.".to_string());
    }
    if focus_set.contains("cliffhanger") {
        lines.push("- Cliffhanger repair focus: the final paragraph must escalate into a concrete reveal, order, timer, threat, identity shift, or pending forced choice.".to_string());
    }
}

fn extract_edge_anchors(text: &str) -> (String, String) {
    let raw = text.replace('\r', "").trim().to_string();
    if raw.is_empty() {
        return (String::new(), String::new());
    }
    let paragraphs = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let opening_source = paragraphs.first().copied().unwrap_or(raw.as_str());
    let closing_source = paragraphs.last().copied().unwrap_or(raw.as_str());
    let opening = compact_anchor_text(opening_source, 90);
    let mut closing = compact_anchor_text(closing_source, 90);
    if closing == opening && raw.chars().count() > 160 {
        closing = compact_anchor_text(
            &raw.chars()
                .rev()
                .take(140)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>(),
            90,
        );
    }
    (opening, closing)
}

fn compact_anchor_text(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    format!(
        "{}...",
        normalized
            .chars()
            .take(max_chars)
            .collect::<String>()
            .trim_end()
    )
}

fn is_continuity_focus_area(value: &str) -> bool {
    let normalized = value.to_lowercase();
    [
        "continuity",
        "连续性",
        "接力",
        "账本",
        "ledger",
        "handoff",
        "character_continuity",
        "relationship_continuity",
        "organization_continuity",
        "career_continuity",
        "foreshadow_continuity",
    ]
    .iter()
    .any(|hint| normalized.contains(hint))
}

fn focus_set_for(values: &[String], allowed: &[&str]) -> HashSet<String> {
    values
        .iter()
        .filter(|value| allowed.contains(&value.as_str()))
        .cloned()
        .collect()
}

trait EmptyFallback {
    fn if_empty(self, fallback: String) -> String;
}

impl EmptyFallback for String {
    fn if_empty(self, fallback: String) -> String {
        if self.is_empty() {
            fallback
        } else {
            self
        }
    }
}

fn quality_gate_decision_priority(decision: &str) -> i64 {
    match decision {
        "allow_save" => 3,
        "auto_repair" => 2,
        "manual_review" => 1,
        _ => 0,
    }
}

fn object_from_value(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn object_from_option(value: Option<Value>) -> Map<String, Value> {
    value.map(object_from_value).unwrap_or_default()
}

fn normalize_items(value: Option<&Value>, limit: usize) -> Vec<String> {
    let raw_items = match value {
        Some(Value::Array(items)) => items.iter().collect::<Vec<_>>(),
        Some(value) => vec![value],
        None => Vec::new(),
    };
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for value in raw_items {
        let Some(text) = safe_text(Some(value)) else {
            continue;
        };
        if text.is_empty() || seen.contains(&text) {
            continue;
        }
        seen.insert(text.clone());
        items.push(text);
        if items.len() >= limit {
            break;
        }
    }
    items
}

fn array_items(value: Option<&Value>) -> Vec<&Value> {
    match value {
        Some(Value::Array(items)) => items.iter().collect(),
        _ => Vec::new(),
    }
}

fn safe_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => {
            let text = text.trim().to_string();
            (!text.is_empty()).then_some(text)
        }
        Value::Number(number) => Some(number.to_string()).filter(|text| text != "0"),
        Value::Bool(value) => value.then(|| value.to_string()),
        _ => None,
    }
}

fn safe_float(value: f64) -> Option<f64> {
    value
        .is_finite()
        .then_some(value)
        .filter(|value| *value != 0.0)
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|value| value as i64)),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        Value::Bool(value) => Some(i64::from(*value)),
        _ => None,
    }
}

fn value_to_f64(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        Value::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn map_i64(map: &Map<String, Value>, key: &str) -> i64 {
    map.get(key).and_then(value_to_i64).unwrap_or(0)
}

fn map_f64(map: &Map<String, Value>, key: &str) -> f64 {
    value_to_f64(map.get(key)).unwrap_or(0.0)
}

fn map_text(map: &Map<String, Value>, key: &str) -> String {
    safe_text(map.get(key)).unwrap_or_default()
}

fn round_to(value: f64, places: i32) -> f64 {
    let factor = 10_f64.powi(places);
    (value * factor).round() / factor
}

fn insert_i64(map: &mut Map<String, Value>, key: &str, value: i64) {
    map.insert(key.to_string(), Value::Number(Number::from(value)));
}

fn insert_f64(map: &mut Map<String, Value>, key: &str, value: f64) {
    if let Some(number) = Number::from_f64(value) {
        map.insert(key.to_string(), Value::Number(number));
    }
}

fn insert_string(map: &mut Map<String, Value>, key: &str, value: String) {
    map.insert(key.to_string(), Value::String(value));
}

fn insert_optional_string(map: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    }) {
        insert_string(map, key, value);
    }
}

fn push_line(lines: &mut Vec<String>, label: &str, value: &str) {
    if !value.trim().is_empty() {
        lines.push(format!("- {label}: {}", value.trim()));
    }
}

fn push_joined_line(lines: &mut Vec<String>, label: &str, values: &[String]) {
    if !values.is_empty() {
        lines.push(format!("- {label}: {}", values.join(" / ")));
    }
}

pub(crate) fn build_chapter_candidate_rerank_owner_contract() -> Value {
    json!({
        "owner": "chapter_candidate_rerank_service",
        "scope": "candidate_rerank_retry_repair_formula_owner",
        "python_source_map": [
            "backend/app/services/chapter_candidate_rerank_service.py",
            "backend/app/services/chapter_candidate_generation_service.py",
            "backend/app/services/chapter_candidate_finalize_service.py",
            "backend/app/services/chapter_candidate_word_budget_repair_service.py",
            "backend/app/services/chapter_candidate_targeted_final_repair_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_candidate_rerank_service.rs",
            "backend-rs/src/services/chapter_candidate_generation_service.rs",
            "backend-rs/src/services/chapter_candidate_finalize_service.rs",
            "backend-rs/src/services/chapter_candidate_word_budget_repair_service.rs",
            "backend-rs/src/services/chapter_candidate_targeted_final_repair_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "normalize_candidate_quality_gate_plan",
                "build_candidate_retry_prompt_suffix",
                "build_candidate_retry_strategy_suffix",
                "resolve_candidate_retry_temperature",
                "build_candidate_selection_metadata",
                "attach_candidate_selection_metadata",
                "build_candidate_pool_summary",
                "select_best_generation_candidate",
                "should_generate_additional_candidate",
                "should_apply_word_budget_repair",
                "build_word_budget_repair_suffix",
                "resolve_word_budget_repair_temperature",
                "should_apply_targeted_final_repair",
                "build_targeted_final_repair_suffix",
                "resolve_targeted_final_repair_temperature",
                "select_targeted_final_repair_seed_candidate"
            ],
            "formula_groups": [
                "quality gate normalization under word-pressure",
                "candidate selection metadata and summary projection",
                "pool winner ranking by gate priority, selection score, overall score, word fit, and candidate index",
                "additional candidate pressure from quality gate and retry capacity",
                "retry prompt and strategy suffix materialization",
                "retry temperature adjustment from runtime context",
                "word-budget repair char/token/keep/prefer formulas",
                "targeted final repair seed/followup/keep/prefer formulas"
            ],
            "ranking_policy": [
                "prefer higher quality gate priority",
                "prefer higher selection score",
                "prefer higher overall score",
                "prefer higher word count fit score",
                "prefer lower candidate index as stable tie-breaker"
            ],
            "repair_policy": [
                "word-budget repair is driven by severe length pressure and repair quality",
                "targeted final repair is driven by manual-review focus areas and followup pressure",
                "content-sensitive focus areas relax repair length limits",
                "edge anchors are preserved for word-budget repair suffixes"
            ],
            "error_contract": [
                "safe text and numeric coercion ignore malformed values",
                "non-finite temperatures are rejected before JSON number insertion",
                "empty or non-object candidates are ranked below valid candidates"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_candidate_rerank_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_candidate_generation_service",
            "chapter_candidate_finalize_service",
            "chapter_candidate_word_budget_repair_service",
            "chapter_candidate_targeted_final_repair_service",
            "chapter_candidate_executor_default_dependency_service",
            "chapter_candidate_route_gateway_service"
        ],
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner"
            ],
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 17,
            "python_fallback_probe_count": 0,
            "quality_gate_normalization_owner": "normalize_candidate_quality_gate_plan",
            "selection_metadata_owner": "build_candidate_selection_metadata",
            "candidate_pool_summary_owner": "build_candidate_pool_summary",
            "candidate_selection_owner": "select_best_generation_candidate",
            "additional_candidate_pressure_owner": "should_generate_additional_candidate",
            "retry_prompt_owner": "build_candidate_retry_prompt_suffix",
            "retry_strategy_owner": "build_candidate_retry_strategy_suffix",
            "retry_temperature_owner": "resolve_candidate_retry_temperature",
            "word_budget_repair_owner": "build_word_budget_repair_suffix",
            "targeted_final_repair_owner": "build_targeted_final_repair_suffix",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": false,
            "remaining_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
            "status": "rust_chapter_candidate_rerank_owner_ready_for_source_map_closeout_review"
        },
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_rerank_python_source_map",
            "python_fallback_removal_ready": false,
            "approval_required": "explicit source-map freeze/delete/repoint approval"
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        active_story_repair_payload_object_from_plan, attach_candidate_selection_metadata,
        build_candidate_pool_summary, build_candidate_retry_prompt_suffix,
        build_candidate_selection_metadata, build_chapter_candidate_rerank_owner_contract,
        build_targeted_final_repair_suffix, build_word_budget_repair_suffix,
        normalize_candidate_quality_gate_plan, resolve_candidate_retry_temperature,
        resolve_targeted_final_repair_char_limit, resolve_targeted_final_repair_max_tokens,
        resolve_word_budget_repair_char_limit, resolve_word_budget_repair_max_tokens,
        select_best_generation_candidate, select_targeted_final_repair_seed_candidate,
        should_apply_followup_targeted_final_repair, should_apply_targeted_final_repair,
        should_apply_word_budget_repair, should_generate_additional_candidate,
        should_keep_targeted_final_repair_candidate, should_keep_word_budget_repair_candidate,
        should_prefer_targeted_final_repair_candidate, should_prefer_word_budget_repair_candidate,
        CandidateSelectionMetadataInput,
    };

    #[test]
    fn normalizes_allow_save_gate_under_severe_word_pressure() {
        let plan = normalize_candidate_quality_gate_plan(
            json!({"quality_gate": {"decision": "allow_save"}}),
            1600,
            800,
            json!({}),
        );

        assert_eq!(plan["quality_gate"]["decision"], "auto_repair");
        assert_eq!(plan["quality_gate"]["allow_save"], false);
        assert_eq!(plan["quality_gate"]["can_auto_repair"], true);
    }

    #[test]
    fn should_extract_active_story_repair_payload_object_from_quality_gate_plan() {
        let plan = json!({
            "quality_gate": {"decision": "auto_repair"},
            "active_story_repair_payload": {
                "summary": "ending is soft",
                "repair_targets": ["final hook"]
            }
        });

        let payload = active_story_repair_payload_object_from_plan(plan.as_object());

        assert_eq!(
            payload.and_then(|payload| payload.get("summary")),
            Some(&json!("ending is soft"))
        );
        assert_eq!(
            payload
                .and_then(|payload| payload.get("repair_targets"))
                .and_then(|items| items.as_array())
                .and_then(|items| items.first()),
            Some(&json!("final hook"))
        );
    }

    #[test]
    fn builds_selection_metadata_and_attaches_to_metrics() {
        let metadata = build_candidate_selection_metadata(CandidateSelectionMetadataInput {
            quality_metrics: Some(json!({
                "overall_score": 88.4,
                "pacing_score": 8.2,
                "continuity_preflight": {"warning_count": 1}
            })),
            word_count: 780,
            target_word_count: 800,
            candidate_index: 2,
            candidate_count: 3,
            source: "chapter".to_string(),
            quality_gate_plan: Some(
                json!({"quality_gate": {"decision": "allow_save", "status": "pass"}}),
            ),
            generation_path: Some("rerank_retry".to_string()),
            attempt_kind: Some("rerank_candidate".to_string()),
            rerank_used: Some(true),
            word_budget_repair_used: Some(false),
            winner_candidate_index: Some(2),
            repair_seed_candidate_index: None,
            repair_seed_generation_path: None,
            repair_seed_attempt_kind: None,
        });
        let metrics =
            attach_candidate_selection_metadata(json!({"overall_score": 88.4}), metadata.clone());

        assert_eq!(metadata["quality_gate_priority"], 3);
        assert_eq!(metadata["generation_path"], "rerank_retry");
        assert_eq!(metadata["rerank_used"], true);
        assert_eq!(metrics["candidate_selection"]["candidate_index"], 2);
    }

    #[test]
    fn selects_best_generation_candidate_by_gate_score_and_index() {
        let winner = select_best_generation_candidate(vec![
            json!({"candidate_index": 1, "quality_gate_priority": 2, "selection_score": 90.0, "overall_score": 91.0, "word_count_fit_score": 96.0}),
            json!({"candidate_index": 2, "quality_gate_priority": 3, "selection_score": 70.0, "overall_score": 78.0, "word_count_fit_score": 90.0}),
            json!({"candidate_index": 3, "quality_gate_priority": 3, "selection_score": 70.0, "overall_score": 78.0, "word_count_fit_score": 90.0}),
        ])
        .expect("winner");

        assert_eq!(winner["candidate_index"], 2);
        assert_eq!(winner["rerank_pool_size"], 3);
    }

    #[test]
    fn detects_additional_candidate_pressure() {
        let candidate = json!({
            "quality_gate_decision": "auto_repair",
            "quality_gate_plan": {
                "quality_gate": {"failed_metrics": [{"label": "cliffhanger", "focus_area": "cliffhanger"}]}
            }
        });

        assert!(should_generate_additional_candidate(candidate, 1, 3));
    }

    #[test]
    fn owns_word_budget_repair_formulas() {
        let selected = json!({
            "candidate_index": 1,
            "target_word_count": 800,
            "word_count": 1300,
            "quality_gate_decision": "auto_repair",
            "overall_score": 92.0,
            "quality_gate_plan": {"quality_gate": {"failed_metrics": [{"label": "too long"}]}}
        });
        let repair = json!({
            "candidate_index": 2,
            "target_word_count": 800,
            "word_count": 880,
            "quality_gate_decision": "auto_repair",
            "overall_score": 85.0,
            "quality_gate_plan": {"quality_gate": {"failed_metrics": [{"label": "too long"}]}}
        });

        assert!(should_apply_word_budget_repair(selected.clone()));
        assert_eq!(
            resolve_word_budget_repair_max_tokens(800, 1300, false),
            414_i64.max(520)
        );
        assert!(resolve_word_budget_repair_char_limit(800, false).unwrap() > 900);
        assert!(should_keep_word_budget_repair_candidate(
            selected.clone(),
            repair.clone()
        ));
        assert!(should_prefer_word_budget_repair_candidate(selected, repair));
    }

    #[test]
    fn owns_targeted_final_repair_seed_and_followup_formulas() {
        let candidate = json!({
            "candidate_index": 2,
            "target_word_count": 800,
            "word_count": 890,
            "overall_score": 91.0,
            "quality_gate_decision": "manual_review",
            "attempt_kind": "targeted_quality_repair",
            "quality_gate_plan": {
                "quality_gate": {
                    "decision": "manual_review",
                    "failed_metrics": [{"label": "cliffhanger", "focus_area": "cliffhanger"}],
                    "continuity_warning_count": 0
                }
            }
        });

        assert!(should_apply_targeted_final_repair(candidate.clone()));
        assert!(should_apply_followup_targeted_final_repair(
            candidate.clone()
        ));
        assert_eq!(resolve_targeted_final_repair_max_tokens(800, 890), 520);
        assert!(resolve_targeted_final_repair_char_limit(800).unwrap() > 950);

        let seed = select_targeted_final_repair_seed_candidate(
            candidate.clone(),
            vec![json!({"candidate_index": 3, "target_word_count": 800, "word_count": 900})],
        )
        .expect("seed");
        assert_eq!(seed["candidate_index"], 2);
    }

    #[test]
    fn keeps_and_prefers_targeted_repair_candidate_when_quality_improves() {
        let selected = json!({
            "target_word_count": 800,
            "word_count": 890,
            "overall_score": 88.0,
            "quality_gate_decision": "manual_review",
            "quality_gate_plan": {"quality_gate": {"failed_metrics": [
                {"label": "cliffhanger", "focus_area": "cliffhanger"},
                {"label": "dialogue", "focus_area": "dialogue"}
            ]}}
        });
        let repair = json!({
            "target_word_count": 800,
            "word_count": 860,
            "overall_score": 90.0,
            "quality_gate_decision": "manual_review",
            "quality_gate_plan": {"quality_gate": {"failed_metrics": [
                {"label": "cliffhanger", "focus_area": "cliffhanger"}
            ]}}
        });

        assert!(should_keep_targeted_final_repair_candidate(
            selected.clone(),
            repair.clone()
        ));
        assert!(should_prefer_targeted_final_repair_candidate(
            selected.clone(),
            repair.clone()
        ));
        assert!(super::should_adopt_targeted_final_repair_candidate(
            selected, repair
        ));
    }

    #[test]
    fn builds_prompt_suffixes_with_key_contract_lines() {
        let plan = json!({
            "quality_gate": {"failed_metrics": [{"label": "Cliffhanger", "focus_area": "cliffhanger"}]},
            "active_story_repair_payload": {"summary": "ending is soft", "repair_targets": ["final hook"]}
        });
        let retry = build_candidate_retry_prompt_suffix(Some(plan.clone()), 2).expect("retry");
        let word_budget = build_word_budget_repair_suffix(
            Some(json!({"candidate_selection": {"word_count": 1300}})),
            Some(plan.clone()),
            Some("Opening beat.\n\nClosing beat.".to_string()),
            800,
            3,
            "chapter".to_string(),
        )
        .expect("word budget suffix");
        let targeted =
            build_targeted_final_repair_suffix(None, Some(plan), 800, 4, "chapter".to_string())
                .expect("targeted suffix");

        assert!(retry.contains("Revision attempt #2"));
        assert!(word_budget.contains("Word-budget repair pass #3"));
        assert!(word_budget.contains("Preserve this opening anchor"));
        assert!(targeted.contains("Targeted quality repair pass #4"));
        assert!(targeted.contains("Cliffhanger repair focus"));
    }

    #[test]
    fn builds_candidate_pool_summary() {
        let summary = build_candidate_pool_summary(
            vec![json!({
                "candidate_index": 2,
                "quality_metrics": {
                    "candidate_selection": {"selection_score": 91.22, "word_count": 790},
                    "quality_gate": {"decision": "allow_save", "failed_metrics": [{"label": "x"}]}
                }
            })],
            Some(2),
            None,
        );

        assert_eq!(summary[0]["candidate_index"], 2);
        assert_eq!(summary[0]["is_winner"], true);
        assert_eq!(summary[0]["failed_metrics"][0], "x");
    }

    #[test]
    fn retry_temperature_respects_runtime_context() {
        let temperature = resolve_candidate_retry_temperature(
            0.8,
            Some(json!({
                "quality_runtime_context": {"quality_preset": "clean_prose", "creative_mode": "hook"},
                "candidate_selection": {"word_count": 1300, "target_word_count": 800}
            })),
            Some(json!({"quality_gate": {"decision": "manual_review"}})),
            3,
        )
        .expect("temperature");

        assert_eq!(temperature, 0.62);
    }

    #[test]
    fn should_publish_chapter_candidate_rerank_owner_contract() {
        let contract = build_chapter_candidate_rerank_owner_contract();

        assert_eq!(contract["owner"], "chapter_candidate_rerank_service");
        assert_eq!(
            contract["scope"],
            "candidate_rerank_retry_repair_formula_owner"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/services/chapter_candidate_rerank_service.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_candidate_rerank_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][7],
            "select_best_generation_candidate"
        );
        assert_eq!(
            contract["behavior_contract"]["formula_groups"][6],
            "word-budget repair char/token/keep/prefer formulas"
        );
        assert_eq!(
            contract["behavior_contract"]["ranking_policy"][4],
            "prefer lower candidate index as stable tie-breaker"
        );
        assert_eq!(
            contract["active_consumers"][0],
            "chapter_candidate_generation_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][1],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            json!(11)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["candidate_selection_owner"],
            "select_best_generation_candidate"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["word_budget_repair_owner"],
            "build_word_budget_repair_suffix"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["targeted_final_repair_owner"],
            "build_targeted_final_repair_suffix"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            false
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_rerank_owner_ready_for_source_map_closeout_review"
        );
    }
}
