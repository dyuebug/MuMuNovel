use serde_json::{json, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    manual_review_label_from_quality_context_with_retry_budget,
    retryable_repair_label_from_quality_context_with_retry_budget,
};
use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SingleGenerationQualityGateTerminalState {
    pub(crate) checkpoint_payload: Value,
    pub(crate) error_message: String,
    pub(crate) failed_entry: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SingleGenerationFollowUpAnalysisDecision {
    pub(crate) manual_review_label: String,
    pub(crate) quality_metrics: Option<Value>,
}

pub(crate) fn build_single_generation_terminal_state_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_runtime_state_service::terminal_state_owner",
        "scope": "quality_gate_terminal_state_retry_error_projection_and_non_blocking_manual_review_policy",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_runtime_state_service/terminal_state_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "resolve_single_generation_manual_review_label_from_analysis_payload",
                "resolve_single_generation_quality_gate_terminal_state",
                "build_single_generation_error_terminal_state"
            ],
            "terminal_state_fields": [
                "checkpoint_payload",
                "error_message",
                "failed_entry"
            ],
            "manual_review_contract": [
                "manual review labels may still be read from quality context for telemetry",
                "single generation manual_review must not create a failed terminal state"
            ]
        },
        "active_consumers": [
            "chapter_single_generation_runtime_state_service",
            "chapter_single_generation_active_gateway_smoke_service",
            "chapter_generation_routes"
        ],
        "validation_boundary": [
            "cargo test chapter_single_generation_runtime_state_service",
            "cargo test api::health",
            "cargo check"
        ]
    })
}

pub(crate) fn resolve_single_generation_manual_review_label_from_analysis_payload(
    payload: &Value,
) -> Option<String> {
    let quality_metrics = payload.get("quality_metrics");
    manual_review_label_from_quality_context_with_retry_budget(
        None,
        quality_metrics,
        quality_metrics,
        0,
        0,
    )
}

pub(crate) fn resolve_single_generation_quality_gate_terminal_state(
    persisted_task: &Option<batch_generation_task::Model>,
    generated_result: &GeneratedChapterResult,
    analysis_decision: Option<&SingleGenerationFollowUpAnalysisDecision>,
) -> Option<SingleGenerationQualityGateTerminalState> {
    let current_retry_count = persisted_task
        .as_ref()
        .map(|task| task.current_retry_count)
        .unwrap_or(0);
    let max_retries = persisted_task
        .as_ref()
        .map(|task| task.max_retries)
        .unwrap_or(0);
    let quality_metrics = analysis_decision
        .and_then(|decision| decision.quality_metrics.as_ref())
        .or(generated_result.quality_metrics.as_ref());

    if generated_result_requires_retry_follow_up(generated_result) {
        let retry_label = resolve_single_generation_retry_terminal_label(
            generated_result,
            quality_metrics,
            current_retry_count,
            max_retries,
        );
        return Some(build_single_generation_retry_terminal_state(
            persisted_task,
            generated_result,
            retry_label.as_deref(),
            quality_metrics,
        ));
    }

    None
}

pub(crate) fn build_single_generation_error_terminal_state(
    persisted_task: &Option<batch_generation_task::Model>,
    chapter_id: &str,
    chapter_number: Option<i32>,
    chapter_title: Option<&str>,
    error_message: &str,
) -> SingleGenerationQualityGateTerminalState {
    let failed_entry = build_single_generation_failed_chapter_entry(
        Some(chapter_id),
        chapter_number,
        chapter_title,
        error_message,
        persisted_task
            .as_ref()
            .map(|task| task.current_retry_count)
            .unwrap_or(0),
    );

    SingleGenerationQualityGateTerminalState {
        checkpoint_payload: json!({
            "analysis_task_message": Value::Null,
            "analysis_task_progress": 100,
            "analysis_last_error": error_message,
            "phase": "failed",
        }),
        error_message: error_message.to_string(),
        failed_entry,
    }
}

fn generated_result_requires_retry_follow_up(generated_result: &GeneratedChapterResult) -> bool {
    matches!(
        generated_result.quality_gate_action.as_deref(),
        Some("retry")
    ) || generated_result.provisional_draft_saved
        || (!generated_result.content_applied && generated_result.attempt_state.trim() == "retry")
}

fn resolve_single_generation_retry_terminal_label(
    generated_result: &GeneratedChapterResult,
    quality_metrics: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    generated_result
        .quality_gate_message
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            retryable_repair_label_from_quality_context_with_retry_budget(
                None,
                quality_metrics,
                quality_metrics,
                current_retry_count,
                max_retries,
            )
        })
        .or_else(|| Some("可自动修复后重试".to_string()))
}

fn build_single_generation_failed_chapter_entry(
    chapter_id: Option<&str>,
    chapter_number: Option<i32>,
    chapter_title: Option<&str>,
    error_message: &str,
    retry_count: i32,
) -> Value {
    json!({
        "chapter_id": chapter_id,
        "chapter_number": chapter_number,
        "title": chapter_title,
        "error": error_message,
        "retry_count": retry_count.max(0),
    })
}

fn apply_single_generation_quality_gate_terminal_fields(
    entry: &mut Value,
    decision: &str,
    label: &str,
    phase: &str,
    quality_metrics: Option<&Value>,
) {
    let failed_metric_labels = quality_metrics
        .and_then(|metrics| metrics.get("quality_gate"))
        .and_then(|gate| gate.get("failed_metrics"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("label").and_then(Value::as_str))
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Some(object) = entry.as_object_mut() {
        object.insert("phase".to_string(), json!(phase));
        object.insert("quality_gate_status".to_string(), json!("failed"));
        object.insert("quality_gate_decision".to_string(), json!(decision));
        object.insert("quality_gate_label".to_string(), json!(label));
        object.insert(
            "quality_gate_failed_metrics".to_string(),
            json!(failed_metric_labels),
        );
    }
}

fn build_single_generation_retry_terminal_state(
    persisted_task: &Option<batch_generation_task::Model>,
    generated_result: &GeneratedChapterResult,
    retry_label: Option<&str>,
    quality_metrics: Option<&Value>,
) -> SingleGenerationQualityGateTerminalState {
    let retry_label = retry_label
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("可自动修复后重试");
    let error_message = format!("章节触发质量修复重试: {retry_label}");
    let mut failed_entry = build_single_generation_failed_chapter_entry(
        Some(&generated_result.chapter_id),
        Some(generated_result.chapter_number),
        Some(&generated_result.title),
        &error_message,
        persisted_task
            .as_ref()
            .map(|task| task.current_retry_count)
            .unwrap_or(0),
    );
    apply_single_generation_quality_gate_terminal_fields(
        &mut failed_entry,
        "auto_repair",
        retry_label,
        "quality_retry",
        quality_metrics,
    );

    SingleGenerationQualityGateTerminalState {
        checkpoint_payload: json!({
            "analysis_task_message": "单章生成已保存修复草稿，等待后续重试",
            "analysis_task_progress": 100,
            "analysis_last_error": Value::Null,
            "quality_gate_decision": "auto_repair",
            "quality_gate_label": retry_label,
            "phase": "quality_retry",
        }),
        error_message,
        failed_entry,
    }
}
