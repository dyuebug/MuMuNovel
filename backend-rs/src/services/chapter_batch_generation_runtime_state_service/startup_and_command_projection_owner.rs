use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use serde_json::{json, Value};

use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_batch_generation_read_context_service::{
    load_owned_batch_generation_task_sources, LoadOwnedBatchGenerationTaskError,
    LoadOwnedBatchGenerationTaskSourcesError,
};
use crate::services::chapter_batch_generation_task_payload_base_service::{
    batch_generation_task_type, build_batch_generation_command_summary_payload,
    build_batch_generation_status_task_payload_with_quality_context,
    build_batch_generation_task_response_payload_from_runtime_parts,
    BatchGenerationCommandProgressSummary, BatchGenerationTaskKind,
    BatchGenerationTaskResponsePayloadOptions, BatchGenerationTaskResponseQualityPayload,
};
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use crate::services::chapter_generation_execution_contract_service::{
    active_story_repair_payload_from_runtime_state, parse_batch_generation_request_runtime_state,
};
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    resolve_batch_quality_runtime_context_from_persisted_sources,
    resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state,
    resolve_generation_quality_runtime_context_from_persisted_sources,
    BatchGenerationQualityRuntimeContext, GenerationQualityRuntimeContext,
};
use crate::services::cooperative_cancellation_service::{
    global_cooperative_cancellation_registry, CooperativeCancellationScope,
};

use super::{
    build_batch_generation_runtime_checkpoint_for_stage,
    build_batch_generation_runtime_launch_input_from_runtime_state_seed,
    merge_batch_generation_runtime_state, persist_batch_generation_runtime_snapshot_replace,
    project_merged_batch_generation_runtime_state, upsert_batch_generation_runtime_snapshot,
    BatchGenerationExecutionInput, BatchGenerationResumeTaskResetMutationPlan,
    BatchGenerationSnapshotStage, BatchGenerationTaskStage, ResumeBatchGenerationCommandState,
    ResumeResetSemantics,
};

pub(crate) fn build_batch_generation_startup_and_command_projection_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::startup_cancel_resume_task_payload_projection",
        "scope": "queued_startup_snapshot_cancel_persistence_resume_reset_and_task_response_projection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/startup_and_command_projection_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/api/chapter_batch_generation.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "startup_snapshot_entrypoints": [
                "BatchGenerationQueuedSnapshotPlan::from_runtime_state_seed",
                "BatchGenerationQueuedSnapshotPlan::into_create_response_payload",
                "build_batch_generation_startup_snapshot_and_runtime_launch_input_from_runtime_state_seed",
                "dispatch_batch_generation_runtime"
            ],
            "cancel_command_entrypoints": [
                "BatchGenerationCancelledPersistencePlan::from_sources",
                "prepare_batch_generation_cancel_persistence_plan",
                "BatchGenerationCancelledPersistencePlan::persist",
                "cancel_owned_batch_generation_runtime_command"
            ],
            "resume_reset_entrypoints": [
                "BatchGenerationResumeResetPersistencePlan::from_resume_task_with_existing_runtime_state",
                "BatchGenerationResumeResetPersistencePlan::into_resume_response_payload",
                "BatchGenerationResumeSnapshotPlan::from_resume_checkpoint",
                "reset_batch_generation_task_for_resume"
            ],
            "response_projection_fields": [
                "checkpoint",
                "message",
                "completed_chapters",
                "total_chapters",
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary",
                "active_story_repair_payload",
                "terminal_reason",
                "terminal_label",
                "review_required",
                "can_resume"
            ],
            "runtime_state_seed_dependencies": [
                "runtime_state_with_candidate_gateway_metadata",
                "project_merged_batch_generation_runtime_state",
                "build_batch_generation_runtime_checkpoint_for_stage",
                "build_batch_generation_command_summary_payload"
            ],
            "command_contract": {
                "queued_startup_contract": "startup snapshot merges runtime seed, quality context, and candidate gateway metadata before create response projection",
                "cancel_contract": "cancel command rejects terminal tasks, persists cancelled task state, and returns status payload with terminal semantics",
                "resume_reset_contract": "resume reset rebuilds checkpoint/runtime state and reprojects response payload without changing external task payload shape"
            }
        },
        "active_consumers": [
            "chapter_batch_generation_write_workflow_service",
            "chapter_batch_generation_resume_task_command_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "task_payload_owner_contract": crate::services::chapter_batch_generation_task_payload_base_service::build_chapter_batch_generation_task_payload_base_owner_contract(),
        "quality_runtime_owner_contract": crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract(),
        "request_runtime_state_owner_contract": crate::services::chapter_generation_execution_contract_service::build_batch_request_runtime_state_owner_contract(),
        "story_repair_quality_context_owner_contract": crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::build_story_repair_quality_context_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "batch_generation_startup_command_projection_owner_is_rust_only_and_surviving_startup_cancel_resume_surfaces_are_tracked_by_external_route_contracts",
            "runtime_state_keys": [
                "candidate_gateway",
                "checkpoint",
                "quality_metrics_summary",
                "quality_metrics_history",
                "latest_quality_metrics",
                "active_story_repair_payload",
                "quality_history_context",
                "resumed_from_batch_id"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_batch_route_smoke"
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrepareBatchGenerationCancelPersistenceError {
    InvalidStatus(String),
}

impl PrepareBatchGenerationCancelPersistenceError {
    pub(crate) fn detail_message(&self) -> String {
        match self {
            Self::InvalidStatus(status) => {
                format!("Cannot cancel task in status {status}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationQueuedSnapshotPlan {
    runtime_state: Value,
    quality_runtime_context: BatchGenerationQualityRuntimeContext,
    active_story_repair_payload: Option<Value>,
    quality_history_context: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationQueuedCreateResponseChapter {
    pub(crate) id: String,
    pub(crate) chapter_number: i32,
    pub(crate) title: String,
}

impl BatchGenerationQueuedSnapshotPlan {
    pub(crate) fn from_runtime_state_seed(
        total_chapters: i32,
        runtime_state_seed: Option<Value>,
    ) -> Self {
        let runtime_state = match runtime_state_seed {
            Some(seed) => merge_batch_generation_runtime_state(
                Some(build_batch_generation_runtime_checkpoint_for_stage(
                    BatchGenerationSnapshotStage::Queued,
                    None,
                    None,
                    0,
                    total_chapters,
                )),
                seed,
            ),
            None => build_batch_generation_runtime_checkpoint_for_stage(
                BatchGenerationSnapshotStage::Queued,
                None,
                None,
                0,
                total_chapters,
            ),
        };

        let quality_runtime_context =
            resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state(
                None,
                Some(&runtime_state),
            );
        let active_story_repair_payload =
            active_story_repair_payload_from_runtime_state(Some(&runtime_state));
        let quality_history_context = runtime_state
            .get("quality_history_context")
            .cloned()
            .or_else(|| quality_runtime_context.quality_history_context.clone());

        Self {
            runtime_state,
            quality_runtime_context,
            active_story_repair_payload,
            quality_history_context,
        }
    }

    #[cfg(test)]
    pub(crate) fn runtime_state(&self) -> &Value {
        &self.runtime_state
    }

    pub(crate) fn quality_runtime_context(&self) -> BatchGenerationQualityRuntimeContext {
        self.quality_runtime_context.clone()
    }

    pub(crate) fn quality_metrics_summary(&self) -> Option<&Value> {
        self.quality_runtime_context
            .quality_metrics_summary
            .as_ref()
    }

    pub(crate) fn active_story_repair_payload(&self) -> Option<Value> {
        self.active_story_repair_payload.clone()
    }

    pub(crate) fn quality_history_context(&self) -> Option<Value> {
        self.quality_history_context.clone()
    }

    pub(crate) fn into_create_response_payload(
        self,
        batch_id: &str,
        project_id: &str,
        chapters_to_generate: &[BatchGenerationQueuedCreateResponseChapter],
        target_word_count: i32,
        enable_analysis: bool,
    ) -> Value {
        let total_chapters = chapters_to_generate.len();
        let task_kind = if total_chapters == 1 {
            BatchGenerationTaskKind::SingleChapter
        } else {
            BatchGenerationTaskKind::Batch
        };
        let payload = build_batch_generation_task_response_payload_from_runtime_parts(
            batch_id,
            batch_generation_task_type(task_kind),
            project_id,
            "pending",
            None,
            None,
            Some(&self.runtime_state),
            BatchGenerationTaskResponsePayloadOptions {
                quality_payload: Some(BatchGenerationTaskResponseQualityPayload::Batch {
                    quality_runtime_context: self.quality_runtime_context(),
                    quality_metrics_summary: self.quality_metrics_summary().cloned(),
                }),
                active_story_repair_payload: self.active_story_repair_payload(),
                quality_history_context: self.quality_history_context(),
                extra_fields: vec![
                    ("total_chapters".to_string(), json!(total_chapters)),
                    ("completed_chapters".to_string(), json!(0)),
                    (
                        "message".to_string(),
                        json!(format!("已创建批量生成任务，共 {} 章", total_chapters)),
                    ),
                    (
                        "chapters_to_generate".to_string(),
                        Value::Array(
                            chapters_to_generate
                                .iter()
                                .map(|target| {
                                    json!({
                                        "id": target.id,
                                        "chapter_number": target.chapter_number,
                                        "title": target.title,
                                    })
                                })
                                .collect::<Vec<_>>(),
                        ),
                    ),
                    (
                        "estimated_time_minutes".to_string(),
                        json!(super::estimated_task_minutes(
                            total_chapters,
                            target_word_count,
                            enable_analysis,
                        )),
                    ),
                ],
                ..Default::default()
            },
        );

        Value::Object(payload)
    }

    pub(crate) async fn persist(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> Result<(), String> {
        upsert_batch_generation_runtime_snapshot(db, task_id, self.runtime_state).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationResumeSnapshotPlan {
    runtime_state: Value,
}

impl BatchGenerationResumeSnapshotPlan {
    pub(crate) fn from_resume_checkpoint(
        existing_workflow_runtime_state: Option<Value>,
        resume_checkpoint: Value,
    ) -> Self {
        Self {
            runtime_state: merge_batch_generation_runtime_state(
                existing_workflow_runtime_state,
                resume_checkpoint,
            ),
        }
    }

    #[cfg(test)]
    pub(crate) fn runtime_state(&self) -> &Value {
        &self.runtime_state
    }

    async fn persist_replace(self, db: &DatabaseConnection, task_id: &str) -> Result<(), String> {
        persist_batch_generation_runtime_snapshot_replace(db, task_id, self.runtime_state).await
    }
}

fn batch_generation_candidate_gateway_metadata_from_config(
    candidate_gateway_config: &ChapterCandidateRouteGatewayConfig,
) -> Value {
    let execution_path = if candidate_gateway_config.rust_executor_enabled {
        "rust_candidate_executor"
    } else {
        "python_fallback"
    };

    json!({
        "execution_path": execution_path,
        "fallback_applied": !candidate_gateway_config.rust_executor_enabled,
        "rollback_boundary": candidate_gateway_config.rollback_boundary,
        "rust_executor_enabled": candidate_gateway_config.rust_executor_enabled,
        "fallback_on_rust_error": candidate_gateway_config.fallback_on_rust_error,
        "disabled_reason": candidate_gateway_config.disabled_reason,
    })
}

pub(crate) fn runtime_state_with_candidate_gateway_metadata(
    runtime_state_seed: Value,
    candidate_gateway_config: &ChapterCandidateRouteGatewayConfig,
) -> Value {
    let candidate_gateway =
        batch_generation_candidate_gateway_metadata_from_config(candidate_gateway_config);
    merge_batch_generation_runtime_state(
        Some(runtime_state_seed),
        json!({ "candidate_gateway": candidate_gateway }),
    )
}

pub(crate) fn build_batch_generation_startup_snapshot_and_runtime_launch_input_from_runtime_state_seed(
    user_id: String,
    chapter_ids: Vec<String>,
    total_chapters: i32,
    target_word_count: i32,
    runtime_state_seed: Value,
    execution_config: super::PreparedGenerationExecutionConfig,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
) -> (
    BatchGenerationQueuedSnapshotPlan,
    BatchGenerationExecutionInput,
) {
    let runtime_state_seed = runtime_state_with_candidate_gateway_metadata(
        runtime_state_seed,
        &candidate_gateway_config,
    );
    let request_runtime_state =
        parse_batch_generation_request_runtime_state(Some(&runtime_state_seed));
    let runtime_input = build_batch_generation_runtime_launch_input_from_runtime_state_seed(
        user_id,
        chapter_ids,
        target_word_count,
        &request_runtime_state,
        Some(&runtime_state_seed),
        execution_config,
        candidate_gateway_config,
    );
    let startup_snapshot_plan = BatchGenerationQueuedSnapshotPlan::from_runtime_state_seed(
        total_chapters,
        Some(runtime_state_seed),
    );

    (startup_snapshot_plan, runtime_input)
}

pub(crate) async fn reset_batch_generation_task_for_resume(
    db: &DatabaseConnection,
    plan: BatchGenerationResumeResetPersistencePlan,
) -> Result<(), String> {
    plan.persist(db).await
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CancelBatchGenerationTaskCommandError {
    Task(LoadOwnedBatchGenerationTaskError),
    Domain(String),
}

fn map_prepare_owned_batch_generation_cancel_sources_error(
    error: LoadOwnedBatchGenerationTaskSourcesError,
) -> CancelBatchGenerationTaskCommandError {
    match error {
        LoadOwnedBatchGenerationTaskSourcesError::Task(error) => {
            CancelBatchGenerationTaskCommandError::Task(error)
        }
        LoadOwnedBatchGenerationTaskSourcesError::Snapshot(error) => {
            CancelBatchGenerationTaskCommandError::Domain(error)
        }
    }
}

fn map_prepare_batch_generation_cancel_persistence_error(
    error: PrepareBatchGenerationCancelPersistenceError,
) -> CancelBatchGenerationTaskCommandError {
    CancelBatchGenerationTaskCommandError::Domain(error.detail_message())
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationCancelledPersistencePlan {
    batch_id: String,
    merged_runtime_state: Value,
    quality_status_context: super::BatchGenerationQualityStatusContext,
}

impl BatchGenerationCancelledPersistencePlan {
    pub(crate) fn from_sources(
        task: &batch_generation_task::Model,
        snapshot: Option<&batch_generation_snapshot::Model>,
    ) -> Self {
        let checkpoint = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Cancelled,
            None,
            None,
            task.completed_chapters,
            task.total_chapters,
        );
        let merged_runtime_state = project_merged_batch_generation_runtime_state(
            snapshot.and_then(|item| item.workflow_runtime_state.as_ref()),
            &checkpoint,
        );
        let quality_status_context =
            super::BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                snapshot,
                Some(&merged_runtime_state),
            );

        Self {
            batch_id: task.id.clone(),
            merged_runtime_state,
            quality_status_context,
        }
    }

    fn build_status_payload_for_task(&self, task: &batch_generation_task::Model) -> Value {
        build_batch_generation_status_task_payload_with_quality_context(
            task,
            Some(&self.merged_runtime_state),
            &self.quality_status_context,
        )
    }

    pub(crate) fn build_response_payload_for_task(
        &self,
        task: batch_generation_task::Model,
    ) -> Value {
        let mut payload = match self.build_status_payload_for_task(&task) {
            Value::Object(payload) => payload,
            _ => serde_json::Map::new(),
        };
        let summary_payload = build_batch_generation_command_summary_payload(
            BatchGenerationCommandProgressSummary {
                batch_id: task.id.clone(),
                total_chapters: task.total_chapters,
                completed_chapters: task.completed_chapters,
            },
            "Batch generation cancelled",
        );
        if let Value::Object(summary_fields) = summary_payload {
            payload.extend(summary_fields);
        }

        Value::Object(payload)
    }

    #[cfg(test)]
    pub(crate) fn response_payload_for_test(&self, task: batch_generation_task::Model) -> Value {
        self.build_response_payload_for_task(task)
    }

    pub(crate) async fn persist(self, db: &DatabaseConnection) -> Result<Value, String> {
        let BatchGenerationCancelledPersistencePlan {
            batch_id,
            merged_runtime_state,
            quality_status_context,
        } = self;
        let transaction = db.begin().await.map_err(|error| error.to_string())?;

        let task_model = batch_generation_task::Entity::find_by_id(&batch_id)
            .one(&transaction)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                "Batch generation task not found during cancel persistence".to_string()
            })?;
        let completed_chapters = task_model.completed_chapters;
        let total_chapters = task_model.total_chapters;
        let mut active: batch_generation_task::ActiveModel = task_model.into();
        BatchGenerationTaskStage::Cancelled.apply_to_active_model(
            &mut active,
            None,
            None,
            completed_chapters,
            total_chapters,
            None,
            Utc::now().naive_utc(),
        );

        let update_result = batch_generation_task::Entity::update_many()
            .set(active)
            .filter(batch_generation_task::Column::Id.eq(&batch_id))
            .filter(batch_generation_task::Column::Status.is_in(["pending", "running"]))
            .exec(&transaction)
            .await
            .map_err(|error| error.to_string())?;
        if update_result.rows_affected == 0 {
            return Err(
                "Batch generation cancel persistence rejected by inactive task status".to_string(),
            );
        }

        upsert_batch_generation_runtime_snapshot(
            &transaction,
            &batch_id,
            merged_runtime_state.clone(),
        )
        .await?;
        let response_task = batch_generation_task::Entity::find_by_id(&batch_id)
            .one(&transaction)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                "Batch generation task not found after cancel persistence".to_string()
            })?;
        transaction
            .commit()
            .await
            .map_err(|error| error.to_string())?;

        Ok(BatchGenerationCancelledPersistencePlan {
            batch_id,
            merged_runtime_state,
            quality_status_context,
        }
        .build_response_payload_for_task(response_task))
    }
}

pub(crate) fn prepare_batch_generation_cancel_persistence_plan(
    task: &batch_generation_task::Model,
    snapshot: Option<&batch_generation_snapshot::Model>,
) -> Result<BatchGenerationCancelledPersistencePlan, PrepareBatchGenerationCancelPersistenceError> {
    if matches!(task.status.as_str(), "completed" | "failed" | "cancelled") {
        return Err(PrepareBatchGenerationCancelPersistenceError::InvalidStatus(
            task.status.clone(),
        ));
    }

    Ok(BatchGenerationCancelledPersistencePlan::from_sources(
        task, snapshot,
    ))
}

pub(crate) async fn cancel_owned_batch_generation_runtime_command(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Value, CancelBatchGenerationTaskCommandError> {
    let (task, snapshot) = load_owned_batch_generation_task_sources(db, batch_id, user_id)
        .await
        .map_err(map_prepare_owned_batch_generation_cancel_sources_error)?
        .into_parts();

    let response_payload =
        prepare_batch_generation_cancel_persistence_plan(&task, snapshot.as_ref())
            .map_err(map_prepare_batch_generation_cancel_persistence_error)?
            .persist(db)
            .await
            .map_err(CancelBatchGenerationTaskCommandError::Domain)?;
    global_cooperative_cancellation_registry()
        .cancel(CooperativeCancellationScope::BatchGeneration, batch_id);
    Ok(response_payload)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationResumeResetPersistencePlan {
    batch_id: String,
    total_chapters: i32,
    reset_semantics: ResumeResetSemantics,
    resume_checkpoint: Value,
    task_reset_plan: BatchGenerationResumeTaskResetMutationPlan,
    resume_snapshot_plan: BatchGenerationResumeSnapshotPlan,
}

impl BatchGenerationResumeResetPersistencePlan {
    #[cfg(test)]
    pub(crate) fn from_resume_task(
        task: &ResumeBatchGenerationCommandState,
        runtime_state_seed: Option<Value>,
    ) -> Self {
        Self::from_resume_task_with_existing_runtime_state(task, runtime_state_seed, None)
    }

    pub(crate) fn from_resume_task_with_existing_runtime_state(
        task: &ResumeBatchGenerationCommandState,
        runtime_state_seed: Option<Value>,
        existing_workflow_runtime_state: Option<Value>,
    ) -> Self {
        let reset_semantics = task.resolve_reset_semantics();
        let resume_checkpoint = reset_semantics
            .build_resume_checkpoint_with_seed(task.total_chapters, runtime_state_seed);
        Self {
            batch_id: task.batch_id.clone(),
            total_chapters: task.total_chapters,
            task_reset_plan: BatchGenerationResumeTaskResetMutationPlan::from_reset_semantics(
                task.total_chapters,
                &reset_semantics,
            ),
            resume_snapshot_plan: BatchGenerationResumeSnapshotPlan::from_resume_checkpoint(
                existing_workflow_runtime_state,
                resume_checkpoint.clone(),
            ),
            resume_checkpoint,
            reset_semantics,
        }
    }

    pub(crate) fn total_chapters(&self) -> i32 {
        self.total_chapters
    }

    pub(crate) fn completed_chapters(&self) -> i32 {
        self.reset_semantics.completed_chapters
    }

    pub(crate) fn status(&self) -> &'static str {
        self.reset_semantics.status
    }

    pub(crate) fn current_chapter_id(&self) -> Option<&str> {
        self.reset_semantics.current_chapter_id.as_deref()
    }

    pub(crate) fn checkpoint(&self) -> &Value {
        &self.resume_checkpoint
    }

    #[cfg(test)]
    pub(crate) fn resume_snapshot_plan(&self) -> &BatchGenerationResumeSnapshotPlan {
        &self.resume_snapshot_plan
    }

    pub(crate) fn single_quality_runtime_context(&self) -> GenerationQualityRuntimeContext {
        resolve_generation_quality_runtime_context_from_persisted_sources(
            "chapter",
            self.latest_quality_metrics(),
            self.quality_metrics_history(),
            self.quality_metrics_summary_state(),
            self.quality_metrics_summary(),
        )
    }

    pub(crate) fn batch_quality_runtime_context(&self) -> BatchGenerationQualityRuntimeContext {
        resolve_batch_quality_runtime_context_from_persisted_sources(
            self.latest_quality_metrics(),
            self.quality_metrics_history(),
            self.quality_metrics_summary_state(),
            self.quality_metrics_summary(),
        )
    }

    pub(crate) fn latest_quality_metrics(&self) -> Option<&Value> {
        self.resume_checkpoint.get("latest_quality_metrics")
    }

    pub(crate) fn quality_metrics_history(&self) -> Option<&Value> {
        self.resume_checkpoint.get("quality_metrics_history")
    }

    pub(crate) fn quality_metrics_summary_state(&self) -> Option<&Value> {
        self.resume_checkpoint.get("quality_metrics_summary_state")
    }

    pub(crate) fn quality_metrics_summary(&self) -> Option<&Value> {
        self.resume_checkpoint.get("quality_metrics_summary")
    }

    pub(crate) fn active_story_repair_payload(&self) -> Option<Value> {
        active_story_repair_payload_from_runtime_state(Some(&self.resume_checkpoint))
    }

    pub(crate) fn quality_history_context_for_task_kind(
        &self,
        task_kind: BatchGenerationTaskKind,
    ) -> Option<Value> {
        self.resume_checkpoint
            .get("quality_history_context")
            .cloned()
            .or_else(|| match task_kind {
                BatchGenerationTaskKind::SingleChapter => {
                    self.single_quality_runtime_context()
                        .quality_history_context
                }
                BatchGenerationTaskKind::Batch => {
                    self.batch_quality_runtime_context().quality_history_context
                }
            })
    }

    pub(crate) fn into_resume_response_payload(
        self,
        command_state: &ResumeBatchGenerationCommandState,
    ) -> Value {
        let task_kind = command_state.task_kind();
        let summary = BatchGenerationCommandProgressSummary {
            batch_id: command_state.batch_id.clone(),
            total_chapters: self.total_chapters(),
            completed_chapters: self.completed_chapters(),
        };
        let quality_payload = match task_kind {
            BatchGenerationTaskKind::SingleChapter => {
                Some(BatchGenerationTaskResponseQualityPayload::Single {
                    quality_runtime_context: self.single_quality_runtime_context(),
                    latest_quality_metrics: self.latest_quality_metrics().cloned(),
                    quality_metrics_summary: self.quality_metrics_summary().cloned(),
                    quality_metrics_history: self.quality_metrics_history().cloned(),
                })
            }
            BatchGenerationTaskKind::Batch => {
                Some(BatchGenerationTaskResponseQualityPayload::Batch {
                    quality_runtime_context: self.batch_quality_runtime_context(),
                    quality_metrics_summary: self.quality_metrics_summary().cloned(),
                })
            }
        };
        let payload = build_batch_generation_task_response_payload_from_runtime_parts(
            command_state.batch_id.as_str(),
            batch_generation_task_type(task_kind),
            &command_state.project_id,
            self.status(),
            self.current_chapter_id(),
            command_state.created_at,
            Some(self.checkpoint()),
            BatchGenerationTaskResponsePayloadOptions {
                checkpoint_override: Some((
                    "chapter_id".to_string(),
                    json!(self.current_chapter_id()),
                )),
                summary_payload: Some(build_batch_generation_command_summary_payload(
                    summary,
                    "Task resumed and queued",
                )),
                quality_payload,
                active_story_repair_payload: self.active_story_repair_payload(),
                quality_history_context: self.quality_history_context_for_task_kind(task_kind),
                extra_fields: vec![(
                    "resumed_from_batch_id".to_string(),
                    json!(command_state.batch_id.clone()),
                )],
                apply_loading_stage_fields: true,
            },
        );

        Value::Object(payload)
    }

    #[cfg(test)]
    pub(crate) fn from_contract_for_test(
        batch_id: String,
        total_chapters: i32,
        reset_semantics: ResumeResetSemantics,
        resume_checkpoint: Value,
    ) -> Self {
        let task_reset_plan = BatchGenerationResumeTaskResetMutationPlan::from_reset_semantics(
            total_chapters,
            &reset_semantics,
        );
        let resume_snapshot_plan = BatchGenerationResumeSnapshotPlan::from_resume_checkpoint(
            None,
            resume_checkpoint.clone(),
        );
        Self {
            batch_id,
            total_chapters,
            reset_semantics,
            resume_checkpoint,
            task_reset_plan,
            resume_snapshot_plan,
        }
    }

    pub(crate) async fn persist(self, db: &DatabaseConnection) -> Result<(), String> {
        let BatchGenerationResumeResetPersistencePlan {
            batch_id,
            task_reset_plan,
            resume_snapshot_plan,
            ..
        } = self;
        let mut active = batch_generation_task::ActiveModel {
            id: Set(batch_id.clone()),
            ..Default::default()
        };
        task_reset_plan.apply_to_active_model(&mut active, Utc::now().naive_utc());

        active.update(db).await.map_err(|error| error.to_string())?;
        resume_snapshot_plan.persist_replace(db, &batch_id).await
    }
}
