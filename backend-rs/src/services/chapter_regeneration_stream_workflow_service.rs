use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
use crate::services::chapter_regeneration_prepare_service::{
    prepare_chapter_regeneration_stream, prepare_partial_regeneration_stream,
    validate_full_chapter_regeneration_stream_request_bounds,
    validate_partial_regeneration_stream_request_bounds, BuildRegenerationAiServiceError,
    FullChapterRegenerationStreamInput, FullChapterRegenerationStreamRequest,
    PartialChapterRegenerationStreamInput, PartialRegenerationStreamWorkflowRequest,
    PreparePartialRegenerationStreamError,
};
use crate::services::chapter_regeneration_stream_launch_service::{
    build_owned_regeneration_stream, OwnedRegenerationInitialEvent, OwnedRegenerationStream,
    OwnedRegenerationStreamLaunchInput, RegenerationChunkProgress,
};
use crate::services::chapter_regeneration_text_service::{
    finalize_chapter_regeneration_result, finalize_partial_regeneration_result,
};

fn build_full_chapter_regeneration_stream_launch_input(
    input: FullChapterRegenerationStreamInput,
) -> (OwnedRegenerationStreamLaunchInput, String) {
    let FullChapterRegenerationStreamInput {
        chapter_id,
        chapter_word_count,
        prompt,
        ai_service,
    } = input;

    (
        OwnedRegenerationStreamLaunchInput {
            task_label: "Chapter Rewrite".to_string(),
            prompt,
            ai_service,
            initial_events: vec![
                OwnedRegenerationInitialEvent::Preparing {
                    message: Some("Building rewrite prompt...".to_string()),
                },
                OwnedRegenerationInitialEvent::Generating {
                    message: Some("Rewriting chapter...".to_string()),
                    progress_range: (20, 95),
                    char_count: chapter_word_count,
                    retry_count: None,
                },
            ],
            completion_message: "Rewrite complete".to_string(),
        },
        chapter_id,
    )
}

fn build_full_chapter_regeneration_stream(
    input: FullChapterRegenerationStreamInput,
) -> OwnedRegenerationStream {
    let (launch_input, chapter_id) = build_full_chapter_regeneration_stream_launch_input(input);

    build_owned_regeneration_stream(
        launch_input,
        |_, _| None,
        move |full_content| finalize_chapter_regeneration_result(full_content, &chapter_id),
    )
}

fn build_partial_chapter_regeneration_stream_launch_input(
    input: PartialChapterRegenerationStreamInput,
) -> (
    OwnedRegenerationStreamLaunchInput,
    usize,
    usize,
    usize,
    usize,
) {
    let PartialChapterRegenerationStreamInput {
        target_words,
        original_word_count,
        start_position,
        end_position,
        prompt,
        ai_service,
    } = input;

    (
        OwnedRegenerationStreamLaunchInput {
            task_label: "Partial Rewrite".to_string(),
            prompt,
            ai_service,
            initial_events: vec![
                OwnedRegenerationInitialEvent::Preparing {
                    message: Some("Preparing rewrite context...".to_string()),
                },
                OwnedRegenerationInitialEvent::Preparing {
                    message: Some("Starting generation...".to_string()),
                },
            ],
            completion_message: "Rewrite complete".to_string(),
        },
        target_words,
        original_word_count,
        start_position,
        end_position,
    )
}

fn build_partial_chapter_regeneration_stream(
    input: PartialChapterRegenerationStreamInput,
) -> OwnedRegenerationStream {
    let (launch_input, target_words, original_word_count, start_position, end_position) =
        build_partial_chapter_regeneration_stream_launch_input(input);

    build_owned_regeneration_stream(
        launch_input,
        move |tracker, progress: RegenerationChunkProgress| {
            if progress.chunk_count % 5 == 0 {
                Some(tracker.generating(
                    Some(&format!(
                        "Generating rewrite... {}/{} chars",
                        progress.full_content_len, target_words
                    )),
                    (35, 95),
                    progress.full_content_len,
                    None,
                ))
            } else {
                None
            }
        },
        move |full_content| {
            finalize_partial_regeneration_result(
                full_content,
                original_word_count,
                start_position,
                end_position,
            )
        },
    )
}

pub enum CreateRegenerationStreamWorkflowError<TPrepareError> {
    Chapter(LoadAccessibleChapterError),
    Prepare(TPrepareError),
}

pub type CreateChapterRegenerationStreamWorkflowError =
    CreateRegenerationStreamWorkflowError<BuildRegenerationAiServiceError>;

pub type CreatePartialRegenerationStreamWorkflowError =
    CreateRegenerationStreamWorkflowError<PreparePartialRegenerationStreamError>;

pub async fn create_chapter_regeneration_stream_workflow(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    request: FullChapterRegenerationStreamRequest,
) -> Result<OwnedRegenerationStream, CreateChapterRegenerationStreamWorkflowError> {
    validate_full_chapter_regeneration_stream_request_bounds(&request)
        .map_err(CreateChapterRegenerationStreamWorkflowError::Prepare)?;

    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(CreateChapterRegenerationStreamWorkflowError::Chapter)?;
    let stream_input = prepare_chapter_regeneration_stream(db, user_id, &chapter, &request)
        .await
        .map_err(CreateChapterRegenerationStreamWorkflowError::Prepare)?;

    Ok(build_full_chapter_regeneration_stream(stream_input))
}

pub async fn create_partial_regeneration_stream_workflow(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    request: PartialRegenerationStreamWorkflowRequest,
) -> Result<OwnedRegenerationStream, CreatePartialRegenerationStreamWorkflowError> {
    validate_partial_regeneration_stream_request_bounds(&request)
        .map_err(PreparePartialRegenerationStreamError::Input)
        .map_err(CreatePartialRegenerationStreamWorkflowError::Prepare)?;

    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(CreatePartialRegenerationStreamWorkflowError::Chapter)?;
    let stream_input = prepare_partial_regeneration_stream(db, user_id, &chapter, &request)
        .await
        .map_err(CreatePartialRegenerationStreamWorkflowError::Prepare)?;

    Ok(build_partial_chapter_regeneration_stream(stream_input))
}

#[cfg(test)]
mod tests {
    use super::{
        build_full_chapter_regeneration_stream_launch_input,
        build_partial_chapter_regeneration_stream_launch_input,
        CreateChapterRegenerationStreamWorkflowError, CreatePartialRegenerationStreamWorkflowError,
        CreateRegenerationStreamWorkflowError,
    };
    use crate::ai::AIConfig;
    use crate::services::chapter_access_service::LoadAccessibleChapterError;
    use crate::services::chapter_regeneration_prepare_service::{
        BuildRegenerationAiServiceError, PreparePartialRegenerationError,
        PreparePartialRegenerationStreamError,
    };
    use crate::services::chapter_regeneration_prepare_service::{
        FullChapterRegenerationStreamInput, PartialChapterRegenerationStreamInput,
    };
    use crate::services::chapter_regeneration_stream_launch_service::OwnedRegenerationInitialEvent;

    #[test]
    fn full_regeneration_stream_workflow_error_alias_keeps_shared_outer_owner() {
        let error: CreateChapterRegenerationStreamWorkflowError =
            CreateRegenerationStreamWorkflowError::Chapter(
                LoadAccessibleChapterError::NotFoundOrAccessDenied,
            );

        assert!(matches!(
            error,
            CreateRegenerationStreamWorkflowError::Chapter(
                LoadAccessibleChapterError::NotFoundOrAccessDenied
            )
        ));
    }

    #[test]
    fn partial_regeneration_stream_workflow_error_alias_keeps_shared_outer_owner() {
        let error: CreatePartialRegenerationStreamWorkflowError =
            CreateRegenerationStreamWorkflowError::Prepare(
                PreparePartialRegenerationStreamError::Style("bad style".to_string()),
            );

        assert!(matches!(
            error,
            CreateRegenerationStreamWorkflowError::Prepare(
                PreparePartialRegenerationStreamError::Style(detail)
            ) if detail == "bad style"
        ));
    }

    #[test]
    fn shared_outer_owner_preserves_prepare_type_specificity() {
        let error: CreatePartialRegenerationStreamWorkflowError =
            CreateRegenerationStreamWorkflowError::Prepare(
                PreparePartialRegenerationStreamError::Input(
                    PreparePartialRegenerationError::InvalidRange,
                ),
            );

        assert!(matches!(
            error,
            CreateRegenerationStreamWorkflowError::Prepare(
                PreparePartialRegenerationStreamError::Input(
                    PreparePartialRegenerationError::InvalidRange
                )
            )
        ));

        let full_error: CreateChapterRegenerationStreamWorkflowError =
            CreateRegenerationStreamWorkflowError::Prepare(
                BuildRegenerationAiServiceError::InvalidConfig("missing provider".to_string()),
            );

        assert!(matches!(
            full_error,
            CreateRegenerationStreamWorkflowError::Prepare(
                BuildRegenerationAiServiceError::InvalidConfig(detail)
            ) if detail == "missing provider"
        ));
    }

    #[test]
    fn should_build_full_chapter_regeneration_stream_launch_input_contract() {
        let (launch_input, chapter_id) = build_full_chapter_regeneration_stream_launch_input(
            FullChapterRegenerationStreamInput {
                chapter_id: "chapter-1".to_string(),
                chapter_word_count: 2400,
                prompt: "prompt".to_string(),
                ai_service: crate::ai::service::AIService::new(AIConfig::default()),
            },
        );

        assert_eq!(chapter_id, "chapter-1");
        assert_eq!(launch_input.task_label, "Chapter Rewrite");
        assert_eq!(launch_input.prompt, "prompt");
        assert_eq!(launch_input.completion_message, "Rewrite complete");
        assert_eq!(launch_input.initial_events.len(), 2);
        assert!(matches!(
            &launch_input.initial_events[0],
            OwnedRegenerationInitialEvent::Preparing { message }
            if message.as_deref() == Some("Building rewrite prompt...")
        ));
        assert!(matches!(
            &launch_input.initial_events[1],
            OwnedRegenerationInitialEvent::Generating {
                message,
                progress_range,
                char_count,
                retry_count
            }
            if message.as_deref() == Some("Rewriting chapter...")
                && *progress_range == (20, 95)
                && *char_count == 2400
                && retry_count.is_none()
        ));
    }

    #[test]
    fn should_build_partial_chapter_regeneration_stream_launch_input_contract() {
        let (launch_input, target_words, original_word_count, start_position, end_position) =
            build_partial_chapter_regeneration_stream_launch_input(
                PartialChapterRegenerationStreamInput {
                    target_words: 1800,
                    original_word_count: 900,
                    start_position: 12,
                    end_position: 36,
                    prompt: "prompt".to_string(),
                    ai_service: crate::ai::service::AIService::new(AIConfig::default()),
                },
            );

        assert_eq!(target_words, 1800);
        assert_eq!(original_word_count, 900);
        assert_eq!(start_position, 12);
        assert_eq!(end_position, 36);
        assert_eq!(launch_input.task_label, "Partial Rewrite");
        assert_eq!(launch_input.prompt, "prompt");
        assert_eq!(launch_input.completion_message, "Rewrite complete");
        assert_eq!(launch_input.initial_events.len(), 2);
        assert!(matches!(
            &launch_input.initial_events[0],
            OwnedRegenerationInitialEvent::Preparing { message }
            if message.as_deref() == Some("Preparing rewrite context...")
        ));
        assert!(matches!(
            &launch_input.initial_events[1],
            OwnedRegenerationInitialEvent::Preparing { message }
            if message.as_deref() == Some("Starting generation...")
        ));
    }
}
