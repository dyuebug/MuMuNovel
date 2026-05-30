use std::convert::Infallible;

use axum::response::sse::Event;
use sea_orm::DatabaseConnection;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::services::chapter_analysis_runtime_service::{
    dispatch_prepared_chapter_analysis_trigger, prepare_chapter_analysis_trigger,
};
use crate::utils::sse::{sse_done, sse_error, sse_json, sse_result, SseProgress};

use super::chapter_single_generation_prepare_service::{
    prepare_single_chapter_generation_request, PrepareSingleChapterGenerationRequestError,
    SingleChapterGenerationRequest,
};
use super::chapter_single_generation_runtime_state_service::{
    execute_owned_single_chapter_generation, SingleGenerationRuntimeLaunchInput,
};

pub(crate) type SingleChapterGenerationStream = ReceiverStream<Result<Event, Infallible>>;

pub(crate) async fn create_single_generation_stream_workflow(
    db: DatabaseConnection,
    user_id: String,
    chapter_id: String,
    request: SingleChapterGenerationRequest,
) -> Result<
    tokio_stream::wrappers::ReceiverStream<
        Result<axum::response::sse::Event, std::convert::Infallible>,
    >,
    PrepareSingleChapterGenerationRequestError,
> {
    let (chapter_target, execution_input) =
        prepare_single_chapter_generation_request(&db, &chapter_id, &user_id, &request).await?;
    let runtime_input = SingleGenerationRuntimeLaunchInput {
        chapter_id: chapter_target.chapter_id,
        user_id,
        execution_input,
    };
    let stream = launch_owned_single_chapter_generation_stream(db, runtime_input);

    Ok(stream)
}

fn launch_owned_single_chapter_generation_stream(
    db: DatabaseConnection,
    launch: SingleGenerationRuntimeLaunchInput,
) -> SingleChapterGenerationStream {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);

    tokio::spawn(async move {
        let target_word_count = launch.execution_input.target_word_count;
        let enable_analysis = launch.execution_input.compat_options.enable_analysis();
        let runtime_user_id = launch.user_id.clone();
        let mut tracker = SseProgress::new("Chapter Generation");
        let _ = tx.send(Ok(tracker.start())).await;
        let _ = tx
            .send(Ok(tracker.preparing(Some("Preparing chapter generation..."))))
            .await;
        let _ = tx
            .send(Ok(tracker.generating(
                Some("Generating chapter content..."),
                (15, 95),
                target_word_count as usize,
                None,
            )))
            .await;

        match execute_owned_single_chapter_generation(&db, launch).await {
            Ok(result) => {
                let analysis_task_id = if enable_analysis {
                    match prepare_chapter_analysis_trigger(&db, &result.chapter_id, &runtime_user_id).await {
                        Ok(create_state) => {
                            let task_id = create_state.task_id.clone();
                            dispatch_prepared_chapter_analysis_trigger(
                                db.clone(),
                                runtime_user_id.clone(),
                                create_state,
                            );
                            Some(task_id)
                        }
                        Err(_) => None,
                    }
                } else {
                    None
                };

                if enable_analysis {
                    let _ = tx
                        .send(Ok(sse_json(&json!({
                            "task_id": analysis_task_id,
                            "message": "章节分析任务已启动",
                            "type": "analysis_started",
                        }))))
                        .await;
                }
                let _ = tx
                    .send(Ok(tracker.complete(Some("Generation complete"))))
                    .await;
                let response_payload =
                    build_single_generation_stream_result_payload(&result, analysis_task_id.as_deref());
                let _ = tx.send(Ok(sse_result(&response_payload))).await;
                let _ = tx.send(Ok(sse_done())).await;
            }
            Err(error_message) => {
                let _ = tx.send(Ok(sse_error(&error_message, 500))).await;
            }
        }
    });

    ReceiverStream::new(rx)
}

fn build_single_generation_stream_result_payload(
    result: &crate::services::chapter_generation_runtime_service::GeneratedChapterResult,
    analysis_task_id: Option<&str>,
) -> serde_json::Value {
    json!({
        "chapter_id": result.chapter_id,
        "chapter_number": result.chapter_number,
        "title": result.title,
        "content": result.content,
        "word_count": result.word_count,
        "saved_word_count": result.word_count,
        "chapter_status": "draft",
        "content_applied": true,
        "content_source": "chapter",
        "analysis_task_id": analysis_task_id,
    })
}

#[cfg(test)]
mod tests {
    use sea_orm::Set;
    use serde_json::json;

    use super::{
        build_single_generation_stream_result_payload, launch_owned_single_chapter_generation_stream,
        SingleChapterGenerationRequest,
    };
    use crate::ai::AIConfig;
    use crate::services::chapter_batch_generation_access_service::LoadAccessibleChapterForGenerationError;
    use crate::services::chapter_batch_generation_task_model_service::build_batch_generation_task_active_model;
    use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
    use crate::services::chapter_single_generation_prepare_service::{
        PrepareSingleChapterGenerationRequestError, SingleChapterGenerationCompatOptions,
        SingleChapterGenerationExecutionInput, SingleChapterGenerationTarget,
    };
    use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;

    fn empty_compat_options() -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: None,
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: false,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: Vec::new(),
            story_preserve_strengths: Vec::new(),
        }
    }

    #[test]
    fn should_keep_background_workflow_error_contract_shape() {
        let chapter_error = PrepareSingleChapterGenerationRequestError::Chapter(
            LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
        );
        let config_error =
            PrepareSingleChapterGenerationRequestError::Config("model missing".to_string());
        let internal_error =
            PrepareSingleChapterGenerationRequestError::Internal("db failed".to_string());

        assert!(matches!(
            chapter_error,
            PrepareSingleChapterGenerationRequestError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied
            )
        ));
        assert!(matches!(
            config_error,
            PrepareSingleChapterGenerationRequestError::Config(detail) if detail == "model missing"
        ));
        assert!(matches!(
            internal_error,
            PrepareSingleChapterGenerationRequestError::Chapter(_)
                | PrepareSingleChapterGenerationRequestError::Config(_)
                | PrepareSingleChapterGenerationRequestError::Internal(_)
        ));
    }

    #[test]
    fn should_build_single_generation_task_chapter_payload_from_parts() {
        let payload = build_batch_generation_task_active_model(
            "task-2".to_string(),
            "project-2".to_string(),
            "user-2".to_string(),
            8,
            1,
            json!([{
                "id": "chapter-2",
                "chapter_number": 8,
                "title": "第八章",
            }]),
            None,
            2100,
            false,
            1,
            Some("chapter-2".to_string()),
            Some(8),
            0,
            chrono::NaiveDateTime::default(),
        );

        assert_eq!(payload.chapter_ids, Set(json!([{
            "id": "chapter-2",
            "chapter_number": 8,
            "title": "第八章",
        }])));
    }

    #[test]
    fn should_build_single_generation_background_runtime_input_contract() {
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-9".to_string(),
            user_id: "user-42".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2400,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: String::new(),
                        previous_chapter_summary: String::new(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                },
            },
        };

        assert_eq!(runtime_input.chapter_id, "chapter-9");
        assert_eq!(runtime_input.user_id, "user-42");
        assert_eq!(runtime_input.execution_input.target_word_count, 2400);
        assert_eq!(
            runtime_input
                .execution_input
                .execution_config
                .provider_payload
                .characters_info,
            "[]"
        );
    }

    #[test]
    fn should_build_single_generation_stream_launch_input_from_runtime_parts() {
        let chapter_target = SingleChapterGenerationTarget {
            chapter_id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
        };
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2600,
            compat_options: empty_compat_options(),
            execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                ai_config: AIConfig::default(),
                provider_payload: PromptContextProviderPayload {
                    recent_chapters_context: String::new(),
                    previous_chapter_summary: String::new(),
                    chapter_careers: "[]".to_string(),
                    characters_info: "[]".to_string(),
                    foreshadow_reminders: "[]".to_string(),
                    relevant_memories: "[]".to_string(),
                    research_query: String::new(),
                    research_assets: "[]".to_string(),
                    external_assets: "[]".to_string(),
                    reference_assets: "[]".to_string(),
                    mcp_references: String::new(),
                },
            },
        };

        let chapter_id = chapter_target.chapter_id.clone();
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: chapter_target.chapter_id,
            user_id: "user-1".to_string(),
            execution_input,
        };

        assert_eq!(launch.chapter_id, "chapter-7");
        assert_eq!(launch.user_id, "user-1");
        assert_eq!(launch.execution_input.target_word_count, 2600);
        assert_eq!(chapter_id, "chapter-7");
    }

    #[test]
    fn should_build_single_generation_stream_runtime_input_contract() {
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-11".to_string(),
            user_id: "user-77".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 1800,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: String::new(),
                        previous_chapter_summary: String::new(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                },
            },
        };

        assert_eq!(runtime_input.chapter_id, "chapter-11");
        assert_eq!(runtime_input.user_id, "user-77");
        assert_eq!(runtime_input.execution_input.target_word_count, 1800);
    }

    #[test]
    fn should_keep_single_generation_stream_launch_input_contract() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-9".to_string(),
            user_id: "user-42".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2400,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: String::new(),
                        previous_chapter_summary: String::new(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                },
            },
        };

        assert_eq!(launch.chapter_id, "chapter-9");
        assert_eq!(launch.user_id, "user-42");
        assert_eq!(launch.execution_input.target_word_count, 2400);
    }

    #[test]
    fn should_build_single_generation_stream_terminal_success_payload() {
        let result = crate::services::chapter_generation_runtime_service::GeneratedChapterResult {
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
            content: "content".to_string(),
            word_count: 2600,
        };

        let response_payload =
            build_single_generation_stream_result_payload(&result, Some("analysis-task-1"));

        assert_eq!(response_payload["chapter_id"], "chapter-7");
        assert_eq!(response_payload["chapter_number"], 7);
        assert_eq!(response_payload["word_count"], 2600);
        assert_eq!(response_payload["saved_word_count"], 2600);
        assert_eq!(response_payload["chapter_status"], "draft");
        assert_eq!(response_payload["content_applied"], true);
        assert_eq!(response_payload["content_source"], "chapter");
        assert_eq!(response_payload["analysis_task_id"], "analysis-task-1");
    }

    #[test]
    fn should_build_single_generation_stream_terminal_failure_event() {
        let error = Err::<
            crate::services::chapter_generation_runtime_service::GeneratedChapterResult,
            _,
        >("generation failed".to_string())
        .expect_err("expected failure");

        assert_eq!(error, "generation failed");
    }

    #[tokio::test]
    async fn should_build_single_generation_stream_even_when_runtime_will_fail_later() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-1".to_string(),
            user_id: "user-1".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2000,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: String::new(),
                        previous_chapter_summary: String::new(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                },
            },
        };
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");

        let _stream = launch_owned_single_chapter_generation_stream(db, launch);
    }

    #[tokio::test]
    async fn should_build_single_generation_stream_when_follow_up_analysis_disabled() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-2".to_string(),
            user_id: "user-2".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 1600,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: false,
                    ..empty_compat_options()
                },
                execution_config:
                    crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
                        ai_config: AIConfig::default(),
                        provider_payload: PromptContextProviderPayload {
                            recent_chapters_context: String::new(),
                            previous_chapter_summary: String::new(),
                            chapter_careers: "[]".to_string(),
                            characters_info: "[]".to_string(),
                            foreshadow_reminders: "[]".to_string(),
                            relevant_memories: "[]".to_string(),
                            research_query: String::new(),
                            research_assets: "[]".to_string(),
                            external_assets: "[]".to_string(),
                            reference_assets: "[]".to_string(),
                            mcp_references: String::new(),
                        },
                    },
            },
        };
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");

        let _stream = launch_owned_single_chapter_generation_stream(db, launch);
    }

    #[test]
    fn should_keep_single_chapter_generation_request_contract_minimal() {
        let request = SingleChapterGenerationRequest {
            style_id: None,
            target_word_count: Some(2200),
            model: Some("gpt-test".to_string()),
            enable_analysis: None,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: None,
            story_preserve_strengths: None,
        };

        assert_eq!(request.style_id, None);
        assert_eq!(request.target_word_count, Some(2200));
        assert_eq!(request.model.as_deref(), Some("gpt-test"));
    }
}
