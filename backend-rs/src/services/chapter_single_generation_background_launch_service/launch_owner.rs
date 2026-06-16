use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde_json::{json, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_generation_execution_contract_service::active_story_repair_payload_from_runtime_state;
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    apply_generation_quality_runtime_context_to_payload,
    build_generation_quality_runtime_owner_contract,
    resolve_generation_quality_runtime_context_from_persisted_sources,
    GenerationQualityRuntimeContext,
};
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::{
    build_chapter_generation_snapshot_owner_contract, upsert_chapter_generation_runtime_snapshot,
};
use crate::services::chapter_single_generation_prepare_service::{
    build_single_generation_runtime_payload_base, estimated_single_generation_task_minutes,
    PrepareSingleChapterGenerationRequestError, SingleChapterGenerationTarget,
};
use crate::services::chapter_single_generation_runtime_state_service::{
    build_single_generation_runtime_checkpoint_for_stage, SingleGenerationRuntimeLaunchInput,
    SingleGenerationRuntimeLifecyclePlan, SingleGenerationSnapshotStage,
};

use super::super::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;

#[cfg(test)]
use crate::services::chapter_generation_execution_contract_service::{
    BatchGenerationRequestRuntimeState, SingleChapterGenerationExecutionInput,
};

const SINGLE_GENERATION_BACKGROUND_MAX_RETRIES: i32 = 3;

pub(crate) fn build_single_generation_startup_snapshot_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_background_launch_service::launch_owner::startup_snapshot_owner",
        "scope": "pending_checkpoint_runtime_state_seed_quality_context_and_startup_snapshot_persist",
        "python_source_map": [
            "backend/app/api/chapter_generation_routes.py",
            "backend/app/api/chapters.py",
            "backend/app/services/chapter_generation/stream/service.py",
            "backend/app/services/chapter_generation/stream/execution_service.py",
            "backend/app/services/story_repair_payload_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_background_launch_service.rs",
            "backend-rs/src/services/chapter_single_generation_background_launch_service/launch_owner.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_single_generation_pending_checkpoint",
                "SingleGenerationStartupSnapshotPlan::from_pending_checkpoint",
                "SingleGenerationStartupSnapshotPlan::persist"
            ],
            "runtime_state_seed_contract": [
                "merge_single_generation_runtime_state",
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary_state",
                "quality_metrics_summary",
                "quality_history_context",
                "active_story_repair_payload"
            ],
            "persisted_snapshot_fields": [
                "checkpoint phase",
                "quality runtime context",
                "quality history context",
                "active story repair payload"
            ]
        },
        "active_consumers": [
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_single_generation_runtime_restore_workflow_service::prepare_background_launch_parts_from_route_target",
            "chapter_single_generation_active_gateway_smoke_service"
        ],
        "validation_boundary": [
            "cargo test chapter_single_generation_runtime_restore_workflow_service",
            "cargo test api::health",
            "cargo check"
        ]
    })
}

pub(crate) fn build_single_generation_background_launch_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_background_launch_service::launch_owner",
        "scope": "background_task_seed_persist_dispatch_and_create_response_payload",
        "python_source_map": [
            "backend/app/api/chapter_generation_routes.py",
            "backend/app/api/chapters.py",
            "backend/app/services/chapter_generation/route_wiring_service.py",
            "backend/app/services/chapter_generation/stream/entry_service.py",
            "backend/app/services/chapter_generation/stream/wiring_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_background_launch_service.rs",
            "backend-rs/src/services/chapter_single_generation_background_launch_service/launch_owner.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_background_launch_parts_from_restored_launch",
                "PreparedSingleGenerationBackgroundLaunchParts::persist_and_dispatch",
                "SingleGenerationBackgroundLaunchPersistenceDispatchPlan::persist_and_dispatch"
            ],
            "task_seed_contract": [
                "SingleGenerationTaskPersistenceSeed::into_active_model",
                "single chapter background task insert before runtime spawn"
            ],
            "response_payload_entrypoints": [
                "build_single_generation_background_create_response_payload"
            ],
            "gateway_dispatch_contract": [
                "SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config",
                "ChapterCandidateRouteGatewayConfig"
            ]
        },
        "active_consumers": [
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_generation_routes::generate_chapter_background",
            "chapter_single_generation_active_gateway_smoke_service"
        ],
        "startup_snapshot_owner_contract": build_single_generation_startup_snapshot_owner_contract(),
        "snapshot_persistence_owner_contract": build_chapter_generation_snapshot_owner_contract(),
        "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_single_generation_runtime_restore_workflow_service",
            "cargo test api::health",
            "cargo check"
        ]
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SingleGenerationTaskPersistenceSeed {
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

impl SingleGenerationTaskPersistenceSeed {
    pub(crate) fn into_active_model(
        self,
        now: chrono::NaiveDateTime,
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

pub(crate) fn build_single_generation_pending_checkpoint(
    chapter_target: &SingleChapterGenerationTarget,
) -> Value {
    build_single_generation_runtime_checkpoint_for_stage(
        SingleGenerationSnapshotStage::Pending,
        &chapter_target.chapter_id,
        Some(chapter_target.chapter_number),
        None,
    )
}

pub(crate) fn build_single_generation_background_task_persistence_seed(
    task_id: String,
    chapter_target: &SingleChapterGenerationTarget,
    user_id: String,
    target_word_count: i32,
    enable_analysis: bool,
) -> SingleGenerationTaskPersistenceSeed {
    SingleGenerationTaskPersistenceSeed {
        id: task_id,
        project_id: chapter_target.project_id.clone(),
        user_id,
        start_chapter_number: chapter_target.chapter_number,
        chapter_count: 1,
        chapter_ids: json!([{
            "id": chapter_target.chapter_id,
            "chapter_number": chapter_target.chapter_number,
            "title": chapter_target.title,
        }]),
        style_id: None,
        target_word_count,
        enable_analysis,
        total_chapters: 1,
        current_chapter_id: Some(chapter_target.chapter_id.clone()),
        current_chapter_number: Some(chapter_target.chapter_number),
        max_retries: SINGLE_GENERATION_BACKGROUND_MAX_RETRIES,
    }
}

#[cfg(test)]
pub(crate) fn build_single_generation_background_task_active_model(
    task_id: String,
    chapter_target: &SingleChapterGenerationTarget,
    user_id: String,
    target_word_count: i32,
    enable_analysis: bool,
    now: chrono::NaiveDateTime,
) -> batch_generation_task::ActiveModel {
    build_single_generation_background_task_persistence_seed(
        task_id,
        chapter_target,
        user_id,
        target_word_count,
        enable_analysis,
    )
    .into_active_model(now)
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SingleGenerationStartupSnapshotPlan {
    runtime_state: Value,
    quality_runtime_context: GenerationQualityRuntimeContext,
    active_story_repair_payload: Option<Value>,
    quality_history_context: Option<Value>,
}

impl SingleGenerationStartupSnapshotPlan {
    pub(crate) fn from_pending_checkpoint(
        pending_checkpoint: Value,
        runtime_state_seed: Value,
    ) -> Self {
        let runtime_state =
            merge_single_generation_runtime_state(Some(&pending_checkpoint), &runtime_state_seed);
        let quality_runtime_context =
            resolve_generation_quality_runtime_context_from_persisted_sources(
                "chapter",
                runtime_state.get("latest_quality_metrics"),
                runtime_state.get("quality_metrics_history"),
                runtime_state.get("quality_metrics_summary_state"),
                runtime_state.get("quality_metrics_summary"),
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

    pub(crate) fn runtime_state(&self) -> &Value {
        &self.runtime_state
    }

    pub(crate) fn quality_runtime_context(&self) -> GenerationQualityRuntimeContext {
        self.quality_runtime_context.clone()
    }

    pub(crate) fn latest_quality_metrics(&self) -> Option<&Value> {
        self.quality_runtime_context.latest_quality_metrics.as_ref()
    }

    pub(crate) fn quality_metrics_history(&self) -> Option<&Value> {
        self.quality_runtime_context
            .quality_metrics_history
            .as_ref()
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

    pub(crate) async fn persist(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> Result<(), String> {
        upsert_chapter_generation_runtime_snapshot(
            db,
            task_id,
            self.runtime_state,
            chrono::Utc::now().naive_utc(),
        )
        .await
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedSingleGenerationBackgroundLaunchParts {
    pub(crate) task_seed: SingleGenerationTaskPersistenceSeed,
    pub(crate) startup_snapshot_plan: SingleGenerationStartupSnapshotPlan,
    pub(crate) response_payload: Value,
    pub(crate) runtime_input: SingleGenerationRuntimeLaunchInput,
}

#[derive(Debug, Clone)]
pub(crate) struct SingleGenerationBackgroundLaunchPersistenceDispatchPlan {
    pub(crate) task_id: String,
    pub(crate) task: batch_generation_task::ActiveModel,
    pub(crate) startup_snapshot_plan: SingleGenerationStartupSnapshotPlan,
    pub(crate) response_payload: Value,
    pub(crate) runtime_input: SingleGenerationRuntimeLaunchInput,
}

impl SingleGenerationBackgroundLaunchPersistenceDispatchPlan {
    pub(crate) fn from_launch_parts(
        launch_parts: PreparedSingleGenerationBackgroundLaunchParts,
        now: chrono::NaiveDateTime,
    ) -> Self {
        let PreparedSingleGenerationBackgroundLaunchParts {
            task_seed,
            startup_snapshot_plan,
            response_payload,
            runtime_input,
        } = launch_parts;
        let task_id = task_seed.id.clone();
        let task = task_seed.into_active_model(now);

        Self {
            task_id,
            task,
            startup_snapshot_plan,
            response_payload,
            runtime_input,
        }
    }

    pub(crate) async fn persist_and_dispatch(
        self,
        db: &DatabaseConnection,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
        let Self {
            task_id,
            task,
            startup_snapshot_plan,
            response_payload,
            runtime_input,
        } = self;

        task.insert(db).await.map_err(|error| {
            PrepareSingleChapterGenerationRequestError::Internal(error.to_string())
        })?;
        startup_snapshot_plan
            .persist(db, &task_id)
            .await
            .map_err(PrepareSingleChapterGenerationRequestError::Internal)?;
        SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config(
            task_id,
            runtime_input,
            candidate_gateway_config,
        )
        .spawn(db.clone());

        Ok(response_payload)
    }
}

impl PreparedSingleGenerationBackgroundLaunchParts {
    pub(crate) async fn persist_and_dispatch(
        self,
        db: &DatabaseConnection,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
        now: chrono::NaiveDateTime,
    ) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
        SingleGenerationBackgroundLaunchPersistenceDispatchPlan::from_launch_parts(self, now)
            .persist_and_dispatch(db, candidate_gateway_config)
            .await
    }
}

pub(crate) fn build_background_launch_parts_from_restored_launch(
    task_id: String,
    chapter_target: SingleChapterGenerationTarget,
    startup_snapshot_plan: SingleGenerationStartupSnapshotPlan,
    runtime_input: SingleGenerationRuntimeLaunchInput,
) -> PreparedSingleGenerationBackgroundLaunchParts {
    let response_payload = build_single_generation_background_create_response_payload(
        &task_id,
        &chapter_target,
        &startup_snapshot_plan,
        &runtime_input,
    );
    let task_seed = build_single_generation_background_task_persistence_seed(
        task_id,
        &chapter_target,
        runtime_input.user_id.clone(),
        runtime_input.execution_input.target_word_count,
        runtime_input
            .execution_input
            .compat_options
            .enable_analysis(),
    );

    PreparedSingleGenerationBackgroundLaunchParts {
        task_seed,
        startup_snapshot_plan,
        response_payload,
        runtime_input,
    }
}

pub(crate) fn build_single_generation_background_create_response_payload(
    task_id: &str,
    chapter_target: &SingleChapterGenerationTarget,
    startup_snapshot_plan: &SingleGenerationStartupSnapshotPlan,
    runtime_input: &SingleGenerationRuntimeLaunchInput,
) -> Value {
    let workflow_runtime_state = startup_snapshot_plan.runtime_state();
    let mut payload = build_single_generation_runtime_payload_base(
        task_id,
        &chapter_target.project_id,
        Some(&chapter_target.chapter_id),
        "pending",
        Some(workflow_runtime_state),
        None,
    );
    let restored_quality_context = startup_snapshot_plan.quality_runtime_context();
    let active_story_repair_payload = startup_snapshot_plan.active_story_repair_payload();
    apply_generation_quality_runtime_context_to_payload(
        &mut payload,
        restored_quality_context,
        startup_snapshot_plan.latest_quality_metrics().cloned(),
        startup_snapshot_plan.quality_metrics_summary().cloned(),
        startup_snapshot_plan.quality_metrics_history().cloned(),
    );
    payload.insert(
        "active_story_repair_payload".to_string(),
        json!(active_story_repair_payload),
    );
    if let Some(quality_history_context) = startup_snapshot_plan.quality_history_context() {
        payload.insert(
            "quality_history_context".to_string(),
            quality_history_context,
        );
    }

    let estimated_minutes = estimated_single_generation_task_minutes(
        runtime_input.execution_input.target_word_count,
        runtime_input
            .execution_input
            .compat_options
            .enable_analysis(),
    );
    let compatibility_payload = build_single_generation_background_response_payload(
        task_id,
        chapter_target,
        estimated_minutes,
        active_story_repair_payload.as_ref(),
    );
    if let Value::Object(compatibility_payload) = compatibility_payload {
        payload.extend(compatibility_payload);
    }

    Value::Object(payload)
}

fn build_single_generation_background_response_payload(
    task_id: &str,
    chapter_target: &SingleChapterGenerationTarget,
    estimated_minutes: i32,
    active_story_repair_payload: Option<&Value>,
) -> Value {
    json!({
        "task_id": task_id,
        "chapter_id": chapter_target.chapter_id,
        "status": "pending",
        "message": "单章后台生成任务已创建",
        "estimated_time_minutes": estimated_minutes,
        "active_story_repair_payload": active_story_repair_payload.cloned(),
    })
}

#[cfg(test)]
pub(crate) fn build_test_single_generation_background_response_payload(
    task_id: &str,
    chapter_target: &SingleChapterGenerationTarget,
    estimated_minutes: i32,
    active_story_repair_payload: Option<&Value>,
) -> Value {
    build_single_generation_background_response_payload(
        task_id,
        chapter_target,
        estimated_minutes,
        active_story_repair_payload,
    )
}

#[cfg(test)]
pub(crate) fn build_background_launch_parts_from_prepared_request(
    task_id: String,
    user_id: &str,
    chapter_target: SingleChapterGenerationTarget,
    execution_input: SingleChapterGenerationExecutionInput,
    request_runtime_state: BatchGenerationRequestRuntimeState,
    runtime_state_payload: Value,
) -> PreparedSingleGenerationBackgroundLaunchParts {
    let target_word_count = execution_input.target_word_count;
    let execution_config = execution_input.execution_config.clone();
    let runtime_input =
        crate::services::chapter_single_generation_runtime_seed_service::build_single_generation_runtime_launch_input_from_request_runtime_state(
            &chapter_target,
            user_id,
            target_word_count,
            &request_runtime_state,
            execution_config,
        );
    let startup_snapshot_plan = SingleGenerationStartupSnapshotPlan::from_pending_checkpoint(
        build_single_generation_pending_checkpoint(&chapter_target),
        runtime_state_payload,
    );
    build_background_launch_parts_from_restored_launch(
        task_id,
        chapter_target,
        startup_snapshot_plan,
        runtime_input,
    )
}

fn merge_single_generation_runtime_state(
    current_workflow_runtime_state: Option<&Value>,
    incoming_workflow_runtime_state: &Value,
) -> Value {
    match (
        current_workflow_runtime_state.cloned(),
        incoming_workflow_runtime_state.clone(),
    ) {
        (Some(Value::Object(mut current)), Value::Object(incoming)) => {
            for (key, value) in incoming {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, incoming) => incoming,
    }
}
