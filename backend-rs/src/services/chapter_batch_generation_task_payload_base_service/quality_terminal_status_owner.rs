use serde_json::{json, Map, Value};

use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_generation_execution_contract_service::active_story_repair_payload_from_runtime_state;
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    build_generation_quality_runtime_owner_contract,
    resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state, retryable_repair_label,
    retryable_repair_label_from_quality_context_with_retry_budget,
    BatchGenerationQualityRuntimeContext,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BatchGenerationQualityStatusContext {
    pub latest_quality_metrics: Option<Value>,
    pub quality_metrics_history: Option<Value>,
    pub quality_metrics_summary_state: Option<Value>,
    pub quality_metrics_summary: Option<Value>,
    pub quality_history_context: Option<Value>,
    pub active_story_repair_payload: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationFailedTerminalKind {
    ManualReview,
    Retry,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationFailedTerminalSemantics {
    pub(crate) kind: BatchGenerationFailedTerminalKind,
    pub(crate) reason: &'static str,
    pub(crate) label: String,
    pub(crate) review_required: bool,
    pub(crate) can_resume: bool,
}

impl BatchGenerationQualityStatusContext {
    pub fn from_snapshot_and_runtime_state(
        snapshot: Option<&batch_generation_snapshot::Model>,
        workflow_runtime_state: Option<&Value>,
    ) -> Self {
        let active_story_repair_payload =
            active_story_repair_payload_from_runtime_state(workflow_runtime_state);
        let quality_runtime_context =
            resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state(
                snapshot,
                workflow_runtime_state,
            );

        Self::from_runtime_quality_context_and_active_payload(
            &quality_runtime_context,
            active_story_repair_payload.as_ref(),
        )
    }

    pub fn insert_into_payload(&self, payload: &mut Map<String, Value>) {
        payload.insert(
            "latest_quality_metrics".to_string(),
            serde_json::json!(self.latest_quality_metrics),
        );
        payload.insert(
            "quality_metrics_history".to_string(),
            serde_json::json!(self.quality_metrics_history),
        );
        payload.insert(
            "quality_metrics_summary_state".to_string(),
            serde_json::json!(self.quality_metrics_summary_state),
        );
        payload.insert(
            "quality_metrics_summary".to_string(),
            serde_json::json!(self.quality_metrics_summary),
        );
        payload.insert(
            "quality_history_context".to_string(),
            serde_json::json!(self.quality_history_context),
        );
        payload.insert(
            "active_story_repair_payload".to_string(),
            serde_json::json!(self.active_story_repair_payload),
        );
    }

    pub fn from_runtime_quality_context_and_active_payload(
        quality_runtime_context: &BatchGenerationQualityRuntimeContext,
        active_story_repair_payload: Option<&Value>,
    ) -> Self {
        Self {
            latest_quality_metrics: quality_runtime_context.latest_quality_metrics.clone(),
            quality_metrics_history: quality_runtime_context.quality_metrics_history.clone(),
            quality_metrics_summary_state: quality_runtime_context
                .quality_metrics_summary_state
                .clone(),
            quality_metrics_summary: quality_runtime_context.quality_metrics_summary.clone(),
            quality_history_context: quality_runtime_context.quality_history_context.clone(),
            active_story_repair_payload: active_story_repair_payload.cloned(),
        }
    }
}

pub(crate) fn insert_batch_generation_terminal_status_payload(
    payload: &mut Map<String, Value>,
    task: &batch_generation_task::Model,
    failed_chapters: Option<&Value>,
    quality_status_context: Option<&BatchGenerationQualityStatusContext>,
) {
    let (terminal_reason, terminal_label, review_required, can_resume) = if task.status == "failed"
    {
        resolve_failed_terminal_semantics(task, failed_chapters, quality_status_context)
            .map(|semantics| match semantics.kind {
                BatchGenerationFailedTerminalKind::ManualReview
                | BatchGenerationFailedTerminalKind::Retry
                | BatchGenerationFailedTerminalKind::Error => {
                    (Some("error"), Some("执行失败".to_string()), false, true)
                }
            })
            .unwrap_or((Some("error"), Some("执行失败".to_string()), false, true))
    } else {
        match task.status.as_str() {
            "completed" => (Some("completed"), Some("已完成".to_string()), false, false),
            "cancelled" => (Some("cancelled"), Some("已取消".to_string()), false, true),
            _ => (None, None, false, false),
        }
    };

    payload.insert(
        "terminal_reason".to_string(),
        serde_json::json!(terminal_reason),
    );
    payload.insert(
        "terminal_label".to_string(),
        serde_json::json!(terminal_label),
    );
    payload.insert(
        "review_required".to_string(),
        serde_json::json!(review_required),
    );
    payload.insert("can_resume".to_string(), serde_json::json!(can_resume));
}

pub(crate) fn resolve_failed_terminal_semantics(
    task: &batch_generation_task::Model,
    failed_chapters: Option<&Value>,
    quality_status_context: Option<&BatchGenerationQualityStatusContext>,
) -> Option<BatchGenerationFailedTerminalSemantics> {
    resolve_failed_terminal_semantics_from_sources(
        failed_chapters,
        quality_status_context,
        task.current_retry_count,
        task.max_retries,
    )
}

pub(crate) fn resolve_failed_terminal_semantics_from_sources(
    failed_chapters: Option<&Value>,
    quality_status_context: Option<&BatchGenerationQualityStatusContext>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<BatchGenerationFailedTerminalSemantics> {
    if let Some(label) = retryable_repair_label(failed_chapters, current_retry_count, max_retries)
        .or_else(|| {
            quality_status_context.and_then(|context| {
                retryable_repair_label_from_quality_context_with_retry_budget(
                    context.active_story_repair_payload.as_ref(),
                    context.quality_metrics_summary.as_ref(),
                    context.latest_quality_metrics.as_ref(),
                    current_retry_count,
                    max_retries,
                )
            })
        })
    {
        return Some(BatchGenerationFailedTerminalSemantics {
            kind: BatchGenerationFailedTerminalKind::Retry,
            reason: "retry",
            label,
            review_required: false,
            can_resume: true,
        });
    }

    Some(BatchGenerationFailedTerminalSemantics {
        kind: BatchGenerationFailedTerminalKind::Error,
        reason: "error",
        label: "执行失败".to_string(),
        review_required: false,
        can_resume: true,
    })
}

pub(crate) fn build_batch_generation_quality_terminal_status_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_task_payload_base_service::quality_terminal_status_owner",
        "scope": "batch_generation_quality_status_context_failed_terminal_semantics_and_status_payload_projection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service/quality_terminal_status_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "quality_status_context_entrypoints": [
                "BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state",
                "BatchGenerationQualityStatusContext::from_runtime_quality_context_and_active_payload",
                "BatchGenerationQualityStatusContext::insert_into_payload"
            ],
            "failed_terminal_semantics_entrypoints": [
                "insert_batch_generation_terminal_status_payload",
                "resolve_failed_terminal_semantics",
                "resolve_failed_terminal_semantics_from_sources"
            ],
            "status_payload_projection_entrypoints": [
                "build_batch_generation_task_view_payload_with_quality_context",
                "build_batch_generation_status_task_payload_with_quality_context",
                "build_batch_generation_status_task_payload_from_task_and_snapshot_projection"
            ],
            "projected_fields": [
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary_state",
                "quality_metrics_summary",
                "quality_history_context",
                "active_story_repair_payload",
                "terminal_reason",
                "terminal_label",
                "review_required",
                "can_resume"
            ],
            "failed_terminal_contract": {
                "manual_review": "manual review is telemetry-only and must not create review_required terminal semantics",
                "retry": "retry and generic error keep can_resume=true and preserve execution_failed_label fallback",
                "completed_cancelled": "completed and cancelled states project stable terminal_reason/terminal_label pairs"
            },
            "quality_context_dependencies": [
                "resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state",
                "active_story_repair_payload_from_runtime_state",
                "retryable_repair_label_from_quality_context_with_retry_budget"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_task_payload_base_service",
            "chapter_batch_generation_read_context_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_write_workflow_service",
            "chapter_batch_generation_resume_task_command_service"
        ],
        "validation_boundary": [
            "cargo test chapter_batch_generation_task_payload_base_service",
            "cargo test api::health",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 11,
            "python_fallback_probe_count": 0,
            "quality_status_context_owner": "BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state",
            "failed_terminal_semantics_owner": "resolve_failed_terminal_semantics_from_sources",
            "status_payload_projection_owner": "build_batch_generation_status_task_payload_from_task_and_snapshot_projection",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "batch-generation quality-terminal-status direct source-map deleted; surviving Python closeout work for this owner is now limited to shared batch-generation-snapshot schema/runtime/database/API-test hold and shared batch-generation-task schema/runtime/API/test-support packages",
            "status": "rust_batch_generation_quality_terminal_status_owner_direct_source_map_deleted"
        },
        "shared_schema_hold_status": {
            "batch_generation_task_model": "shared_python_runtime_api_and_test_support_reference",
            "default_python_module_consumers": [
                "backend/tests/test_support/database_test_support.py",
                "backend/tests/test_support/task_system/snapshot_runtime_persistence.py"
            ],
            "dedicated_python_regression_surfaces": [
                "backend/tests/test_api/test_chapters.py",
                "backend/tests/test_api/test_chapters_batch_generation.py",
                "backend/tests/test_api/test_chapters_batch_status_resume.py"
            ],
            "test_support_consumers": [
                "backend/tests/test_support/batch_generation_status_read_owner_test_adapter.py",
                "backend/tests/test_support/batch_generation_orchestration_test_adapter.py",
                "backend/tests/test_support/batch_generation_route_test_adapter.py"
            ],
            "physical_closeout_ready": false
        },
        "rollback_boundary": {
            "source_map_policy": "batch_generation_quality_terminal_status_owner_is_rust_only_and_surviving_python_schema_runtime_surfaces_are_tracked_by_shared_task_contracts",
            "payload_fields": [
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary_state",
                "quality_metrics_summary",
                "quality_history_context",
                "active_story_repair_payload",
                "terminal_reason",
                "terminal_label",
                "review_required",
                "can_resume"
            ]
        },
        "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract()
    })
}
