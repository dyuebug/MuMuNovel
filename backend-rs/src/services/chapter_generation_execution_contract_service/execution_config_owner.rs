use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::ai::AIConfig;
use crate::services::chapter_generation_prompt_service::{
    build_placeholder_prompt_context_provider_payload, PromptContextProviderPayload,
};
use crate::services::generation_contract_service::GenerationIntentKind;
use crate::services::role_model_policy_service::ResolvedRoleModelPolicyV1;
use crate::services::settings_service::SettingsService;

#[derive(Debug, Clone)]
pub(crate) struct PreparedRoleModelPolicyContext {
    pub(crate) resolved_policy: ResolvedRoleModelPolicyV1,
    pub(crate) allow_model_fallback: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedGenerationExecutionConfig {
    pub(crate) ai_config: AIConfig,
    pub(crate) provider_payload: PromptContextProviderPayload,
    pub(crate) role_policy_context: Option<PreparedRoleModelPolicyContext>,
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
        role_policy_context: None,
    })
}

pub(crate) async fn prepare_role_aware_generation_execution_config_with_provider_payload(
    db: &DatabaseConnection,
    user_id: &str,
    intent_kind: GenerationIntentKind,
    model_override: Option<&str>,
    provider_payload: PromptContextProviderPayload,
) -> Result<PreparedGenerationExecutionConfig, String> {
    let prepared = SettingsService::build_role_aware_ai_config(
        db,
        user_id,
        intent_kind,
        None,
        model_override,
        None,
    )
    .await?;

    Ok(PreparedGenerationExecutionConfig {
        ai_config: prepared.ai_config,
        provider_payload,
        role_policy_context: Some(PreparedRoleModelPolicyContext {
            resolved_policy: prepared.resolved_policy,
            allow_model_fallback: prepared.allow_model_fallback,
        }),
    })
}

pub(crate) async fn prepare_role_aware_generation_execution_config(
    db: &DatabaseConnection,
    user_id: &str,
    intent_kind: GenerationIntentKind,
    model_override: Option<&str>,
) -> Result<PreparedGenerationExecutionConfig, String> {
    prepare_role_aware_generation_execution_config_with_provider_payload(
        db,
        user_id,
        intent_kind,
        model_override,
        build_placeholder_prompt_context_provider_payload(),
    )
    .await
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
        "python_source_map": [],
        "historical_python_test_support": [
            "backend/tests/test_support/ai_gateway/ai_config.py",
            "backend/tests/test_support/ai_gateway/ai_service.py"
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
            "prepare_role_aware_generation_execution_config",
            "prepare_role_aware_generation_execution_config_with_provider_payload",
            "build_user_ai_config",
            "PreparedGenerationExecutionConfig"
        ],
        "behavior_contract": {
            "ai_config_owner": "SettingsService::build_ai_config",
            "role_aware_ai_config_owner": "SettingsService::build_role_aware_ai_config",
            "role_policy_owner": "role_model_policy_service",
            "model_override_forwarded": true,
            "provider_payload_passthrough": true,
            "role_policy_context_forwarded": true,
            "default_provider_payload": "build_placeholder_prompt_context_provider_payload",
            "prepared_fields": ["ai_config", "provider_payload", "role_policy_context"],
            "legacy_entrypoints_remain_compatible": true,
            "error_boundary": "SettingsService AI config builder string error",
            "route_payload_shape_changed": false
        },
        "active_consumers": [
            "chapter_single_generation_prepare_service",
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_batch_generation_write_workflow_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_generation_execution_contract_service",
            "chapter-single-generation-active-gateway-smoke-rust",
            "chapter-batch-generation-active-gateway-smoke-rust"
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
            "remaining_cutover_gate": "none_python_ai_gateway_source_map_deleted",
            "status": "rust_shared_execution_config_owner_with_deleted_python_ai_gateway_source_map"
        },
        "rollback_boundary": {
            "source_map_policy": "production_python_ai_gateway_source_map_deleted_historical_fixtures_live_under_tests_test_support",
            "runtime_knob": "SettingsService AI provider configuration plus ChapterCandidateRouteGatewayConfig",
            "compatibility_note": "Execution config must keep model_override and provider payload behavior stable for single and batch generation"
        }
    })
}
