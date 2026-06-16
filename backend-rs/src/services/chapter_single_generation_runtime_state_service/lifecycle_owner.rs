use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::Value;

use crate::models::batch_generation_task;
use crate::services::chapter_analysis_runtime_service::analyze_generated_chapter_follow_up;
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use crate::services::chapter_generation_execution_contract_service::{
    build_prompt_overrides_from_compat_options, SingleChapterGenerationExecutionInput,
};
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::upsert_chapter_generation_runtime_snapshot;
use crate::services::chapter_generation_runtime_service::{
    generate_and_persist_chapter_content_with_candidate_route_gateway, GeneratedChapterResult,
};
use crate::services::chapter_single_generation_runtime_state_service::{
    build_single_generation_error_terminal_state,
    build_single_generation_runtime_terminal_checkpoint_projection,
    resolve_single_generation_manual_review_label_from_analysis_payload,
    resolve_single_generation_quality_gate_terminal_state,
    SingleGenerationFollowUpAnalysisDecision, SingleGenerationQualityGateTerminalState,
    SingleGenerationSnapshotStage, SingleGenerationTaskStage,
};

pub(crate) fn append_single_generation_failed_chapter_entry(
    failed_chapters: &Value,
    failed_entry: Option<&Value>,
) -> Value {
    let mut items = failed_chapters.as_array().cloned().unwrap_or_default();
    if let Some(entry) = failed_entry.filter(|entry| entry.is_object()) {
        items.push(entry.clone());
    }
    Value::Array(items)
}

#[derive(Debug, Clone)]
pub(crate) struct SingleGenerationRuntimeLaunchInput {
    pub(crate) chapter_id: String,
    pub(crate) user_id: String,
    pub(crate) execution_input: SingleChapterGenerationExecutionInput,
}

impl SingleGenerationRuntimeLaunchInput {
    pub(crate) async fn execute_generation_with_gateway_config(
        self,
        db: &DatabaseConnection,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Result<GeneratedChapterResult, String> {
        let Self {
            chapter_id,
            user_id,
            execution_input,
        } = self;
        let SingleChapterGenerationExecutionInput {
            target_word_count,
            compat_options,
            execution_config,
        } = execution_input;
        let crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
            ai_config,
            provider_payload,
        } = execution_config;

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat_options);
        generate_and_persist_chapter_content_with_candidate_route_gateway(
            db,
            &user_id,
            &chapter_id,
            target_word_count,
            provider_payload,
            &prompt_overrides,
            ai_config,
            candidate_gateway_config,
        )
        .await
    }
}

#[cfg(test)]
pub(crate) fn default_single_generation_candidate_gateway_config(
) -> ChapterCandidateRouteGatewayConfig {
    ChapterCandidateRouteGatewayConfig {
        rust_executor_enabled: false,
        fallback_on_rust_error: true,
        disabled_reason: Some("single generation direct AI fallback remains default".to_string()),
        rollback_boundary: "legacy_single_generation_direct_ai".to_string(),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SingleGenerationRuntimeLifecyclePlan {
    pub(crate) task_id: String,
    pub(crate) chapter_id: String,
    pub(crate) runtime_user_id: String,
    pub(crate) enable_analysis: bool,
    pub(crate) candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    pub(crate) runtime_input: SingleGenerationRuntimeLaunchInput,
}

impl SingleGenerationRuntimeLifecyclePlan {
    #[cfg(test)]
    pub(crate) fn from_runtime_launch(
        task_id: String,
        runtime_input: SingleGenerationRuntimeLaunchInput,
    ) -> Self {
        Self::from_runtime_launch_with_gateway_config(
            task_id,
            runtime_input,
            default_single_generation_candidate_gateway_config(),
        )
    }

    pub(crate) fn from_runtime_launch_with_gateway_config(
        task_id: String,
        runtime_input: SingleGenerationRuntimeLaunchInput,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Self {
        let chapter_id = runtime_input.chapter_id.clone();
        let runtime_user_id = runtime_input.user_id.clone();
        let enable_analysis = runtime_input
            .execution_input
            .compat_options
            .enable_analysis();

        Self {
            task_id,
            chapter_id,
            runtime_user_id,
            enable_analysis,
            candidate_gateway_config,
            runtime_input,
        }
    }

    pub(crate) fn spawn(self, db: DatabaseConnection) {
        tokio::spawn(async move {
            self.execute(&db).await;
        });
    }

    async fn execute(self, db: &DatabaseConnection) {
        let _ = SingleGenerationTaskStage::persist_runtime_preparation(
            db,
            &self.task_id,
            &self.chapter_id,
        )
        .await;
        let outcome = self.outcome();

        let _ = match self.execute_generation(db).await {
            Ok(generated_result) => {
                outcome
                    .persist_generated_result(db, &generated_result)
                    .await
            }
            Err(error) => outcome.persist_failed_generation(db, error).await,
        };
    }

    async fn execute_generation(
        &self,
        db: &DatabaseConnection,
    ) -> Result<GeneratedChapterResult, String> {
        self.runtime_input
            .clone()
            .execute_generation_with_gateway_config(db, self.candidate_gateway_config.clone())
            .await
    }

    fn outcome(&self) -> SingleGenerationRuntimeOutcome {
        SingleGenerationRuntimeOutcome::new(
            self.task_id.clone(),
            self.chapter_id.clone(),
            self.runtime_user_id.clone(),
            self.enable_analysis,
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SingleGenerationRuntimeOutcome {
    task_id: String,
    chapter_id: String,
    runtime_user_id: String,
    enable_analysis: bool,
}

impl SingleGenerationRuntimeOutcome {
    pub(crate) fn new(
        task_id: String,
        chapter_id: String,
        runtime_user_id: String,
        enable_analysis: bool,
    ) -> Self {
        Self {
            task_id,
            chapter_id,
            runtime_user_id,
            enable_analysis,
        }
    }

    async fn persist_generated_result(
        &self,
        db: &DatabaseConnection,
        generated_result: &GeneratedChapterResult,
    ) -> Result<(), String> {
        let persisted_task = self.load_task(db).await?;
        if let Some(terminal_state) = resolve_single_generation_quality_gate_terminal_state(
            &persisted_task,
            generated_result,
            None,
        ) {
            return self
                .persist_quality_gate_terminal_generation(db, generated_result, terminal_state)
                .await;
        }

        if let Some(analysis_decision) = self.run_follow_up_analysis(db, generated_result).await {
            let terminal_state = resolve_single_generation_quality_gate_terminal_state(
                &persisted_task,
                generated_result,
                Some(&analysis_decision),
            )
            .expect("analysis decision should produce terminal state");
            return self
                .persist_quality_gate_terminal_generation(db, generated_result, terminal_state)
                .await;
        }

        self.persist_completed_generation(db, generated_result)
            .await
    }

    async fn persist_failed_generation(
        &self,
        db: &DatabaseConnection,
        error: String,
    ) -> Result<(), String> {
        let persisted_task = self.load_task(db).await?;
        let terminal_state = build_single_generation_error_terminal_state(
            &persisted_task,
            &self.chapter_id,
            None,
            None,
            &error,
        );

        self.persist_quality_gate_terminal_generation_without_result(db, terminal_state)
            .await
    }

    pub(crate) async fn run_follow_up_analysis(
        &self,
        db: &DatabaseConnection,
        generated_result: &GeneratedChapterResult,
    ) -> Option<SingleGenerationFollowUpAnalysisDecision> {
        if !self.enable_analysis {
            return None;
        }

        analyze_generated_chapter_follow_up(db, &self.runtime_user_id, generated_result)
            .await
            .ok()
            .and_then(|payload| {
                resolve_single_generation_manual_review_label_from_analysis_payload(&payload).map(
                    |manual_review_label| SingleGenerationFollowUpAnalysisDecision {
                        manual_review_label,
                        quality_metrics: payload.get("quality_metrics").cloned(),
                    },
                )
            })
    }

    async fn persist_quality_gate_terminal_generation(
        &self,
        db: &DatabaseConnection,
        generated_result: &GeneratedChapterResult,
        terminal_state: SingleGenerationQualityGateTerminalState,
    ) -> Result<(), String> {
        let chapter_number = Some(generated_result.chapter_number);
        let word_count = Some(generated_result.word_count);
        self.persist_quality_gate_terminal_generation_inner(
            db,
            &generated_result.chapter_id,
            chapter_number,
            word_count,
            Some(generated_result),
            terminal_state,
        )
        .await
    }

    async fn persist_quality_gate_terminal_generation_without_result(
        &self,
        db: &DatabaseConnection,
        terminal_state: SingleGenerationQualityGateTerminalState,
    ) -> Result<(), String> {
        self.persist_quality_gate_terminal_generation_inner(
            db,
            &self.chapter_id,
            None,
            None,
            None,
            terminal_state,
        )
        .await
    }

    async fn persist_quality_gate_terminal_generation_inner(
        &self,
        db: &DatabaseConnection,
        chapter_id: &str,
        chapter_number: Option<i32>,
        word_count: Option<i32>,
        generated_result: Option<&GeneratedChapterResult>,
        terminal_state: SingleGenerationQualityGateTerminalState,
    ) -> Result<(), String> {
        let SingleGenerationQualityGateTerminalState {
            checkpoint_payload,
            error_message,
            failed_entry,
        } = terminal_state;
        let merged_checkpoint_payload =
            build_single_generation_runtime_terminal_checkpoint_projection(
                SingleGenerationSnapshotStage::Failed,
                chapter_id,
                chapter_number,
                word_count,
                Some(checkpoint_payload),
                generated_result,
            );
        upsert_chapter_generation_runtime_snapshot(
            db,
            &self.task_id,
            merged_checkpoint_payload,
            Utc::now().naive_utc(),
        )
        .await?;

        let Some(task_model) = self.load_task(db).await? else {
            return Ok(());
        };

        let mut active: batch_generation_task::ActiveModel = task_model.clone().into();
        let now = Utc::now().naive_utc();
        SingleGenerationTaskStage::Failed.apply_to_active_model(
            &mut active,
            chapter_id,
            chapter_number,
            Some(error_message),
            now,
        );
        active.failed_chapters = Set(append_single_generation_failed_chapter_entry(
            &task_model.failed_chapters,
            Some(&failed_entry),
        ));
        active.update(db).await.map_err(|error| error.to_string())?;
        Ok(())
    }

    async fn persist_completed_generation(
        &self,
        db: &DatabaseConnection,
        generated_result: &GeneratedChapterResult,
    ) -> Result<(), String> {
        let GeneratedChapterResult {
            chapter_id,
            chapter_number,
            word_count,
            ..
        } = generated_result;
        let finalizing_checkpoint = build_single_generation_runtime_terminal_checkpoint_projection(
            SingleGenerationSnapshotStage::Finalizing,
            chapter_id,
            Some(*chapter_number),
            Some(*word_count),
            None,
            Some(generated_result),
        );
        let completed_checkpoint = build_single_generation_runtime_terminal_checkpoint_projection(
            SingleGenerationSnapshotStage::Completed,
            chapter_id,
            Some(*chapter_number),
            Some(*word_count),
            None,
            Some(generated_result),
        );

        SingleGenerationTaskStage::Completed
            .persist_with_checkpoint_payload(
                db,
                &self.task_id,
                chapter_id,
                Some(*chapter_number),
                None,
                finalizing_checkpoint,
            )
            .await?;

        SingleGenerationTaskStage::Completed
            .persist_with_checkpoint_payload(
                db,
                &self.task_id,
                chapter_id,
                Some(*chapter_number),
                None,
                completed_checkpoint,
            )
            .await
    }

    async fn load_task(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Option<batch_generation_task::Model>, String> {
        batch_generation_task::Entity::find_by_id(&self.task_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())
    }
}
