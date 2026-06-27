use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::{json, Value};

use crate::models::{batch_generation_snapshot, batch_generation_task, chapter};
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::load_recent_batch_story_repair_quality_summary;
use crate::services::chapter_single_generation_prepare_service::check_chapter_generation_prerequisites;

use super::{
    build_batch_generation_selected_candidate_event_snapshot, load_chapter_generation_snapshot,
    persist_batch_generation_runtime_plan,
    resolve_batch_generation_quality_gate_terminal_semantics,
    upsert_batch_generation_runtime_snapshot, BatchGenerationAttemptInputPlan,
    BatchGenerationExecutionInput, BatchGenerationFailureKind, BatchGenerationFollowUpAnalysisPlan,
    BatchGenerationGenericFailureRoutingPlan, BatchGenerationPersistedRuntimeContext,
    BatchGenerationQualityGateRoutingPlan, BatchGenerationRuntimePersistencePlan,
    BatchGenerationRuntimeSession,
};

pub(crate) fn build_batch_generation_runtime_driver_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::runtime_driver_execution_chain",
        "scope": "runtime_lifecycle_step_execution_post_write_guard_post_analysis_terminal_and_follow_up_loop",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_driver_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "runtime_lifecycle_entrypoints": [
                "BatchGenerationRuntimeLifecyclePlan::start",
                "BatchGenerationRuntimeLifecyclePlan::from_execution_input",
                "BatchGenerationRuntimeLifecyclePlan::execute"
            ],
            "step_execution_entrypoints": [
                "PreparedBatchGenerationStepExecution::start",
                "PreparedBatchGenerationStepExecution::prepare",
                "PreparedBatchGenerationStepExecution::execute"
            ],
            "post_write_and_terminal_entrypoints": [
                "BatchGenerationPostWriteGuardPlan::for_chapter",
                "BatchGenerationPostWriteGuardPlan::execute",
                "BatchGenerationPostAnalysisTerminalPlan::on_success",
                "BatchGenerationPostAnalysisTerminalPlan::on_failure",
                "BatchGenerationPostAnalysisTerminalPlan::execute"
            ],
            "follow_up_analysis_entrypoints": [
                "BatchGenerationFollowUpAnalysisPlan::from_generated_result",
                "BatchGenerationFollowUpAnalysisPlan::execute"
            ],
            "driver_outcome_contract": {
                "continue_owner": "BatchGenerationRuntimeDriverProgression::Continue",
                "stop_owner": "BatchGenerationRuntimeDriverProgression::Stop",
                "retry_owner": "BatchGenerationAttemptProgression::Retry",
                "task_cancelled_contract": "prepare step stops driver and persists cancelled runtime stage",
                "quality_gate_contract": "post-analysis terminal resolves quality gate retry/manual-review stop through runtime-state routing"
            },
            "runtime_dependencies": [
                "check_chapter_generation_prerequisites",
                "BatchGenerationAttemptInputPlan::execute",
                "build_batch_generation_selected_candidate_event_snapshot",
                "load_recent_batch_story_repair_quality_summary"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "selected_candidate_event_owner_contract": crate::services::chapter_batch_generation_runtime_state_service::build_batch_generation_selected_candidate_event_owner_contract(),
        "follow_up_analysis_owner_contract": crate::services::chapter_batch_generation_runtime_state_service::build_batch_generation_follow_up_analysis_owner_contract(),
        "story_repair_quality_context_owner_contract": crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::build_story_repair_quality_context_owner_contract(),
        "snapshot_persistence_owner_contract": crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::build_chapter_generation_snapshot_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "batch_generation_runtime_driver_owner_is_rust_only_and_surviving_driver_orchestration_surfaces_are_tracked_by_external_runtime_contracts",
            "runtime_state_keys": [
                "phase",
                "progress",
                "status",
                "last_event",
                "last_message",
                "selected_candidate_events",
                "analysis_task_message",
                "analysis_last_error",
                "active_story_repair_payload"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_runtime_driver_smoke"
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationStepProgress {
    pub(crate) completed: i32,
    pub(crate) total_chapters: i32,
}

impl BatchGenerationStepProgress {
    pub(crate) fn new(completed: i32, total_chapters: i32) -> Self {
        Self {
            completed,
            total_chapters,
        }
    }

    pub(crate) fn advance(&self) -> Self {
        Self {
            completed: self.completed + 1,
            total_chapters: self.total_chapters,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedBatchGenerationStepExecution {
    pub(crate) chapter_model: chapter::Model,
    pub(crate) retry_count: i32,
    pub(crate) max_retries: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BatchGenerationRuntimeDriverProgression {
    Continue(BatchGenerationStepProgress),
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BatchGenerationAttemptProgression {
    Retry(i32),
    Driver(BatchGenerationRuntimeDriverProgression),
}

impl PreparedBatchGenerationStepExecution {
    pub(crate) async fn start(
        db: &DatabaseConnection,
        task_id: &str,
        session: &BatchGenerationRuntimeSession,
        chapter_id: &str,
        progress: &BatchGenerationStepProgress,
    ) -> BatchGenerationRuntimeDriverProgression {
        let mut preparation_retry_count = None;

        loop {
            let prepared_step = match Self::prepare(db, task_id, chapter_id, progress).await {
                Ok(prepared_step) => prepared_step,
                Err(BatchGenerationAttemptProgression::Retry(next_retry_count)) => {
                    preparation_retry_count = Some(next_retry_count);
                    continue;
                }
                Err(BatchGenerationAttemptProgression::Driver(driver_progression)) => {
                    return driver_progression;
                }
            };

            let prepared_step = if let Some(retry_count) = preparation_retry_count.take() {
                Self {
                    retry_count,
                    ..prepared_step
                }
            } else {
                prepared_step
            };

            return prepared_step.execute(db, task_id, session, progress).await;
        }
    }

    pub(crate) async fn prepare(
        db: &DatabaseConnection,
        task_id: &str,
        chapter_id: &str,
        progress: &BatchGenerationStepProgress,
    ) -> Result<Self, BatchGenerationAttemptProgression> {
        let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .ok()
            .flatten()
        else {
            return Err(BatchGenerationAttemptProgression::Driver(
                BatchGenerationRuntimeDriverProgression::Stop,
            ));
        };
        if task_model.status == "cancelled" {
            persist_batch_generation_runtime_plan(
                db,
                task_id,
                BatchGenerationRuntimePersistencePlan::cancelled(
                    progress.completed,
                    progress.total_chapters,
                ),
            )
            .await;
            return Err(BatchGenerationAttemptProgression::Driver(
                BatchGenerationRuntimeDriverProgression::Stop,
            ));
        }

        let chapter_model = match chapter::Entity::find_by_id(chapter_id).one(db).await {
            Ok(Some(chapter_model)) => chapter_model,
            Ok(None) => {
                return Err(BatchGenerationGenericFailureRoutingPlan::from_step_context(
                    chapter_id,
                    None,
                    None,
                    progress,
                    task_model.current_retry_count,
                    task_model.max_retries.max(0),
                    BatchGenerationFailureKind::MissingChapter,
                    &format!("章节 {} 不存在", chapter_id),
                )
                .persist_and_resolve(db, task_id)
                .await);
            }
            Err(error) => {
                return Err(BatchGenerationGenericFailureRoutingPlan::from_step_context(
                    chapter_id,
                    None,
                    None,
                    progress,
                    task_model.current_retry_count,
                    task_model.max_retries.max(0),
                    BatchGenerationFailureKind::LoadChapterError,
                    &format!("加载章节失败: {}", error),
                )
                .persist_and_resolve(db, task_id)
                .await);
            }
        };
        if chapter_model.project_id != task_model.project_id {
            return Err(BatchGenerationGenericFailureRoutingPlan::from_step_context(
                chapter_id,
                None,
                None,
                progress,
                task_model.current_retry_count,
                task_model.max_retries.max(0),
                BatchGenerationFailureKind::GenerationError,
                &format!("章节 {} 项目不匹配", chapter_id),
            )
            .persist_and_resolve(db, task_id)
            .await);
        }

        Ok(Self {
            chapter_model,
            retry_count: task_model.current_retry_count.max(0),
            max_retries: task_model.max_retries.max(0),
        })
    }

    pub(crate) async fn execute(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        session: &BatchGenerationRuntimeSession,
        progress: &BatchGenerationStepProgress,
    ) -> BatchGenerationRuntimeDriverProgression {
        let Self {
            chapter_model,
            retry_count: initial_retry_count,
            max_retries,
        } = self;
        let mut retry_count = initial_retry_count;

        loop {
            let _ = BatchGenerationRuntimePersistencePlan::chapter_started(
                &chapter_model,
                progress.completed,
                progress.total_chapters,
                retry_count,
            )
            .persist(db, task_id)
            .await;

            let prerequisite =
                match check_chapter_generation_prerequisites(db, &chapter_model).await {
                    Ok(prerequisite) => prerequisite,
                    Err(error) => {
                        match BatchGenerationGenericFailureRoutingPlan::from_step_error(
                            &chapter_model,
                            progress,
                            retry_count,
                            max_retries,
                            BatchGenerationFailureKind::GenerationError,
                            &error,
                        )
                        .persist_and_resolve(db, task_id)
                        .await
                        {
                            BatchGenerationAttemptProgression::Retry(next_retry_count) => {
                                retry_count = next_retry_count;
                                continue;
                            }
                            BatchGenerationAttemptProgression::Driver(driver_progression) => {
                                return driver_progression;
                            }
                        }
                    }
                };
            if !prerequisite.can_generate {
                match BatchGenerationGenericFailureRoutingPlan::from_step_error(
                    &chapter_model,
                    progress,
                    retry_count,
                    max_retries,
                    BatchGenerationFailureKind::GenerationError,
                    &format!("章节生成失败: {}", prerequisite.error_message),
                )
                .persist_and_resolve(db, task_id)
                .await
                {
                    BatchGenerationAttemptProgression::Retry(next_retry_count) => {
                        retry_count = next_retry_count;
                        continue;
                    }
                    BatchGenerationAttemptProgression::Driver(driver_progression) => {
                        return driver_progression;
                    }
                }
            }

            match BatchGenerationAttemptInputPlan::execute(db, task_id, session, &chapter_model)
                .await
            {
                Ok(generated_result) => {
                    if let Some(selected_candidate_event_snapshot) =
                        build_batch_generation_selected_candidate_event_snapshot(
                            &generated_result,
                            session.total_chapters == 1,
                        )
                    {
                        let _ = upsert_batch_generation_runtime_snapshot(
                            db,
                            task_id,
                            selected_candidate_event_snapshot,
                        )
                        .await;
                    }

                    match BatchGenerationPostWriteGuardPlan::for_chapter(&chapter_model.id)
                        .execute(db, task_id)
                        .await
                    {
                        Ok(BatchGenerationPostWriteGuardOutcome::Continue) => {}
                        Ok(BatchGenerationPostWriteGuardOutcome::Stop) => {
                            return BatchGenerationRuntimeDriverProgression::Stop;
                        }
                        Err(error) => {
                            match BatchGenerationGenericFailureRoutingPlan::from_step_error(
                                &chapter_model,
                                progress,
                                retry_count,
                                max_retries,
                                BatchGenerationFailureKind::GenerationError,
                                &error,
                            )
                            .persist_and_resolve(db, task_id)
                            .await
                            {
                                BatchGenerationAttemptProgression::Retry(next_retry_count) => {
                                    retry_count = next_retry_count;
                                    continue;
                                }
                                BatchGenerationAttemptProgression::Driver(driver_progression) => {
                                    return driver_progression;
                                }
                            }
                        }
                    }

                    match BatchGenerationFollowUpAnalysisPlan::from_generated_result(
                        &generated_result,
                    )
                    .execute(db, task_id, session)
                    .await
                    {
                        Ok(current_quality_runtime_state) => {
                            match BatchGenerationPostAnalysisTerminalPlan::on_success(
                                &chapter_model,
                                progress,
                                current_quality_runtime_state,
                            )
                            .execute(db, task_id)
                            .await
                            {
                                BatchGenerationAttemptProgression::Retry(next_retry_count) => {
                                    retry_count = next_retry_count;
                                    continue;
                                }
                                BatchGenerationAttemptProgression::Driver(driver_progression) => {
                                    return driver_progression;
                                }
                            }
                        }
                        Err(analysis_error) => {
                            match BatchGenerationPostAnalysisTerminalPlan::on_failure(
                                &chapter_model,
                                progress,
                                analysis_error,
                            )
                            .execute(db, task_id)
                            .await
                            {
                                BatchGenerationAttemptProgression::Retry(next_retry_count) => {
                                    retry_count = next_retry_count;
                                    continue;
                                }
                                BatchGenerationAttemptProgression::Driver(driver_progression) => {
                                    return driver_progression;
                                }
                            }
                        }
                    }
                }
                Err(task_error_message) => {
                    match BatchGenerationGenericFailureRoutingPlan::from_step_error(
                        &chapter_model,
                        progress,
                        retry_count,
                        max_retries,
                        BatchGenerationFailureKind::GenerationError,
                        &task_error_message,
                    )
                    .persist_and_resolve(db, task_id)
                    .await
                    {
                        BatchGenerationAttemptProgression::Retry(next_retry_count) => {
                            retry_count = next_retry_count;
                        }
                        BatchGenerationAttemptProgression::Driver(driver_progression) => {
                            return driver_progression;
                        }
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn from_task_and_chapter(
        task_model: &batch_generation_task::Model,
        chapter_model: &chapter::Model,
    ) -> Self {
        Self {
            chapter_model: chapter_model.clone(),
            retry_count: task_model.current_retry_count.max(0),
            max_retries: task_model.max_retries.max(0),
        }
    }
}

pub(crate) struct BatchGenerationRuntimeLifecyclePlan {
    pub(crate) session: BatchGenerationRuntimeSession,
    pub(crate) chapter_ids: Vec<String>,
}

impl BatchGenerationRuntimeLifecyclePlan {
    pub(crate) async fn start(
        db: &DatabaseConnection,
        task_id: &str,
        execution_input: BatchGenerationExecutionInput,
    ) {
        Self::from_execution_input(execution_input)
            .execute(db, task_id)
            .await;
    }

    pub(crate) fn from_execution_input(execution_input: BatchGenerationExecutionInput) -> Self {
        let (session, chapter_ids) =
            BatchGenerationRuntimeSession::from_execution_input(execution_input);

        Self {
            session,
            chapter_ids,
        }
    }

    pub(crate) async fn execute(self, db: &DatabaseConnection, task_id: &str) {
        let _ = BatchGenerationRuntimePersistencePlan::preparing(self.session.total_chapters)
            .persist(db, task_id)
            .await;
        let mut progress = BatchGenerationStepProgress::new(0, self.session.total_chapters);

        for chapter_id in &self.chapter_ids {
            match PreparedBatchGenerationStepExecution::start(
                db,
                task_id,
                &self.session,
                chapter_id,
                &progress,
            )
            .await
            {
                BatchGenerationRuntimeDriverProgression::Continue(next_progress) => {
                    progress = next_progress;
                }
                BatchGenerationRuntimeDriverProgression::Stop => {
                    return;
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationPostWriteGuardPlan {
    pub(crate) chapter_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationPostWriteGuardOutcome {
    Continue,
    Stop,
}

impl BatchGenerationPostWriteGuardPlan {
    pub(crate) fn for_chapter(chapter_id: &str) -> Self {
        Self {
            chapter_id: chapter_id.to_string(),
        }
    }

    pub(crate) async fn execute(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> Result<BatchGenerationPostWriteGuardOutcome, String> {
        let task_exists = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?
            .is_some();
        if !task_exists {
            return Ok(Self::resolve(false, true));
        }

        let chapter_exists = chapter::Entity::find_by_id(&self.chapter_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?
            .is_some();
        Ok(Self::resolve(true, chapter_exists))
    }

    pub(crate) fn resolve(
        task_exists: bool,
        chapter_exists: bool,
    ) -> BatchGenerationPostWriteGuardOutcome {
        if task_exists && chapter_exists {
            BatchGenerationPostWriteGuardOutcome::Continue
        } else {
            BatchGenerationPostWriteGuardOutcome::Stop
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BatchGenerationPostAnalysisTerminalOutcome {
    Success {
        current_quality_runtime_state: Option<Value>,
    },
    Failure {
        analysis_error: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationPostAnalysisTerminalPlan {
    pub(crate) chapter_model: chapter::Model,
    pub(crate) progress: BatchGenerationStepProgress,
    pub(crate) outcome: BatchGenerationPostAnalysisTerminalOutcome,
}

impl BatchGenerationPostAnalysisTerminalPlan {
    pub(crate) fn on_success(
        chapter_model: &chapter::Model,
        progress: &BatchGenerationStepProgress,
        current_quality_runtime_state: Option<Value>,
    ) -> Self {
        Self {
            chapter_model: chapter_model.clone(),
            progress: progress.clone(),
            outcome: BatchGenerationPostAnalysisTerminalOutcome::Success {
                current_quality_runtime_state,
            },
        }
    }

    pub(crate) fn on_failure(
        chapter_model: &chapter::Model,
        progress: &BatchGenerationStepProgress,
        analysis_error: String,
    ) -> Self {
        Self {
            chapter_model: chapter_model.clone(),
            progress: progress.clone(),
            outcome: BatchGenerationPostAnalysisTerminalOutcome::Failure { analysis_error },
        }
    }

    pub(crate) async fn execute(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> BatchGenerationAttemptProgression {
        let Self {
            chapter_model,
            progress,
            outcome,
        } = self;

        match outcome {
            BatchGenerationPostAnalysisTerminalOutcome::Success {
                current_quality_runtime_state,
            } => {
                Self {
                    chapter_model,
                    progress,
                    outcome: BatchGenerationPostAnalysisTerminalOutcome::Success {
                        current_quality_runtime_state: None,
                    },
                }
                .resolve_analysis_success_outcome(db, task_id, current_quality_runtime_state)
                .await
            }
            BatchGenerationPostAnalysisTerminalOutcome::Failure { analysis_error } => {
                Self {
                    chapter_model,
                    progress,
                    outcome: BatchGenerationPostAnalysisTerminalOutcome::Failure {
                        analysis_error: String::new(),
                    },
                }
                .fail_after_analysis(db, task_id, analysis_error)
                .await
            }
        }
    }

    async fn resolve_analysis_success_outcome(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        current_quality_runtime_state: Option<Value>,
    ) -> BatchGenerationAttemptProgression {
        if let Some(quality_gate_outcome) = self
            .resolve_quality_gate_outcome(db, task_id, current_quality_runtime_state.as_ref())
            .await
        {
            return quality_gate_outcome;
        }

        let next_progress = self.progress.advance();
        BatchGenerationAttemptProgression::Driver(
            self.persist_post_generation_success(db, task_id, next_progress)
                .await,
        )
    }

    async fn resolve_quality_gate_outcome(
        &self,
        db: &DatabaseConnection,
        task_id: &str,
        current_quality_runtime_state: Option<&Value>,
    ) -> Option<BatchGenerationAttemptProgression> {
        let (snapshot, current_retry_count, max_retries) =
            Self::load_quality_gate_retry_budget_context(db, task_id).await;
        let persisted_workflow_runtime_state = snapshot
            .as_ref()
            .and_then(|item| item.workflow_runtime_state.as_ref());
        let workflow_runtime_state =
            current_quality_runtime_state.or(persisted_workflow_runtime_state);
        let Some(terminal_semantics) = resolve_batch_generation_quality_gate_terminal_semantics(
            snapshot.as_ref(),
            workflow_runtime_state,
            current_retry_count,
            max_retries,
        ) else {
            return None;
        };

        let routing_plan = BatchGenerationQualityGateRoutingPlan::from_terminal_semantics(
            &self.chapter_model,
            &self.progress,
            workflow_runtime_state,
            current_retry_count,
            max_retries,
            terminal_semantics,
        )?;

        Some(routing_plan.persist_and_resolve(db, task_id).await)
    }

    async fn load_quality_gate_retry_budget_context(
        db: &DatabaseConnection,
        task_id: &str,
    ) -> (Option<batch_generation_snapshot::Model>, i32, i32) {
        let snapshot = load_chapter_generation_snapshot(db, task_id)
            .await
            .ok()
            .flatten();
        let task_retry_context = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .ok()
            .flatten();
        let current_retry_count = task_retry_context
            .as_ref()
            .map(|task| task.current_retry_count.max(0))
            .unwrap_or(0);
        let max_retries = task_retry_context
            .as_ref()
            .map(|task| task.max_retries.max(0))
            .unwrap_or(0);

        (snapshot, current_retry_count, max_retries)
    }

    async fn fail_after_analysis(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        analysis_error: String,
    ) -> BatchGenerationAttemptProgression {
        let _ = upsert_batch_generation_runtime_snapshot(
            db,
            task_id,
            json!({
                "last_event": "analysis_failed",
                "last_message": format!("第 {} 章分析失败，批量任务终止", self.chapter_model.chapter_number),
                "progress": 100,
                "phase": "failed",
                "analysis_task_message": format!("第 {} 章分析失败，批量任务终止", self.chapter_model.chapter_number),
                "analysis_task_progress": 100,
                "analysis_last_error": analysis_error,
                "analysis_retry_count": 3,
                "analysis_max_retries": 3,
            }),
        )
        .await;

        persist_batch_generation_runtime_plan(
            db,
            task_id,
            BatchGenerationRuntimePersistencePlan::failed(
                Some(&self.chapter_model.id),
                Some(self.chapter_model.chapter_number),
                Some(&self.chapter_model.title),
                self.progress.completed,
                self.progress.total_chapters,
                BatchGenerationFailureKind::GenerationError,
                3,
                format!("章节分析失败，已重试3次: {}", analysis_error),
                format!(
                    "第{}章分析失败，已重试3次: {}",
                    self.chapter_model.chapter_number, analysis_error
                ),
            ),
        )
        .await;

        BatchGenerationAttemptProgression::Driver(BatchGenerationRuntimeDriverProgression::Stop)
    }

    async fn persist_post_generation_success(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        next_progress: BatchGenerationStepProgress,
    ) -> BatchGenerationRuntimeDriverProgression {
        self.refresh_runtime_story_repair_state(db, task_id).await;
        persist_batch_generation_runtime_plan(
            db,
            task_id,
            BatchGenerationRuntimePersistencePlan::chapter_succeeded(
                &self.chapter_model,
                next_progress.completed,
                next_progress.total_chapters,
            ),
        )
        .await;

        BatchGenerationRuntimeDriverProgression::Continue(next_progress)
    }

    async fn refresh_runtime_story_repair_state(&self, db: &DatabaseConnection, task_id: &str) {
        let persisted_runtime_context = load_chapter_generation_snapshot(db, task_id)
            .await
            .ok()
            .map(BatchGenerationPersistedRuntimeContext::from_snapshot)
            .unwrap_or_default();
        if !persisted_runtime_context.has_workflow_runtime_state() {
            return;
        }

        let quality_summary = load_recent_batch_story_repair_quality_summary(
            db,
            &self.chapter_model.project_id,
            self.chapter_model.chapter_number + 1,
        )
        .await
        .ok()
        .flatten();
        let Some(refreshed_runtime_state) = persisted_runtime_context
            .build_refreshed_runtime_state_preserving_quality(quality_summary.as_ref())
        else {
            return;
        };

        let _ =
            upsert_batch_generation_runtime_snapshot(db, task_id, refreshed_runtime_state).await;
    }
}
