use serde_json::{json, Value};

use crate::services::chapter_generation_execution_contract_service::active_story_repair_payload_ref_from_runtime_state;
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    normalize_terminal_quality_gate_payload, normalize_terminal_quality_history,
    normalize_terminal_quality_history_context, normalize_terminal_quality_summary_state,
    resolve_generation_quality_runtime_context_from_persisted_sources,
};

use super::{
    build_batch_request_runtime_state_owner_contract,
    build_generation_quality_runtime_owner_contract,
};

pub(crate) fn build_generation_terminal_runtime_patch_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::terminal_runtime_patch",
        "scope": "terminal_quality_runtime_patch_manual_review_and_retry",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/terminal_runtime_patch_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "patch_entrypoints": [
                "build_quality_gate_blocked_runtime_state_patch",
                "build_quality_gate_blocked_runtime_state_patch_from_workflow_state",
                "build_retry_quality_runtime_patch_contract",
                "build_retry_quality_runtime_patch_contract_from_workflow_state"
            ],
            "patch_helpers": [
                "infer_quality_scope_from_workflow_runtime_state",
                "build_resolved_terminal_quality_payload_source",
                "apply_terminal_quality_runtime_patch_contract",
                "apply_retry_terminal_quality_runtime_patch_contract",
                "build_manual_review_terminal_runtime_patch_contract",
                "insert_retry_active_story_repair_payload"
            ],
            "manual_review_patch_fields": [
                "analysis_task_message",
                "analysis_task_progress",
                "analysis_last_error",
                "quality_gate_decision",
                "quality_gate_label",
                "phase"
            ],
            "manual_review_decision": "manual_review",
            "manual_review_phase": "quality_blocked",
            "retry_decision": "auto_repair",
            "retry_phase": "repair_pending",
            "active_story_repair_payload_normalized_for": [
                "manual_review",
                "retry"
            ],
            "quality_runtime_fields_normalized": [
                "quality_metrics_summary",
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary_state",
                "quality_history_context"
            ],
            "scope_inference_inputs": [
                "quality_metrics_summary_state.scope",
                "quality_history_context.scope",
                "quality_history_context.history_scope",
                "active_story_repair_payload.scope",
                "quality_metrics_summary.quality_runtime_context.scope",
                "quality_metrics_summary.quality_runtime_context.history_scope",
                "batch_request_runtime_state"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service",
            "chapter-single-generation-active-gateway-smoke-rust",
            "chapter-batch-generation-active-gateway-smoke-rust"
        ],
        "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
        "request_runtime_state_owner_contract": build_batch_request_runtime_state_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "batch_generation_terminal_runtime_patch_owner_is_rust_only_and_surviving_story_repair_task_runtime_surfaces_are_tracked_by_external_runtime_contracts",
            "runtime_state_keys": [
                "active_story_repair_payload",
                "quality_metrics_summary",
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary_state",
                "quality_history_context"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_route_smoke"
        }
    })
}

fn infer_quality_scope_from_workflow_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> String {
    let state = workflow_runtime_state.and_then(Value::as_object);
    state
        .and_then(|state| {
            state
                .get("quality_metrics_summary_state")
                .and_then(Value::as_object)
                .and_then(|summary_state| summary_state.get("scope"))
                .and_then(Value::as_str)
                .or_else(|| {
                    state
                        .get("quality_history_context")
                        .and_then(Value::as_object)
                        .and_then(|context| {
                            context
                                .get("scope")
                                .or_else(|| context.get("history_scope"))
                        })
                        .and_then(Value::as_str)
                })
                .or_else(|| {
                    state
                        .get("active_story_repair_payload")
                        .and_then(Value::as_object)
                        .and_then(|payload| payload.get("scope"))
                        .and_then(Value::as_str)
                })
                .or_else(|| {
                    state
                        .get("quality_metrics_summary")
                        .and_then(Value::as_object)
                        .and_then(|summary| summary.get("quality_runtime_context"))
                        .and_then(Value::as_object)
                        .and_then(|context| {
                            context
                                .get("scope")
                                .or_else(|| context.get("history_scope"))
                        })
                        .and_then(Value::as_str)
                })
        })
        .map(str::to_string)
        .unwrap_or_else(|| {
            if state
                .and_then(|state| state.get("batch_request_runtime_state"))
                .is_some()
            {
                "batch".to_string()
            } else {
                "chapter".to_string()
            }
        })
}

fn build_resolved_terminal_quality_payload_source(workflow_runtime_state: Option<&Value>) -> Value {
    let Some(state) = workflow_runtime_state.and_then(Value::as_object) else {
        return Value::Null;
    };
    let scope = infer_quality_scope_from_workflow_runtime_state(workflow_runtime_state);
    let resolved_quality_context =
        resolve_generation_quality_runtime_context_from_persisted_sources(
            scope.as_str(),
            state.get("latest_quality_metrics"),
            state.get("quality_metrics_history"),
            state.get("quality_metrics_summary_state"),
            state.get("quality_metrics_summary"),
        );

    let mut payload = serde_json::Map::new();
    if let Some(quality_metrics_summary) = resolved_quality_context.quality_metrics_summary {
        payload.insert(
            "quality_metrics_summary".to_string(),
            quality_metrics_summary,
        );
    }
    if let Some(latest_quality_metrics) = resolved_quality_context.latest_quality_metrics {
        payload.insert("latest_quality_metrics".to_string(), latest_quality_metrics);
    }
    if let Some(quality_metrics_history) = resolved_quality_context.quality_metrics_history {
        payload.insert(
            "quality_metrics_history".to_string(),
            quality_metrics_history,
        );
    }
    if let Some(quality_metrics_summary_state) =
        resolved_quality_context.quality_metrics_summary_state
    {
        payload.insert(
            "quality_metrics_summary_state".to_string(),
            quality_metrics_summary_state,
        );
    }
    if let Some(quality_history_context) = state
        .get("quality_history_context")
        .cloned()
        .filter(|value| !value.is_null())
        .or_else(|| {
            resolved_quality_context
                .quality_history_context
                .filter(|value| !value.is_null())
        })
    {
        payload.insert(
            "quality_history_context".to_string(),
            quality_history_context,
        );
    }

    Value::Object(payload)
}

#[allow(dead_code)]
pub(crate) fn apply_manual_review_terminal_fields(
    object: &mut serde_json::Map<String, Value>,
    manual_review_label: &str,
) {
    object.insert("quality_gate_decision".to_string(), json!("manual_review"));
    object.insert("quality_gate_label".to_string(), json!(manual_review_label));
    object.insert("phase".to_string(), json!("quality_blocked"));
}

fn apply_retry_terminal_quality_gate_fields(
    object: &mut serde_json::Map<String, Value>,
    retry_label: &str,
) {
    object.insert("quality_gate_decision".to_string(), json!("auto_repair"));
    object.insert("quality_gate_label".to_string(), json!(retry_label));
    object.insert("phase".to_string(), json!("repair_pending"));
}

fn normalize_retry_terminal_quality_gate_payload(payload: &mut Value, retry_label: &str) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };

    let quality_gate = object
        .entry("quality_gate".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if let Some(gate_object) = quality_gate.as_object_mut() {
        gate_object.insert("status".to_string(), Value::from("warning"));
        gate_object.insert("decision".to_string(), Value::from("auto_repair"));
        gate_object.insert("label".to_string(), Value::from(retry_label));
    }
}

fn normalize_retry_terminal_quality_summary_state(summary_state: &mut Value, retry_label: &str) {
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
    normalize_retry_terminal_quality_gate_payload(last_metric, retry_label);
}

fn normalize_retry_terminal_quality_history(
    quality_metrics_history: &mut Value,
    retry_label: &str,
) {
    let Some(last_metric) = quality_metrics_history
        .as_array_mut()
        .and_then(|history| history.last_mut())
    else {
        return;
    };
    normalize_retry_terminal_quality_gate_payload(last_metric, retry_label);
}

fn increment_retry_terminal_quality_gate_counts(
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

fn normalize_retry_terminal_quality_history_context(
    quality_history_context: &mut Value,
    retry_label: &str,
) {
    let mut quality_gate_counts = serde_json::Map::new();
    let mut recent_manual_review_count = 0_i64;
    let mut recent_auto_repair_count = 0_i64;
    if let Some(recent_metrics) = quality_history_context
        .get_mut("recent_metrics")
        .and_then(Value::as_array_mut)
    {
        for metric in recent_metrics {
            normalize_retry_terminal_quality_gate_payload(metric, retry_label);
            increment_retry_terminal_quality_gate_counts(
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

fn insert_normalized_terminal_quality_payload_field(
    payload: &mut serde_json::Map<String, Value>,
    workflow_runtime_state: Option<&Value>,
    field_name: &str,
    normalize: fn(&mut Value, &str),
    manual_review_label: &str,
) {
    let Some(mut value) = workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get(field_name))
        .cloned()
    else {
        return;
    };
    normalize(&mut value, manual_review_label);
    payload.insert(field_name.to_string(), value);
}

#[allow(dead_code)]
fn insert_terminal_active_story_repair_payload(
    payload: &mut serde_json::Map<String, Value>,
    active_story_repair_payload: Option<&Value>,
    manual_review_label: &str,
) {
    let Some(mut active_story_repair_payload) = active_story_repair_payload.cloned() else {
        return;
    };
    if let Some(object) = active_story_repair_payload.as_object_mut() {
        apply_manual_review_terminal_fields(object, manual_review_label);
    }
    payload.insert(
        "active_story_repair_payload".to_string(),
        active_story_repair_payload,
    );
}

fn insert_retry_active_story_repair_payload(
    payload: &mut serde_json::Map<String, Value>,
    active_story_repair_payload: Option<&Value>,
    retry_label: &str,
) {
    let Some(active_story_repair_payload) =
        active_story_repair_payload.and_then(|payload| payload.as_object().cloned())
    else {
        return;
    };

    let mut next_active_story_repair_payload = active_story_repair_payload;
    apply_retry_terminal_quality_gate_fields(&mut next_active_story_repair_payload, retry_label);

    payload.insert(
        "active_story_repair_payload".to_string(),
        Value::Object(next_active_story_repair_payload),
    );
}

pub(crate) fn apply_retry_terminal_quality_runtime_patch_contract(
    payload: &mut serde_json::Map<String, Value>,
    workflow_runtime_state: Option<&Value>,
    retry_label: &str,
) {
    let resolved_quality_payload_source =
        build_resolved_terminal_quality_payload_source(workflow_runtime_state);
    let resolved_quality_payload_source =
        (!resolved_quality_payload_source.is_null()).then_some(&resolved_quality_payload_source);

    insert_normalized_terminal_quality_payload_field(
        payload,
        resolved_quality_payload_source,
        "quality_metrics_summary",
        normalize_retry_terminal_quality_gate_payload,
        retry_label,
    );
    insert_normalized_terminal_quality_payload_field(
        payload,
        resolved_quality_payload_source,
        "latest_quality_metrics",
        normalize_retry_terminal_quality_gate_payload,
        retry_label,
    );
    insert_normalized_terminal_quality_payload_field(
        payload,
        resolved_quality_payload_source,
        "quality_metrics_history",
        normalize_retry_terminal_quality_history,
        retry_label,
    );
    insert_normalized_terminal_quality_payload_field(
        payload,
        resolved_quality_payload_source,
        "quality_metrics_summary_state",
        normalize_retry_terminal_quality_summary_state,
        retry_label,
    );
    insert_normalized_terminal_quality_payload_field(
        payload,
        resolved_quality_payload_source,
        "quality_history_context",
        normalize_retry_terminal_quality_history_context,
        retry_label,
    );
}

#[allow(dead_code)]
pub(crate) fn apply_terminal_quality_runtime_patch_contract(
    payload: &mut serde_json::Map<String, Value>,
    workflow_runtime_state: Option<&Value>,
    active_story_repair_payload: Option<&Value>,
    manual_review_label: &str,
) {
    let resolved_quality_payload_source =
        build_resolved_terminal_quality_payload_source(workflow_runtime_state);
    let resolved_quality_payload_source =
        (!resolved_quality_payload_source.is_null()).then_some(&resolved_quality_payload_source);

    insert_normalized_terminal_quality_payload_field(
        payload,
        resolved_quality_payload_source,
        "quality_metrics_summary",
        normalize_terminal_quality_gate_payload,
        manual_review_label,
    );
    insert_normalized_terminal_quality_payload_field(
        payload,
        resolved_quality_payload_source,
        "latest_quality_metrics",
        normalize_terminal_quality_gate_payload,
        manual_review_label,
    );
    insert_normalized_terminal_quality_payload_field(
        payload,
        resolved_quality_payload_source,
        "quality_metrics_history",
        normalize_terminal_quality_history,
        manual_review_label,
    );
    insert_normalized_terminal_quality_payload_field(
        payload,
        resolved_quality_payload_source,
        "quality_metrics_summary_state",
        normalize_terminal_quality_summary_state,
        manual_review_label,
    );
    insert_normalized_terminal_quality_payload_field(
        payload,
        resolved_quality_payload_source,
        "quality_history_context",
        normalize_terminal_quality_history_context,
        manual_review_label,
    );
    insert_terminal_active_story_repair_payload(
        payload,
        active_story_repair_payload,
        manual_review_label,
    );
}

#[allow(dead_code)]
pub(crate) fn build_manual_review_terminal_runtime_patch_contract(
    chapter_number: i32,
    manual_review_label: &str,
) -> serde_json::Map<String, Value> {
    serde_json::Map::from_iter([
        (
            "analysis_task_message".to_string(),
            json!(format!(
                "第 {} 章质量门禁未通过，建议继续修复",
                chapter_number
            )),
        ),
        ("analysis_task_progress".to_string(), json!(100)),
        ("analysis_last_error".to_string(), Value::Null),
        ("quality_gate_decision".to_string(), json!("manual_review")),
        ("quality_gate_label".to_string(), json!(manual_review_label)),
        ("phase".to_string(), json!("quality_blocked")),
    ])
}

#[allow(dead_code)]
pub(crate) fn build_quality_gate_blocked_runtime_state_patch(
    workflow_runtime_state: Option<&Value>,
    active_story_repair_payload: Option<&Value>,
    chapter_number: i32,
    manual_review_label: &str,
) -> Value {
    let mut payload =
        build_manual_review_terminal_runtime_patch_contract(chapter_number, manual_review_label);
    apply_terminal_quality_runtime_patch_contract(
        &mut payload,
        workflow_runtime_state,
        active_story_repair_payload,
        manual_review_label,
    );
    Value::Object(payload)
}

#[allow(dead_code)]
pub(crate) fn build_quality_gate_blocked_runtime_state_patch_from_workflow_state(
    workflow_runtime_state: Option<&Value>,
    chapter_number: i32,
    manual_review_label: &str,
) -> Value {
    build_quality_gate_blocked_runtime_state_patch(
        workflow_runtime_state,
        active_story_repair_payload_ref_from_runtime_state(workflow_runtime_state),
        chapter_number,
        manual_review_label,
    )
}

pub(crate) fn build_retry_quality_runtime_patch_contract(
    workflow_runtime_state: Option<&Value>,
    active_story_repair_payload: Option<&Value>,
    chapter_number: i32,
    retry_label: &str,
) -> serde_json::Map<String, Value> {
    let mut payload = serde_json::Map::from_iter([
        (
            "analysis_task_message".to_string(),
            json!(format!("第 {} 章触发质量修复，等待重试", chapter_number)),
        ),
        ("analysis_task_progress".to_string(), json!(100)),
        ("analysis_last_error".to_string(), Value::Null),
        ("quality_gate_decision".to_string(), json!("auto_repair")),
        ("quality_gate_label".to_string(), json!(retry_label)),
        ("phase".to_string(), json!("repair_pending")),
    ]);
    apply_retry_terminal_quality_runtime_patch_contract(
        &mut payload,
        workflow_runtime_state,
        retry_label,
    );
    insert_retry_active_story_repair_payload(
        &mut payload,
        active_story_repair_payload,
        retry_label,
    );
    payload
}

pub(crate) fn build_retry_quality_runtime_patch_contract_from_workflow_state(
    workflow_runtime_state: Option<&Value>,
    chapter_number: i32,
    retry_label: &str,
) -> serde_json::Map<String, Value> {
    build_retry_quality_runtime_patch_contract(
        workflow_runtime_state,
        active_story_repair_payload_ref_from_runtime_state(workflow_runtime_state),
        chapter_number,
        retry_label,
    )
}
