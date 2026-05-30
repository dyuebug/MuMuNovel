use serde_json::{json, Value};
use std::collections::HashSet;

use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions;

const MANUAL_REQUEST_SOURCE: &str = "manual_request";
const MANUAL_REQUEST_SOURCE_LABEL: &str = "Manual request";
const CURRENT_CHAPTER_QUALITY_SOURCE: &str = "current_chapter_quality";
const CURRENT_CHAPTER_QUALITY_SOURCE_LABEL: &str = "Current chapter quality";
const MANUAL_PLUS_CURRENT_CHAPTER_QUALITY_SOURCE: &str =
    "manual_plus_current_chapter_quality";
const MANUAL_PLUS_CURRENT_CHAPTER_QUALITY_SOURCE_LABEL: &str =
    "Manual + current chapter quality";
const MANUAL_PLUS_RECENT_HISTORY_SUMMARY_SOURCE: &str =
    "manual_plus_recent_history_summary";
const MANUAL_PLUS_RECENT_HISTORY_SUMMARY_SOURCE_LABEL: &str =
    "Manual + recent history summary";

pub(crate) fn has_explicit_story_repair_input(
    compat_options: &SingleChapterGenerationCompatOptions,
) -> bool {
    !compat_options.story_repair_summary().trim().is_empty()
        || !compat_options.story_repair_targets().is_empty()
        || !compat_options.story_preserve_strengths().is_empty()
}

pub(crate) fn restore_story_repair_compat_options_from_active_snapshot(
    compat_options: &SingleChapterGenerationCompatOptions,
    active_story_repair_payload: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> SingleChapterGenerationCompatOptions {
    if has_explicit_story_repair_input(compat_options) {
        return compat_options.clone();
    }

    let snapshot_payload = active_story_repair_payload
        .and_then(Value::as_object)
        .cloned()
        .or_else(|| {
            restore_story_repair_payload_from_quality_context(
                quality_metrics_summary,
                latest_quality_metrics,
            )
        });

    let Some(snapshot) = snapshot_payload else {
        return compat_options.clone();
    };

    let summary = snapshot
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let repair_targets = snapshot
        .get("repair_targets")
        .and_then(Value::as_array)
        .map(|items| normalize_guidance_items(items, 4))
        .unwrap_or_default();
    let preserve_strengths = snapshot
        .get("preserve_strengths")
        .and_then(Value::as_array)
        .map(|items| normalize_guidance_items(items, 2))
        .unwrap_or_default();

    let mut restored = compat_options.clone();
    restored.story_repair_summary = summary;
    restored.story_repair_targets = repair_targets;
    restored.story_preserve_strengths = preserve_strengths;
    restored
}

pub(crate) fn restore_story_repair_payload_from_quality_context(
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Option<serde_json::Map<String, Value>> {
    quality_repair_guidance_from_quality_context(quality_metrics_summary, latest_quality_metrics)
        .map(|guidance| {
            let mut payload = serde_json::Map::new();
            if let Some(summary) = guidance.get("summary").cloned() {
                payload.insert("summary".to_string(), summary);
            }
            if let Some(repair_targets) = guidance.get("repair_targets").cloned() {
                payload.insert("repair_targets".to_string(), repair_targets);
            }
            if let Some(preserve_strengths) = guidance.get("preserve_strengths").cloned() {
                payload.insert("preserve_strengths".to_string(), preserve_strengths);
            }
            payload
        })
        .filter(|payload| !payload.is_empty())
}

pub(crate) fn restore_active_story_repair_payload_from_quality_context(
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    scope: &str,
    source: &str,
    source_label: &str,
) -> Option<Value> {
    let guidance =
        quality_repair_guidance_from_quality_context(quality_metrics_summary, latest_quality_metrics)?;
    let payload = restore_story_repair_payload_from_quality_context(
        quality_metrics_summary,
        latest_quality_metrics,
    )?;
    let quality_gate = quality_gate_from_quality_context(quality_metrics_summary, latest_quality_metrics);

    let summary = payload.get("summary").cloned().unwrap_or(Value::Null);
    let repair_targets = payload
        .get("repair_targets")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let preserve_strengths = payload
        .get("preserve_strengths")
        .cloned()
        .unwrap_or_else(|| json!([]));

    let focus_areas = guidance
        .get("focus_areas")
        .and_then(Value::as_array)
        .map(|items| normalize_guidance_items(items, 4))
        .unwrap_or_default();
    let weakest_metric_key = guidance
        .get("weakest_metric_key")
        .cloned()
        .filter(|value| value.is_string());
    let weakest_metric_label = guidance
        .get("weakest_metric_label")
        .cloned()
        .filter(|value| value.is_string());
    let weakest_metric_value = guidance
        .get("weakest_metric_value")
        .cloned()
        .filter(|value| value.is_number());
    let quality_gate_status = quality_gate
        .as_ref()
        .and_then(|gate| gate.get("status"))
        .cloned()
        .filter(|value| value.is_string());
    let quality_gate_decision = quality_gate
        .as_ref()
        .and_then(|gate| gate.get("decision"))
        .cloned()
        .filter(|value| value.is_string());
    let quality_gate_label = quality_gate
        .as_ref()
        .and_then(|gate| gate.get("label"))
        .cloned()
        .filter(|value| value.is_string());
    let quality_gate_summary = quality_gate
        .as_ref()
        .and_then(|gate| gate.get("summary"))
        .cloned()
        .filter(|value| value.is_string());
    let quality_gate_failed_metrics = quality_gate
        .as_ref()
        .and_then(|gate| gate.get("failed_metrics"))
        .and_then(Value::as_array)
        .map(|items| {
            items.iter()
                .filter_map(Value::as_object)
                .filter_map(|item| item.get("label"))
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(json!({
        "summary": summary,
        "repair_targets": repair_targets,
        "preserve_strengths": preserve_strengths,
        "focus_areas": focus_areas,
        "weakest_metric_key": weakest_metric_key.unwrap_or(Value::Null),
        "weakest_metric_label": weakest_metric_label.unwrap_or(Value::Null),
        "weakest_metric_value": weakest_metric_value.unwrap_or(Value::Null),
        "quality_gate": quality_gate.unwrap_or(Value::Null),
        "quality_gate_status": quality_gate_status.unwrap_or(Value::Null),
        "quality_gate_decision": quality_gate_decision.unwrap_or(Value::Null),
        "quality_gate_label": quality_gate_label.unwrap_or(Value::Null),
        "quality_gate_summary": quality_gate_summary.unwrap_or(Value::Null),
        "quality_gate_failed_metrics": quality_gate_failed_metrics,
        "source": source,
        "source_label": source_label,
        "scope": scope,
        "updated_at": Value::Null,
    }))
}

pub(crate) fn merge_active_story_repair_payloads(
    explicit_payload: Option<&Value>,
    derived_payload: Option<&Value>,
    scope: &str,
    derived_source: &str,
    derived_source_label: &str,
) -> Option<Value> {
    let explicit_payload = explicit_payload.and_then(normalize_active_story_repair_payload);
    let derived_payload = derived_payload.and_then(normalize_active_story_repair_payload);

    match (explicit_payload, derived_payload) {
        (None, None) => None,
        (Some(explicit), None) => Some(stamp_story_repair_payload(
            explicit,
            MANUAL_REQUEST_SOURCE,
            MANUAL_REQUEST_SOURCE_LABEL,
            scope,
        )),
        (None, Some(derived)) => Some(stamp_story_repair_payload(
            derived,
            derived_source,
            derived_source_label,
            scope,
        )),
        (Some(explicit), Some(derived)) => {
            let (merged_source, merged_source_label) =
                merged_story_repair_source_label(derived_source);
            let summary = value_or_fallback(explicit.get("summary"), derived.get("summary"));
            let repair_targets = merge_guidance_value_lists(
                explicit.get("repair_targets"),
                derived.get("repair_targets"),
                4,
            );
            let preserve_strengths = merge_guidance_value_lists(
                explicit.get("preserve_strengths"),
                derived.get("preserve_strengths"),
                2,
            );
            let focus_areas = merge_guidance_value_lists(
                explicit.get("focus_areas"),
                derived.get("focus_areas"),
                4,
            );

            let mut merged = derived;
            merged.insert("summary".to_string(), summary.unwrap_or(Value::Null));
            merged.insert("repair_targets".to_string(), json!(repair_targets));
            merged.insert("preserve_strengths".to_string(), json!(preserve_strengths));
            merged.insert("focus_areas".to_string(), json!(focus_areas));

            Some(stamp_story_repair_payload(
                merged,
                merged_source,
                merged_source_label,
                scope,
            ))
        }
    }
}

pub(crate) fn extract_quality_history_context(
    quality_metrics_summary: Option<&Value>,
) -> Option<Value> {
    quality_metrics_summary
        .and_then(|summary| summary.get("quality_runtime_context"))
        .filter(|context| context.is_object())
        .and_then(|context| {
            context
                .as_object()
                .filter(|value| !value.is_empty())
                .cloned()
                .map(Value::Object)
        })
}

const QUALITY_SUMMARY_METRIC_FIELDS: [(&str, &str); 3] = [
    ("overall_score", "avg_overall_score"),
    ("engagement_score", "avg_engagement_score"),
    ("coherence_score", "avg_coherence_score"),
];

pub(crate) fn build_quality_metrics_summary_state_from_history(
    history: &[Value],
    scope: &str,
) -> Option<Value> {
    let normalized_history = history
        .iter()
        .filter(|item| item.is_object())
        .cloned()
        .collect::<Vec<_>>();
    if normalized_history.is_empty() {
        return None;
    }

    let first_overall_score = normalized_history
        .first()
        .and_then(|item| item.get("overall_score"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let last_overall_score = normalized_history
        .last()
        .and_then(|item| item.get("overall_score"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let recent_history = normalized_history
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    let mut state = serde_json::Map::from_iter([
        ("scope".to_string(), json!(scope)),
        (
            "chapter_count".to_string(),
            json!(normalized_history.len() as i64),
        ),
        ("first_overall_score".to_string(), json!(first_overall_score)),
        ("last_overall_score".to_string(), json!(last_overall_score)),
        ("recent_history".to_string(), Value::Array(recent_history)),
        ("pacing_score_total".to_string(), json!(0.0)),
        ("pacing_score_count".to_string(), json!(0_i64)),
    ]);

    for (metric_key, _avg_key) in QUALITY_SUMMARY_METRIC_FIELDS {
        let total = normalized_history
            .iter()
            .filter_map(|item| item.get(metric_key).and_then(Value::as_f64))
            .sum::<f64>();
        state.insert(
            format!("{metric_key}_total"),
            json!(round_quality_metric(total)),
        );
    }

    let pacing_values = normalized_history
        .iter()
        .filter_map(|item| item.get("pacing_score").and_then(Value::as_f64))
        .collect::<Vec<_>>();
    if !pacing_values.is_empty() {
        state.insert(
            "pacing_score_total".to_string(),
            json!(round_quality_metric(pacing_values.iter().sum::<f64>())),
        );
        state.insert(
            "pacing_score_count".to_string(),
            json!(pacing_values.len() as i64),
        );
    }

    Some(Value::Object(state))
}

pub(crate) fn advance_quality_metrics_summary_state(
    summary_state: Option<&Value>,
    appended_event: &Value,
    current_history: &[Value],
    dropped_event: Option<&Value>,
    scope: &str,
) -> Option<Value> {
    if !appended_event.is_object() || current_history.is_empty() {
        return None;
    }
    let Some(existing_state) = summary_state.and_then(Value::as_object) else {
        return build_quality_metrics_summary_state_from_history(current_history, scope);
    };

    let chapter_count = current_history.len() as i64;
    let first_history_event = current_history
        .first()
        .cloned()
        .unwrap_or_else(|| appended_event.clone());
    let recent_history = current_history
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    let mut state = existing_state.clone();
    state.insert("scope".to_string(), json!(scope));
    state.insert("chapter_count".to_string(), json!(chapter_count));
    state.insert(
        "first_overall_score".to_string(),
        json!(
            first_history_event
                .get("overall_score")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
        ),
    );
    state.insert(
        "last_overall_score".to_string(),
        json!(appended_event
            .get("overall_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)),
    );
    state.insert("recent_history".to_string(), Value::Array(recent_history));

    for (metric_key, _avg_key) in QUALITY_SUMMARY_METRIC_FIELDS {
        let total_key = format!("{metric_key}_total");
        let appended = appended_event
            .get(metric_key)
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let dropped = dropped_event
            .and_then(|item| item.get(metric_key))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let current_total = state.get(&total_key).and_then(Value::as_f64).unwrap_or(0.0);
        state.insert(
            total_key,
            json!(round_quality_metric(current_total + appended - dropped)),
        );
    }

    let appended_pacing = appended_event.get("pacing_score").and_then(Value::as_f64);
    let dropped_pacing = dropped_event
        .and_then(|item| item.get("pacing_score"))
        .and_then(Value::as_f64);
    let current_pacing_total = state
        .get("pacing_score_total")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let current_pacing_count = state
        .get("pacing_score_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let next_pacing_total =
        current_pacing_total + appended_pacing.unwrap_or(0.0) - dropped_pacing.unwrap_or(0.0);
    let next_pacing_count = current_pacing_count
        + i64::from(appended_pacing.is_some())
        - i64::from(dropped_pacing.is_some());
    state.insert(
        "pacing_score_total".to_string(),
        json!(round_quality_metric(next_pacing_total)),
    );
    state.insert(
        "pacing_score_count".to_string(),
        json!(next_pacing_count.max(0)),
    );

    Some(Value::Object(state))
}

pub(crate) fn build_quality_metrics_summary_from_state(
    summary_state: Option<&Value>,
    fallback_history: &[Value],
    scope: &str,
) -> Option<Value> {
    let Some(state) = summary_state.and_then(Value::as_object) else {
        return aggregate_story_repair_quality_summaries(
            &fallback_history.iter().rev().cloned().collect::<Vec<_>>(),
            scope,
        );
    };

    let chapter_count = state
        .get("chapter_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if chapter_count <= 0 {
        return None;
    }

    let recent_history = state
        .get("recent_history")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut fallback_summary =
        aggregate_story_repair_quality_summaries(&fallback_history.iter().rev().cloned().collect::<Vec<_>>(), scope)?;
    let fallback_object = fallback_summary.as_object_mut()?;

    fallback_object.insert("chapter_count".to_string(), json!(chapter_count));
    fallback_object.insert(
        "overall_score".to_string(),
        state.get("last_overall_score").cloned().unwrap_or(Value::Null),
    );

    let first_overall_score = state
        .get("first_overall_score")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let last_overall_score = state
        .get("last_overall_score")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let delta = if chapter_count > 1 {
        round_quality_metric(last_overall_score - first_overall_score)
    } else {
        0.0
    };
    fallback_object.insert("overall_score_delta".to_string(), json!(delta));
    fallback_object.insert(
        "overall_score_trend".to_string(),
        json!(if delta >= 2.0 {
            "rising"
        } else if delta <= -2.0 {
            "falling"
        } else {
            "stable"
        }),
    );

    for (metric_key, avg_key) in QUALITY_SUMMARY_METRIC_FIELDS {
        let avg = state
            .get(&format!("{metric_key}_total"))
            .and_then(Value::as_f64)
            .map(|total| round_quality_metric(total / chapter_count as f64))
            .unwrap_or(0.0);
        fallback_object.insert(avg_key.to_string(), json!(avg));
    }

    let avg_pacing_score = match (
        state.get("pacing_score_total").and_then(Value::as_f64),
        state.get("pacing_score_count").and_then(Value::as_i64),
    ) {
        (Some(total), Some(count)) if count > 0 => Some(round_quality_metric(total / count as f64)),
        _ => None,
    };
    fallback_object.insert(
        "avg_pacing_score".to_string(),
        avg_pacing_score.map(Value::from).unwrap_or(Value::Null),
    );
    fallback_object.insert(
        "quality_runtime_context".to_string(),
        json!({
            "scope": scope,
            "recent_metrics": recent_history
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    json!({
                        "history_index": index,
                        "overall_score": item.get("overall_score").cloned().unwrap_or(Value::Null),
                        "repair_guidance": item.get("repair_guidance").cloned().unwrap_or(Value::Null),
                        "quality_gate": item.get("quality_gate").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect::<Vec<_>>()
        }),
    );

    Some(Value::Object(fallback_object.clone()))
}

pub(crate) fn aggregate_story_repair_quality_summaries(
    summaries: &[Value],
    scope: &str,
) -> Option<Value> {
    let normalized_summaries = summaries
        .iter()
        .filter(|summary| summary.is_object())
        .collect::<Vec<_>>();
    if normalized_summaries.is_empty() {
        return None;
    }

    let chapter_count = normalized_summaries.len() as i64;
    let overall_scores = normalized_summaries
        .iter()
        .filter_map(|summary| summary.get("overall_score").and_then(Value::as_f64))
        .collect::<Vec<_>>();
    let latest_overall_score = overall_scores.first().copied();
    let oldest_overall_score = overall_scores.last().copied();
    let overall_score_delta = match (latest_overall_score, oldest_overall_score) {
        (Some(latest), Some(oldest)) if overall_scores.len() > 1 => {
            Some(round_quality_metric(latest - oldest))
        }
        _ => None,
    };
    let overall_score_trend = overall_score_delta.map(|delta| {
        if delta >= 2.0 {
            "rising"
        } else if delta <= -2.0 {
            "falling"
        } else {
            "stable"
        }
    });

    let merged_repair_targets = normalized_summaries
        .iter()
        .filter_map(|summary| extract_repair_guidance_object(Some(summary)))
        .filter_map(|guidance| guidance.get("repair_targets").cloned())
        .fold(Vec::new(), |merged, value| {
            merge_guidance_lists_with_limit(merged, Some(&value), 4)
        });
    let merged_preserve_strengths = normalized_summaries
        .iter()
        .filter_map(|summary| extract_repair_guidance_object(Some(summary)))
        .filter_map(|guidance| guidance.get("preserve_strengths").cloned())
        .fold(Vec::new(), |merged, value| {
            merge_guidance_lists_with_limit(merged, Some(&value), 2)
        });
    let merged_focus_areas = normalized_summaries
        .iter()
        .filter_map(|summary| extract_repair_guidance_object(Some(summary)))
        .filter_map(|guidance| guidance.get("focus_areas").cloned())
        .fold(Vec::new(), |merged, value| {
            merge_guidance_lists_with_limit(merged, Some(&value), 4)
        });
    let recent_focus_areas = merged_focus_areas.clone();

    let latest_guidance = normalized_summaries
        .iter()
        .find_map(|summary| extract_repair_guidance_object(Some(summary)));
    let latest_quality_gate = normalized_summaries
        .iter()
        .find_map(|summary| extract_quality_gate_object(Some(summary)));

    let repair_guidance = latest_guidance.map(|guidance| {
        let mut payload = serde_json::Map::new();
        payload.insert(
            "summary".to_string(),
            guidance.get("summary").cloned().unwrap_or(Value::Null),
        );
        payload.insert(
            "repair_targets".to_string(),
            json!(merged_repair_targets),
        );
        payload.insert(
            "preserve_strengths".to_string(),
            json!(merged_preserve_strengths),
        );
        payload.insert("focus_areas".to_string(), json!(merged_focus_areas));
        if let Some(value) = guidance.get("weakest_metric_key").cloned() {
            payload.insert("weakest_metric_key".to_string(), value);
        }
        if let Some(value) = guidance.get("weakest_metric_label").cloned() {
            payload.insert("weakest_metric_label".to_string(), value);
        }
        if let Some(value) = guidance.get("weakest_metric_value").cloned() {
            payload.insert("weakest_metric_value".to_string(), value);
        }
        if let Some(value) = guidance.get("quality_stage").cloned() {
            payload.insert("quality_stage".to_string(), value);
        }
        if let Some(value) = guidance.get("quality_stage_label").cloned() {
            payload.insert("quality_stage_label".to_string(), value);
        }
        if let Some(value) = guidance.get("quality_runtime_pressure").cloned() {
            payload.insert("quality_runtime_pressure".to_string(), value);
        }
        Value::Object(payload)
    });

    let mut failed_metric_counts = serde_json::Map::new();
    let mut quality_gate_counts = serde_json::Map::new();
    let mut recent_manual_review_count = 0_i64;
    let mut recent_auto_repair_count = 0_i64;

    for summary in &normalized_summaries {
        if let Some(quality_gate) = extract_quality_gate_object(Some(summary)) {
            if let Some(decision) = quality_gate
                .get("decision")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                increment_object_count(&mut quality_gate_counts, decision);
                match decision {
                    "manual_review" => recent_manual_review_count += 1,
                    "auto_repair" | "repair" => recent_auto_repair_count += 1,
                    _ => {}
                }
            }

            if let Some(items) = quality_gate.get("failed_metrics").and_then(Value::as_array) {
                for label in items
                    .iter()
                    .filter_map(Value::as_object)
                    .filter_map(|item| item.get("label"))
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    increment_object_count(&mut failed_metric_counts, label);
                }
            }
        }
    }

    let recent_metrics = normalized_summaries
        .iter()
        .enumerate()
        .map(|(index, summary)| {
            json!({
                "history_index": index,
                "overall_score": summary.get("overall_score").cloned().unwrap_or(Value::Null),
                "repair_guidance": summary.get("repair_guidance").cloned().unwrap_or(Value::Null),
                "quality_gate": summary.get("quality_gate").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();

    let mut payload = serde_json::Map::new();
    payload.insert("chapter_count".to_string(), json!(chapter_count));
    payload.insert(
        "overall_score".to_string(),
        latest_overall_score.map(Value::from).unwrap_or(Value::Null),
    );
    if let Some(delta) = overall_score_delta {
        payload.insert("overall_score_delta".to_string(), json!(delta));
    }
    if let Some(trend) = overall_score_trend {
        payload.insert("overall_score_trend".to_string(), json!(trend));
    }
    payload.insert("recent_focus_areas".to_string(), json!(recent_focus_areas));
    payload.insert(
        "recent_failed_metric_counts".to_string(),
        Value::Object(failed_metric_counts),
    );
    payload.insert(
        "quality_gate_counts".to_string(),
        Value::Object(quality_gate_counts),
    );
    payload.insert(
        "recent_manual_review_count".to_string(),
        json!(recent_manual_review_count),
    );
    payload.insert(
        "recent_auto_repair_count".to_string(),
        json!(recent_auto_repair_count),
    );
    payload.insert(
        "quality_runtime_context".to_string(),
        json!({
            "scope": scope,
            "recent_metrics": recent_metrics,
        }),
    );
    if let Some(repair_guidance) = repair_guidance {
        payload.insert("repair_guidance".to_string(), repair_guidance);
    }
    if let Some(quality_gate) = latest_quality_gate {
        payload.insert("quality_gate".to_string(), quality_gate);
    }

    Some(Value::Object(payload))
}

pub(crate) fn quality_repair_guidance_from_quality_context(
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Option<serde_json::Map<String, Value>> {
    extract_repair_guidance_object(quality_metrics_summary)
        .or_else(|| extract_repair_guidance_object(latest_quality_metrics))
}

pub(crate) fn quality_gate_from_quality_context(
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Option<Value> {
    extract_quality_gate_object(quality_metrics_summary)
        .or_else(|| extract_quality_gate_object(latest_quality_metrics))
}

pub(crate) fn extract_repair_guidance_object(
    value: Option<&Value>,
) -> Option<serde_json::Map<String, Value>> {
    let value = value?;
    if let Some(repair_guidance) = value.get("repair_guidance").and_then(Value::as_object) {
        return Some(repair_guidance.clone());
    }

    value
        .get("raw")
        .and_then(|raw| raw.get("repair_guidance"))
        .and_then(Value::as_object)
        .cloned()
}

pub(crate) fn extract_quality_gate_object(value: Option<&Value>) -> Option<Value> {
    let value = value?;
    if let Some(quality_gate) = value.get("quality_gate") {
        return Some(quality_gate.clone());
    }

    value.get("raw").and_then(|raw| raw.get("quality_gate")).cloned()
}

pub(crate) fn normalize_guidance_items(values: &[Value], limit: usize) -> Vec<String> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for value in values {
        let Some(text) = value.as_str().map(str::trim) else {
            continue;
        };
        if text.is_empty() || !seen.insert(text.to_string()) {
            continue;
        }
        items.push(text.to_string());
        if items.len() >= limit {
            break;
        }
    }
    items
}

fn normalize_active_story_repair_payload(
    payload: &Value,
) -> Option<serde_json::Map<String, Value>> {
    let payload = payload.as_object()?;
    let summary = payload
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| json!(value));
    let repair_targets = payload
        .get("repair_targets")
        .and_then(Value::as_array)
        .map(|values| normalize_guidance_items(values, 4))
        .unwrap_or_default();
    let preserve_strengths = payload
        .get("preserve_strengths")
        .and_then(Value::as_array)
        .map(|values| normalize_guidance_items(values, 2))
        .unwrap_or_default();

    if summary.is_none() && repair_targets.is_empty() && preserve_strengths.is_empty() {
        return None;
    }

    let mut normalized = payload.clone();
    normalized.insert("summary".to_string(), summary.unwrap_or(Value::Null));
    normalized.insert("repair_targets".to_string(), json!(repair_targets));
    normalized.insert("preserve_strengths".to_string(), json!(preserve_strengths));
    Some(normalized)
}

fn merge_guidance_value_lists(
    primary: Option<&Value>,
    fallback: Option<&Value>,
    limit: usize,
) -> Vec<String> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();
    for value in [primary, fallback].into_iter().flatten() {
        let Some(items) = value.as_array() else {
            continue;
        };
        for item in items {
            let Some(text) = item.as_str().map(str::trim) else {
                continue;
            };
            if text.is_empty() || !seen.insert(text.to_string()) {
                continue;
            }
            merged.push(text.to_string());
            if merged.len() >= limit {
                return merged;
            }
        }
    }
    merged
}

fn merge_guidance_lists_with_limit(
    mut existing: Vec<String>,
    additional: Option<&Value>,
    limit: usize,
) -> Vec<String> {
    if existing.len() >= limit {
        existing.truncate(limit);
        return existing;
    }

    let mut seen = existing.iter().cloned().collect::<HashSet<_>>();
    let Some(additional_items) = additional.and_then(Value::as_array) else {
        return existing;
    };

    for item in additional_items {
        let Some(text) = item.as_str().map(str::trim) else {
            continue;
        };
        if text.is_empty() || !seen.insert(text.to_string()) {
            continue;
        }
        existing.push(text.to_string());
        if existing.len() >= limit {
            break;
        }
    }

    existing
}

fn increment_object_count(target: &mut serde_json::Map<String, Value>, key: &str) {
    let current = target.get(key).and_then(Value::as_i64).unwrap_or(0);
    target.insert(key.to_string(), json!(current + 1));
}

fn round_quality_metric(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn value_or_fallback(primary: Option<&Value>, fallback: Option<&Value>) -> Option<Value> {
    primary
        .filter(|value| match value {
            Value::Null => false,
            Value::String(text) => !text.trim().is_empty(),
            Value::Array(items) => !items.is_empty(),
            Value::Object(items) => !items.is_empty(),
            _ => true,
        })
        .cloned()
        .or_else(|| fallback.cloned())
}

fn stamp_story_repair_payload(
    mut payload: serde_json::Map<String, Value>,
    source: &str,
    source_label: &str,
    scope: &str,
) -> Value {
    payload.insert("source".to_string(), json!(source));
    payload.insert("source_label".to_string(), json!(source_label));
    payload.insert("scope".to_string(), json!(scope));
    payload.insert("updated_at".to_string(), Value::Null);
    Value::Object(payload)
}

fn merged_story_repair_source_label(derived_source: &str) -> (&'static str, &'static str) {
    match derived_source {
        CURRENT_CHAPTER_QUALITY_SOURCE => (
            MANUAL_PLUS_CURRENT_CHAPTER_QUALITY_SOURCE,
            MANUAL_PLUS_CURRENT_CHAPTER_QUALITY_SOURCE_LABEL,
        ),
        _ => (
            MANUAL_PLUS_RECENT_HISTORY_SUMMARY_SOURCE,
            MANUAL_PLUS_RECENT_HISTORY_SUMMARY_SOURCE_LABEL,
        ),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        aggregate_story_repair_quality_summaries, extract_quality_history_context,
        merge_active_story_repair_payloads,
    };

    #[test]
    fn should_merge_manual_and_recent_history_story_repair_payloads() {
        let explicit_payload = json!({
            "summary": "手工摘要",
            "repair_targets": ["手工目标", "共同目标"],
            "preserve_strengths": ["手工优点"],
            "focus_areas": ["手工焦点"]
        });
        let derived_payload = json!({
            "summary": "历史摘要",
            "repair_targets": ["共同目标", "历史目标"],
            "preserve_strengths": ["历史优点"],
            "focus_areas": ["历史焦点", "手工焦点"],
            "quality_gate_status": "warning",
            "quality_gate_summary": "近期质量波动",
            "source": "recent_history_summary",
            "source_label": "Recent history summary",
            "scope": "batch"
        });

        let merged = merge_active_story_repair_payloads(
            Some(&explicit_payload),
            Some(&derived_payload),
            "batch",
            "recent_history_summary",
            "Recent history summary",
        )
        .expect("merged payload");

        assert_eq!(merged["summary"], "手工摘要");
        assert_eq!(merged["repair_targets"], json!(["手工目标", "共同目标", "历史目标"]));
        assert_eq!(merged["preserve_strengths"], json!(["手工优点", "历史优点"]));
        assert_eq!(merged["focus_areas"], json!(["手工焦点", "历史焦点"]));
        assert_eq!(merged["quality_gate_status"], "warning");
        assert_eq!(
            merged["source"],
            "manual_plus_recent_history_summary"
        );
        assert_eq!(
            merged["source_label"],
            "Manual + recent history summary"
        );
        assert_eq!(merged["scope"], "batch");
    }

    #[test]
    fn should_extract_quality_history_context_from_summary() {
        let summary = json!({
            "summary": "ok",
            "quality_runtime_context": {
                "recent_metrics": [{"score": 91}],
                "trend": "up"
            }
        });

        let context = extract_quality_history_context(Some(&summary)).expect("history context");

        assert_eq!(
            context,
            json!({
                "recent_metrics": [{"score": 91}],
                "trend": "up"
            })
        );
    }

    #[test]
    fn should_aggregate_recent_story_repair_quality_summaries() {
        let latest = json!({
            "overall_score": 88,
            "repair_guidance": {
                "summary": "优先压缩说明段落",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["尾章钩子"],
                "focus_areas": ["pacing", "conflict"],
                "weakest_metric_key": "pacing",
                "weakest_metric_label": "节奏",
                "weakest_metric_value": 0.61
            },
            "quality_gate": {
                "decision": "repair",
                "failed_metrics": [{"label": "Pacing"}]
            }
        });
        let previous = json!({
            "overall_score": 82,
            "repair_guidance": {
                "summary": "补强角色动机",
                "repair_targets": ["强化动机", "提前冲突"],
                "preserve_strengths": ["人物口吻"],
                "focus_areas": ["character", "pacing"]
            },
            "quality_gate": {
                "decision": "manual_review",
                "failed_metrics": [{"label": "Character"}]
            }
        });

        let aggregated = aggregate_story_repair_quality_summaries(
            &[latest.clone(), previous.clone()],
            "batch",
        )
        .expect("aggregated summary");

        assert_eq!(aggregated["chapter_count"], 2);
        assert_eq!(aggregated["overall_score"], 88.0);
        assert_eq!(aggregated["overall_score_delta"], 6.0);
        assert_eq!(aggregated["overall_score_trend"], "rising");
        assert_eq!(
            aggregated["repair_guidance"]["repair_targets"],
            json!(["压缩说明", "提前冲突", "强化动机"])
        );
        assert_eq!(
            aggregated["repair_guidance"]["preserve_strengths"],
            json!(["尾章钩子", "人物口吻"])
        );
        assert_eq!(
            aggregated["repair_guidance"]["focus_areas"],
            json!(["pacing", "conflict", "character"])
        );
        assert_eq!(aggregated["quality_gate_counts"]["repair"], 1);
        assert_eq!(aggregated["quality_gate_counts"]["manual_review"], 1);
        assert_eq!(aggregated["recent_manual_review_count"], 1);
        assert_eq!(aggregated["recent_auto_repair_count"], 1);
        assert_eq!(aggregated["recent_failed_metric_counts"]["Pacing"], 1);
        assert_eq!(aggregated["recent_failed_metric_counts"]["Character"], 1);
        assert_eq!(
            aggregated["quality_runtime_context"]["recent_metrics"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
    }
}
