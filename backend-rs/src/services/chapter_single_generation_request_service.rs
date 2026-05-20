use sea_orm::DatabaseConnection;

use crate::ai::AIConfig;
use crate::models::chapter;
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;

use super::chapter_batch_generation_access_service::{
    load_accessible_chapter_for_generation, prepare_generation_execution_config,
    LoadAccessibleChapterForGenerationError,
};

#[derive(Debug, Clone)]
pub(crate) struct SingleChapterGenerationRequest {
    pub(crate) target_word_count: Option<i32>,
    pub(crate) model: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct SingleChapterGenerationExecutionInput {
    pub(crate) chapter_id: String,
    pub(crate) target_word_count: i32,
    pub(crate) ai_config: AIConfig,
    pub(crate) provider_payload: PromptContextProviderPayload,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SingleChapterGenerationRequestCompatFields {
    pub(crate) enable_analysis: Option<bool>,
}

pub(crate) fn build_single_chapter_generation_request(
    target_word_count: Option<i32>,
    model: Option<String>,
) -> SingleChapterGenerationRequest {
    SingleChapterGenerationRequest {
        target_word_count,
        model,
    }
}

pub(crate) fn consume_single_chapter_generation_request_compat_fields(
    fields: &SingleChapterGenerationRequestCompatFields,
) {
    let _ = fields.enable_analysis;
}

#[derive(Debug)]
pub(crate) enum PrepareSingleChapterGenerationRequestError {
    ChapterNotFound,
    ChapterNotFoundOrAccessDenied,
    Config(String),
    Internal(String),
}

#[derive(Debug)]
pub(crate) struct PreparedSingleChapterGenerationRequest {
    pub(crate) chapter_model: chapter::Model,
    pub(crate) execution_input: SingleChapterGenerationExecutionInput,
}

fn normalize_single_chapter_generation_target_word_count(target_word_count: Option<i32>) -> i32 {
    target_word_count.unwrap_or(3000).max(1)
}

fn load_chapter_generation_target(request: &SingleChapterGenerationRequest) -> i32 {
    normalize_single_chapter_generation_target_word_count(request.target_word_count)
}

pub(crate) async fn prepare_single_chapter_generation_request(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    request: &SingleChapterGenerationRequest,
) -> Result<PreparedSingleChapterGenerationRequest, PrepareSingleChapterGenerationRequestError> {
    let chapter_model = load_accessible_chapter_for_generation(db, chapter_id, user_id)
        .await
        .map_err(|error| match error {
            LoadAccessibleChapterForGenerationError::ChapterNotFound => {
                PrepareSingleChapterGenerationRequestError::ChapterNotFound
            }
            LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied => {
                PrepareSingleChapterGenerationRequestError::ChapterNotFoundOrAccessDenied
            }
            LoadAccessibleChapterForGenerationError::Internal(error) => {
                PrepareSingleChapterGenerationRequestError::Internal(error)
            }
        })?;
    let execution_config =
        prepare_generation_execution_config(db, user_id, request.model.as_deref())
            .await
            .map_err(PrepareSingleChapterGenerationRequestError::Config)?;

    Ok(PreparedSingleChapterGenerationRequest {
        execution_input: SingleChapterGenerationExecutionInput {
            chapter_id: chapter_model.id.clone(),
            target_word_count: load_chapter_generation_target(request),
            ai_config: execution_config.ai_config,
            provider_payload: execution_config.provider_payload,
        },
        chapter_model,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_single_chapter_generation_request,
        consume_single_chapter_generation_request_compat_fields, load_chapter_generation_target,
        normalize_single_chapter_generation_target_word_count, SingleChapterGenerationRequest,
        SingleChapterGenerationRequestCompatFields,
    };

    #[test]
    fn should_normalize_single_chapter_generation_target_word_count() {
        assert_eq!(
            normalize_single_chapter_generation_target_word_count(None),
            3000
        );
        assert_eq!(
            normalize_single_chapter_generation_target_word_count(Some(-100)),
            1
        );
        assert_eq!(
            normalize_single_chapter_generation_target_word_count(Some(0)),
            1
        );
        assert_eq!(
            normalize_single_chapter_generation_target_word_count(Some(2500)),
            2500
        );
    }

    #[test]
    fn should_load_single_chapter_generation_target_from_request() {
        let request = SingleChapterGenerationRequest {
            target_word_count: Some(1800),
            model: None,
        };

        assert_eq!(load_chapter_generation_target(&request), 1800);
    }

    #[test]
    fn should_keep_single_chapter_generation_request_contract_minimal() {
        let request = SingleChapterGenerationRequest {
            target_word_count: Some(2200),
            model: Some("gpt-test".to_string()),
        };

        assert_eq!(request.target_word_count, Some(2200));
        assert_eq!(request.model.as_deref(), Some("gpt-test"));
    }

    #[test]
    fn should_build_single_chapter_generation_request_from_route_payload() {
        let request =
            build_single_chapter_generation_request(Some(2600), Some("gpt-4.1".to_string()));

        assert_eq!(request.target_word_count, Some(2600));
        assert_eq!(request.model.as_deref(), Some("gpt-4.1"));
    }

    #[test]
    fn should_consume_single_chapter_generation_request_compat_fields() {
        let fields = SingleChapterGenerationRequestCompatFields {
            enable_analysis: Some(true),
        };

        consume_single_chapter_generation_request_compat_fields(&fields);

        assert_eq!(fields.enable_analysis, Some(true));
    }
}
