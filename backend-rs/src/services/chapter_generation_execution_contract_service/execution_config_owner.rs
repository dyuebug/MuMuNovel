use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::ai::AIConfig;
use crate::services::chapter_generation_prompt_service::{
    build_placeholder_prompt_context_provider_payload, PromptContextProviderPayload,
};
use crate::services::settings_service::SettingsService;

#[derive(Debug, Clone)]
pub(crate) struct PreparedGenerationExecutionConfig {
    pub(crate) ai_config: AIConfig,
    pub(crate) provider_payload: PromptContextProviderPayload,
}

async fn build_user_ai_config(
    db: &DatabaseConnection,
    user_id: &str,
    model_override: Option<&str>,
) -> Result<AIConfig, String> {
    SettingsService::build_ai_config(db, user_id, None, model_override, None).await
}

pub(crate) async fn prepare_generation_execution_config_with_provider_payload(
    db: &DatabaseConnection,
    user_id: &str,
    model_override: Option<&str>,
    provider_payload: PromptContextProviderPayload,
) -> Result<PreparedGenerationExecutionConfig, String> {
    let ai_config = build_user_ai_config(db, user_id, model_override).await?;

    Ok(PreparedGenerationExecutionConfig {
        ai_config,
        provider_payload,
    })
}

pub(crate) async fn prepare_generation_execution_config(
    db: &DatabaseConnection,
    user_id: &str,
    model_override: Option<&str>,
) -> Result<PreparedGenerationExecutionConfig, String> {
    prepare_generation_execution_config_with_provider_payload(
        db,
        user_id,
        model_override,
        build_placeholder_prompt_context_provider_payload(),
    )
    .await
}

pub(crate) fn build_generation_execution_config_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_execution_contract_service::execution_config",
        "scope": "shared_generation_execution_config_bridge",
        "python_source_map": [
            "backend/app/services/chapter_generation/stream/execution_service.py",
            "backend/app/services/chapter_generation/stream/service.py",
            "backend/app/services/batch_generation_execution_service.py",
            "backend/app/services/ai_config.py",
            "backend/app/services/ai_gateway/ai_config.py"
        ],
        "rust_target_map": [
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs",
            "backend-rs/src/services/chapter_generation_execution_contract_service/execution_config_owner.rs",
            "backend-rs/src/services/settings_service.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "rust_owner_functions": [
            "prepare_generation_execution_config",
            "prepare_generation_execution_config_with_provider_payload",
            "build_user_ai_config",
            "PreparedGenerationExecutionConfig"
        ],
        "behavior_contract": {
            "ai_config_owner": "SettingsService::build_ai_config",
            "model_override_forwarded": true,
            "provider_payload_passthrough": true,
            "default_provider_payload": "build_placeholder_prompt_context_provider_payload",
            "prepared_fields": ["ai_config", "provider_payload"],
            "error_boundary": "SettingsService::build_ai_config string error",
            "route_payload_shape_changed": false
        },
        "active_consumers": [
            "chapter_single_generation_prepare_service",
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_batch_generation_write_workflow_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_generation_execution_contract_service",
            "chapter_single_generation_active_gateway_smoke_service",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "validation_boundary": [
            "cargo test chapter_generation_execution_contract_service",
            "cargo test chapter_single_generation_prepare_service",
            "cargo test chapter_batch_generation_write_workflow_service",
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test api::health",
            "cargo check --manifest-path backend-rs/Cargo.toml"
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
            "execution_config_owner": "prepare_generation_execution_config_with_provider_payload",
            "provider_payload_owner": "PreparedGenerationExecutionConfig",
            "source_map_closeout_ready": true,
            "remaining_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
            "status": "rust_shared_execution_config_owner_ready_for_source_map_closeout_review"
        },
        "rollback_boundary": {
            "source_map_policy": "keep_python_generation_execution_config_shells_as_source_map_until_explicit_freeze_delete_round",
            "runtime_knob": "SettingsService AI provider configuration plus ChapterCandidateRouteGatewayConfig",
            "compatibility_note": "Execution config must keep model_override and provider payload behavior stable for single and batch generation"
        }
    })
}
