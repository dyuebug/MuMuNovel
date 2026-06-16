use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
#[cfg(test)]
use crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationExecutionInput;
use crate::services::chapter_single_generation_existing_background_task_service::load_owned_single_generation_existing_background_task_payload;
#[cfg(test)]
use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationTarget;
use crate::services::chapter_single_generation_prepare_service::{
    load_single_chapter_generation_target, PrepareSingleChapterGenerationRequestError,
    SingleChapterGenerationRouteRequest,
};

use super::restore_owner::PreparedSingleChapterGenerationRestoredRuntimeLaunch;

#[derive(Debug, Clone)]
pub(crate) enum SingleGenerationBackgroundWriteWorkflowEntry {
    ExistingTaskPayload(Value),
    Launch(
        crate::services::chapter_single_generation_background_launch_service::PreparedSingleGenerationBackgroundLaunchParts,
    ),
}

impl SingleGenerationBackgroundWriteWorkflowEntry {
    pub(crate) async fn start_from_route_payload(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        route_request: SingleChapterGenerationRouteRequest,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
        now: chrono::NaiveDateTime,
    ) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
        Self::prepare(db, chapter_id, user_id, route_request)
            .await?
            .persist_and_dispatch(db, candidate_gateway_config, now)
            .await
    }

    pub(crate) async fn prepare(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        route_request: SingleChapterGenerationRouteRequest,
    ) -> Result<Self, PrepareSingleChapterGenerationRequestError> {
        let chapter_target = load_single_chapter_generation_target(db, chapter_id, user_id).await?;
        if let Some(existing_task_payload) =
            load_owned_single_generation_existing_background_task_payload(
                db,
                chapter_id,
                &chapter_target.project_id,
                user_id,
            )
            .await
            .map_err(PrepareSingleChapterGenerationRequestError::Internal)?
        {
            return Ok(Self::ExistingTaskPayload(existing_task_payload));
        }

        let task_id = Uuid::new_v4().to_string();
        let launch_parts = PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_background_launch_parts_from_route_target(
            db,
            user_id,
            route_request,
            chapter_target,
            task_id,
        )
        .await?;

        Ok(Self::Launch(launch_parts))
    }

    pub(crate) async fn persist_and_dispatch(
        self,
        db: &DatabaseConnection,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
        now: chrono::NaiveDateTime,
    ) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
        match self {
            Self::ExistingTaskPayload(payload) => Ok(payload),
            Self::Launch(launch_parts) => {
                launch_parts
                    .persist_and_dispatch(db, candidate_gateway_config, now)
                    .await
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn from_existing_task_payload(payload: Value) -> Self {
        Self::ExistingTaskPayload(payload)
    }

    #[cfg(test)]
    pub(crate) fn from_prepared_request(
        task_id: String,
        user_id: &str,
        chapter_target: SingleChapterGenerationTarget,
        execution_input: SingleChapterGenerationExecutionInput,
        request_runtime_state: crate::services::chapter_generation_execution_contract_service::BatchGenerationRequestRuntimeState,
        runtime_state_payload: Value,
    ) -> Self {
        Self::Launch(
            crate::services::chapter_single_generation_background_launch_service::build_background_launch_parts_from_prepared_request(
                task_id,
                user_id,
                chapter_target,
                execution_input,
                request_runtime_state,
                runtime_state_payload,
            ),
        )
    }
}

pub(crate) fn build_single_generation_write_workflow_runtime_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_runtime_restore_workflow_service::write_workflow_owner",
        "scope": "single_generation_background_write_existing_task_check_launch_and_dispatch",
        "python_source_map": [
            "backend/app/api/chapter_generation_routes.py",
            "backend/app/api/chapters.py",
            "backend/app/services/chapter_generation/route_wiring_service.py",
            "backend/app/services/chapter_generation/stream/entry_service.py",
            "backend/app/services/chapter_generation/stream/wiring_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service/write_workflow_owner.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service/restore_owner.rs",
            "backend-rs/src/services/chapter_single_generation_background_launch_service.rs",
            "backend-rs/src/api/chapter_generation_routes.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "SingleGenerationBackgroundWriteWorkflowEntry::start_from_route_payload",
                "SingleGenerationBackgroundWriteWorkflowEntry::prepare",
                "SingleGenerationBackgroundWriteWorkflowEntry::persist_and_dispatch"
            ],
            "decision_path": [
                "load_single_chapter_generation_target",
                "load_owned_single_generation_existing_background_task_payload",
                "prepare_background_launch_parts_from_route_target",
                "persist_and_dispatch"
            ],
            "result_branches": [
                "ExistingTaskPayload",
                "Launch"
            ]
        },
        "validation_boundary": [
            "cargo test chapter_single_generation_runtime_restore_workflow_service",
            "cargo check"
        ],
        "rollback_boundary": {
            "python_source_map_retained": true,
            "same_round_python_edit_required": false
        }
    })
}
