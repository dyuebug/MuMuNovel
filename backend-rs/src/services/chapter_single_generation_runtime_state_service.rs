use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};

use crate::ai::service::AIService;
use crate::models::batch_generation_task;
use crate::services::chapter_batch_generation_snapshot_service::upsert_batch_generation_runtime_snapshot;
use crate::services::chapter_analysis_runtime_service::analyze_generated_chapter_follow_up;
use crate::services::chapter_batch_generation_quality_status_service::manual_review_label_from_quality_context;
use crate::services::chapter_generation_runtime_service::{
    generate_and_persist_chapter_content_with_provider_payload, GeneratedChapterResult,
};
use crate::services::chapter_generation_prompt_service::ChapterGenerationPromptOverrides;
use crate::services::chapter_single_generation_runtime_checkpoint_service::{
    build_single_generation_runtime_checkpoint_for_stage, SingleGenerationSnapshotStage,
};

use super::chapter_single_generation_prepare_service::{
    SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
};

#[derive(Debug, Clone)]
pub(crate) struct SingleGenerationRuntimeLaunchInput {
    pub(crate) chapter_id: String,
    pub(crate) user_id: String,
    pub(crate) execution_input: SingleChapterGenerationExecutionInput,
}

pub(crate) fn dispatch_single_chapter_generation_runtime(
    db: DatabaseConnection,
    task_id: String,
    runtime_input: SingleGenerationRuntimeLaunchInput,
) {
    tokio::spawn(async move {
        execute_single_generation_runtime(&db, &task_id, runtime_input).await;
    });
}

pub(crate) async fn execute_owned_single_chapter_generation(
    db: &DatabaseConnection,
    runtime_input: SingleGenerationRuntimeLaunchInput,
) -> Result<GeneratedChapterResult, String> {
    let SingleGenerationRuntimeLaunchInput {
        chapter_id,
        user_id,
        execution_input,
    } = runtime_input;
    let SingleChapterGenerationExecutionInput {
        target_word_count,
        compat_options,
        execution_config,
    } = execution_input;
    let crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
        ai_config,
        provider_payload,
    } = execution_config;

    let ai_service = AIService::new(ai_config);
    let prompt_overrides = build_prompt_overrides_from_compat_options(&compat_options);
    generate_and_persist_chapter_content_with_provider_payload(
        db,
        &ai_service,
        &user_id,
        &chapter_id,
        target_word_count,
        provider_payload,
        &prompt_overrides,
    )
    .await
}

async fn run_single_generation_follow_up_analysis(
    db: &DatabaseConnection,
    user_id: &str,
    enable_analysis: bool,
    generated: &GeneratedChapterResult,
) -> Option<String> {
    if !enable_analysis {
        return None;
    }

    analyze_generated_chapter_follow_up(db, user_id, generated)
        .await
        .ok()
        .and_then(|payload| resolve_single_generation_manual_review_label_from_analysis_payload(&payload))
}

fn resolve_single_generation_manual_review_label_from_analysis_payload(
    payload: &serde_json::Value,
) -> Option<String> {
    let quality_metrics = payload.get("quality_metrics");
    manual_review_label_from_quality_context(None, quality_metrics, quality_metrics)
}

async fn maybe_fail_single_generation_for_quality_gate_manual_review(
    db: &DatabaseConnection,
    task_id: &str,
    generated: &GeneratedChapterResult,
    manual_review_label: &str,
) -> Result<(), String> {
    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        serde_json::json!({
            "analysis_task_message": "单章生成触发质量门禁，需人工复核",
            "analysis_task_progress": 100,
            "analysis_last_error": serde_json::Value::Null,
            "quality_gate_decision": "manual_review",
            "quality_gate_label": manual_review_label,
            "phase": "quality_blocked",
        }),
    )
    .await?;

    SingleGenerationTaskStage::Failed
        .persist_with_checkpoint(
            db,
            task_id,
            SingleGenerationSnapshotStage::Failed,
            &generated.chapter_id,
            Some(generated.chapter_number),
            Some(generated.word_count),
            Some(format!("章节触发质量门禁，需人工复核: {}", manual_review_label)),
        )
        .await
}

fn option_from_non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn build_prompt_overrides_from_compat_options(
    compat_options: &SingleChapterGenerationCompatOptions,
) -> ChapterGenerationPromptOverrides {
    ChapterGenerationPromptOverrides {
        narrative_perspective: option_from_non_empty(compat_options.narrative_perspective()),
        creative_mode: option_from_non_empty(compat_options.creative_mode()),
        story_focus: option_from_non_empty(compat_options.story_focus()),
        plot_stage: option_from_non_empty(compat_options.plot_stage()),
        story_creation_brief: option_from_non_empty(compat_options.story_creation_brief()),
        quality_preset: option_from_non_empty(compat_options.quality_preset()),
        quality_notes: option_from_non_empty(compat_options.quality_notes()),
        web_research_enabled: compat_options.web_research_enabled(),
        web_research_query: compat_options.web_research_query().map(str::to_string),
        story_repair_summary: option_from_non_empty(compat_options.story_repair_summary()),
        story_repair_targets: compat_options.story_repair_targets().to_vec(),
        story_preserve_strengths: compat_options.story_preserve_strengths().to_vec(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelFieldUpdate<T> {
    Keep,
    Set(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskTimestampUpdate {
    Keep,
    Clear,
    Now,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SingleGenerationTaskStage {
    Preparing,
    Completed,
    Failed,
}

impl SingleGenerationTaskStage {
    fn status(self) -> &'static str {
        match self {
            SingleGenerationTaskStage::Preparing => "running",
            SingleGenerationTaskStage::Completed => "completed",
            SingleGenerationTaskStage::Failed => "failed",
        }
    }

    fn started_at_update(self) -> TaskTimestampUpdate {
        match self {
            SingleGenerationTaskStage::Preparing => TaskTimestampUpdate::Now,
            SingleGenerationTaskStage::Completed | SingleGenerationTaskStage::Failed => {
                TaskTimestampUpdate::Keep
            }
        }
    }

    fn completed_at_update(self) -> TaskTimestampUpdate {
        match self {
            SingleGenerationTaskStage::Preparing => TaskTimestampUpdate::Clear,
            SingleGenerationTaskStage::Completed | SingleGenerationTaskStage::Failed => {
                TaskTimestampUpdate::Now
            }
        }
    }

    fn completed_chapters_update(self) -> ModelFieldUpdate<i32> {
        match self {
            SingleGenerationTaskStage::Preparing | SingleGenerationTaskStage::Failed => {
                ModelFieldUpdate::Keep
            }
            SingleGenerationTaskStage::Completed => ModelFieldUpdate::Set(1),
        }
    }

    fn current_retry_count_update(self) -> ModelFieldUpdate<i32> {
        match self {
            SingleGenerationTaskStage::Preparing => ModelFieldUpdate::Set(0),
            SingleGenerationTaskStage::Completed | SingleGenerationTaskStage::Failed => {
                ModelFieldUpdate::Keep
            }
        }
    }

    fn current_chapter_id_update(self, chapter_id: &str) -> ModelFieldUpdate<Option<String>> {
        match self {
            SingleGenerationTaskStage::Preparing | SingleGenerationTaskStage::Completed => {
                ModelFieldUpdate::Set(Some(chapter_id.to_string()))
            }
            SingleGenerationTaskStage::Failed => ModelFieldUpdate::Keep,
        }
    }

    fn current_chapter_number_update(self, chapter_number: Option<i32>) -> ModelFieldUpdate<Option<i32>> {
        match self {
            SingleGenerationTaskStage::Preparing | SingleGenerationTaskStage::Failed => {
                ModelFieldUpdate::Keep
            }
            SingleGenerationTaskStage::Completed => ModelFieldUpdate::Set(chapter_number),
        }
    }

    async fn persist_for_task(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        chapter_id: &str,
        chapter_number: Option<i32>,
        error_message: Option<String>,
        now: chrono::NaiveDateTime,
    ) -> Result<(), String> {
        if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?
        {
            let mut active: batch_generation_task::ActiveModel = task_model.into();
            self.apply_to_active_model(
                &mut active,
                chapter_id,
                chapter_number,
                error_message,
                now,
            );
            active.update(db).await.map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    fn apply_to_active_model(
        self,
        active: &mut batch_generation_task::ActiveModel,
        chapter_id: &str,
        chapter_number: Option<i32>,
        error_message: Option<String>,
        now: chrono::NaiveDateTime,
    ) {
        active.status = Set(self.status().to_string());

        match self.started_at_update() {
            TaskTimestampUpdate::Keep => {}
            TaskTimestampUpdate::Clear => active.started_at = Set(None),
            TaskTimestampUpdate::Now => active.started_at = Set(Some(now)),
        }

        match self.completed_at_update() {
            TaskTimestampUpdate::Keep => {}
            TaskTimestampUpdate::Clear => active.completed_at = Set(None),
            TaskTimestampUpdate::Now => active.completed_at = Set(Some(now)),
        }

        active.error_message = Set(match self {
            SingleGenerationTaskStage::Preparing | SingleGenerationTaskStage::Completed => None,
            SingleGenerationTaskStage::Failed => error_message,
        });

        match self.completed_chapters_update() {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.completed_chapters = Set(value),
        }

        match self.current_retry_count_update() {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_retry_count = Set(value),
        }

        match self.current_chapter_id_update(chapter_id) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_chapter_id = Set(value),
        }

        match self.current_chapter_number_update(chapter_number) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_chapter_number = Set(value),
        }
    }

    async fn persist_runtime_preparation(
        db: &DatabaseConnection,
        task_id: &str,
        chapter_id: &str,
    ) -> Result<(), String> {
        let now = Utc::now().naive_utc();
        Self::Preparing
            .persist_for_task(db, task_id, chapter_id, None, None, now)
            .await?;

        upsert_batch_generation_runtime_snapshot(
            db,
            task_id,
            build_single_generation_runtime_checkpoint_for_stage(
                SingleGenerationSnapshotStage::Preparing,
                chapter_id,
                None,
                None,
            ),
        )
        .await?;
        upsert_batch_generation_runtime_snapshot(
            db,
            task_id,
            build_single_generation_runtime_checkpoint_for_stage(
                SingleGenerationSnapshotStage::Generating,
                chapter_id,
                None,
                None,
            ),
        )
        .await
    }

    async fn persist_with_checkpoint(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        checkpoint_stage: SingleGenerationSnapshotStage,
        chapter_id: &str,
        chapter_number: Option<i32>,
        word_count: Option<i32>,
        error_message: Option<String>,
    ) -> Result<(), String> {
        let now = Utc::now().naive_utc();
        self.persist_for_task(
            db,
            task_id,
            chapter_id,
            chapter_number,
            error_message.clone(),
            now,
        )
        .await?;

        upsert_batch_generation_runtime_snapshot(
            db,
            task_id,
            build_single_generation_runtime_checkpoint_for_stage(
                checkpoint_stage,
                chapter_id,
                chapter_number,
                word_count,
            ),
        )
        .await
    }
}

pub(crate) async fn execute_single_generation_runtime(
    db: &DatabaseConnection,
    task_id: &str,
    runtime_input: SingleGenerationRuntimeLaunchInput,
) {
    let chapter_id = runtime_input.chapter_id.clone();
    let should_run_analysis = runtime_input.execution_input.compat_options.enable_analysis();
    let runtime_user_id = runtime_input.user_id.clone();
    let _ =
        SingleGenerationTaskStage::persist_runtime_preparation(db, task_id, &chapter_id).await;

    let _ = match execute_owned_single_chapter_generation(db, runtime_input).await {
        Ok(generated_result) => {
            if should_run_analysis {
                if let Some(manual_review_label) = run_single_generation_follow_up_analysis(
                    db,
                    &runtime_user_id,
                    should_run_analysis,
                    &generated_result,
                )
                .await
                {
                    maybe_fail_single_generation_for_quality_gate_manual_review(
                        db,
                        task_id,
                        &generated_result,
                        &manual_review_label,
                    )
                    .await
                } else {
                    let GeneratedChapterResult {
                        chapter_id: resolved_chapter_id,
                        chapter_number,
                        word_count,
                        ..
                    } = generated_result;
                    match SingleGenerationTaskStage::Completed
                        .persist_with_checkpoint(
                            db,
                            task_id,
                            SingleGenerationSnapshotStage::Finalizing,
                            &resolved_chapter_id,
                            Some(chapter_number),
                            Some(word_count),
                            None,
                        )
                        .await
                    {
                        Ok(()) => {
                            SingleGenerationTaskStage::Completed
                                .persist_with_checkpoint(
                                    db,
                                    task_id,
                                    SingleGenerationSnapshotStage::Completed,
                                    &resolved_chapter_id,
                                    Some(chapter_number),
                                    Some(word_count),
                                    None,
                                )
                                .await
                        }
                        Err(error) => Err(error),
                    }
                }
            } else {
                let GeneratedChapterResult {
                    chapter_id: resolved_chapter_id,
                    chapter_number,
                    word_count,
                    ..
                } = generated_result;
                match SingleGenerationTaskStage::Completed
                    .persist_with_checkpoint(
                        db,
                        task_id,
                        SingleGenerationSnapshotStage::Finalizing,
                        &resolved_chapter_id,
                        Some(chapter_number),
                        Some(word_count),
                        None,
                    )
                    .await
                {
                    Ok(()) => {
                        SingleGenerationTaskStage::Completed
                            .persist_with_checkpoint(
                                db,
                                task_id,
                                SingleGenerationSnapshotStage::Completed,
                                &resolved_chapter_id,
                                Some(chapter_number),
                                Some(word_count),
                                None,
                            )
                            .await
                    }
                    Err(error) => Err(error),
                }
            }
        }
        Err(error) => {
            SingleGenerationTaskStage::Failed
                .persist_with_checkpoint(
                    db,
                    task_id,
                    SingleGenerationSnapshotStage::Failed,
                    &chapter_id,
                    None,
                    None,
                    Some(error),
                )
                .await
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::ai::AIConfig;
    use chrono::NaiveDate;
    use sea_orm::Set;
    use serde_json::json;

    use super::{
        build_prompt_overrides_from_compat_options, dispatch_single_chapter_generation_runtime,
        resolve_single_generation_manual_review_label_from_analysis_payload,
        run_single_generation_follow_up_analysis, ModelFieldUpdate,
        SingleGenerationRuntimeLaunchInput, SingleGenerationTaskStage, TaskTimestampUpdate,
    };
    use crate::models::batch_generation_task;
    use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
    use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
    use crate::services::chapter_single_generation_prepare_service::{
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
    };
    use crate::services::chapter_single_generation_runtime_checkpoint_service::{
        build_single_generation_runtime_checkpoint_for_stage, SingleGenerationSnapshotStage,
    };

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

    fn build_task(status: &str) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 1,
            chapter_ids: json!(["chapter-1"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(1),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    #[test]
    fn should_resolve_single_generation_task_stage_mutation_contracts() {
        let preparing = SingleGenerationTaskStage::Preparing;
        assert_eq!(preparing.status(), "running");
        assert!(matches!(preparing.started_at_update(), TaskTimestampUpdate::Now));
        assert!(matches!(
            preparing.completed_at_update(),
            TaskTimestampUpdate::Clear
        ));
        assert!(matches!(
            preparing.current_retry_count_update(),
            ModelFieldUpdate::Set(0)
        ));
        assert!(matches!(
            preparing.current_chapter_id_update("chapter-1"),
            ModelFieldUpdate::Set(Some(ref id)) if id == "chapter-1"
        ));

        let completed = SingleGenerationTaskStage::Completed;
        assert_eq!(completed.status(), "completed");
        assert!(matches!(
            completed.completed_at_update(),
            TaskTimestampUpdate::Now
        ));
        assert!(matches!(
            completed.completed_chapters_update(),
            ModelFieldUpdate::Set(1)
        ));
        assert!(matches!(
            completed.current_chapter_number_update(Some(2)),
            ModelFieldUpdate::Set(Some(2))
        ));

        let failed = SingleGenerationTaskStage::Failed;
        assert_eq!(failed.status(), "failed");
        assert!(matches!(
            failed.completed_at_update(),
            TaskTimestampUpdate::Now
        ));
        assert!(matches!(
            failed.current_chapter_id_update("chapter-3"),
            ModelFieldUpdate::Keep
        ));
    }

    #[test]
    fn should_apply_single_generation_task_mutation_plan() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(0, 20, 0)
            .expect("valid time");
        let mut active: batch_generation_task::ActiveModel = build_task("pending").into();

        SingleGenerationTaskStage::Completed.apply_to_active_model(
            &mut active,
            "chapter-8",
            Some(8),
            None,
            now,
        );

        assert_eq!(active.status, Set("completed".to_string()));
        assert_eq!(active.completed_at, Set(Some(now)));
        assert_eq!(active.error_message, Set(None));
        assert_eq!(active.completed_chapters, Set(1));
        assert_eq!(
            active.current_chapter_id,
            Set(Some("chapter-8".to_string()))
        );
        assert_eq!(active.current_chapter_number, Set(Some(8)));
    }

    #[test]
    fn should_build_single_generation_runtime_launch_input_contract() {
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-7".to_string(),
            user_id: "user-7".to_string(),
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

        assert_eq!(runtime_input.chapter_id, "chapter-7");
        assert_eq!(runtime_input.user_id, "user-7");
        assert_eq!(runtime_input.execution_input.target_word_count, 2400);
        assert!(runtime_input.execution_input.compat_options.enable_analysis());
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
    fn should_keep_single_generation_runtime_persistence_contract_for_stage_owner() {
        assert_eq!(
            SingleGenerationSnapshotStage::Finalizing,
            SingleGenerationSnapshotStage::Finalizing
        );
        assert_eq!(
            SingleGenerationSnapshotStage::Completed,
            SingleGenerationSnapshotStage::Completed
        );
        assert_eq!(
            SingleGenerationSnapshotStage::Failed,
            SingleGenerationSnapshotStage::Failed
        );
        let completed_stage = SingleGenerationTaskStage::Completed;
        let failed_stage = SingleGenerationTaskStage::Failed;

        assert_eq!(completed_stage.status(), "completed");
        assert_eq!(failed_stage.status(), "failed");
    }

    #[test]
    fn should_keep_single_generation_runtime_preparation_persist_contract() {
        let chapter_id = "chapter-7";
        let preparing_checkpoint = build_single_generation_runtime_checkpoint_for_stage(
            SingleGenerationSnapshotStage::Preparing,
            chapter_id,
            None,
            None,
        );
        let generating_checkpoint = build_single_generation_runtime_checkpoint_for_stage(
            SingleGenerationSnapshotStage::Generating,
            chapter_id,
            None,
            None,
        );

        assert_eq!(preparing_checkpoint["phase"], "generating");
        assert_eq!(preparing_checkpoint["status"], "running");
        assert_eq!(preparing_checkpoint["progress"], 15);
        assert_eq!(preparing_checkpoint["current_chapter_id"], chapter_id);
        assert_eq!(generating_checkpoint["phase"], "generating");
        assert_eq!(generating_checkpoint["status"], "running");
        assert_eq!(generating_checkpoint["progress"], 65);
        assert_eq!(generating_checkpoint["current_chapter_id"], chapter_id);
    }

    #[tokio::test]
    async fn should_keep_single_generation_runtime_dispatch_contract() {
        dispatch_single_chapter_generation_runtime(
            sea_orm::DatabaseConnection::Disconnected,
            "task-7".to_string(),
            SingleGenerationRuntimeLaunchInput {
                chapter_id: "chapter-7".to_string(),
                user_id: "user-7".to_string(),
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
            },
        );
    }

    #[test]
    fn should_keep_single_generation_runtime_compat_options_on_launch_contract() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-compat".to_string(),
            user_id: "user-compat".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 3100,
                compat_options: SingleChapterGenerationCompatOptions {
                    style_id: Some(12),
                    enable_analysis: false,
                    enable_mcp: false,
                    web_research_enabled: true,
                    web_research_query: Some("late qing trade routes".to_string()),
                    narrative_perspective: Some("omniscient".to_string()),
                    creative_mode: Some("suspense".to_string()),
                    story_focus: Some("reveal_mystery".to_string()),
                    plot_stage: Some("climax".to_string()),
                    story_creation_brief: Some("push toward reveal".to_string()),
                    quality_preset: Some("immersive".to_string()),
                    quality_notes: Some("lean prose".to_string()),
                    story_repair_summary: Some("repair pacing".to_string()),
                    story_repair_targets: vec!["tighten setup".to_string()],
                    story_preserve_strengths: vec!["voice".to_string()],
                },
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

        assert_eq!(launch.execution_input.compat_options.style_id(), Some(12));
        assert!(!launch.execution_input.compat_options.enable_analysis());
        assert!(!launch.execution_input.compat_options.enable_mcp());
        assert!(launch.execution_input.compat_options.web_research_enabled());
        assert_eq!(
            launch.execution_input.compat_options.web_research_query(),
            Some("late qing trade routes")
        );
        assert_eq!(
            launch.execution_input.compat_options.creative_mode(),
            "suspense"
        );
        assert_eq!(
            launch.execution_input.compat_options.story_focus(),
            "reveal_mystery"
        );
        assert_eq!(launch.execution_input.compat_options.plot_stage(), "climax");
        assert_eq!(
            launch.execution_input.compat_options.quality_preset(),
            "immersive"
        );
    }

    #[test]
    fn should_build_prompt_overrides_from_single_generation_compat_options() {
        let compat = SingleChapterGenerationCompatOptions {
            style_id: Some(5),
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: false,
            web_research_query: None,
            narrative_perspective: Some("第一人称".to_string()),
            creative_mode: Some("hook".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("development".to_string()),
            story_creation_brief: Some("本章集中推进逃亡计划".to_string()),
            quality_preset: Some("plot_drive".to_string()),
            quality_notes: Some("减少旁白解释".to_string()),
            story_repair_summary: None,
            story_repair_targets: Vec::new(),
            story_preserve_strengths: Vec::new(),
        };

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat);

        assert_eq!(
            prompt_overrides.narrative_perspective.as_deref(),
            Some("第一人称")
        );
        assert_eq!(prompt_overrides.creative_mode.as_deref(), Some("hook"));
        assert_eq!(
            prompt_overrides.story_focus.as_deref(),
            Some("advance_plot")
        );
        assert_eq!(prompt_overrides.plot_stage.as_deref(), Some("development"));
        assert_eq!(
            prompt_overrides.story_creation_brief.as_deref(),
            Some("本章集中推进逃亡计划")
        );
        assert_eq!(
            prompt_overrides.quality_preset.as_deref(),
            Some("plot_drive")
        );
        assert_eq!(
            prompt_overrides.quality_notes.as_deref(),
            Some("减少旁白解释")
        );
        assert!(!prompt_overrides.web_research_enabled);
        assert_eq!(prompt_overrides.web_research_query, None);
        assert_eq!(prompt_overrides.story_repair_summary, None);
        assert!(prompt_overrides.story_repair_targets.is_empty());
        assert!(prompt_overrides.story_preserve_strengths.is_empty());
    }

    #[test]
    fn should_include_story_repair_fields_in_prompt_overrides() {
        let compat = SingleChapterGenerationCompatOptions {
            style_id: Some(9),
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
            story_repair_summary: Some("上一章后段信息重复，需要压缩".to_string()),
            story_repair_targets: vec![
                "收紧中段说明".to_string(),
                "让冲突更早落地".to_string(),
            ],
            story_preserve_strengths: vec!["角色张力".to_string(), "章节结尾钩子".to_string()],
        };

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat);

        assert_eq!(
            prompt_overrides.story_repair_summary.as_deref(),
            Some("上一章后段信息重复，需要压缩")
        );
        assert_eq!(
            prompt_overrides.story_repair_targets,
            vec!["收紧中段说明".to_string(), "让冲突更早落地".to_string()]
        );
        assert_eq!(
            prompt_overrides.story_preserve_strengths,
            vec!["角色张力".to_string(), "章节结尾钩子".to_string()]
        );
    }

    #[test]
    fn should_include_web_research_fields_in_prompt_overrides() {
        let compat = SingleChapterGenerationCompatOptions {
            style_id: Some(3),
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: true,
            web_research_query: Some("民国报馆夜班排印流程".to_string()),
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
        };

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat);

        assert!(prompt_overrides.web_research_enabled);
        assert_eq!(
            prompt_overrides.web_research_query.as_deref(),
            Some("民国报馆夜班排印流程")
        );
    }

    #[tokio::test]
    async fn should_skip_single_generation_follow_up_analysis_when_disabled() {
        let result = run_single_generation_follow_up_analysis(
            &sea_orm::DatabaseConnection::Disconnected,
            "user-1",
            false,
            &GeneratedChapterResult {
                chapter_id: "chapter-1".to_string(),
                chapter_number: 1,
                title: "第一章".to_string(),
                content: "正文".to_string(),
                word_count: 2,
            },
        )
        .await;

        assert_eq!(result, None);
    }

    #[test]
    fn should_resolve_single_generation_manual_review_label_from_analysis_payload() {
        let label = resolve_single_generation_manual_review_label_from_analysis_payload(&json!({
            "quality_metrics": {
                "quality_gate": {
                    "decision": "manual_review",
                    "label": "需要人工复核"
                }
            }
        }));

        assert_eq!(label.as_deref(), Some("需要人工复核"));
    }
}
