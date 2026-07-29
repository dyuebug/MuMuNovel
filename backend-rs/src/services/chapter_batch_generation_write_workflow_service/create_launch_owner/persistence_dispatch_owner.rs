use chrono::NaiveDateTime;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{batch_generation_task, project};
use crate::services::chapter_batch_generation_runtime_state_service::{
    dispatch_batch_generation_runtime, BatchGenerationExecutionInput,
    BatchGenerationQueuedCreateResponseChapter, BatchGenerationQueuedSnapshotPlan,
};
use crate::services::chapter_batch_generation_write_workflow_service::{
    BatchGenerationCreateChapterTarget, BatchGenerationCreateTaskSpec,
};
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use crate::services::chapter_generation_execution_contract_service::{
    build_prompt_overrides_from_compat_options, prepare_role_aware_generation_execution_config,
    PreparedGenerationExecutionConfig,
};
use crate::services::chapter_generation_runtime_service::build_batch_generation_contract_snapshot;
use crate::services::chapter_generation_runtime_service::story_continuity_ledger_owner::load_project_continuity_ledger;
use crate::services::generation_contract_service::GenerationIntentKind;
use crate::services::settings_service::SettingsService;

use super::startup_seed_owner::{
    resolve_batch_generation_create_effective_style_id, BatchGenerationCreateRuntimeSeed,
};
use super::{BatchGenerationCreateWorkflowRequest, CreateBatchGenerationWriteWorkflowError};

pub(crate) fn build_batch_generation_create_persistence_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_write_workflow_service::create_launch_persistence_dispatch_owner",
        "scope": "create_workflow_launch_projection_response_payload_persistence_and_runtime_dispatch",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service/create_launch_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service/create_launch_owner/persistence_dispatch_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/api/chapter_batch_generation.rs"
        ],
        "behavior_contract": {
            "launch_projection_entrypoints": [
                "PreparedBatchGenerationCreateWorkflowLaunch::prepare",
                "PreparedBatchGenerationCreateWorkflowLaunch::from_runtime_seed",
                "BatchGenerationCreateLaunchPersistencePlan::prepare",
                "BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch"
            ],
            "persistence_and_dispatch_entrypoints": [
                "BatchGenerationTaskPersistenceSeed::into_active_model",
                "BatchGenerationCreateLaunchPersistencePlan::persist_and_dispatch",
                "BatchGenerationQueuedSnapshotPlan::persist",
                "dispatch_batch_generation_runtime"
            ],
            "response_projection_entrypoints": [
                "BatchGenerationQueuedSnapshotPlan::into_create_response_payload"
            ],
            "response_projection_fields": [
                "batch_id",
                "project_id",
                "message",
                "chapters_to_generate",
                "estimated_time_minutes",
                "checkpoint",
                "candidate_gateway",
                "active_story_repair_payload",
                "quality_metrics_summary",
                "quality_history_context"
            ]
        },
        "validation_boundary": [
            "cargo test chapter_batch_generation_write_workflow_service",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "rollback_boundary": {
            "runtime_knob": "python_candidate_executor_fallback",
            "source_map_policy": "batch_generation_create_persistence_owner_is_rust_only_and_surviving_create_persistence_dispatch_surfaces_are_tracked_by_external_runtime_contracts",
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_batch_create_route_smoke"
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationTaskPersistenceSeed {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) user_id: String,
    pub(crate) start_chapter_number: i32,
    pub(crate) chapter_count: i32,
    pub(crate) chapter_ids: Value,
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: i32,
    pub(crate) enable_analysis: bool,
    pub(crate) total_chapters: i32,
    pub(crate) current_chapter_id: Option<String>,
    pub(crate) current_chapter_number: Option<i32>,
    pub(crate) max_retries: i32,
}

impl BatchGenerationTaskPersistenceSeed {
    pub(crate) fn into_active_model(
        self,
        now: NaiveDateTime,
    ) -> batch_generation_task::ActiveModel {
        batch_generation_task::ActiveModel {
            id: Set(self.id),
            project_id: Set(self.project_id),
            user_id: Set(self.user_id),
            start_chapter_number: Set(self.start_chapter_number),
            chapter_count: Set(self.chapter_count),
            chapter_ids: Set(self.chapter_ids),
            style_id: Set(self.style_id),
            target_word_count: Set(self.target_word_count),
            enable_analysis: Set(self.enable_analysis),
            status: Set("pending".to_string()),
            total_chapters: Set(self.total_chapters),
            completed_chapters: Set(0),
            failed_chapters: Set(json!([])),
            current_chapter_id: Set(self.current_chapter_id),
            current_chapter_number: Set(self.current_chapter_number),
            current_retry_count: Set(0),
            max_retries: Set(self.max_retries),
            created_at: Set(Some(now)),
            started_at: Set(None),
            completed_at: Set(None),
            error_message: Set(None),
        }
    }
}

#[cfg(test)]
pub(crate) fn build_batch_generation_task_active_model(
    id: String,
    project_id: String,
    user_id: String,
    start_chapter_number: i32,
    chapter_count: i32,
    chapter_ids: Value,
    style_id: Option<i32>,
    target_word_count: i32,
    enable_analysis: bool,
    total_chapters: i32,
    current_chapter_id: Option<String>,
    current_chapter_number: Option<i32>,
    max_retries: i32,
    now: NaiveDateTime,
) -> batch_generation_task::ActiveModel {
    BatchGenerationTaskPersistenceSeed {
        id,
        project_id,
        user_id,
        start_chapter_number,
        chapter_count,
        chapter_ids,
        style_id,
        target_word_count,
        enable_analysis,
        total_chapters,
        current_chapter_id,
        current_chapter_number,
        max_retries,
    }
    .into_active_model(now)
}

#[derive(Debug)]
pub(crate) struct BatchGenerationCreateLaunchPersistencePlan {
    pub(crate) task_seed: BatchGenerationTaskPersistenceSeed,
    pub(crate) startup_snapshot_plan: BatchGenerationQueuedSnapshotPlan,
    response_payload: Value,
    pub(crate) runtime_input: BatchGenerationExecutionInput,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedBatchGenerationCreateWorkflowLaunch {
    pub(crate) task_spec: BatchGenerationCreateTaskSpec,
    pub(crate) chapters_to_generate: Vec<BatchGenerationCreateChapterTarget>,
    pub(crate) startup_snapshot_plan: BatchGenerationQueuedSnapshotPlan,
    pub(crate) runtime_input: BatchGenerationExecutionInput,
}

impl PreparedBatchGenerationCreateWorkflowLaunch {
    pub(crate) async fn prepare(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        request: BatchGenerationCreateWorkflowRequest,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Result<Self, CreateBatchGenerationWriteWorkflowError> {
        let target_word_count_overridden = request.target_word_count.is_some();
        let (normalized_target_word_count, chapter_targets) = request
            .prepare(db, project_id)
            .await
            .map_err(CreateBatchGenerationWriteWorkflowError::Prepare)?;
        let task_spec = request.task_spec();
        let effective_style_id =
            resolve_batch_generation_create_effective_style_id(db, project_id, task_spec.style_id)
                .await
                .map_err(CreateBatchGenerationWriteWorkflowError::Internal)?;
        let task_spec = task_spec.with_effective_style_id(effective_style_id);
        let web_research_default = SettingsService::resolve_web_research_enabled(db, user_id)
            .await
            .map_err(|error| CreateBatchGenerationWriteWorkflowError::Config(error.to_string()))?;
        let request_runtime_state = request.into_request_runtime_state(web_research_default);
        let model_override = request_runtime_state.model_override.clone();
        let compat_options = request_runtime_state.compat_options.clone();
        let mut runtime_seed = BatchGenerationCreateRuntimeSeed::prepare(
            db,
            project_id,
            task_spec.start_chapter_number,
            request_runtime_state,
        )
        .await
        .map_err(CreateBatchGenerationWriteWorkflowError::Internal)?;
        let project_model = project::Entity::find_by_id(project_id)
            .one(db)
            .await
            .map_err(|error| CreateBatchGenerationWriteWorkflowError::Internal(error.to_string()))?
            .ok_or_else(|| {
                CreateBatchGenerationWriteWorkflowError::Internal("Project not found".to_owned())
            })?;
        let continuity_ledger = load_project_continuity_ledger(db, Some(project_id), 4)
            .await
            .map_err(CreateBatchGenerationWriteWorkflowError::Internal)?;
        let chapter_ids = chapter_targets
            .iter()
            .map(|target| target.id.clone())
            .collect::<Vec<_>>();
        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat_options);
        let generation_contract_snapshot = build_batch_generation_contract_snapshot(
            &project_model,
            chapter_ids,
            task_spec.start_chapter_number,
            normalized_target_word_count,
            target_word_count_overridden,
            &prompt_overrides,
            Some(&continuity_ledger),
        )
        .map_err(CreateBatchGenerationWriteWorkflowError::Internal)?;
        runtime_seed
            .merge_generation_contract_snapshot(&generation_contract_snapshot)
            .map_err(|error| {
                CreateBatchGenerationWriteWorkflowError::Internal(error.to_string())
            })?;
        let execution_config = prepare_role_aware_generation_execution_config(
            db,
            user_id,
            GenerationIntentKind::BatchChapterGenerate,
            model_override.as_deref(),
        )
        .await
        .map_err(CreateBatchGenerationWriteWorkflowError::Config)?;
        Ok(Self::from_runtime_seed(
            task_spec,
            normalized_target_word_count,
            chapter_targets,
            user_id,
            runtime_seed,
            execution_config,
            candidate_gateway_config,
        ))
    }

    pub(crate) fn from_runtime_seed(
        task_spec: BatchGenerationCreateTaskSpec,
        normalized_target_word_count: i32,
        chapters_to_generate: Vec<BatchGenerationCreateChapterTarget>,
        user_id: &str,
        runtime_seed: BatchGenerationCreateRuntimeSeed,
        execution_config: PreparedGenerationExecutionConfig,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Self {
        let total_chapters = chapters_to_generate.len() as i32;
        let chapter_ids = chapters_to_generate
            .iter()
            .map(|target| target.id.clone())
            .collect();
        let (startup_snapshot_plan, runtime_input) = runtime_seed.into_workflow_launch_parts(
            user_id.to_string(),
            chapter_ids,
            total_chapters,
            normalized_target_word_count,
            execution_config,
            candidate_gateway_config,
        );

        Self {
            task_spec,
            chapters_to_generate,
            startup_snapshot_plan,
            runtime_input,
        }
    }
}

impl BatchGenerationCreateLaunchPersistencePlan {
    pub(crate) async fn prepare(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        request: BatchGenerationCreateWorkflowRequest,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Result<Self, CreateBatchGenerationWriteWorkflowError> {
        let workflow_launch = PreparedBatchGenerationCreateWorkflowLaunch::prepare(
            db,
            project_id,
            user_id,
            request,
            candidate_gateway_config,
        )
        .await?;
        Ok(Self::from_workflow_launch(
            Uuid::new_v4().to_string(),
            project_id.to_string(),
            workflow_launch,
        ))
    }

    pub(crate) fn from_workflow_launch(
        task_id: String,
        project_id: String,
        workflow_launch: PreparedBatchGenerationCreateWorkflowLaunch,
    ) -> Self {
        let PreparedBatchGenerationCreateWorkflowLaunch {
            task_spec,
            chapters_to_generate,
            startup_snapshot_plan,
            runtime_input,
        } = workflow_launch;
        let total_chapters = chapters_to_generate.len() as i32;
        let response_chapters = chapters_to_generate
            .iter()
            .map(|target| BatchGenerationQueuedCreateResponseChapter {
                id: target.id.clone(),
                chapter_number: target.chapter_number,
                title: target.title.clone(),
            })
            .collect::<Vec<_>>();
        let response_payload = startup_snapshot_plan.clone().into_create_response_payload(
            &task_id,
            &project_id,
            &response_chapters,
            runtime_input.target_word_count,
            task_spec.enable_analysis,
        );
        let task_seed = BatchGenerationTaskPersistenceSeed {
            id: task_id,
            project_id,
            user_id: runtime_input.user_id.clone(),
            start_chapter_number: task_spec.start_chapter_number,
            chapter_count: total_chapters,
            chapter_ids: Value::Array(
                runtime_input
                    .chapter_ids
                    .iter()
                    .map(|chapter_id| json!(chapter_id))
                    .collect(),
            ),
            style_id: task_spec.style_id,
            target_word_count: runtime_input.target_word_count,
            enable_analysis: task_spec.enable_analysis,
            total_chapters,
            current_chapter_id: None,
            current_chapter_number: None,
            max_retries: task_spec.max_retries,
        };

        Self {
            task_seed,
            startup_snapshot_plan,
            response_payload,
            runtime_input,
        }
    }

    #[cfg(test)]
    pub(crate) fn response_payload(&self) -> Value {
        self.response_payload.clone()
    }

    #[cfg(test)]
    pub(crate) fn background_task_active_model(
        &self,
        now: NaiveDateTime,
    ) -> batch_generation_task::ActiveModel {
        self.task_seed.clone().into_active_model(now)
    }

    pub(crate) async fn persist_and_dispatch(
        self,
        db: &DatabaseConnection,
        now: NaiveDateTime,
    ) -> Result<Value, CreateBatchGenerationWriteWorkflowError> {
        let BatchGenerationCreateLaunchPersistencePlan {
            task_seed,
            startup_snapshot_plan,
            response_payload,
            runtime_input,
        } = self;
        let task_id = task_seed.id.clone();
        let task = task_seed.into_active_model(now);

        task.insert(db).await.map_err(|error| {
            CreateBatchGenerationWriteWorkflowError::Internal(error.to_string())
        })?;
        startup_snapshot_plan
            .persist(db, &task_id)
            .await
            .map_err(CreateBatchGenerationWriteWorkflowError::Internal)?;
        dispatch_batch_generation_runtime(db.clone(), task_id, runtime_input);

        Ok(response_payload)
    }
}

pub(crate) async fn prepare_owned_batch_generation_create_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    request: BatchGenerationCreateWorkflowRequest,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
) -> Result<BatchGenerationCreateLaunchPersistencePlan, CreateBatchGenerationWriteWorkflowError> {
    BatchGenerationCreateLaunchPersistencePlan::prepare(
        db,
        project_id,
        user_id,
        request,
        candidate_gateway_config,
    )
    .await
}

pub(crate) async fn start_owned_batch_generation_create_launch(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    request: BatchGenerationCreateWorkflowRequest,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    now: NaiveDateTime,
) -> Result<Value, CreateBatchGenerationWriteWorkflowError> {
    prepare_owned_batch_generation_create_workflow(
        db,
        project_id,
        user_id,
        request,
        candidate_gateway_config,
    )
    .await?
    .persist_and_dispatch(db, now)
    .await
}
