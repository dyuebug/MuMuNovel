use sea_orm::DatabaseConnection;

use crate::ai::AIConfig;
use crate::services::chapter_generation_prompt_context_provider_service::{
    build_placeholder_prompt_context_provider_payload, PromptContextProviderPayload,
};

use super::settings_service::SettingsService;

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
