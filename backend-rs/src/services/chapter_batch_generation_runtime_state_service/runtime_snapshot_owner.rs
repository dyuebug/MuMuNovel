use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseConnection};
use serde_json::{json, Value};

use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::{
    build_chapter_generation_snapshot_owner_contract, merge_chapter_generation_runtime_state,
    persist_chapter_generation_runtime_snapshot, upsert_chapter_generation_runtime_snapshot,
    ChapterGenerationSnapshotWriteMode,
};

use super::BatchGenerationRuntimePersistencePlan;

pub(crate) fn build_batch_generation_runtime_snapshot_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::runtime_snapshot_merge_write_projection",
        "scope": "runtime_state_merge_snapshot_upsert_and_runtime_persistence_write_handoff",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_snapshot_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/startup_and_command_projection_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_driver_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/retry_routing_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/follow_up_analysis_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_persistence_owner.rs"
        ],
        "behavior_contract": {
            "runtime_state_entrypoints": [
                "merge_batch_generation_runtime_state",
                "project_merged_batch_generation_runtime_state"
            ],
            "snapshot_write_entrypoints": [
                "upsert_batch_generation_runtime_snapshot",
                "persist_batch_generation_runtime_plan"
            ],
            "state_contract": {
                "merge_owner": "batch runtime-state patch projection still reuses the shared snapshot merge semantics without changing field precedence",
                "upsert_owner": "runtime-state snapshot writes still use current UTC timestamp and shared chapter-generation snapshot persistence path",
                "persistence_handoff_owner": "runtime persistence plan execution stays fire-and-forget from higher-level runtime/retry/follow-up owners"
            }
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service::startup_and_command_projection_owner",
            "chapter_batch_generation_runtime_state_service::runtime_driver_owner",
            "chapter_batch_generation_runtime_state_service::retry_routing_owner",
            "chapter_batch_generation_runtime_state_service::follow_up_analysis_owner",
            "chapter_batch_generation_runtime_state_service::runtime_persistence_owner"
        ],
        "snapshot_persistence_owner_contract": build_chapter_generation_snapshot_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "batch_generation_runtime_snapshot_owner_is_rust_only_and_surviving_snapshot_runtime_surfaces_are_tracked_by_external_persistence_contracts",
            "runtime_state_keys": [
                "checkpoint",
                "candidate_gateway",
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary",
                "active_story_repair_payload",
                "quality_history_context"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_runtime_snapshot_smoke"
        }
    })
}

pub(crate) fn merge_batch_generation_runtime_state(
    current_workflow_runtime_state: Option<Value>,
    incoming_workflow_runtime_state: Value,
) -> Value {
    merge_chapter_generation_runtime_state(
        current_workflow_runtime_state,
        incoming_workflow_runtime_state,
    )
}

pub(crate) fn project_merged_batch_generation_runtime_state(
    current_workflow_runtime_state: Option<&Value>,
    incoming_workflow_runtime_state: &Value,
) -> Value {
    merge_batch_generation_runtime_state(
        current_workflow_runtime_state.cloned(),
        incoming_workflow_runtime_state.clone(),
    )
}

pub(crate) async fn upsert_batch_generation_runtime_snapshot(
    db: &impl ConnectionTrait,
    task_id: &str,
    workflow_runtime_state: Value,
) -> Result<(), String> {
    upsert_chapter_generation_runtime_snapshot(
        db,
        task_id,
        workflow_runtime_state,
        Utc::now().naive_utc(),
    )
    .await
}

pub(crate) async fn persist_batch_generation_runtime_plan(
    db: &DatabaseConnection,
    task_id: &str,
    persistence_plan: BatchGenerationRuntimePersistencePlan,
) {
    let _ = persistence_plan.persist(db, task_id).await;
}

pub(crate) async fn persist_batch_generation_runtime_snapshot_replace(
    db: &DatabaseConnection,
    task_id: &str,
    workflow_runtime_state: Value,
) -> Result<(), String> {
    persist_chapter_generation_runtime_snapshot(
        db,
        task_id,
        workflow_runtime_state,
        ChapterGenerationSnapshotWriteMode::ReplaceRuntimeState,
        Utc::now().naive_utc(),
    )
    .await
}
