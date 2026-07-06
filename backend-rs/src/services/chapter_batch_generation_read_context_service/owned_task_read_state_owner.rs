use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use super::{load_owned_task, LoadOwnedBatchGenerationTaskError};
use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_batch_generation_task_payload_base_service::build_batch_generation_status_task_payload_from_task_and_snapshot_projection;
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::load_chapter_generation_snapshot;

pub(crate) fn build_batch_generation_owned_task_read_state_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_read_context_service::owned_task_read_state_owner",
        "scope": "owned_task_sources_read_state_and_status_payload",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service/owned_task_read_state_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs",
            "backend-rs/src/api/chapter_batch_generation.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "owned_source_entrypoints": [
                "load_owned_task",
                "load_owned_batch_generation_task_sources"
            ],
            "owned_read_state_entrypoints": [
                "load_owned_batch_generation_task_read_state"
            ],
            "status_payload_entrypoints": [
                "build_owned_batch_generation_status_payload_from_read_state",
                "load_owned_batch_generation_status_payload"
            ],
            "error_contracts": [
                "LoadOwnedBatchGenerationTaskError",
                "LoadOwnedBatchGenerationTaskSourcesError"
            ],
            "snapshot_dependency": [
                "load_chapter_generation_snapshot"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_read_context_service",
            "chapter_batch_generation_resume_task_command_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation"
        ],
        "validation_boundary": [
            "cargo test chapter_batch_generation_read_context_service",
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test api::health",
            "cargo check"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 11,
            "python_fallback_probe_count": 0,
            "owned_sources_owner": "load_owned_batch_generation_task_sources",
            "owned_read_state_owner": "load_owned_batch_generation_task_read_state",
            "status_payload_owner": "load_owned_batch_generation_status_payload",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "batch-generation read-context source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages",
            "status": "rust_batch_generation_owned_read_state_owner_source_map_deleted"
        },
        "rollback_boundary": {
            "source_map_policy": "batch_generation_read_context_owner_is_rust_only_and_surviving_python_query_status_surfaces_are_tracked_by_external_shared_runtime_projection_contracts",
            "status_payload_shape": [
                "batch_id",
                "checkpoint",
                "active_story_repair_payload",
                "candidate_gateway"
            ]
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadOwnedBatchGenerationTaskSourcesError {
    Task(LoadOwnedBatchGenerationTaskError),
    Snapshot(String),
}

#[derive(Debug, Clone)]
pub(crate) struct OwnedBatchGenerationTaskSources {
    task: batch_generation_task::Model,
    snapshot: Option<batch_generation_snapshot::Model>,
}

#[derive(Debug, Clone)]
pub(crate) struct OwnedBatchGenerationTaskReadState {
    task: batch_generation_task::Model,
    snapshot: Option<batch_generation_snapshot::Model>,
}

impl OwnedBatchGenerationTaskSources {
    pub(crate) fn into_parts(
        self,
    ) -> (
        batch_generation_task::Model,
        Option<batch_generation_snapshot::Model>,
    ) {
        (self.task, self.snapshot)
    }

    #[cfg(test)]
    pub(crate) fn from_parts(
        task: batch_generation_task::Model,
        snapshot: Option<batch_generation_snapshot::Model>,
    ) -> Self {
        Self { task, snapshot }
    }

    #[cfg(test)]
    pub(crate) fn task(&self) -> &batch_generation_task::Model {
        &self.task
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Option<&batch_generation_snapshot::Model> {
        self.snapshot.as_ref()
    }
}

impl OwnedBatchGenerationTaskReadState {
    pub(crate) fn into_parts(
        self,
    ) -> (
        batch_generation_task::Model,
        Option<batch_generation_snapshot::Model>,
    ) {
        (self.task, self.snapshot)
    }

    #[cfg(test)]
    pub(crate) fn from_parts(
        task: batch_generation_task::Model,
        snapshot: Option<batch_generation_snapshot::Model>,
    ) -> Self {
        Self { task, snapshot }
    }

    #[cfg(test)]
    pub(crate) fn task(&self) -> &batch_generation_task::Model {
        &self.task
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Option<&batch_generation_snapshot::Model> {
        self.snapshot.as_ref()
    }
}

pub(crate) async fn load_owned_batch_generation_task_sources(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<OwnedBatchGenerationTaskSources, LoadOwnedBatchGenerationTaskSourcesError> {
    let task = load_owned_task(db, batch_id, user_id)
        .await
        .map_err(|error| {
            LoadOwnedBatchGenerationTaskSourcesError::Task(
                LoadOwnedBatchGenerationTaskError::Internal(error),
            )
        })?
        .ok_or(LoadOwnedBatchGenerationTaskSourcesError::Task(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
        ))?;
    let snapshot = load_chapter_generation_snapshot(db, &task.id)
        .await
        .map_err(LoadOwnedBatchGenerationTaskSourcesError::Snapshot)?;

    Ok(OwnedBatchGenerationTaskSources { task, snapshot })
}

pub(crate) async fn load_owned_batch_generation_task_read_state(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<OwnedBatchGenerationTaskReadState, LoadOwnedBatchGenerationTaskError> {
    let (task, snapshot) =
        match load_owned_batch_generation_task_sources(db, batch_id, user_id).await {
            Ok(sources) => sources.into_parts(),
            Err(LoadOwnedBatchGenerationTaskSourcesError::Task(error)) => return Err(error),
            Err(LoadOwnedBatchGenerationTaskSourcesError::Snapshot(error)) => {
                return Err(LoadOwnedBatchGenerationTaskError::Internal(error))
            }
        };
    let (task, _) =
        super::recover_generation_task_if_needed_with_snapshot(db, task, snapshot.as_ref())
            .await
            .map_err(LoadOwnedBatchGenerationTaskError::Internal)?;

    Ok(OwnedBatchGenerationTaskReadState { task, snapshot })
}

pub(crate) fn build_owned_batch_generation_status_payload_from_read_state(
    read_state: OwnedBatchGenerationTaskReadState,
) -> Value {
    let (task, snapshot) = read_state.into_parts();
    let workflow_runtime_state = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.clone());

    build_batch_generation_status_task_payload_from_task_and_snapshot_projection(
        &task,
        snapshot.as_ref(),
        workflow_runtime_state.as_ref(),
    )
}

pub(crate) async fn load_owned_batch_generation_status_payload(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Value, LoadOwnedBatchGenerationTaskError> {
    Ok(build_owned_batch_generation_status_payload_from_read_state(
        load_owned_batch_generation_task_read_state(db, batch_id, user_id).await?,
    ))
}
