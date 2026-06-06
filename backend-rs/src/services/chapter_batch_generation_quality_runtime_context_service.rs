use serde_json::Value;

use crate::models::batch_generation_snapshot;
use crate::services::chapter_story_repair_quality_context_service::{
    advance_quality_metrics_summary_state, build_quality_metrics_summary_from_state,
    build_quality_metrics_summary_state_from_history, extract_quality_history_context,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct BatchGenerationQualityRuntimeContext {
    pub(crate) latest_quality_metrics: Option<Value>,
    pub(crate) quality_metrics_history: Option<Value>,
    pub(crate) quality_metrics_summary_state: Option<Value>,
    pub(crate) quality_metrics_summary: Option<Value>,
    pub(crate) quality_history_context: Option<Value>,
}

const DEFAULT_MAX_BATCH_QUALITY_METRICS_HISTORY: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchQualitySummaryResolutionMode {
    PreferRebuilt,
    PreferExplicit,
}

fn merge_batch_quality_history_context(
    derived_quality_summary: &Value,
    fallback_quality_summary: Option<&Value>,
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
    } else if merged_context.is_null() {
        merged_context = fallback_context.unwrap_or(Value::Null);
    }

    merged_context
}

pub(crate) fn build_batch_quality_metrics_history_from_summary(
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

    let history = recent_metrics
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(index, metric)| metric.as_object().map(|metric| (index, metric)))
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
                (original_index == 0)
                    .then(|| summary_repair_guidance.cloned())
                    .flatten()
            }) {
                payload.insert("repair_guidance".to_string(), value);
            }
            if let Some(value) = metric.get("quality_gate").cloned().or_else(|| {
                (original_index == 0)
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

fn resolve_quality_metrics_summary_state(
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

fn resolve_latest_quality_metrics(
    explicit_latest_quality_metrics: Option<&Value>,
    quality_metrics_history: Option<&Value>,
) -> Option<Value> {
    explicit_latest_quality_metrics.cloned().or_else(|| {
        quality_metrics_history
            .and_then(Value::as_array)
            .and_then(|history| history.last().cloned())
    })
}

fn latest_quality_metrics_from_snapshot_or_runtime_state(
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

fn quality_metrics_summary_from_snapshot_or_runtime_state(
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

fn quality_metrics_history_from_snapshot_or_runtime_state(
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

fn quality_metrics_summary_state_from_runtime_state_or_history(
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

fn rebuild_quality_metrics_summary_from_history(
    quality_metrics_summary_state: Option<&Value>,
    quality_metrics_history: Option<&Value>,
) -> Option<Value> {
    quality_metrics_history
        .and_then(Value::as_array)
        .and_then(|history| {
            build_quality_metrics_summary_from_state(
                quality_metrics_summary_state,
                history,
                "batch",
            )
        })
}

fn resolve_batch_quality_runtime_context(
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
    let quality_metrics_summary_state = resolve_quality_metrics_summary_state(
        explicit_quality_metrics_summary_state,
        quality_metrics_history.as_ref(),
    );
    let rebuilt_quality_metrics_summary = rebuild_quality_metrics_summary_from_history(
        quality_metrics_summary_state.as_ref(),
        quality_metrics_history.as_ref(),
    );
    let quality_metrics_summary = match summary_resolution_mode {
        BatchQualitySummaryResolutionMode::PreferRebuilt => {
            rebuilt_quality_metrics_summary.or_else(|| explicit_quality_metrics_summary.cloned())
        }
        BatchQualitySummaryResolutionMode::PreferExplicit => explicit_quality_metrics_summary
            .cloned()
            .or_else(|| rebuilt_quality_metrics_summary),
    };
    let latest_quality_metrics = resolve_latest_quality_metrics(
        explicit_latest_quality_metrics,
        quality_metrics_history.as_ref(),
    );
    let quality_history_context = quality_metrics_summary
        .as_ref()
        .map(|summary| {
            if merge_fallback_history_context {
                merge_batch_quality_history_context(summary, explicit_quality_metrics_summary)
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
    resolve_batch_quality_runtime_context(
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
    resolve_batch_quality_runtime_context(
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
    let latest_quality_metrics =
        latest_quality_metrics_from_snapshot_or_runtime_state(snapshot, workflow_runtime_state);
    let quality_metrics_history =
        quality_metrics_history_from_snapshot_or_runtime_state(snapshot, workflow_runtime_state);
    let quality_metrics_summary_state = quality_metrics_summary_state_from_runtime_state_or_history(
        workflow_runtime_state,
        quality_metrics_history.as_ref(),
    );
    let quality_metrics_summary =
        quality_metrics_summary_from_snapshot_or_runtime_state(snapshot, workflow_runtime_state);

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
    let mut history = existing_quality_metrics_history
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

fn build_batch_quality_summary_from_state_or_history(
    quality_metrics_summary_state: Option<&Value>,
    quality_metrics_history: &Value,
    fallback_quality_summary: &Value,
) -> Value {
    let history = quality_metrics_history
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|item| item.is_object())
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    build_quality_metrics_summary_from_state(quality_metrics_summary_state, &history, "batch")
        .unwrap_or_else(|| fallback_quality_summary.clone())
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        append_batch_quality_metrics_history_event, apply_batch_quality_runtime_context_to_payload,
        build_batch_quality_metrics_history_from_summary,
        resolve_batch_quality_runtime_context_from_current_quality,
        resolve_batch_quality_runtime_context_from_persisted_sources,
        resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state,
        BatchGenerationQualityRuntimeContext,
    };

    #[test]
    fn should_rebuild_batch_quality_history_from_recent_summary_in_oldest_to_latest_order() {
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
                        "overall_score": 86,
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
            build_batch_quality_metrics_history_from_summary(Some(&summary)),
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
                    "overall_score": 86,
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
    fn should_restore_persisted_batch_quality_runtime_context_from_summary_only_when_history_missing(
    ) {
        let summary = json!({
            "overall_score": 87,
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 87}],
                "history_scope": "batch"
            }
        });

        let resolved = resolve_batch_quality_runtime_context_from_persisted_sources(
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
                .and_then(|state| state.get("chapter_count")),
            Some(&json!(1))
        );
        assert_eq!(
            resolved.quality_history_context,
            Some(json!({
                "recent_metrics": [{"overall_score": 87}],
                "history_scope": "batch"
            }))
        );
    }

    #[test]
    fn should_append_batch_quality_history_with_bounded_oldest_to_latest_order() {
        let existing_history = json!([
            {"overall_score": 81},
            {"overall_score": 84}
        ]);
        let latest_quality_metrics = json!({"overall_score": 88});

        let (history, dropped_event) = append_batch_quality_metrics_history_event(
            Some(&existing_history),
            &latest_quality_metrics,
            2,
        );

        assert_eq!(
            history,
            json!([
                {"overall_score": 84},
                {"overall_score": 88}
            ])
        );
        assert_eq!(dropped_event, Some(json!({"overall_score": 81})));
    }

    #[test]
    fn should_restore_persisted_batch_quality_runtime_context_from_runtime_state_only() {
        let runtime_state = json!({
            "latest_quality_metrics": {
                "overall_score": 82,
                "quality_gate": {
                    "decision": "manual_review",
                    "label": "等待人工复核"
                }
            },
            "quality_metrics_summary": {
                "overall_score": 82,
                "quality_gate": {
                    "decision": "manual_review",
                    "label": "等待人工复核"
                }
            },
            "quality_metrics_history": [
                {"overall_score": 88},
                {"overall_score": 84}
            ]
        });

        let restored = resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state(
            None,
            Some(&runtime_state),
        );

        assert_eq!(
            restored.latest_quality_metrics,
            Some(json!({
                "overall_score": 82,
                "quality_gate": {
                    "decision": "manual_review",
                    "label": "等待人工复核"
                }
            }))
        );
        assert_eq!(
            restored.quality_metrics_summary,
            Some(json!({
                "overall_score": 82,
                "quality_gate": {
                    "decision": "manual_review",
                    "label": "等待人工复核"
                }
            }))
        );
        assert_eq!(
            restored.quality_metrics_history,
            Some(json!([
                {"overall_score": 88},
                {"overall_score": 84}
            ]))
        );
        assert_eq!(
            restored
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|state| state.get("chapter_count")),
            Some(&json!(2))
        );
    }

    #[test]
    fn should_build_current_quality_runtime_context_from_existing_history() {
        let existing_history = json!([
            {
                "overall_score": 88,
                "repair_guidance": {
                    "summary": "保持优势",
                    "focus_areas": ["pacing"]
                },
                "quality_gate": {
                    "decision": "passed"
                }
            }
        ]);
        let fallback_quality_summary = json!({
            "overall_score": 84,
            "quality_gate": {
                "decision": "auto_repair"
            }
        });
        let latest_quality_metrics = json!({
            "overall_score": 84,
            "repair_guidance": {
                "summary": "建议继续修复",
                "focus_areas": ["character"]
            },
            "quality_gate": {
                "decision": "auto_repair"
            }
        });

        let resolved = resolve_batch_quality_runtime_context_from_current_quality(
            None,
            Some(&existing_history),
            &fallback_quality_summary,
            Some(&latest_quality_metrics),
        );

        assert_eq!(
            resolved.quality_metrics_history,
            Some(json!([
                {
                    "overall_score": 88,
                    "repair_guidance": {
                        "summary": "保持优势",
                        "focus_areas": ["pacing"]
                    },
                    "quality_gate": {
                        "decision": "passed"
                    }
                },
                {
                    "overall_score": 84,
                    "repair_guidance": {
                        "summary": "建议继续修复",
                        "focus_areas": ["character"]
                    },
                    "quality_gate": {
                        "decision": "auto_repair"
                    }
                }
            ]))
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
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|state| state.get("last_overall_score")),
            Some(&json!(84.0))
        );
    }

    #[test]
    fn should_apply_batch_quality_runtime_context_fields_to_payload() {
        let resolved = BatchGenerationQualityRuntimeContext {
            latest_quality_metrics: Some(json!({"overall_score": 84})),
            quality_metrics_history: Some(json!([{"overall_score": 88}, {"overall_score": 84}])),
            quality_metrics_summary_state: Some(json!({"chapter_count": 2})),
            quality_metrics_summary: Some(json!({"overall_score": 84.0})),
            quality_history_context: Some(json!({"scope": "batch"})),
        };
        let mut payload = serde_json::Map::new();

        apply_batch_quality_runtime_context_to_payload(
            &mut payload,
            resolved,
            Some(json!({"overall_score": 70.0})),
        );

        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 84.0);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 84);
        assert_eq!(payload["quality_metrics_history"][1]["overall_score"], 84);
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(payload["quality_history_context"]["scope"], "batch");
    }
}
