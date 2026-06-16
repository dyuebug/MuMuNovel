use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};

use super::{SingleChapterGenerationCompatOptions, BATCH_REQUEST_RUNTIME_STATE_KEY};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BatchGenerationRequestRuntimeState {
    pub(crate) compat_options: SingleChapterGenerationCompatOptions,
    pub(crate) model_override: Option<String>,
}

pub(crate) fn deserialize_optional_non_null<'de, D, T>(
    deserializer: D,
) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

impl BatchGenerationRequestRuntimeState {
    pub(crate) fn new(
        compat_options: SingleChapterGenerationCompatOptions,
        model_override: Option<String>,
    ) -> Self {
        Self {
            compat_options,
            model_override,
        }
    }

    pub(crate) fn active_story_repair_payload_with_scope(&self, scope: &str) -> Option<Value> {
        let summary = self.compat_options.story_repair_summary().trim();
        let repair_targets = self
            .compat_options
            .story_repair_targets()
            .iter()
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let preserve_strengths = self
            .compat_options
            .story_preserve_strengths()
            .iter()
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();

        if summary.is_empty() && repair_targets.is_empty() && preserve_strengths.is_empty() {
            return None;
        }

        Some(json!({
            "summary": if summary.is_empty() { Value::Null } else { json!(summary) },
            "repair_targets": repair_targets,
            "preserve_strengths": preserve_strengths,
            "focus_areas": Vec::<String>::new(),
            "weakest_metric_key": Value::Null,
            "weakest_metric_label": Value::Null,
            "weakest_metric_value": Value::Null,
            "quality_gate": Value::Null,
            "quality_gate_status": Value::Null,
            "quality_gate_decision": Value::Null,
            "quality_gate_label": Value::Null,
            "quality_gate_summary": Value::Null,
            "quality_gate_failed_metrics": Vec::<String>::new(),
            "source": "manual_request",
            "source_label": "Manual request",
            "scope": scope,
            "updated_at": Value::Null,
        }))
    }
}

pub(crate) fn batch_generation_request_runtime_state_payload(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
) -> Value {
    let mut payload = serde_json::Map::from_iter([(
        BATCH_REQUEST_RUNTIME_STATE_KEY.to_string(),
        json!(request_runtime_state),
    )]);
    if let Some(active_story_repair_payload) =
        request_runtime_state.active_story_repair_payload_with_scope("batch")
    {
        payload.insert(
            "active_story_repair_payload".to_string(),
            active_story_repair_payload,
        );
    }

    Value::Object(payload)
}

pub(crate) fn parse_batch_generation_request_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> BatchGenerationRequestRuntimeState {
    workflow_runtime_state
        .and_then(|state| state.get(BATCH_REQUEST_RUNTIME_STATE_KEY).cloned())
        .and_then(|value| serde_json::from_value::<BatchGenerationRequestRuntimeState>(value).ok())
        .unwrap_or_default()
}

pub(crate) fn active_story_repair_payload_from_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> Option<Value> {
    active_story_repair_payload_ref_from_runtime_state(workflow_runtime_state).cloned()
}

pub(crate) fn active_story_repair_payload_ref_from_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> Option<&Value> {
    workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("active_story_repair_payload"))
        .filter(|payload| payload.is_object())
}

pub(crate) fn build_batch_request_runtime_state_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_execution_contract_service::request_runtime_state",
        "scope": "batch_request_runtime_state_payload_and_story_repair_projection",
        "python_source_map": [
            "backend/app/services/batch_generation/create_service.py",
            "backend/app/services/batch_generation_orchestration_service.py",
            "backend/app/services/story_repair_payload_service.py"
        ],
        "rust_target_map": [
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs",
            "backend-rs/src/services/chapter_generation_execution_contract_service/request_runtime_state_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "request_runtime_state_key": BATCH_REQUEST_RUNTIME_STATE_KEY,
            "request_runtime_state_fields": [
                "compat_options",
                "model_override",
                "active_story_repair_payload"
            ],
            "entrypoints": [
                "BatchGenerationRequestRuntimeState::new",
                "BatchGenerationRequestRuntimeState::active_story_repair_payload_with_scope",
                "batch_generation_request_runtime_state_payload",
                "parse_batch_generation_request_runtime_state",
                "active_story_repair_payload_from_runtime_state",
                "active_story_repair_payload_ref_from_runtime_state"
            ],
            "empty_payload_policy": "empty_summary_targets_and_strengths_do_not_emit_active_story_repair_payload",
            "parse_fallback_policy": "missing_or_malformed_batch_request_runtime_state_returns_default",
            "payload_extraction_policy": "active_story_repair_payload_must_be_object"
        },
        "active_consumers": [
            "chapter_generation_execution_contract_service",
            "chapter_batch_generation_write_workflow_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_resume_task_command_service",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "validation_boundary": [
            "cargo test chapter_generation_execution_contract_service",
            "cargo test chapter_batch_generation_write_workflow_service",
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_batch_request_runtime_state_shells_as_source_map_until_explicit_freeze_delete_round",
            "runtime_knob": "BatchGenerationRequestRuntimeState plus active story repair payload projection",
            "compatibility_note": "Batch request runtime-state key and active story-repair payload extraction remain stable for batch create, runtime, and resume flows"
        }
    })
}
