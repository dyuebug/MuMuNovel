use serde_json::{json, Value};

use crate::services::chapter_batch_generation_read_context_service::stream_state_owner::insert_stream_candidate_gateway;

pub(crate) fn build_batch_generation_stream_progress_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_read_context_service::stream_progress_owner",
        "scope": "batch_generation_stream_progress_event_projection",
        "python_source_map": [
            "backend/app/services/chapter_candidate_event_service.py",
            "backend/app/services/batch_generation/status_response_builder.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service/stream_progress_owner.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_batch_generation_stream_progress_event"
            ],
            "event_type": "progress",
            "fields": [
                "message",
                "progress",
                "status",
                "phase",
                "current_retry_count",
                "max_retries",
                "candidate_gateway"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_read_context_service",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "validation_boundary": [
            "cargo test chapter_batch_generation_read_context_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 11,
            "python_fallback_probe_count": 0,
            "stream_progress_event_owner": "build_batch_generation_stream_progress_event",
            "stream_state_event_owner": "BatchGenerationStreamState::events",
            "event_type": "progress",
            "candidate_gateway_projection_owner": "BatchGenerationStreamProgressEventInput.candidate_gateway",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": false,
            "remaining_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
            "status": "rust_batch_generation_stream_progress_owner_ready_for_source_map_closeout_review"
        },
        "rollback_boundary": {
            "source_map_policy": "keep_python_candidate_event_projection_as_source_map_until_same_round_route_readiness_closeout",
            "runtime_state_keys": [
                "progress",
                "phase",
                "last_message"
            ]
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationStreamProgressEventInput {
    pub(crate) message: String,
    pub(crate) progress: i32,
    pub(crate) status: &'static str,
    pub(crate) phase: String,
    pub(crate) current_retry_count: i32,
    pub(crate) max_retries: i32,
    pub(crate) candidate_gateway: Option<Value>,
}

pub(crate) fn build_batch_generation_stream_progress_event(
    input: BatchGenerationStreamProgressEventInput,
) -> Value {
    let mut event = json!({
        "type": "progress",
        "message": input.message,
        "progress": input.progress,
        "status": input.status,
        "phase": input.phase,
        "current_retry_count": input.current_retry_count,
        "max_retries": input.max_retries,
    });
    insert_stream_candidate_gateway(&mut event, input.candidate_gateway.as_ref());
    event
}
