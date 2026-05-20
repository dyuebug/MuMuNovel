use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::services::chapter_batch_generation_dispatch_service::dispatch_batch_generation_runtime;

use super::chapter_batch_generation_access_service::{
    prepare_generation_execution_config, verify_project_access,
};
use super::chapter_batch_generation_create_service::{
    prepare_batch_generation_create_request, PrepareBatchGenerationCreateRequestError,
};
use super::chapter_batch_generation_request_compat_service::BatchGenerationRequestCompatFields;
use super::chapter_batch_generation_task_command_service::create_batch_generation_task_plan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CreateBatchGenerationWorkflowDomainError {
    InvalidCount,
    ChaptersNotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CreateBatchGenerationWorkflowError {
    ProjectNotFoundOrAccessDenied,
    Domain(CreateBatchGenerationWorkflowDomainError),
    Config(String),
    Internal(String),
}

fn map_prepare_error(
    error: PrepareBatchGenerationCreateRequestError,
) -> CreateBatchGenerationWorkflowError {
    match error {
        PrepareBatchGenerationCreateRequestError::InvalidCount => {
            CreateBatchGenerationWorkflowError::Domain(
                CreateBatchGenerationWorkflowDomainError::InvalidCount,
            )
        }
        PrepareBatchGenerationCreateRequestError::ChaptersNotFound => {
            CreateBatchGenerationWorkflowError::Domain(
                CreateBatchGenerationWorkflowDomainError::ChaptersNotFound,
            )
        }
        PrepareBatchGenerationCreateRequestError::Internal(error) => {
            CreateBatchGenerationWorkflowError::Internal(error)
        }
    }
}

fn normalize_batch_generation_enable_analysis(enable_analysis: Option<bool>) -> bool {
    enable_analysis.unwrap_or(false)
}

fn normalize_batch_generation_max_retries(max_retries: Option<i32>) -> i32 {
    max_retries.unwrap_or(3)
}

pub(crate) async fn start_owned_batch_generation_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    start_chapter_number: i32,
    count: i32,
    style_id: Option<i32>,
    target_word_count: Option<i32>,
    enable_analysis: Option<bool>,
    max_retries: Option<i32>,
    model_override: Option<String>,
    compat_fields: BatchGenerationRequestCompatFields,
) -> Result<Value, CreateBatchGenerationWorkflowError> {
    let _ = (
        compat_fields.enable_mcp,
        compat_fields.enable_web_research,
        compat_fields.web_research_query.as_deref(),
        compat_fields.creative_mode.as_deref(),
        compat_fields.story_focus.as_deref(),
        compat_fields.plot_stage.as_deref(),
        compat_fields.story_creation_brief.as_deref(),
        compat_fields.quality_preset.as_deref(),
        compat_fields.quality_notes.as_deref(),
        compat_fields.story_repair_summary.as_deref(),
        compat_fields.story_repair_targets.as_deref(),
        compat_fields.story_preserve_strengths.as_deref(),
    );
    let has_access = verify_project_access(db, project_id, user_id)
        .await
        .map_err(CreateBatchGenerationWorkflowError::Internal)?;
    if !has_access {
        return Err(CreateBatchGenerationWorkflowError::ProjectNotFoundOrAccessDenied);
    }

    let prepared = prepare_batch_generation_create_request(
        db,
        project_id,
        start_chapter_number,
        count,
        target_word_count,
    )
    .await
    .map_err(map_prepare_error)?;

    let execution_config = prepare_generation_execution_config(db, user_id, model_override.as_deref())
        .await
        .map_err(CreateBatchGenerationWorkflowError::Config)?;

    let plan = create_batch_generation_task_plan(
        db,
        project_id,
        user_id,
        start_chapter_number,
        &prepared.chapters_to_generate,
        style_id,
        prepared.normalized_target_word_count,
        normalize_batch_generation_enable_analysis(enable_analysis),
        normalize_batch_generation_max_retries(max_retries),
    )
    .await
    .map_err(CreateBatchGenerationWorkflowError::Internal)?;

    let response_payload = plan.response_payload;
    let chapter_ids = plan.chapter_ids;
    let target_word_count = plan.target_word_count;
    let created_task_id = plan.created_task_id;
    let ai_config = execution_config.ai_config;
    let provider_payload = execution_config.provider_payload;

    dispatch_batch_generation_runtime(
        db.clone(),
        created_task_id,
        user_id.to_string(),
        chapter_ids,
        target_word_count,
        ai_config,
        provider_payload,
    );

    Ok(response_payload)
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_batch_generation_enable_analysis, normalize_batch_generation_max_retries,
    };

    #[test]
    fn should_normalize_batch_generation_enable_analysis() {
        assert!(!normalize_batch_generation_enable_analysis(None));
        assert!(!normalize_batch_generation_enable_analysis(Some(false)));
        assert!(normalize_batch_generation_enable_analysis(Some(true)));
    }

    #[test]
    fn should_normalize_batch_generation_max_retries() {
        assert_eq!(normalize_batch_generation_max_retries(None), 3);
        assert_eq!(normalize_batch_generation_max_retries(Some(0)), 0);
        assert_eq!(normalize_batch_generation_max_retries(Some(5)), 5);
    }
}
