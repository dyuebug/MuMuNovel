use serde_json::{json, Value};

use crate::models::batch_generation_snapshot;
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::{
    advance_quality_metrics_summary_state, build_quality_metrics_summary_from_state,
    build_quality_metrics_summary_state_from_history, extract_quality_history_context,
};
use crate::services::chapter_quality_metrics_query_service::build_quality_metrics_summary_from_metrics;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GenerationQualityRuntimeContext {
    pub(crate) latest_quality_metrics: Option<Value>,
    pub(crate) quality_metrics_history: Option<Value>,
    pub(crate) quality_metrics_summary_state: Option<Value>,
    pub(crate) quality_metrics_summary: Option<Value>,
    pub(crate) quality_history_context: Option<Value>,
}

pub(crate) type BatchGenerationQualityRuntimeContext = GenerationQualityRuntimeContext;

const DEFAULT_MAX_BATCH_QUALITY_METRICS_HISTORY: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchQualitySummaryResolutionMode {
    PreferRebuilt,
    PreferExplicit,
}

pub(crate) fn build_generation_quality_runtime_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_runtime_service::quality_runtime_context_owner",
        "scope": "shared_generation_quality_runtime_context",
        "python_source_map": [],
        "rust_target_file": "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
        "rust_owner_map": [
            "resolve_generation_quality_runtime_context_from_current_quality",
            "resolve_generation_quality_runtime_context_for_seed",
            "resolve_generation_quality_runtime_context_from_persisted_sources",
            "resolve_batch_quality_runtime_context_from_persisted_sources",
            "resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state",
            "resolve_batch_quality_runtime_context_from_current_quality",
            "apply_generation_quality_runtime_context_to_payload",
            "apply_batch_quality_runtime_context_to_payload",
            "manual_review_label",
            "retryable_repair_label",
            "manual_review_label_from_quality_context",
            "manual_review_label_from_quality_context_with_retry_budget",
            "retryable_repair_label_from_quality_context_with_retry_budget",
            "normalize_terminal_quality_gate_payload",
            "normalize_terminal_quality_summary_state",
            "normalize_terminal_quality_history",
            "normalize_terminal_quality_history_context"
        ],
        "behavior_contract": {
            "quality_fields": [
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary_state",
                "quality_metrics_summary",
                "quality_history_context"
            ],
            "source_modes": [
                "current_quality",
                "seed",
                "persisted_sources",
                "snapshot_and_runtime_state",
                "terminal_normalization"
            ],
            "batch_history_limit": DEFAULT_MAX_BATCH_QUALITY_METRICS_HISTORY,
            "terminal_quality_gate_decision": "manual_review",
            "quality_gate_semantics": {
                "manual_review_decisions": ["manual_review"],
                "manual_review_phase": "quality_blocked",
                "retryable_decisions": ["auto_repair", "repair"],
                "manual_review_default_label": "需人工复核",
                "retryable_default_label": "可自动修复后重试",
                "quality_context_lookup_order": [
                    "active_story_repair_payload",
                    "quality_metrics_summary",
                    "quality_metrics_summary.quality_gate",
                    "latest_quality_metrics",
                    "latest_quality_metrics.quality_gate"
                ],
                "retry_budget_policy": {
                    "retryable_when": "max_retries >= 0 && current_retry_count < max_retries",
                    "manual_review_when_exhausted": "max_retries >= 0 && current_retry_count >= max_retries"
                }
            }
        },
        "active_consumers": [
            "chapter_single_generation_runtime_state_service",
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_read_context_service",
            "chapter_batch_generation_task_payload_base_service",
            "chapter_batch_generation_resume_task_command_service",
            "chapter_generation_runtime_service::story_repair_quality_context_owner"
        ],
        "validation_boundary": {
            "focused_test": "chapter_generation_runtime_service::quality_runtime_context_owner",
            "active_single_gateway_smoke": "chapter-single-generation-active-gateway-smoke-rust",
            "active_batch_gateway_smoke": "chapter-batch-generation-active-gateway-smoke-rust"
        },
        "rollback_boundary": {
            "source_map_policy": "shared_generation_quality_runtime_owner_no_longer_tracks_python_query_owner_source_map",
            "rollback_owner": "legacy_generation_quality_runtime_context_contract_only"
        }
    })
}

pub(crate) fn manual_review_label(failed_chapters: Option<&Value>) -> Option<String> {
    failed_chapters.and_then(latest_failed_chapter_manual_review_label)
}

pub(crate) fn retryable_repair_label(
    failed_chapters: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    failed_chapters.and_then(|items| {
        latest_failed_chapter_retryable_repair_label(items, current_retry_count, max_retries)
    })
}

pub(crate) fn manual_review_label_from_quality_context(
    active_story_repair_payload: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Option<String> {
    manual_review_label_from_payload(active_story_repair_payload)
        .or_else(|| manual_review_label_from_payload(quality_metrics_summary))
        .or_else(|| {
            quality_metrics_summary
                .and_then(|summary| summary.get("quality_gate"))
                .and_then(|payload| manual_review_label_from_payload(Some(payload)))
        })
        .or_else(|| manual_review_label_from_payload(latest_quality_metrics))
        .or_else(|| {
            latest_quality_metrics
                .and_then(|metrics| metrics.get("quality_gate"))
                .and_then(|payload| manual_review_label_from_payload(Some(payload)))
        })
}

pub(crate) fn manual_review_label_from_quality_context_with_retry_budget(
    active_story_repair_payload: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    manual_review_label_from_quality_context(
        active_story_repair_payload,
        quality_metrics_summary,
        latest_quality_metrics,
    )
    .or_else(|| {
        exhausted_auto_repair_label_from_quality_context(
            active_story_repair_payload,
            quality_metrics_summary,
            latest_quality_metrics,
            current_retry_count,
            max_retries,
        )
    })
}

pub(crate) fn retryable_repair_label_from_quality_context_with_retry_budget(
    active_story_repair_payload: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    retryable_repair_label_from_payload(
        active_story_repair_payload,
        current_retry_count,
        max_retries,
    )
    .or_else(|| {
        retryable_repair_label_from_payload(
            quality_metrics_summary,
            current_retry_count,
            max_retries,
        )
    })
    .or_else(|| {
        quality_metrics_summary
            .and_then(|summary| summary.get("quality_gate"))
            .and_then(|payload| {
                retryable_repair_label_from_payload(Some(payload), current_retry_count, max_retries)
            })
    })
    .or_else(|| {
        retryable_repair_label_from_payload(
            latest_quality_metrics,
            current_retry_count,
            max_retries,
        )
    })
    .or_else(|| {
        latest_quality_metrics
            .and_then(|metrics| metrics.get("quality_gate"))
            .and_then(|payload| {
                retryable_repair_label_from_payload(Some(payload), current_retry_count, max_retries)
            })
    })
}

fn latest_failed_chapter_manual_review_label(failed_chapters: &Value) -> Option<String> {
    let items = failed_chapters.as_array()?;
    let latest = items.iter().rev().find(|item| item.is_object())?;
    manual_review_label_from_payload(Some(latest))
}

fn latest_failed_chapter_retryable_repair_label(
    failed_chapters: &Value,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    let items = failed_chapters.as_array()?;
    let latest = items.iter().rev().find(|item| item.is_object())?;
    retryable_repair_label_from_payload(Some(latest), current_retry_count, max_retries)
}

fn manual_review_label_from_payload(value: Option<&Value>) -> Option<String> {
    let payload = value?.as_object()?;
    let failure_phase = payload
        .get("phase")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let decision = payload
        .get("quality_gate_decision")
        .and_then(Value::as_str)
        .or_else(|| payload.get("decision").and_then(Value::as_str));
    let is_manual_review = decision
        .map(str::trim)
        .is_some_and(|value| value == "manual_review")
        || failure_phase.is_some_and(|value| value == "quality_blocked");
    if !is_manual_review {
        return None;
    }

    payload
        .get("quality_gate_label")
        .or_else(|| payload.get("label"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| Some("需人工复核".to_string()))
}

fn retryable_repair_label_from_payload(
    value: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    if max_retries < 0 || current_retry_count >= max_retries {
        return None;
    }

    let payload = value?.as_object()?;
    let decision = payload
        .get("quality_gate_decision")
        .and_then(Value::as_str)
        .or_else(|| payload.get("decision").and_then(Value::as_str))
        .map(str::trim)?;
    if decision != "auto_repair" && decision != "repair" {
        return None;
    }

    payload
        .get("quality_gate_label")
        .or_else(|| payload.get("label"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| Some("可自动修复后重试".to_string()))
}

fn exhausted_auto_repair_label_from_quality_context(
    active_story_repair_payload: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    exhausted_auto_repair_label_from_payload(
        active_story_repair_payload,
        current_retry_count,
        max_retries,
    )
    .or_else(|| {
        exhausted_auto_repair_label_from_payload(
            quality_metrics_summary,
            current_retry_count,
            max_retries,
        )
    })
    .or_else(|| {
        quality_metrics_summary
            .and_then(|summary| summary.get("quality_gate"))
            .and_then(|payload| {
                exhausted_auto_repair_label_from_payload(
                    Some(payload),
                    current_retry_count,
                    max_retries,
                )
            })
    })
    .or_else(|| {
        exhausted_auto_repair_label_from_payload(
            latest_quality_metrics,
            current_retry_count,
            max_retries,
        )
    })
    .or_else(|| {
        latest_quality_metrics
            .and_then(|metrics| metrics.get("quality_gate"))
            .and_then(|payload| {
                exhausted_auto_repair_label_from_payload(
                    Some(payload),
                    current_retry_count,
                    max_retries,
                )
            })
    })
}

fn exhausted_auto_repair_label_from_payload(
    value: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    if max_retries < 0 || current_retry_count < max_retries {
        return None;
    }

    let payload = value?.as_object()?;
    let decision = payload
        .get("quality_gate_decision")
        .and_then(Value::as_str)
        .or_else(|| payload.get("decision").and_then(Value::as_str))
        .map(str::trim)?;
    if decision != "auto_repair" && decision != "repair" {
        return None;
    }

    payload
        .get("quality_gate_label")
        .or_else(|| payload.get("label"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| Some("需人工复核".to_string()))
}

pub(crate) fn append_generation_quality_metrics_history_event(
    existing_history: Option<&Value>,
    latest_quality_metrics: &Value,
    max_history: usize,
) -> (Value, Option<Value>) {
    let mut history = existing_history
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let dropped_event = if history.len() >= max_history {
        history.first().cloned()
    } else {
        None
    };
    history.push(latest_quality_metrics.clone());
    if history.len() > max_history {
        history = history.split_off(history.len() - max_history);
    }

    (Value::Array(history), dropped_event)
}

pub(crate) fn build_generation_quality_summary_from_state_or_history(
    quality_metrics_summary_state: Option<&Value>,
    quality_metrics_history: &Value,
    fallback_quality_summary: &Value,
    scope: &str,
) -> Value {
    let history = quality_metrics_history
        .as_array()
        .cloned()
        .unwrap_or_default();
    build_quality_metrics_summary_from_state(quality_metrics_summary_state, &history, scope)
        .unwrap_or_else(|| fallback_quality_summary.clone())
}

fn merge_generation_quality_history_context_impl(
    derived_quality_summary: &Value,
    fallback_quality_summary: Option<&Value>,
    merge_recent_metric_fallback_fields: bool,
) -> Value {
    let mut merged_context =
        extract_quality_history_context(Some(derived_quality_summary)).unwrap_or(Value::Null);
    let fallback_context = extract_quality_history_context(fallback_quality_summary);

    if let (Some(merged_object), Some(fallback_object)) = (
        merged_context.as_object_mut(),
        fallback_context.as_ref().and_then(Value::as_object),
    ) {
        for (key, value) in fallback_object {
            merged_object
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }

        if merge_recent_metric_fallback_fields {
            if let (Some(merged_metrics), Some(fallback_metrics)) = (
                merged_object
                    .get_mut("recent_metrics")
                    .and_then(Value::as_array_mut),
                fallback_object
                    .get("recent_metrics")
                    .and_then(Value::as_array),
            ) {
                for (merged_metric, fallback_metric) in
                    merged_metrics.iter_mut().zip(fallback_metrics.iter())
                {
                    if let (Some(merged_metric), Some(fallback_metric)) =
                        (merged_metric.as_object_mut(), fallback_metric.as_object())
                    {
                        for (key, value) in fallback_metric {
                            merged_metric
                                .entry(key.clone())
                                .or_insert_with(|| value.clone());
                        }
                    }
                }
            }
        }
    } else if merged_context.is_null() {
        merged_context = fallback_context.unwrap_or(Value::Null);
    }

    merged_context
}

pub(crate) fn merge_generation_quality_history_context(
    derived_quality_summary: &Value,
    fallback_quality_summary: Option<&Value>,
) -> Value {
    merge_generation_quality_history_context_impl(
        derived_quality_summary,
        fallback_quality_summary,
        false,
    )
}

pub(crate) fn merge_generation_quality_history_context_with_recent_metric_fallback(
    derived_quality_summary: &Value,
    fallback_quality_summary: Option<&Value>,
) -> Value {
    merge_generation_quality_history_context_impl(
        derived_quality_summary,
        fallback_quality_summary,
        true,
    )
}

pub(crate) fn build_generation_quality_metrics_history_from_summary(
    quality_summary: Option<&Value>,
) -> Option<Value> {
    let summary_repair_guidance =
        quality_summary.and_then(|summary| summary.get("repair_guidance"));
    let summary_quality_gate = quality_summary.and_then(|summary| summary.get("quality_gate"));
    let recent_metrics = quality_summary
        .and_then(Value::as_object)
        .and_then(|summary| summary.get("quality_runtime_context"))
        .and_then(Value::as_object)
        .and_then(|context| context.get("recent_metrics"))
        .and_then(Value::as_array)?;

    let mut ordered_metrics = recent_metrics
        .iter()
        .enumerate()
        .filter_map(|(index, metric)| metric.as_object().map(|metric| (index, metric)))
        .collect::<Vec<_>>();
    if ordered_metrics.is_empty() {
        return None;
    }

    let has_history_index = ordered_metrics.iter().any(|(_, metric)| {
        metric
            .get("history_index")
            .and_then(Value::as_i64)
            .is_some()
    });
    let latest_metric_index = if has_history_index {
        ordered_metrics
            .iter()
            .filter_map(|(index, metric)| {
                metric
                    .get("history_index")
                    .and_then(Value::as_i64)
                    .map(|history_index| (*index, history_index))
            })
            .max_by_key(|(_, history_index)| *history_index)
            .map(|(index, _)| index)
            .unwrap_or_else(|| ordered_metrics.last().map(|(index, _)| *index).unwrap_or(0))
    } else {
        ordered_metrics
            .first()
            .map(|(index, _)| *index)
            .unwrap_or(0)
    };

    if has_history_index {
        ordered_metrics.sort_by_key(|(index, metric)| {
            metric
                .get("history_index")
                .and_then(Value::as_i64)
                .unwrap_or(*index as i64)
        });
    } else {
        ordered_metrics.reverse();
    }

    let history = ordered_metrics
        .into_iter()
        .map(|(original_index, metric)| {
            let mut payload = serde_json::Map::new();
            if let Some(value) = metric
                .get("overall_score")
                .cloned()
                .or_else(|| metric.get("score").cloned())
            {
                payload.insert("overall_score".to_string(), value);
            }
            if let Some(value) = metric.get("repair_guidance").cloned().or_else(|| {
                (original_index == latest_metric_index)
                    .then(|| summary_repair_guidance.cloned())
                    .flatten()
            }) {
                payload.insert("repair_guidance".to_string(), value);
            }
            if let Some(value) = metric.get("quality_gate").cloned().or_else(|| {
                (original_index == latest_metric_index)
                    .then(|| summary_quality_gate.cloned())
                    .flatten()
            }) {
                payload.insert("quality_gate".to_string(), value);
            }
            Value::Object(payload)
        })
        .filter(|item| item.as_object().is_some_and(|object| !object.is_empty()))
        .collect::<Vec<_>>();

    (!history.is_empty()).then_some(Value::Array(history))
}

pub(crate) fn apply_generation_quality_runtime_context_to_payload(
    payload: &mut serde_json::Map<String, Value>,
    quality_runtime_context: GenerationQualityRuntimeContext,
    latest_quality_metrics_fallback: Option<Value>,
    quality_summary_fallback: Option<Value>,
    quality_history_fallback: Option<Value>,
) {
    if let Some(latest_quality_metrics) = quality_runtime_context
        .latest_quality_metrics
        .or(latest_quality_metrics_fallback)
    {
        payload.insert("latest_quality_metrics".to_string(), latest_quality_metrics);
    }
    payload.insert(
        "quality_metrics_summary".to_string(),
        quality_runtime_context
            .quality_metrics_summary
            .or(quality_summary_fallback)
            .unwrap_or(Value::Null),
    );
    if let Some(quality_metrics_history) = quality_runtime_context
        .quality_metrics_history
        .or(quality_history_fallback)
    {
        payload.insert(
            "quality_metrics_history".to_string(),
            quality_metrics_history,
        );
    }
    if let Some(quality_metrics_summary_state) =
        quality_runtime_context.quality_metrics_summary_state
    {
        payload.insert(
            "quality_metrics_summary_state".to_string(),
            quality_metrics_summary_state,
        );
    }
    payload.insert(
        "quality_history_context".to_string(),
        quality_runtime_context
            .quality_history_context
            .unwrap_or(Value::Null),
    );
}

pub(crate) fn apply_generation_quality_runtime_context_from_current_quality(
    payload: &mut serde_json::Map<String, Value>,
    scope: &str,
    existing_runtime_state: Option<&Value>,
    existing_quality_metrics_history: Option<&Value>,
    latest_quality_metrics: &Value,
    max_history: usize,
) {
    let existing_quality_metrics_summary_state = existing_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("quality_metrics_summary_state"));
    let fallback_quality_summary =
        build_quality_metrics_summary_from_metrics(latest_quality_metrics, true);
    let resolved_quality_context = resolve_generation_quality_runtime_context_from_current_quality(
        scope,
        existing_quality_metrics_summary_state,
        existing_quality_metrics_history,
        latest_quality_metrics,
        &fallback_quality_summary,
        max_history,
    );
    apply_generation_quality_runtime_context_to_payload(
        payload,
        resolved_quality_context,
        Some(latest_quality_metrics.clone()),
        Some(fallback_quality_summary),
        Some(Value::Array(vec![latest_quality_metrics.clone()])),
    );
}

pub(crate) fn resolve_generation_quality_runtime_context_from_current_quality(
    scope: &str,
    existing_quality_metrics_summary_state: Option<&Value>,
    existing_quality_metrics_history: Option<&Value>,
    latest_quality_metrics: &Value,
    fallback_quality_summary: &Value,
    max_history: usize,
) -> GenerationQualityRuntimeContext {
    let (quality_metrics_history, dropped_event) = append_generation_quality_metrics_history_event(
        existing_quality_metrics_history,
        latest_quality_metrics,
        max_history,
    );
    let history = quality_metrics_history
        .as_array()
        .cloned()
        .unwrap_or_default();
    let quality_metrics_summary_state = advance_quality_metrics_summary_state(
        existing_quality_metrics_summary_state,
        latest_quality_metrics,
        &history,
        dropped_event.as_ref(),
        scope,
    )
    .or_else(|| build_quality_metrics_summary_state_from_history(&history, scope));
    let quality_metrics_summary = build_generation_quality_summary_from_state_or_history(
        quality_metrics_summary_state.as_ref(),
        &quality_metrics_history,
        fallback_quality_summary,
        scope,
    );
    let quality_history_context = merge_generation_quality_history_context(
        &quality_metrics_summary,
        Some(fallback_quality_summary),
    );

    GenerationQualityRuntimeContext {
        latest_quality_metrics: Some(latest_quality_metrics.clone()),
        quality_metrics_history: Some(quality_metrics_history),
        quality_metrics_summary_state,
        quality_metrics_summary: Some(quality_metrics_summary),
        quality_history_context: Some(quality_history_context),
    }
}

pub(crate) fn resolve_generation_quality_runtime_context_for_seed(
    scope: &str,
    existing_quality_metrics_summary_state: Option<&Value>,
    existing_quality_metrics_history: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    fallback_quality_summary: Option<&Value>,
    _max_history: usize,
) -> GenerationQualityRuntimeContext {
    match latest_quality_metrics {
        Some(latest_quality_metrics) => {
            let derived_quality_metrics_history = existing_quality_metrics_history
                .cloned()
                .filter(Value::is_array)
                .or_else(|| Some(Value::Array(vec![latest_quality_metrics.clone()])));
            let history = derived_quality_metrics_history
                .as_ref()
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_else(|| vec![latest_quality_metrics.clone()]);
            let derived_quality_metrics_summary_state = existing_quality_metrics_summary_state
                .cloned()
                .filter(Value::is_object)
                .or_else(|| build_quality_metrics_summary_state_from_history(&history, scope));
            let metrics_summary_fallback =
                build_quality_metrics_summary_from_metrics(latest_quality_metrics, true);
            let quality_metrics_summary = build_quality_metrics_summary_from_state(
                derived_quality_metrics_summary_state.as_ref(),
                &history,
                scope,
            );
            let mut resolved = GenerationQualityRuntimeContext {
                latest_quality_metrics: Some(latest_quality_metrics.clone()),
                quality_metrics_history: derived_quality_metrics_history,
                quality_metrics_summary_state: derived_quality_metrics_summary_state,
                quality_metrics_summary,
                quality_history_context: None,
            };
            let resolved_quality_summary = resolved
                .quality_metrics_summary
                .clone()
                .or_else(|| fallback_quality_summary.cloned())
                .unwrap_or(metrics_summary_fallback);
            resolved.quality_metrics_summary = Some(resolved_quality_summary.clone());
            resolved.quality_history_context = Some(merge_generation_quality_history_context(
                &resolved_quality_summary,
                fallback_quality_summary,
            ));
            resolved
        }
        None => {
            let derived_quality_metrics_history = existing_quality_metrics_history
                .cloned()
                .filter(Value::is_array)
                .or_else(|| {
                    build_generation_quality_metrics_history_from_summary(fallback_quality_summary)
                });
            let derived_quality_metrics_summary_state = existing_quality_metrics_summary_state
                .cloned()
                .filter(Value::is_object)
                .or_else(|| {
                    derived_quality_metrics_history
                        .as_ref()
                        .and_then(Value::as_array)
                        .and_then(|history| {
                            build_quality_metrics_summary_state_from_history(history, scope)
                        })
                });
            let derived_latest_quality_metrics = derived_quality_metrics_history
                .as_ref()
                .and_then(Value::as_array)
                .and_then(|history| history.last().cloned());
            let empty_summary = Value::Null;
            let rebuilt_quality_metrics_summary =
                derived_quality_metrics_history.as_ref().map(|history| {
                    build_generation_quality_summary_from_state_or_history(
                        derived_quality_metrics_summary_state.as_ref(),
                        history,
                        fallback_quality_summary.unwrap_or(&empty_summary),
                        scope,
                    )
                });
            let quality_metrics_summary = fallback_quality_summary
                .cloned()
                .or(rebuilt_quality_metrics_summary);
            let quality_history_context = quality_metrics_summary
                .as_ref()
                .map(|summary| {
                    merge_generation_quality_history_context(summary, fallback_quality_summary)
                })
                .unwrap_or(Value::Null);

            GenerationQualityRuntimeContext {
                latest_quality_metrics: derived_latest_quality_metrics,
                quality_metrics_history: derived_quality_metrics_history,
                quality_metrics_summary_state: derived_quality_metrics_summary_state,
                quality_metrics_summary,
                quality_history_context: Some(quality_history_context),
            }
        }
    }
}

pub(crate) fn resolve_generation_quality_runtime_context_from_persisted_sources(
    scope: &str,
    explicit_latest_quality_metrics: Option<&Value>,
    explicit_quality_metrics_history: Option<&Value>,
    explicit_quality_metrics_summary_state: Option<&Value>,
    explicit_quality_metrics_summary: Option<&Value>,
) -> GenerationQualityRuntimeContext {
    let quality_metrics_history = explicit_quality_metrics_history
        .cloned()
        .filter(Value::is_array)
        .or_else(|| {
            build_generation_quality_metrics_history_from_summary(explicit_quality_metrics_summary)
        });
    let quality_metrics_summary_state = explicit_quality_metrics_summary_state
        .cloned()
        .filter(Value::is_object)
        .or_else(|| {
            quality_metrics_history
                .as_ref()
                .and_then(Value::as_array)
                .and_then(|history| {
                    build_quality_metrics_summary_state_from_history(history, scope)
                })
        });
    let latest_quality_metrics = explicit_latest_quality_metrics.cloned().or_else(|| {
        quality_metrics_history
            .as_ref()
            .and_then(Value::as_array)
            .and_then(|history| history.last().cloned())
    });
    let rebuilt_quality_metrics_summary = quality_metrics_history
        .as_ref()
        .and_then(Value::as_array)
        .and_then(|history| {
            build_quality_metrics_summary_from_state(
                quality_metrics_summary_state.as_ref(),
                history,
                scope,
            )
        });
    let quality_metrics_summary = explicit_quality_metrics_summary
        .cloned()
        .or(rebuilt_quality_metrics_summary);
    let quality_history_context = quality_metrics_summary
        .as_ref()
        .map(|summary| {
            merge_generation_quality_history_context(summary, explicit_quality_metrics_summary)
        })
        .filter(|context| !context.is_null());

    GenerationQualityRuntimeContext {
        latest_quality_metrics,
        quality_metrics_history,
        quality_metrics_summary_state,
        quality_metrics_summary,
        quality_history_context,
    }
}

pub(crate) fn build_batch_quality_metrics_history_from_summary(
    quality_summary: Option<&Value>,
) -> Option<Value> {
    build_generation_quality_metrics_history_from_summary(quality_summary)
}

fn batch_resolve_quality_metrics_summary_state(
    explicit_quality_metrics_summary_state: Option<&Value>,
    quality_metrics_history: Option<&Value>,
) -> Option<Value> {
    explicit_quality_metrics_summary_state
        .cloned()
        .filter(Value::is_object)
        .or_else(|| {
            quality_metrics_history
                .and_then(Value::as_array)
                .and_then(|history| {
                    build_quality_metrics_summary_state_from_history(history, "batch")
                })
        })
}

fn batch_resolve_latest_quality_metrics(
    explicit_latest_quality_metrics: Option<&Value>,
    quality_metrics_history: Option<&Value>,
) -> Option<Value> {
    explicit_latest_quality_metrics.cloned().or_else(|| {
        quality_metrics_history
            .and_then(Value::as_array)
            .and_then(|history| history.last().cloned())
    })
}

fn batch_latest_quality_metrics_from_snapshot_or_runtime_state(
    snapshot: Option<&batch_generation_snapshot::Model>,
    workflow_runtime_state: Option<&Value>,
) -> Option<Value> {
    snapshot
        .and_then(|item| item.latest_quality_metrics.clone())
        .or_else(|| {
            workflow_runtime_state
                .and_then(Value::as_object)
                .and_then(|state| state.get("latest_quality_metrics"))
                .cloned()
        })
}

fn batch_quality_metrics_summary_from_snapshot_or_runtime_state(
    snapshot: Option<&batch_generation_snapshot::Model>,
    workflow_runtime_state: Option<&Value>,
) -> Option<Value> {
    snapshot
        .and_then(|item| item.quality_metrics_summary.clone())
        .or_else(|| {
            workflow_runtime_state
                .and_then(Value::as_object)
                .and_then(|state| state.get("quality_metrics_summary"))
                .cloned()
        })
}

fn batch_quality_metrics_history_from_snapshot_or_runtime_state(
    snapshot: Option<&batch_generation_snapshot::Model>,
    workflow_runtime_state: Option<&Value>,
) -> Option<Value> {
    snapshot
        .and_then(|item| item.quality_metrics_history.clone())
        .or_else(|| {
            workflow_runtime_state
                .and_then(Value::as_object)
                .and_then(|state| state.get("quality_metrics_history"))
                .cloned()
        })
}

fn batch_quality_metrics_summary_state_from_runtime_state_or_history(
    workflow_runtime_state: Option<&Value>,
    quality_metrics_history: Option<&Value>,
) -> Option<Value> {
    workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("quality_metrics_summary_state"))
        .cloned()
        .or_else(|| {
            quality_metrics_history
                .and_then(Value::as_array)
                .and_then(|history| {
                    build_quality_metrics_summary_state_from_history(history, "batch")
                })
        })
}

fn batch_rebuild_quality_metrics_summary_from_history(
    quality_metrics_summary_state: Option<&Value>,
    quality_metrics_history: Option<&Value>,
) -> Option<Value> {
    quality_metrics_history.and_then(|history| {
        let rebuilt = build_generation_quality_summary_from_state_or_history(
            quality_metrics_summary_state,
            history,
            &Value::Null,
            "batch",
        );
        (!rebuilt.is_null()).then_some(rebuilt)
    })
}

fn batch_resolve_quality_runtime_context(
    explicit_latest_quality_metrics: Option<&Value>,
    explicit_quality_metrics_history: Option<&Value>,
    explicit_quality_metrics_summary_state: Option<&Value>,
    explicit_quality_metrics_summary: Option<&Value>,
    summary_resolution_mode: BatchQualitySummaryResolutionMode,
    merge_fallback_history_context: bool,
) -> BatchGenerationQualityRuntimeContext {
    let quality_metrics_history = explicit_quality_metrics_history
        .cloned()
        .filter(Value::is_array)
        .or_else(|| {
            build_batch_quality_metrics_history_from_summary(explicit_quality_metrics_summary)
        });
    let quality_metrics_summary_state = batch_resolve_quality_metrics_summary_state(
        explicit_quality_metrics_summary_state,
        quality_metrics_history.as_ref(),
    );
    let rebuilt_quality_metrics_summary = batch_rebuild_quality_metrics_summary_from_history(
        quality_metrics_summary_state.as_ref(),
        quality_metrics_history.as_ref(),
    );
    let quality_metrics_summary = match summary_resolution_mode {
        BatchQualitySummaryResolutionMode::PreferRebuilt => {
            rebuilt_quality_metrics_summary.or_else(|| explicit_quality_metrics_summary.cloned())
        }
        BatchQualitySummaryResolutionMode::PreferExplicit => explicit_quality_metrics_summary
            .cloned()
            .or(rebuilt_quality_metrics_summary),
    };
    let latest_quality_metrics = batch_resolve_latest_quality_metrics(
        explicit_latest_quality_metrics,
        quality_metrics_history.as_ref(),
    );
    let quality_history_context = quality_metrics_summary
        .as_ref()
        .map(|summary| {
            if merge_fallback_history_context {
                merge_generation_quality_history_context_with_recent_metric_fallback(
                    summary,
                    explicit_quality_metrics_summary,
                )
            } else {
                extract_quality_history_context(Some(summary)).unwrap_or(Value::Null)
            }
        })
        .filter(|context| !context.is_null());

    BatchGenerationQualityRuntimeContext {
        latest_quality_metrics,
        quality_metrics_history,
        quality_metrics_summary_state,
        quality_metrics_summary,
        quality_history_context,
    }
}

pub(crate) fn resolve_batch_quality_runtime_context_for_startup_seed(
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> BatchGenerationQualityRuntimeContext {
    batch_resolve_quality_runtime_context(
        latest_quality_metrics,
        None,
        None,
        quality_metrics_summary,
        BatchQualitySummaryResolutionMode::PreferRebuilt,
        true,
    )
}

pub(crate) fn resolve_batch_quality_runtime_context_from_persisted_sources(
    latest_quality_metrics: Option<&Value>,
    quality_metrics_history: Option<&Value>,
    quality_metrics_summary_state: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
) -> BatchGenerationQualityRuntimeContext {
    batch_resolve_quality_runtime_context(
        latest_quality_metrics,
        quality_metrics_history,
        quality_metrics_summary_state,
        quality_metrics_summary,
        BatchQualitySummaryResolutionMode::PreferExplicit,
        false,
    )
}

pub(crate) fn resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state(
    snapshot: Option<&batch_generation_snapshot::Model>,
    workflow_runtime_state: Option<&Value>,
) -> BatchGenerationQualityRuntimeContext {
    let latest_quality_metrics = batch_latest_quality_metrics_from_snapshot_or_runtime_state(
        snapshot,
        workflow_runtime_state,
    );
    let quality_metrics_history = batch_quality_metrics_history_from_snapshot_or_runtime_state(
        snapshot,
        workflow_runtime_state,
    );
    let quality_metrics_summary_state =
        batch_quality_metrics_summary_state_from_runtime_state_or_history(
            workflow_runtime_state,
            quality_metrics_history.as_ref(),
        );
    let quality_metrics_summary = batch_quality_metrics_summary_from_snapshot_or_runtime_state(
        snapshot,
        workflow_runtime_state,
    );

    resolve_batch_quality_runtime_context_from_persisted_sources(
        latest_quality_metrics.as_ref(),
        quality_metrics_history.as_ref(),
        quality_metrics_summary_state.as_ref(),
        quality_metrics_summary.as_ref(),
    )
}

pub(crate) fn apply_batch_quality_runtime_context_to_payload(
    payload: &mut serde_json::Map<String, Value>,
    quality_runtime_context: BatchGenerationQualityRuntimeContext,
    quality_summary_fallback: Option<Value>,
) {
    payload.insert(
        "quality_metrics_summary".to_string(),
        quality_runtime_context
            .quality_metrics_summary
            .or(quality_summary_fallback)
            .unwrap_or(Value::Null),
    );
    if let Some(latest_quality_metrics) = quality_runtime_context.latest_quality_metrics {
        payload.insert("latest_quality_metrics".to_string(), latest_quality_metrics);
    }
    if let Some(quality_metrics_history) = quality_runtime_context.quality_metrics_history {
        payload.insert(
            "quality_metrics_history".to_string(),
            quality_metrics_history,
        );
    }
    if let Some(quality_metrics_summary_state) =
        quality_runtime_context.quality_metrics_summary_state
    {
        payload.insert(
            "quality_metrics_summary_state".to_string(),
            quality_metrics_summary_state,
        );
    }
    payload.insert(
        "quality_history_context".to_string(),
        quality_runtime_context
            .quality_history_context
            .unwrap_or(Value::Null),
    );
}

pub(crate) fn append_batch_quality_metrics_history_event(
    existing_quality_metrics_history: Option<&Value>,
    latest_quality_metrics: &Value,
    max_history: usize,
) -> (Value, Option<Value>) {
    append_generation_quality_metrics_history_event(
        existing_quality_metrics_history,
        latest_quality_metrics,
        max_history,
    )
}

fn build_batch_quality_summary_from_state_or_history(
    quality_metrics_summary_state: Option<&Value>,
    quality_metrics_history: &Value,
    fallback_quality_summary: &Value,
) -> Value {
    build_generation_quality_summary_from_state_or_history(
        quality_metrics_summary_state,
        quality_metrics_history,
        fallback_quality_summary,
        "batch",
    )
}

pub(crate) fn resolve_batch_quality_runtime_context_preserving_existing_quality_state(
    existing_quality_metrics_summary_state: Option<&Value>,
    existing_quality_metrics_history: Option<&Value>,
    existing_quality_summary: Option<&Value>,
    refreshed_quality_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> BatchGenerationQualityRuntimeContext {
    let quality_metrics_history = existing_quality_metrics_history
        .cloned()
        .filter(Value::is_array);
    let quality_metrics_summary_state = existing_quality_metrics_summary_state
        .cloned()
        .filter(Value::is_object)
        .or_else(|| {
            quality_metrics_history
                .as_ref()
                .and_then(Value::as_array)
                .map(|items| items.to_vec())
                .and_then(|history| {
                    build_quality_metrics_summary_state_from_history(&history, "batch")
                })
        });
    let fallback_quality_summary = refreshed_quality_summary
        .or(existing_quality_summary)
        .cloned()
        .unwrap_or(Value::Null);
    let quality_metrics_summary = quality_metrics_history
        .as_ref()
        .map(|history| {
            build_batch_quality_summary_from_state_or_history(
                quality_metrics_summary_state.as_ref(),
                history,
                &fallback_quality_summary,
            )
        })
        .or_else(|| refreshed_quality_summary.cloned())
        .or_else(|| existing_quality_summary.cloned());
    let quality_history_context = quality_metrics_summary
        .as_ref()
        .and_then(|summary| extract_quality_history_context(Some(summary)));

    BatchGenerationQualityRuntimeContext {
        latest_quality_metrics: latest_quality_metrics.cloned(),
        quality_metrics_history,
        quality_metrics_summary_state,
        quality_metrics_summary,
        quality_history_context,
    }
}

pub(crate) fn resolve_batch_quality_runtime_context_from_current_quality(
    existing_quality_metrics_summary_state: Option<&Value>,
    existing_quality_metrics_history: Option<&Value>,
    quality_summary: &Value,
    latest_quality_metrics: Option<&Value>,
) -> BatchGenerationQualityRuntimeContext {
    let quality_metrics_history_with_drop = latest_quality_metrics.map(|latest_quality_metrics| {
        append_batch_quality_metrics_history_event(
            existing_quality_metrics_history,
            latest_quality_metrics,
            DEFAULT_MAX_BATCH_QUALITY_METRICS_HISTORY,
        )
    });
    let quality_metrics_summary_state = latest_quality_metrics.and_then(|latest_quality_metrics| {
        let history = quality_metrics_history_with_drop
            .as_ref()
            .and_then(|(history, _)| history.as_array())
            .cloned()
            .unwrap_or_default();
        let dropped_event = quality_metrics_history_with_drop
            .as_ref()
            .and_then(|(_, dropped_event)| dropped_event.as_ref());
        advance_quality_metrics_summary_state(
            existing_quality_metrics_summary_state,
            latest_quality_metrics,
            &history,
            dropped_event,
            "batch",
        )
        .or_else(|| build_quality_metrics_summary_state_from_history(&history, "batch"))
    });
    let quality_metrics_summary = quality_metrics_history_with_drop
        .as_ref()
        .map(|(history, _)| {
            build_batch_quality_summary_from_state_or_history(
                quality_metrics_summary_state.as_ref(),
                history,
                quality_summary,
            )
        })
        .or_else(|| Some(quality_summary.clone()));
    let quality_metrics_history = quality_metrics_history_with_drop
        .map(|(quality_metrics_history, _)| quality_metrics_history);
    let quality_history_context = quality_metrics_summary
        .as_ref()
        .and_then(|summary| extract_quality_history_context(Some(summary)));

    BatchGenerationQualityRuntimeContext {
        latest_quality_metrics: latest_quality_metrics.cloned(),
        quality_metrics_history,
        quality_metrics_summary_state,
        quality_metrics_summary,
        quality_history_context,
    }
}

fn increment_generation_quality_gate_terminal_counts(
    quality_gate_counts: &mut serde_json::Map<String, Value>,
    recent_manual_review_count: &mut i64,
    recent_auto_repair_count: &mut i64,
    decision: Option<&str>,
) {
    let Some(decision) = decision.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };

    let current = quality_gate_counts
        .get(decision)
        .and_then(Value::as_i64)
        .unwrap_or(0);
    quality_gate_counts.insert(decision.to_string(), Value::from(current + 1));

    match decision {
        "manual_review" => *recent_manual_review_count += 1,
        "auto_repair" | "repair" => *recent_auto_repair_count += 1,
        _ => {}
    }
}

pub(crate) fn normalize_terminal_quality_gate_payload(
    payload: &mut Value,
    manual_review_label: &str,
) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };

    let quality_gate = object
        .entry("quality_gate".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if let Some(gate_object) = quality_gate.as_object_mut() {
        gate_object.insert("status".to_string(), Value::from("failed"));
        gate_object.insert("decision".to_string(), Value::from("manual_review"));
        gate_object.insert("label".to_string(), Value::from(manual_review_label));
    }
}

pub(crate) fn normalize_terminal_quality_summary_state(
    summary_state: &mut Value,
    manual_review_label: &str,
) {
    let Some(object) = summary_state.as_object_mut() else {
        return;
    };
    let Some(recent_history) = object
        .get_mut("recent_history")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let Some(last_metric) = recent_history.last_mut() else {
        return;
    };
    normalize_terminal_quality_gate_payload(last_metric, manual_review_label);
}

pub(crate) fn normalize_terminal_quality_history(
    quality_metrics_history: &mut Value,
    manual_review_label: &str,
) {
    let Some(last_metric) = quality_metrics_history
        .as_array_mut()
        .and_then(|history| history.last_mut())
    else {
        return;
    };
    normalize_terminal_quality_gate_payload(last_metric, manual_review_label);
}

pub(crate) fn normalize_terminal_quality_history_context(
    quality_history_context: &mut Value,
    manual_review_label: &str,
) {
    let mut quality_gate_counts = serde_json::Map::new();
    let mut recent_manual_review_count = 0_i64;
    let mut recent_auto_repair_count = 0_i64;
    if let Some(recent_metrics) = quality_history_context
        .get_mut("recent_metrics")
        .and_then(Value::as_array_mut)
    {
        for metric in recent_metrics {
            normalize_terminal_quality_gate_payload(metric, manual_review_label);
            increment_generation_quality_gate_terminal_counts(
                &mut quality_gate_counts,
                &mut recent_manual_review_count,
                &mut recent_auto_repair_count,
                metric
                    .get("quality_gate")
                    .and_then(Value::as_object)
                    .and_then(|gate| gate.get("decision"))
                    .and_then(Value::as_str),
            );
        }
    }
    if let Some(object) = quality_history_context.as_object_mut() {
        object.insert(
            "quality_gate_counts".to_string(),
            Value::Object(quality_gate_counts),
        );
        object.insert(
            "recent_manual_review_count".to_string(),
            Value::from(recent_manual_review_count),
        );
        object.insert(
            "recent_auto_repair_count".to_string(),
            Value::from(recent_auto_repair_count),
        );
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        append_generation_quality_metrics_history_event,
        apply_generation_quality_runtime_context_from_current_quality,
        build_generation_quality_metrics_history_from_summary,
        build_generation_quality_runtime_owner_contract, normalize_terminal_quality_gate_payload,
        normalize_terminal_quality_history, normalize_terminal_quality_history_context,
        normalize_terminal_quality_summary_state,
        resolve_generation_quality_runtime_context_for_seed,
        resolve_generation_quality_runtime_context_from_current_quality,
        resolve_generation_quality_runtime_context_from_persisted_sources,
    };

    #[test]
    fn should_append_generation_quality_history_with_bounded_oldest_to_latest_order() {
        let existing_history = json!([
            {"overall_score": 78},
            {"overall_score": 82}
        ]);
        let latest_quality_metrics = json!({"overall_score": 86});

        let (history, dropped_event) = append_generation_quality_metrics_history_event(
            Some(&existing_history),
            &latest_quality_metrics,
            2,
        );

        assert_eq!(
            history,
            json!([
                {"overall_score": 82},
                {"overall_score": 86}
            ])
        );
        assert_eq!(dropped_event, Some(json!({"overall_score": 78})));
    }

    #[test]
    fn should_resolve_generation_quality_runtime_context_from_current_quality() {
        let existing_summary_state = json!({
            "scope": "chapter",
            "chapter_count": 1,
            "first_overall_score": 78.0,
            "last_overall_score": 78.0,
            "overall_score_total": 78.0,
            "engagement_score_total": 0.0,
            "coherence_score_total": 0.0,
            "pacing_score_total": 0.0,
            "pacing_score_count": 0,
            "recent_history": [{
                "overall_score": 78,
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议继续修复"
                }
            }]
        });
        let existing_history = json!([{
            "overall_score": 78,
            "quality_gate": {
                "status": "warning",
                "decision": "auto_repair",
                "label": "建议继续修复"
            }
        }]);
        let latest_quality_metrics = json!({
            "overall_score": 83,
            "quality_gate": {
                "status": "failed",
                "decision": "manual_review",
                "label": "需要人工复核"
            },
            "repair_guidance": {
                "summary": "优先修复当前冲突密度"
            },
            "quality_runtime_context": {
                "scope": "chapter",
                "source": "analysis"
            }
        });
        let fallback_quality_summary = json!({
            "overall_score": 83,
            "repair_guidance": {
                "summary": "优先修复当前冲突密度"
            },
            "quality_gate": {
                "status": "failed",
                "decision": "manual_review",
                "label": "需要人工复核"
            },
            "quality_runtime_context": {
                "scope": "chapter",
                "source": "analysis"
            }
        });

        let resolved = resolve_generation_quality_runtime_context_from_current_quality(
            "chapter",
            Some(&existing_summary_state),
            Some(&existing_history),
            &latest_quality_metrics,
            &fallback_quality_summary,
            20,
        );

        assert_eq!(
            resolved.latest_quality_metrics,
            Some(latest_quality_metrics.clone())
        );
        assert_eq!(
            resolved
                .quality_metrics_history
                .as_ref()
                .and_then(Value::as_array)
                .map(|history| history.len()),
            Some(2)
        );
        assert_eq!(
            resolved
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|state| state.get("chapter_count")),
            Some(&json!(2))
        );
        assert_eq!(
            resolved
                .quality_metrics_summary
                .as_ref()
                .and_then(|summary| summary.get("quality_gate"))
                .and_then(|gate| gate.get("decision")),
            Some(&json!("manual_review"))
        );
        assert_eq!(
            resolved
                .quality_history_context
                .as_ref()
                .and_then(|context| context.get("source")),
            Some(&json!("analysis"))
        );
    }

    #[test]
    fn should_publish_generation_quality_runtime_owner_contract() {
        let contract = build_generation_quality_runtime_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_generation_runtime_service::quality_runtime_context_owner"
        );
        assert_eq!(
            contract["scope"],
            "shared_generation_quality_runtime_context"
        );
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["rust_owner_map"][0],
            "resolve_generation_quality_runtime_context_from_current_quality"
        );
        assert_eq!(
            contract["behavior_contract"]["quality_fields"],
            json!([
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary_state",
                "quality_metrics_summary",
                "quality_history_context"
            ])
        );
        assert_eq!(contract["behavior_contract"]["batch_history_limit"], 20);
        assert_eq!(
            contract["behavior_contract"]["terminal_quality_gate_decision"],
            "manual_review"
        );
        assert_eq!(
            contract["active_consumers"][2],
            "chapter_batch_generation_runtime_state_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "shared_generation_quality_runtime_owner_no_longer_tracks_python_query_owner_source_map"
        );
        assert_eq!(
            contract["validation_boundary"]["active_single_gateway_smoke"],
            "chapter-single-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["validation_boundary"]["active_batch_gateway_smoke"],
            "chapter-batch-generation-active-gateway-smoke-rust"
        );
    }

    #[test]
    fn should_apply_generation_quality_runtime_context_from_current_quality() {
        let mut payload = serde_json::Map::new();
        let existing_runtime_state = json!({
            "quality_metrics_summary_state": {
                "scope": "chapter",
                "chapter_count": 1,
                "first_overall_score": 78.0,
                "last_overall_score": 78.0,
                "overall_score_total": 78.0,
                "engagement_score_total": 0.0,
                "coherence_score_total": 0.0,
                "pacing_score_total": 0.0,
                "pacing_score_count": 0,
                "recent_history": [{
                    "overall_score": 78,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议继续修复"
                    }
                }]
            }
        });
        let existing_history = json!([{
            "overall_score": 78,
            "quality_gate": {
                "status": "warning",
                "decision": "auto_repair",
                "label": "建议继续修复"
            }
        }]);
        let latest_quality_metrics = json!({
            "overall_score": 83,
            "quality_gate": {
                "status": "failed",
                "decision": "manual_review",
                "label": "需要人工复核"
            },
            "repair_guidance": {
                "summary": "优先修复当前冲突密度"
            },
            "quality_runtime_context": {
                "scope": "chapter",
                "source": "analysis"
            }
        });

        apply_generation_quality_runtime_context_from_current_quality(
            &mut payload,
            "chapter",
            Some(&existing_runtime_state),
            Some(&existing_history),
            &latest_quality_metrics,
            20,
        );

        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 83);
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 78);
        assert_eq!(payload["quality_metrics_history"][1]["overall_score"], 83);
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_metrics_summary"]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(payload["quality_history_context"]["source"], "analysis");
    }

    #[test]
    fn should_resolve_generation_quality_runtime_context_from_persisted_sources_for_chapter_scope()
    {
        let resolved = resolve_generation_quality_runtime_context_from_persisted_sources(
            "chapter",
            None,
            Some(&json!([
                {
                    "overall_score": 88,
                    "pacing_score": 8.3,
                    "quality_gate": {
                        "status": "passed",
                        "decision": "continue",
                        "label": "通过"
                    }
                },
                {
                    "overall_score": 84,
                    "pacing_score": 7.5,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议修复"
                    }
                }
            ])),
            None,
            None,
        );

        assert_eq!(
            resolved
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|state| state.get("scope")),
            Some(&json!("chapter"))
        );
        assert_eq!(
            resolved
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|state| state.get("chapter_count")),
            Some(&json!(2))
        );
        assert_eq!(
            resolved
                .quality_metrics_summary
                .as_ref()
                .and_then(|summary| summary.get("chapter_count")),
            Some(&json!(2))
        );
        assert_eq!(
            resolved
                .quality_history_context
                .as_ref()
                .and_then(|context| context.get("scope")),
            Some(&json!("chapter"))
        );
    }

    #[test]
    fn should_resolve_generation_quality_runtime_context_from_persisted_summary_only() {
        let summary = json!({
            "overall_score": 84,
            "repair_guidance": {
                "summary": "当前章需要压缩说明"
            },
            "quality_gate": {
                "decision": "auto_repair",
                "label": "建议修复"
            },
            "quality_runtime_context": {
                "scope": "chapter",
                "recent_metrics": [
                    {
                        "overall_score": 84,
                        "quality_gate": {
                            "decision": "auto_repair",
                            "label": "建议修复"
                        }
                    },
                    {
                        "overall_score": 88,
                        "repair_guidance": {
                            "summary": "上一章总体稳定"
                        },
                        "quality_gate": {
                            "decision": "continue",
                            "label": "通过"
                        }
                    }
                ]
            }
        });

        let resolved = resolve_generation_quality_runtime_context_from_persisted_sources(
            "chapter",
            None,
            None,
            None,
            Some(&summary),
        );

        assert_eq!(
            resolved.latest_quality_metrics,
            Some(json!({
                "overall_score": 84,
                "repair_guidance": {
                    "summary": "当前章需要压缩说明"
                },
                "quality_gate": {
                    "decision": "auto_repair",
                    "label": "建议修复"
                }
            }))
        );
        assert_eq!(
            resolved.quality_metrics_history,
            Some(json!([
                {
                    "overall_score": 88,
                    "repair_guidance": {
                        "summary": "上一章总体稳定"
                    },
                    "quality_gate": {
                        "decision": "continue",
                        "label": "通过"
                    }
                },
                {
                    "overall_score": 84,
                    "repair_guidance": {
                        "summary": "当前章需要压缩说明"
                    },
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "建议修复"
                    }
                }
            ]))
        );
        assert_eq!(
            resolved
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|state| state.get("chapter_count")),
            Some(&json!(2))
        );
        assert_eq!(resolved.quality_metrics_summary, Some(summary.clone()));
        assert_eq!(
            resolved
                .quality_history_context
                .as_ref()
                .and_then(|context| context.get("scope")),
            Some(&json!("chapter"))
        );
    }

    #[test]
    fn should_resolve_generation_quality_runtime_context_from_persisted_summary_only_for_batch_scope(
    ) {
        let summary = json!({
            "overall_score": 87,
            "quality_runtime_context": {
                "history_scope": "batch",
                "recent_metrics": [
                    {
                        "overall_score": 87
                    }
                ]
            }
        });

        let resolved = resolve_generation_quality_runtime_context_from_persisted_sources(
            "batch",
            None,
            None,
            None,
            Some(&summary),
        );

        assert_eq!(
            resolved.quality_metrics_history,
            Some(json!([
                {"overall_score": 87}
            ]))
        );
        assert_eq!(
            resolved.latest_quality_metrics,
            Some(json!({"overall_score": 87}))
        );
        assert_eq!(
            resolved
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|state| state.get("scope")),
            Some(&json!("batch"))
        );
        assert_eq!(
            resolved
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|state| state.get("chapter_count")),
            Some(&json!(1))
        );
        assert_eq!(resolved.quality_metrics_summary, Some(summary.clone()));
        assert_eq!(
            resolved.quality_history_context,
            Some(json!({
                "recent_metrics": [{"overall_score": 87}],
                "history_scope": "batch"
            }))
        );
    }

    #[test]
    fn should_build_generation_quality_history_from_summary_in_oldest_to_latest_order() {
        let summary = json!({
            "repair_guidance": {
                "summary": "最新摘要"
            },
            "quality_gate": {
                "decision": "auto_repair"
            },
            "quality_runtime_context": {
                "recent_metrics": [
                    {
                        "overall_score": 88,
                        "repair_guidance": {
                            "summary": "最新摘要"
                        },
                        "quality_gate": {
                            "decision": "auto_repair"
                        }
                    },
                    {
                        "overall_score": 81,
                        "repair_guidance": {
                            "summary": "较早摘要"
                        },
                        "quality_gate": {
                            "decision": "repair"
                        }
                    }
                ]
            }
        });

        assert_eq!(
            build_generation_quality_metrics_history_from_summary(Some(&summary)),
            Some(json!([
                {
                    "overall_score": 81,
                    "repair_guidance": {
                        "summary": "较早摘要"
                    },
                    "quality_gate": {
                        "decision": "repair"
                    }
                },
                {
                    "overall_score": 88,
                    "repair_guidance": {
                        "summary": "最新摘要"
                    },
                    "quality_gate": {
                        "decision": "auto_repair"
                    }
                }
            ]))
        );
    }

    #[test]
    fn should_build_generation_quality_history_from_summary_with_chronological_recent_metrics() {
        let summary = json!({
            "overall_score": 88,
            "repair_guidance": {
                "summary": "最新摘要"
            },
            "quality_gate": {
                "decision": "auto_repair"
            },
            "quality_runtime_context": {
                "recent_metrics": [
                    {
                        "history_index": 0,
                        "overall_score": 81,
                        "repair_guidance": {
                            "summary": "较早摘要"
                        },
                        "quality_gate": {
                            "decision": "repair"
                        }
                    },
                    {
                        "history_index": 1,
                        "overall_score": 88
                    }
                ]
            }
        });

        assert_eq!(
            build_generation_quality_metrics_history_from_summary(Some(&summary)),
            Some(json!([
                {
                    "overall_score": 81,
                    "repair_guidance": {
                        "summary": "较早摘要"
                    },
                    "quality_gate": {
                        "decision": "repair"
                    }
                },
                {
                    "overall_score": 88,
                    "repair_guidance": {
                        "summary": "最新摘要"
                    },
                    "quality_gate": {
                        "decision": "auto_repair"
                    }
                }
            ]))
        );
    }

    #[test]
    fn should_resolve_generation_quality_runtime_context_from_summary_only_seed() {
        let summary = json!({
            "overall_score": 88,
            "repair_guidance": {
                "summary": "最近质量摘要"
            },
            "quality_gate": {
                "decision": "repair",
                "label": "需修复"
            },
            "quality_runtime_context": {
                "scope": "chapter",
                "recent_metrics": [
                    {
                        "overall_score": 88,
                        "repair_guidance": {
                            "summary": "最近质量摘要"
                        },
                        "quality_gate": {
                            "decision": "repair",
                            "label": "需修复"
                        }
                    },
                    {
                        "overall_score": 81,
                        "repair_guidance": {
                            "summary": "较早摘要"
                        },
                        "quality_gate": {
                            "decision": "auto_repair"
                        }
                    }
                ]
            }
        });

        let resolved = resolve_generation_quality_runtime_context_for_seed(
            "chapter",
            None,
            None,
            None,
            Some(&summary),
            20,
        );

        assert_eq!(
            resolved.latest_quality_metrics,
            Some(json!({
                "overall_score": 88,
                "repair_guidance": {
                    "summary": "最近质量摘要"
                },
                "quality_gate": {
                    "decision": "repair",
                    "label": "需修复"
                }
            }))
        );
        assert_eq!(
            resolved.quality_metrics_history,
            Some(json!([
                {
                    "overall_score": 81,
                    "repair_guidance": {
                        "summary": "较早摘要"
                    },
                    "quality_gate": {
                        "decision": "auto_repair"
                    }
                },
                {
                    "overall_score": 88,
                    "repair_guidance": {
                        "summary": "最近质量摘要"
                    },
                    "quality_gate": {
                        "decision": "repair",
                        "label": "需修复"
                    }
                }
            ]))
        );
        assert_eq!(
            resolved
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|state| state.get("chapter_count")),
            Some(&json!(2))
        );
        assert_eq!(
            resolved
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|state| state.get("first_overall_score")),
            Some(&json!(81.0))
        );
        assert_eq!(
            resolved
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|state| state.get("last_overall_score")),
            Some(&json!(88.0))
        );
        assert_eq!(resolved.quality_metrics_summary, Some(summary.clone()));
        assert_eq!(
            resolved
                .quality_history_context
                .as_ref()
                .and_then(|context| context.get("scope")),
            Some(&json!("chapter"))
        );
    }

    #[test]
    fn should_normalize_terminal_quality_gate_payload_to_manual_review() {
        let mut payload = json!({
            "overall_score": 81,
            "quality_gate": {
                "status": "warning",
                "decision": "auto_repair",
                "label": "建议继续修复"
            }
        });

        normalize_terminal_quality_gate_payload(&mut payload, "等待人工复核");

        assert_eq!(payload["quality_gate"]["status"], "failed");
        assert_eq!(payload["quality_gate"]["decision"], "manual_review");
        assert_eq!(payload["quality_gate"]["label"], "等待人工复核");
    }

    #[test]
    fn should_normalize_terminal_quality_summary_state_only_last_metric() {
        let mut summary_state = json!({
            "recent_history": [
                {
                    "overall_score": 78,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议继续修复"
                    }
                },
                {
                    "overall_score": 81,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "repair",
                        "label": "继续修复"
                    }
                }
            ]
        });

        normalize_terminal_quality_summary_state(&mut summary_state, "等待人工复核");

        assert_eq!(
            summary_state["recent_history"][0]["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(
            summary_state["recent_history"][1]["quality_gate"]["decision"],
            "manual_review"
        );
    }

    #[test]
    fn should_normalize_terminal_quality_history_only_last_metric() {
        let mut history = json!([
            {
                "overall_score": 78,
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议继续修复"
                }
            },
            {
                "overall_score": 81,
                "quality_gate": {
                    "status": "warning",
                    "decision": "repair",
                    "label": "继续修复"
                }
            }
        ]);

        normalize_terminal_quality_history(&mut history, "等待人工复核");

        assert_eq!(history[0]["quality_gate"]["decision"], "auto_repair");
        assert_eq!(history[1]["quality_gate"]["decision"], "manual_review");
        assert_eq!(history[1]["quality_gate"]["status"], "failed");
    }

    #[test]
    fn should_normalize_terminal_quality_history_context_and_recount_terminal_decisions() {
        let mut context = json!({
            "recent_metrics": [
                {
                    "history_index": 0,
                    "overall_score": 78,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议继续修复"
                    }
                },
                {
                    "history_index": 1,
                    "overall_score": 81,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "repair",
                        "label": "继续修复"
                    }
                }
            ],
            "quality_gate_counts": {
                "auto_repair": 2
            },
            "recent_manual_review_count": 0,
            "recent_auto_repair_count": 2
        });

        normalize_terminal_quality_history_context(&mut context, "等待人工复核");

        assert_eq!(
            context["recent_metrics"][0]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            context["recent_metrics"][1]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(context["quality_gate_counts"]["manual_review"], 2);
        assert!(context["quality_gate_counts"].get("auto_repair").is_none());
        assert_eq!(context["recent_manual_review_count"], 2);
        assert_eq!(context["recent_auto_repair_count"], 0);
    }
}
