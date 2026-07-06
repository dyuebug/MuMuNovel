use sea_orm::{DatabaseConnection, EntityTrait};
use serde_json::{json, Value};

pub(crate) mod active_query_owner;
pub(crate) mod owned_task_read_state_owner;
pub(crate) mod stream_progress_owner;
pub(crate) mod stream_state_owner;
pub(crate) mod task_recovery_owner;

pub(crate) use self::active_query_owner::{
    build_batch_generation_active_query_owner_contract,
    load_active_batch_generation_view_from_route_project,
    load_active_user_batch_generation_task_list_view_from_route_query,
    ActiveBatchGenerationTaskListQueryRequestError, ActiveBatchGenerationTaskListRouteQuery,
    ActiveBatchGenerationTaskListRouteQueryError, ActiveProjectBatchGenerationRouteError,
};
#[cfg(test)]
use self::active_query_owner::{
    build_batch_generation_read_contexts_from_snapshot_owner_map, BatchGenerationReadContext,
};
pub(crate) use self::owned_task_read_state_owner::{
    build_batch_generation_owned_task_read_state_owner_contract,
    load_owned_batch_generation_status_payload, load_owned_batch_generation_task_read_state,
    load_owned_batch_generation_task_sources, LoadOwnedBatchGenerationTaskSourcesError,
};
#[cfg(test)]
use self::owned_task_read_state_owner::{
    build_owned_batch_generation_status_payload_from_read_state, OwnedBatchGenerationTaskReadState,
    OwnedBatchGenerationTaskSources,
};
pub(crate) use self::stream_progress_owner::build_batch_generation_stream_progress_owner_contract;
#[cfg(test)]
use self::stream_progress_owner::{
    build_batch_generation_stream_progress_event, BatchGenerationStreamProgressEventInput,
};
#[cfg(test)]
use self::stream_state_owner::{
    batch_generation_stream_connected_event_payload, batch_generation_stream_data_event,
    batch_generation_stream_heartbeat_comment, batch_generation_stream_heartbeat_event,
    batch_generation_stream_task_not_found_event_payload,
    batch_generation_stream_timeout_event_payload,
    build_batch_generation_stream_state_from_task_and_snapshot,
    load_owned_batch_generation_stream_state, BatchGenerationResolvedStreamStatus,
    BatchGenerationStreamCursor, BatchGenerationStreamEventResolution,
    BatchGenerationStreamObservationKey, BatchGenerationStreamTerminalKind,
};
pub(crate) use self::stream_state_owner::{
    build_batch_generation_stream_state_owner_contract, load_owned_batch_generation_status_stream,
    BatchGenerationStreamState,
};
#[cfg(test)]
use self::task_recovery_owner::resolve_generation_task_auto_recovery_error;
pub(crate) use self::task_recovery_owner::{
    build_batch_generation_task_recovery_owner_contract, recover_generation_task_if_needed,
    recover_generation_task_if_needed_with_snapshot,
};

use crate::models::batch_generation_task;
use crate::services::chapter_batch_generation_task_payload_base_service::build_chapter_batch_generation_task_payload_base_owner_contract;
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::build_chapter_generation_snapshot_owner_contract;

const ACTIVE_BATCH_GENERATION_STATUSES: [&str; 2] = ["pending", "running"];

pub(crate) fn build_batch_generation_read_context_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_read_context_service",
        "scope": "batch_generation_status_stream_active_query_and_owned_read_state",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
            "backend-rs/src/api/health.rs",
            "backend-rs/src/api/chapter_batch_generation.rs"
        ],
        "behavior_contract": {
            "owned_read_state_loaders": [
                "load_owned_batch_generation_task_sources",
                "load_owned_batch_generation_task_read_state",
                "load_owned_batch_generation_status_payload",
                "load_owned_batch_generation_stream_state"
            ],
            "active_query_loaders": [
                "load_active_batch_generation_view_from_route_project",
                "load_active_user_batch_generation_task_list_view_from_route_query"
            ],
            "stream_state_owner": [
                "BatchGenerationStreamState::from_task_state",
                "BatchGenerationStreamState::from_task_state_with_quality_context",
                "BatchGenerationStreamState::events",
                "BatchGenerationStreamState::observation_key"
            ],
            "stream_metadata_projection": [
                "candidate_gateway"
            ],
            "route_payloads": [
                "status_payload",
                "stream_state",
                "active_project_payload",
                "active_user_task_list_payload"
            ],
            "stream_transport_events": [
                "data",
                "heartbeat",
                "task_not_found",
                "timeout"
            ],
            "active_query_limit": {
                "default": 20,
                "min": 1,
                "max": 100
            }
        },
        "active_consumers": [
            "chapter_batch_generation",
            "chapter_batch_generation_runtime_state_service",
            "chapter-batch-generation-active-gateway-smoke-rust"
        ],
        "task_payload_owner_contract": build_chapter_batch_generation_task_payload_base_owner_contract(),
        "snapshot_persistence_owner_contract": build_chapter_generation_snapshot_owner_contract(),
        "task_recovery_owner_contract": build_batch_generation_task_recovery_owner_contract(),
        "owned_task_read_state_owner_contract": build_batch_generation_owned_task_read_state_owner_contract(),
        "active_query_owner_contract": build_batch_generation_active_query_owner_contract(),
        "stream_state_owner_contract": build_batch_generation_stream_state_owner_contract(),
        "stream_progress_owner_contract": build_batch_generation_stream_progress_owner_contract(),
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 11,
            "python_fallback_probe_count": 0,
            "route_group_owner": "chapter_batch_generation",
            "owned_sources_owner": "load_owned_batch_generation_task_sources",
            "owned_read_state_owner": "load_owned_batch_generation_task_read_state",
            "status_payload_owner": "load_owned_batch_generation_status_payload",
            "stream_state_owner": "load_owned_batch_generation_stream_state",
            "active_project_query_owner": "load_active_batch_generation_view_from_route_project",
            "active_user_task_list_owner": "load_active_user_batch_generation_task_list_view_from_route_query",
            "stream_event_owner": "BatchGenerationStreamState::events",
            "snapshot_owner": "snapshot_persistence_owner::load_chapter_generation_snapshot_map",
            "response_payload_owner": "build_batch_generation_status_task_payload_from_task_and_snapshot_projection",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "batch-generation read-context source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages",
            "status": "rust_batch_generation_read_context_owner_source_map_deleted"
        },
        "validation_boundary": [
            "cargo test chapter_batch_generation_read_context_service",
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "batch_generation_read_context_owner_is_rust_only_and_surviving_python_query_status_surfaces_are_tracked_by_external_shared_runtime_projection_contracts",
            "runtime_state_keys": [
                "progress",
                "phase",
                "last_message",
                "selected_candidate_events",
                "active_story_repair_payload",
                "quality_metrics_summary"
            ],
            "delete_or_freeze_requires": "same_round_logged_in_db_smoke_and_route_rollback_policy"
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadOwnedBatchGenerationTaskError {
    TaskNotFound,
    Internal(String),
}

pub(crate) async fn load_owned_task(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Option<batch_generation_task::Model>, String> {
    let task = batch_generation_task::Entity::find_by_id(batch_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?;
    Ok(task.filter(|task| task.user_id == user_id))
}

pub(crate) fn active_batch_generation_statuses() -> [&'static str; 2] {
    ACTIVE_BATCH_GENERATION_STATUSES
}

#[cfg(test)]
fn is_active_batch_generation_task_status(status: &str) -> bool {
    active_batch_generation_statuses().contains(&status)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{Duration as ChronoDuration, Utc};
    use serde_json::json;

    use super::{
        build_batch_generation_read_context_owner_contract,
        build_batch_generation_stream_progress_owner_contract,
        build_batch_generation_task_recovery_owner_contract,
        resolve_generation_task_auto_recovery_error,
        task_recovery_owner::resolve_generation_task_auto_recovery_error_with_snapshot,
        BatchGenerationReadContext, LoadOwnedBatchGenerationTaskError,
        OwnedBatchGenerationTaskReadState,
    };
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_batch_generation_task_payload_base_service::BatchGenerationQualityStatusContext;
    fn build_task(status: &str) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", "chapter-2"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-2".to_string()),
            current_chapter_number: Some(2),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn naive_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(year, month, day)
            .expect("valid date")
            .and_hms_opt(hour, minute, second)
            .expect("valid time")
    }

    fn build_snapshot() -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"score": 91})),
            quality_metrics_history: Some(json!([{"score": 90}])),
            quality_metrics_summary: Some(json!({"summary": "ok"})),
            workflow_runtime_state: Some(json!({
                "progress": 60,
                "candidate_gateway": {
                    "execution_path": "rust_candidate_executor",
                    "fallback_applied": false,
                    "fallback_reason": "rust executor completed",
                    "rollback_boundary": "python_candidate_executor_fallback",
                    "rust_error": null
                },
                "active_story_repair_payload": {
                    "mode": "repair"
                }
            })),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn should_publish_batch_generation_read_context_owner_contract() {
        let contract = build_batch_generation_read_context_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_read_context_service"
        );
        assert_eq!(
            contract["behavior_contract"]["owned_read_state_loaders"][2],
            "load_owned_batch_generation_status_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["active_query_limit"]["default"],
            20
        );
        assert_eq!(
            contract["behavior_contract"]["stream_transport_events"][1],
            "heartbeat"
        );
        assert_eq!(
            contract["active_consumers"][2],
            "chapter-batch-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["task_payload_owner_contract"]["owner"],
            "chapter_batch_generation_task_payload_base_service"
        );
        assert_eq!(
            contract["task_payload_owner_contract"]["shared_schema_hold_status"]
                ["batch_generation_task_model"],
            "shared_python_runtime_api_and_test_support_reference"
        );
        assert_eq!(
            contract["task_payload_owner_contract"]["quality_terminal_status_owner_contract"]
                ["owner"],
            "chapter_batch_generation_task_payload_base_service::quality_terminal_status_owner"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            "chapter_generation_runtime_service::snapshot_persistence_owner"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["behavior_contract"]["read_functions"]
                [1],
            "load_chapter_generation_snapshot_map"
        );
        assert_eq!(
            contract["task_recovery_owner_contract"]["scope"],
            "task_recovery_owner"
        );
        assert_eq!(
            contract["stream_progress_owner_contract"]["owner"],
            "chapter_batch_generation_read_context_service::stream_progress_owner"
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_state_keys"][3],
            "selected_candidate_events"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_read_context_owner_is_rust_only_and_surviving_python_query_status_surfaces_are_tracked_by_external_shared_runtime_projection_contracts"
        );
        assert_eq!(
            contract["owned_task_read_state_owner_contract"]["rollback_boundary"]
                ["source_map_policy"],
            "batch_generation_read_context_owner_is_rust_only_and_surviving_python_query_status_surfaces_are_tracked_by_external_shared_runtime_projection_contracts"
        );
        assert_eq!(
            contract["active_query_owner_contract"]["rollback_boundary"]["source_map_policy"],
            "batch_generation_read_context_owner_is_rust_only_and_surviving_python_query_status_surfaces_are_tracked_by_external_shared_runtime_projection_contracts"
        );
        assert_eq!(
            contract["stream_state_owner_contract"]["rollback_boundary"]["source_map_policy"],
            "batch_generation_read_context_owner_is_rust_only_and_surviving_python_query_status_surfaces_are_tracked_by_external_shared_runtime_projection_contracts"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owned_sources_owner"],
            "load_owned_batch_generation_task_sources"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status_payload_owner"],
            "load_owned_batch_generation_status_payload"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["stream_event_owner"],
            "BatchGenerationStreamState::events"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "batch-generation read-context source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_batch_generation_read_context_owner_source_map_deleted"
        );
    }

    #[test]
    fn should_publish_batch_generation_task_recovery_owner_contract() {
        let contract = build_batch_generation_task_recovery_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_read_context_service::task_recovery_owner"
        );
        assert_eq!(contract["scope"], "task_recovery_owner");
        assert_eq!(contract["python_source_map"], json!([]));
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs"
        );
        assert_eq!(
            contract["validation_boundary"][0],
            "cargo test chapter_batch_generation_read_context_service"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"]
                .as_array()
                .expect("entrypoints")
                .iter()
                .any(|entry| entry == "recover_generation_task_if_needed_with_snapshot"),
            true
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"]
                .as_array()
                .expect("entrypoints")
                .iter()
                .any(|entry| entry == "recover_generation_task_if_needed"),
            true
        );
        assert_eq!(
            contract["behavior_contract"]["running_timeout_basis"],
            "latest snapshot/runtime heartbeat, falling back to task.started_at"
        );
        assert_eq!(contract["behavior_contract"]["running_timeout_minutes"], 15);
        assert_eq!(contract["behavior_contract"]["pending_timeout_minutes"], 3);
        assert_eq!(
            contract["behavior_contract"]["mutated_task_fields"][0],
            "status=failed"
        );
        assert_eq!(
            contract["rollback_boundary"],
            "batch_generation_package_query_source_map"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["recovery_error_owner"],
            "resolve_generation_task_auto_recovery_error"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["recovery_mutation_owner"],
            "recover_generation_task_if_needed"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_batch_generation_task_recovery_owner_source_map_deleted"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "batch-generation task-recovery source-map package deleted; surviving Python closeout work is now limited to shared batch-generation-task schema/runtime/API/test-support packages"
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["batch_generation_task_model"],
            "shared_python_runtime_api_and_test_support_reference"
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["default_python_module_consumers"],
            json!([
                "backend/tests/test_support/database_test_support.py",
                "backend/tests/test_support/task_system/snapshot_runtime_persistence.py"
            ])
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["dedicated_python_regression_surfaces"],
            json!([
                "backend/tests/test_api/test_chapters.py",
                "backend/tests/test_api/test_chapters_batch_generation.py",
                "backend/tests/test_api/test_chapters_batch_status_resume.py"
            ])
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["test_support_consumers"],
            json!([
                "backend/tests/test_support/batch_generation_status_read_owner_test_adapter.py",
                "backend/tests/test_support/batch_generation_orchestration_test_adapter.py",
                "backend/tests/test_support/batch_generation_route_test_adapter.py"
            ])
        );
        assert_eq!(
            contract["shared_schema_hold_status"]["physical_closeout_ready"],
            false
        );
    }

    #[test]
    fn should_publish_batch_generation_stream_progress_owner_contract() {
        let contract = build_batch_generation_stream_progress_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_batch_generation_read_context_service::stream_progress_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][0],
            "build_batch_generation_stream_progress_event"
        );
        assert_eq!(contract["behavior_contract"]["event_type"], "progress");
        assert_eq!(
            contract["behavior_contract"]["fields"][6],
            "candidate_gateway"
        );
        assert_eq!(
            contract["active_consumers"][1],
            "chapter-batch-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["stream_progress_event_owner"],
            "build_batch_generation_stream_progress_event"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["candidate_gateway_projection_owner"],
            "BatchGenerationStreamProgressEventInput.candidate_gateway"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "batch-generation read-context source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_batch_generation_stream_progress_owner_source_map_deleted"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "batch_generation_stream_progress_owner_is_rust_only_and_surviving_candidate_event_projection_surfaces_are_tracked_by_external_runtime_contracts"
        );
    }

    #[test]
    fn should_resolve_running_generation_task_auto_recovery_error() {
        let mut task = build_task("running");
        let now = Utc::now().naive_utc();
        task.started_at = Some(now - ChronoDuration::minutes(16));

        let error = resolve_generation_task_auto_recovery_error(&task, now);

        assert_eq!(
            error.as_deref(),
            Some("任务超时（超过15分钟未完成，已自动恢复）")
        );
    }

    #[test]
    fn should_not_recover_running_generation_task_with_recent_runtime_snapshot_activity() {
        let mut task = build_task("running");
        let now = naive_datetime(2026, 5, 31, 21, 0, 0);
        task.started_at = Some(now - ChronoDuration::minutes(40));
        let mut snapshot = build_snapshot();
        snapshot.updated_at = Some(now - ChronoDuration::minutes(2));

        assert_eq!(
            resolve_generation_task_auto_recovery_error_with_snapshot(&task, Some(&snapshot), now),
            None
        );
    }

    #[test]
    fn should_recover_running_generation_task_when_runtime_snapshot_activity_is_stale() {
        let mut task = build_task("running");
        let now = naive_datetime(2026, 5, 31, 21, 0, 0);
        task.started_at = Some(now - ChronoDuration::minutes(40));
        let mut snapshot = build_snapshot();
        snapshot.updated_at = Some(now - ChronoDuration::minutes(16));

        assert_eq!(
            resolve_generation_task_auto_recovery_error_with_snapshot(&task, Some(&snapshot), now)
                .as_deref(),
            Some("任务超时（超过15分钟未完成，已自动恢复）")
        );
    }

    #[test]
    fn should_resolve_pending_generation_task_auto_recovery_error() {
        let mut task = build_task("pending");
        let now = Utc::now().naive_utc();
        task.created_at = Some(now - ChronoDuration::minutes(4));

        let error = resolve_generation_task_auto_recovery_error(&task, now);

        assert_eq!(
            error.as_deref(),
            Some("任务启动超时（超过3分钟未启动，已自动恢复）")
        );
    }

    #[test]
    fn should_not_resolve_generation_task_auto_recovery_error_within_time_budget() {
        let mut running = build_task("running");
        let mut pending = build_task("pending");
        let now = naive_datetime(2026, 5, 31, 21, 0, 0);
        running.started_at = Some(now - ChronoDuration::minutes(10));
        pending.created_at = Some(now - ChronoDuration::minutes(2));

        assert_eq!(
            resolve_generation_task_auto_recovery_error(&running, now),
            None
        );
        assert_eq!(
            resolve_generation_task_auto_recovery_error(&pending, now),
            None
        );
    }

    #[test]
    fn should_build_batch_generation_read_context_from_snapshot() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let context = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        };

        assert_eq!(context.task.id, "task-1");
        assert_eq!(
            context.workflow_runtime_state,
            Some(json!({
                "progress": 60,
                "candidate_gateway": {
                    "execution_path": "rust_candidate_executor",
                    "fallback_applied": false,
                    "fallback_reason": "rust executor completed",
                    "rollback_boundary": "python_candidate_executor_fallback",
                    "rust_error": null
                },
                "active_story_repair_payload": {
                    "mode": "repair"
                }
            }))
        );
        assert_eq!(
            context.quality_status_context.latest_quality_metrics,
            Some(json!({"score": 91}))
        );
        assert_eq!(
            context.quality_status_context.quality_metrics_history,
            Some(json!([{"score": 90}]))
        );
        assert_eq!(
            context.quality_status_context.quality_metrics_summary,
            Some(json!({"summary": "ok"}))
        );
        assert_eq!(
            context
                .quality_status_context
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("scope")),
            Some(&json!("batch"))
        );
        assert_eq!(
            context
                .quality_status_context
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("chapter_count")),
            Some(&json!(1))
        );
        assert_eq!(context.quality_status_context.quality_history_context, None);
        assert_eq!(
            context.quality_status_context.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_build_batch_generation_read_context_without_snapshot() {
        let context = BatchGenerationReadContext {
            task: build_task("pending"),
            workflow_runtime_state: None,
            quality_status_context: BatchGenerationQualityStatusContext::default(),
        };

        assert_eq!(context.task.status, "pending");
        assert_eq!(context.workflow_runtime_state, None);
        assert_eq!(
            context.quality_status_context,
            BatchGenerationQualityStatusContext::default()
        );
    }

    #[test]
    fn should_build_batch_generation_read_contexts_from_snapshot_owner_map() {
        let mut first_task = build_task("running");
        first_task.id = "task-1".to_string();
        let mut second_task = build_task("pending");
        second_task.id = "task-2".to_string();

        let mut second_snapshot = build_snapshot();
        second_snapshot.batch_task_id = "task-2".to_string();
        second_snapshot.workflow_runtime_state = Some(json!({
            "progress": 25,
            "last_message": "等待中"
        }));

        let contexts = super::build_batch_generation_read_contexts_from_snapshot_owner_map(
            vec![first_task, second_task],
            HashMap::from([(String::from("task-2"), second_snapshot)]),
        );

        assert_eq!(contexts.len(), 2);
        assert_eq!(contexts[0].task.id, "task-1");
        assert_eq!(contexts[0].workflow_runtime_state, None);
        assert_eq!(contexts[1].task.id, "task-2");
        assert_eq!(
            contexts[1].workflow_runtime_state,
            Some(json!({
                "progress": 25,
                "last_message": "等待中"
            }))
        );
    }

    #[test]
    fn should_classify_active_batch_generation_task_status() {
        assert!(super::is_active_batch_generation_task_status("pending"));
        assert!(super::is_active_batch_generation_task_status("running"));
        assert!(!super::is_active_batch_generation_task_status("failed"));
        assert!(!super::is_active_batch_generation_task_status("completed"));
    }

    #[test]
    fn should_build_shared_read_payload_plan_from_context_owner() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let (task, payload) = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        }
        .into_payload_parts();

        assert_eq!(task.id, "task-1");
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_history"][0]["score"], 90);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["quality_metrics_summary_state"]["scope"], "batch");
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 1);
        assert!(payload["quality_history_context"].is_null());
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
        assert_eq!(
            payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(payload["candidate_gateway"]["fallback_applied"], false);
        assert_eq!(
            payload["checkpoint"]["candidate_gateway"],
            payload["candidate_gateway"]
        );
    }

    #[test]
    fn should_build_active_project_task_payload_from_read_context() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let payload = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        }
        .into_active_project_task_payload();

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
        assert_eq!(
            payload["candidate_gateway"]["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
        assert!(payload.get("task_type").is_none());
        assert!(payload.get("project_id").is_none());
        assert!(payload.get("current_retry_count").is_none());
        assert!(payload.get("completed_at").is_none());
        assert!(payload.get("error_message").is_none());
        assert!(payload.get("terminal_reason").is_none());
        assert!(payload.get("can_resume").is_none());
    }

    #[test]
    fn should_build_active_task_list_item_payload_without_terminal_fields() {
        let snapshot = build_snapshot();
        let workflow_runtime_state = snapshot
            .workflow_runtime_state
            .clone()
            .expect("snapshot runtime state");
        let quality_status_context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
                Some(&snapshot),
                Some(&workflow_runtime_state),
            );
        let payload = BatchGenerationReadContext {
            task: build_task("running"),
            workflow_runtime_state: Some(workflow_runtime_state),
            quality_status_context,
        }
        .into_active_task_list_item_payload();

        assert_eq!(payload["task_type"], "chapters_batch_generate");
        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(
            payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert!(payload.get("current_retry_count").is_none());
        assert!(payload.get("failed_chapters").is_none());
        assert!(payload.get("terminal_reason").is_none());
    }

    #[test]
    fn should_keep_read_payload_parts_contract() {
        let task = build_task("running");
        let payload = serde_json::Map::from_iter([
            ("batch_id".to_string(), json!("task-1")),
            ("status".to_string(), json!("running")),
        ]);

        assert_eq!(task.id, "task-1");
        assert_eq!(task.failed_chapters, json!([]));
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["status"], "running");
    }

    #[test]
    fn should_keep_owned_status_payload_loader_error_contract_inside_read_context_service() {
        let missing = LoadOwnedBatchGenerationTaskError::TaskNotFound;
        let internal = LoadOwnedBatchGenerationTaskError::Internal("boom".to_string());

        assert_eq!(missing, LoadOwnedBatchGenerationTaskError::TaskNotFound);
        assert_eq!(
            internal,
            LoadOwnedBatchGenerationTaskError::Internal("boom".to_string())
        );
    }

    #[test]
    fn should_keep_owned_status_payload_read_state_projection_contract_inside_read_context_service()
    {
        let mut task = build_task("running");
        task.id = "task-owned-status-1".to_string();
        let payload = super::build_owned_batch_generation_status_payload_from_read_state(
            OwnedBatchGenerationTaskReadState::from_parts(task, Some(build_snapshot())),
        );
        assert_eq!(payload["batch_id"], "task-owned-status-1");
        assert_eq!(payload["checkpoint"]["progress"], 60);
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
        assert_eq!(
            payload["candidate_gateway"]["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            payload["checkpoint"]["candidate_gateway"],
            payload["candidate_gateway"]
        );
    }
}

#[cfg(test)]
mod owned_task_read_context_owner_tests {
    use super::{
        LoadOwnedBatchGenerationTaskError, LoadOwnedBatchGenerationTaskSourcesError,
        OwnedBatchGenerationTaskReadState, OwnedBatchGenerationTaskSources,
    };
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use serde_json::json;

    fn build_task() -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-owned-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", "chapter-2"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: "running".to_string(),
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-2".to_string()),
            current_chapter_number: Some(2),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn build_snapshot() -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-owned-1".to_string(),
            batch_task_id: "task-owned-1".to_string(),
            latest_quality_metrics: None,
            quality_metrics_history: None,
            quality_metrics_summary: None,
            workflow_runtime_state: Some(json!({"progress": 55})),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn should_keep_owned_task_read_state_owner_contract_inside_read_context_service() {
        let state =
            OwnedBatchGenerationTaskReadState::from_parts(build_task(), Some(build_snapshot()));

        assert_eq!(state.task().id, "task-owned-1");
        assert_eq!(
            state
                .snapshot()
                .and_then(|snapshot| snapshot.workflow_runtime_state.as_ref())
                .and_then(|state| state.get("progress"))
                .and_then(|value| value.as_i64()),
            Some(55)
        );
    }

    #[test]
    fn should_keep_owned_task_sources_owner_contract_inside_read_context_service() {
        let sources =
            OwnedBatchGenerationTaskSources::from_parts(build_task(), Some(build_snapshot()));

        assert_eq!(sources.task().id, "task-owned-1");
        assert_eq!(
            sources
                .snapshot()
                .and_then(|snapshot| snapshot.workflow_runtime_state.as_ref())
                .and_then(|state| state.get("progress"))
                .and_then(|value| value.as_i64()),
            Some(55)
        );
    }

    #[test]
    fn should_keep_owned_task_read_state_error_contract_inside_read_context_service() {
        let missing = LoadOwnedBatchGenerationTaskError::TaskNotFound;
        let internal = LoadOwnedBatchGenerationTaskError::Internal("boom".to_string());

        assert_eq!(missing, LoadOwnedBatchGenerationTaskError::TaskNotFound);
        assert_eq!(
            internal,
            LoadOwnedBatchGenerationTaskError::Internal("boom".to_string())
        );
    }

    #[test]
    fn should_keep_owned_task_sources_error_contract_inside_read_context_service() {
        let missing = LoadOwnedBatchGenerationTaskSourcesError::Task(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
        );
        let snapshot = LoadOwnedBatchGenerationTaskSourcesError::Snapshot("boom".to_string());

        assert_eq!(
            missing,
            LoadOwnedBatchGenerationTaskSourcesError::Task(
                LoadOwnedBatchGenerationTaskError::TaskNotFound
            )
        );
        assert_eq!(
            snapshot,
            LoadOwnedBatchGenerationTaskSourcesError::Snapshot("boom".to_string())
        );
    }
}

#[cfg(test)]
mod active_query_owner_tests {
    use super::active_query_owner::{
        build_active_batch_generation_task_list_query_request_from_route_query,
        ActiveBatchGenerationTaskListQueryRequestError, ActiveBatchGenerationTaskListRouteQuery,
        ActiveBatchGenerationTaskListRouteQueryError, ActiveProjectBatchGenerationRouteError,
    };
    use crate::services::project_service::ProjectAccessQueryError;
    use serde_json::json;

    #[test]
    fn should_validate_active_batch_generation_task_list_query_request_limit_like_python_query() {
        assert_eq!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: None }
            )
            .expect("default limit should be valid")
            .limit(),
            20
        );
        assert_eq!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(25) }
            )
            .expect("explicit in-range limit should be valid")
            .limit(),
            25
        );
        assert_eq!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(0) }
            ),
            Err(ActiveBatchGenerationTaskListQueryRequestError::LimitTooSmall)
        );
        assert_eq!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(-1) }
            ),
            Err(ActiveBatchGenerationTaskListQueryRequestError::LimitTooSmall)
        );
        assert_eq!(
            build_active_batch_generation_task_list_query_request_from_route_query(
                ActiveBatchGenerationTaskListRouteQuery { limit: Some(500) }
            ),
            Err(ActiveBatchGenerationTaskListQueryRequestError::LimitTooLarge)
        );
    }

    #[test]
    fn should_keep_active_batch_generation_task_list_route_query_error_shape() {
        let error = build_active_batch_generation_task_list_query_request_from_route_query(
            ActiveBatchGenerationTaskListRouteQuery { limit: Some(0) },
        )
        .map_err(ActiveBatchGenerationTaskListRouteQueryError::Request)
        .expect_err("out-of-range limit should fail before query execution");

        assert_eq!(
            error,
            ActiveBatchGenerationTaskListRouteQueryError::Request(
                ActiveBatchGenerationTaskListQueryRequestError::LimitTooSmall,
            )
        );
    }

    #[test]
    fn should_keep_active_project_batch_generation_route_error_shape() {
        let error = ActiveProjectBatchGenerationRouteError::Query(
            ProjectAccessQueryError::NotFoundOrAccessDenied,
        );

        assert_eq!(
            error,
            ActiveProjectBatchGenerationRouteError::Query(
                ProjectAccessQueryError::NotFoundOrAccessDenied,
            )
        );
    }

    #[test]
    fn should_keep_active_batch_generation_task_list_view_owner_contract() {
        let payload =
            super::active_query_owner::build_active_batch_generation_task_list_view_payload(vec![
                json!({
                    "batch_id": "task-1"
                }),
            ]);

        assert_eq!(payload["total"], 1);
        assert_eq!(payload["items"][0]["batch_id"], "task-1");
    }

    #[test]
    fn should_keep_active_batch_generation_query_view_owner_contract() {
        let payload = super::active_query_owner::build_active_project_batch_generation_view_payload(
            Some(json!({
                "batch_id": "task-2"
            })),
        );

        assert_eq!(payload["has_active_task"], true);
        assert_eq!(payload["task"]["batch_id"], "task-2");
    }

    #[test]
    fn should_build_empty_active_batch_generation_query_response() {
        let payload =
            super::active_query_owner::build_active_project_batch_generation_view_payload(None);

        assert_eq!(payload["has_active_task"], false);
        assert!(payload["task"].is_null());
    }
}

#[cfg(test)]
mod db_backed_batch_generation_business_smoke_tests {
    use chrono::Utc;
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, Schema, Set,
    };
    use serde_json::json;

    use super::{
        load_active_batch_generation_view_from_route_project,
        load_active_user_batch_generation_task_list_view_from_route_query,
        load_owned_batch_generation_status_payload, load_owned_batch_generation_stream_state,
        ActiveBatchGenerationTaskListRouteQuery,
    };
    use crate::models::{batch_generation_snapshot, batch_generation_task, project};

    async fn setup_batch_read_owner_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);

        db.execute(builder.build(&schema.create_table_from_entity(project::Entity)))
            .await
            .expect("create projects table");
        db.execute(builder.build(&schema.create_table_from_entity(batch_generation_task::Entity)))
            .await
            .expect("create batch generation tasks table");
        db.execute(
            builder.build(&schema.create_table_from_entity(batch_generation_snapshot::Entity)),
        )
        .await
        .expect("create batch generation snapshots table");

        db
    }

    async fn seed_logged_in_batch_read_owner_fixture(db: &DatabaseConnection) {
        let now = Utc::now().naive_utc();

        project::ActiveModel {
            id: Set("project-db-smoke".to_string()),
            user_id: Set("user-db-smoke".to_string()),
            title: Set("DB Smoke Project".to_string()),
            description: Set(None),
            theme: Set(None),
            genre: Set(None),
            target_words: Set(12_000),
            current_words: Set(3_000),
            status: Set("active".to_string()),
            wizard_status: Set("completed".to_string()),
            wizard_step: Set(0),
            outline_mode: Set("simple".to_string()),
            world_time_period: Set(None),
            world_location: Set(None),
            world_atmosphere: Set(None),
            world_rules: Set(None),
            chapter_count: Set(Some(3)),
            narrative_perspective: Set(None),
            character_count: Set(0),
            default_creative_mode: Set(None),
            default_story_focus: Set(None),
            default_plot_stage: Set(None),
            default_story_creation_brief: Set(None),
            default_quality_preset: Set(None),
            default_quality_notes: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .expect("insert db smoke project");

        batch_generation_task::ActiveModel {
            id: Set("batch-db-smoke".to_string()),
            project_id: Set("project-db-smoke".to_string()),
            user_id: Set("user-db-smoke".to_string()),
            start_chapter_number: Set(2),
            chapter_count: Set(2),
            chapter_ids: Set(json!(["chapter-db-2", "chapter-db-3"])),
            style_id: Set(None),
            target_word_count: Set(2800),
            enable_analysis: Set(true),
            status: Set("running".to_string()),
            total_chapters: Set(2),
            completed_chapters: Set(1),
            failed_chapters: Set(json!([])),
            current_chapter_id: Set(Some("chapter-db-3".to_string())),
            current_chapter_number: Set(Some(3)),
            current_retry_count: Set(0),
            max_retries: Set(3),
            created_at: Set(Some(now)),
            started_at: Set(Some(now)),
            completed_at: Set(None),
            error_message: Set(None),
        }
        .insert(db)
        .await
        .expect("insert db smoke batch task");

        batch_generation_snapshot::ActiveModel {
            id: Set("snapshot-db-smoke".to_string()),
            batch_task_id: Set("batch-db-smoke".to_string()),
            latest_quality_metrics: Set(Some(json!({
                "overall_score": 91.0,
                "source": "db-backed-smoke"
            }))),
            quality_metrics_history: Set(Some(json!([{
                "overall_score": 90.0,
                "source": "db-backed-smoke-history"
            }]))),
            quality_metrics_summary: Set(Some(json!({
                "chapter_count": 1,
                "avg_score": 91.0
            }))),
            workflow_runtime_state: Set(Some(json!({
                "phase": "generating",
                "progress": 65,
                "last_event": "selected_candidate",
                "last_message": "DB backed Rust batch smoke selected candidate",
                "candidate_gateway": {
                    "execution_path": "rust_candidate_executor",
                    "fallback_applied": false,
                    "rollback_boundary": "python_candidate_executor_fallback",
                    "rust_error": null
                },
                "selected_candidate_events": [
                    {
                        "type": "progress",
                        "event": "selected_candidate",
                        "message": "候选章节已选择"
                    },
                    {
                        "type": "chunk",
                        "content": "DB-backed Rust selected candidate chunk"
                    }
                ],
                "active_story_repair_payload": {
                    "scope": "batch",
                    "mode": "db-backed-smoke"
                }
            }))),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .expect("insert db smoke batch snapshot");
    }

    async fn insert_db_backed_active_task_fixture(
        db: &DatabaseConnection,
        task_id: &str,
        project_id: &str,
        user_id: &str,
        created_at_offset_seconds: i64,
        progress: i32,
        gateway_path: &str,
    ) {
        let now = Utc::now().naive_utc() + chrono::Duration::seconds(created_at_offset_seconds);

        batch_generation_task::ActiveModel {
            id: Set(task_id.to_string()),
            project_id: Set(project_id.to_string()),
            user_id: Set(user_id.to_string()),
            start_chapter_number: Set(1),
            chapter_count: Set(2),
            chapter_ids: Set(json!(["chapter-db-extra-1", "chapter-db-extra-2"])),
            style_id: Set(None),
            target_word_count: Set(2600),
            enable_analysis: Set(false),
            status: Set("running".to_string()),
            total_chapters: Set(2),
            completed_chapters: Set(0),
            failed_chapters: Set(json!([])),
            current_chapter_id: Set(Some("chapter-db-extra-1".to_string())),
            current_chapter_number: Set(Some(1)),
            current_retry_count: Set(0),
            max_retries: Set(3),
            created_at: Set(Some(now)),
            started_at: Set(Some(now)),
            completed_at: Set(None),
            error_message: Set(None),
        }
        .insert(db)
        .await
        .expect("insert extra db smoke batch task");

        batch_generation_snapshot::ActiveModel {
            id: Set(format!("snapshot-{task_id}")),
            batch_task_id: Set(task_id.to_string()),
            latest_quality_metrics: Set(Some(json!({
                "overall_score": 88.0,
                "source": task_id
            }))),
            quality_metrics_history: Set(None),
            quality_metrics_summary: Set(Some(json!({
                "chapter_count": 0,
                "avg_score": 88.0
            }))),
            workflow_runtime_state: Set(Some(json!({
                "phase": "generating",
                "progress": progress,
                "last_message": format!("DB backed active task {task_id}"),
                "candidate_gateway": {
                    "execution_path": gateway_path,
                    "fallback_applied": false,
                    "rollback_boundary": "python_candidate_executor_fallback",
                    "rust_error": null
                },
                "active_story_repair_payload": {
                    "scope": "batch",
                    "mode": task_id
                }
            }))),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .expect("insert extra db smoke batch snapshot");
    }

    #[tokio::test]
    async fn should_load_logged_in_db_backed_batch_status_active_and_list_payloads_from_rust_owner()
    {
        let db = setup_batch_read_owner_db().await;
        seed_logged_in_batch_read_owner_fixture(&db).await;

        let status_payload =
            load_owned_batch_generation_status_payload(&db, "batch-db-smoke", "user-db-smoke")
                .await
                .expect("db-backed status payload");
        let active_payload = load_active_batch_generation_view_from_route_project(
            &db,
            "user-db-smoke",
            "project-db-smoke".to_string(),
        )
        .await
        .expect("db-backed active project payload");
        let list_payload = load_active_user_batch_generation_task_list_view_from_route_query(
            &db,
            "user-db-smoke",
            ActiveBatchGenerationTaskListRouteQuery { limit: Some(5) },
        )
        .await
        .expect("db-backed active task list payload");

        assert_eq!(status_payload["batch_id"], "batch-db-smoke");
        assert_eq!(status_payload["status"], "running");
        assert_eq!(status_payload["checkpoint"]["progress"], 65);
        assert_eq!(
            status_payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(
            status_payload["candidate_gateway"]["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(
            status_payload["active_story_repair_payload"]["mode"],
            "db-backed-smoke"
        );

        assert_eq!(active_payload["has_active_task"], true);
        assert_eq!(active_payload["task"]["batch_id"], "batch-db-smoke");
        assert_eq!(
            active_payload["task"]["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(
            active_payload["task"]["active_story_repair_payload"]["scope"],
            "batch"
        );

        assert_eq!(list_payload["total"], 1);
        assert_eq!(list_payload["items"][0]["batch_id"], "batch-db-smoke");
        assert_eq!(
            list_payload["items"][0]["candidate_gateway"]["fallback_applied"],
            false
        );
        assert_eq!(
            list_payload["items"][0]["latest_quality_metrics"]["source"],
            "db-backed-smoke"
        );
    }

    #[tokio::test]
    async fn should_filter_sort_and_limit_db_backed_active_batch_views_from_rust_read_owner() {
        let db = setup_batch_read_owner_db().await;
        seed_logged_in_batch_read_owner_fixture(&db).await;
        insert_db_backed_active_task_fixture(
            &db,
            "batch-db-newer",
            "project-db-smoke",
            "user-db-smoke",
            30,
            72,
            "rust_candidate_executor_newer",
        )
        .await;
        insert_db_backed_active_task_fixture(
            &db,
            "batch-db-other-project",
            "project-db-other",
            "user-db-smoke",
            60,
            80,
            "rust_candidate_executor_other_project",
        )
        .await;
        insert_db_backed_active_task_fixture(
            &db,
            "batch-db-other-user",
            "project-db-smoke",
            "user-db-other",
            90,
            95,
            "rust_candidate_executor_other_user",
        )
        .await;

        let active_payload = load_active_batch_generation_view_from_route_project(
            &db,
            "user-db-smoke",
            "project-db-smoke".to_string(),
        )
        .await
        .expect("db-backed active project payload");
        let list_payload = load_active_user_batch_generation_task_list_view_from_route_query(
            &db,
            "user-db-smoke",
            ActiveBatchGenerationTaskListRouteQuery { limit: Some(2) },
        )
        .await
        .expect("db-backed active task list payload");

        assert_eq!(active_payload["has_active_task"], true);
        assert_eq!(active_payload["task"]["batch_id"], "batch-db-newer");
        assert_eq!(active_payload["task"]["checkpoint"]["progress"], 72);
        assert_eq!(
            active_payload["task"]["candidate_gateway"]["execution_path"],
            "rust_candidate_executor_newer"
        );
        assert_eq!(
            active_payload["task"]["active_story_repair_payload"]["mode"],
            "batch-db-newer"
        );

        assert_eq!(list_payload["total"], 2);
        assert_eq!(
            list_payload["items"][0]["batch_id"],
            "batch-db-other-project"
        );
        assert_eq!(list_payload["items"][1]["batch_id"], "batch-db-newer");
        assert_eq!(
            list_payload["items"][0]["candidate_gateway"]["execution_path"],
            "rust_candidate_executor_other_project"
        );
        assert_eq!(
            list_payload["items"][1]["candidate_gateway"]["execution_path"],
            "rust_candidate_executor_newer"
        );
        assert!(!list_payload["items"]
            .as_array()
            .expect("active list items")
            .iter()
            .any(|item| item["batch_id"] == "batch-db-other-user"));
        assert!(!list_payload["items"]
            .as_array()
            .expect("active list items")
            .iter()
            .any(|item| item["batch_id"] == "batch-db-smoke"));
    }

    #[tokio::test]
    async fn should_project_db_backed_selected_candidate_events_into_batch_stream_state_from_rust_owner(
    ) {
        let db = setup_batch_read_owner_db().await;
        seed_logged_in_batch_read_owner_fixture(&db).await;

        let stream_state =
            load_owned_batch_generation_stream_state(&db, "batch-db-smoke", "user-db-smoke")
                .await
                .expect("db-backed stream state");
        let events = stream_state.events();

        assert_eq!(stream_state.status, "running");
        assert_eq!(stream_state.progress, 65);
        assert_eq!(
            stream_state.message,
            "DB backed Rust batch smoke selected candidate"
        );
        assert_eq!(stream_state.selected_candidate_events.len(), 2);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0]["type"], "progress");
        assert_eq!(events[1]["message"], "候选章节已选择");
        assert_eq!(events[2]["type"], "chunk");
        assert_eq!(
            events[2]["content"],
            "DB-backed Rust selected candidate chunk"
        );
        assert_eq!(
            stream_state
                .observation_key()
                .selected_candidate_events
                .len(),
            2
        );
    }
}

#[cfg(test)]
mod read_context_stream_tests {
    use super::{
        BatchGenerationResolvedStreamStatus, BatchGenerationStreamObservationKey,
        BatchGenerationStreamProgressEventInput, BatchGenerationStreamState,
        BatchGenerationStreamTerminalKind, OwnedBatchGenerationTaskReadState,
    };
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_batch_generation_task_payload_base_service::BatchGenerationQualityStatusContext;
    use axum::response::sse::Event;
    use serde_json::json;

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
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-2".to_string()),
            current_chapter_number: Some(2),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn build_stream_state(status: &str) -> BatchGenerationStreamState {
        BatchGenerationStreamState {
            task: build_task(status),
            status: status.to_string(),
            completed: 1,
            progress: 65,
            message: "处理中".to_string(),
            phase: "generating".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            selected_candidate_events: vec![],
            quality_gate: None,
            active_story_repair_payload: None,
            candidate_gateway: None,
            terminal_label: None,
        }
    }

    #[test]
    fn should_build_python_compatible_stream_connected_event_payload() {
        let payload = super::batch_generation_stream_connected_event_payload();

        assert_eq!(payload["type"], "progress");
        assert_eq!(payload["message"], "正在连接批量生成任务流");
        assert_eq!(payload["progress"], 0);
        assert_eq!(payload["status"], "processing");
    }

    #[test]
    fn should_build_stream_progress_event_with_candidate_gateway_from_owner() {
        let payload = super::build_batch_generation_stream_progress_event(
            BatchGenerationStreamProgressEventInput {
                message: "处理中".to_string(),
                progress: 42,
                status: "running",
                phase: "generating".to_string(),
                current_retry_count: 1,
                max_retries: 3,
                candidate_gateway: Some(json!({
                    "execution_path": "rust_candidate_executor",
                    "fallback_applied": false,
                    "rollback_boundary": "python_candidate_executor_fallback",
                })),
            },
        );

        assert_eq!(payload["type"], "progress");
        assert_eq!(payload["message"], "处理中");
        assert_eq!(payload["progress"], 42);
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["phase"], "generating");
        assert_eq!(payload["current_retry_count"], 1);
        assert_eq!(payload["max_retries"], 3);
        assert_eq!(
            payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(payload["candidate_gateway"]["fallback_applied"], false);
        assert_eq!(
            payload["candidate_gateway"]["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
    }

    #[test]
    fn should_resolve_stream_poll_from_pending_initial_state_first() {
        let initial_state = build_stream_state("running");
        let mut pending_state = Some(initial_state.clone());

        let state = match pending_state.take() {
            Some(state) => state,
            None => panic!("pending state should be present when checked"),
        };

        assert_eq!(state.status, "running");
        assert_eq!(state.progress, 65);

        assert!(pending_state.is_none());
    }

    #[test]
    fn should_close_stream_poll_when_loaded_state_is_missing() {
        let pending_state: Option<BatchGenerationStreamState> = None;

        assert!(pending_state.is_none());
    }

    #[test]
    fn should_build_task_not_found_stream_system_event_payload() {
        let payload = super::batch_generation_stream_task_not_found_event_payload();

        assert_eq!(payload["type"], "error");
        assert_eq!(payload["error"], "批量生成任务不存在");
        assert_eq!(payload["code"], 404);
    }

    #[test]
    fn should_build_timed_out_stream_system_event_payload() {
        let payload = super::batch_generation_stream_timeout_event_payload();

        assert_eq!(payload["type"], "error");
        assert_eq!(payload["error"], "批量生成任务流等待超时");
        assert_eq!(payload["code"], 408);
    }

    #[test]
    fn should_build_python_compatible_stream_heartbeat_comment() {
        assert_eq!(
            super::batch_generation_stream_heartbeat_comment(),
            "heartbeat"
        );
    }

    #[test]
    fn should_build_stream_state_from_task_and_snapshot_owner_inside_read_context_service() {
        let state = super::build_batch_generation_stream_state_from_task_and_snapshot(
            build_task("running"),
            Some(build_snapshot()),
        );

        assert_eq!(state.status, "running");
        assert_eq!(state.completed, 1);
        assert_eq!(state.progress, 60);
        assert_eq!(state.message, "正在生成正文...");
        assert_eq!(state.event_status, "processing");
        assert_eq!(
            state.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_build_terminal_stream_state_from_task_and_snapshot_owner_inside_read_context_service()
    {
        let state = super::build_batch_generation_stream_state_from_task_and_snapshot(
            build_task("completed"),
            None,
        );

        assert_eq!(state.progress, 100);
        assert_eq!(state.message, "生成完成");
        assert_eq!(state.event_status, "success");
        assert_eq!(
            state.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Completed)
        );
    }

    #[test]
    fn should_build_stream_state_from_shared_owned_read_state_owner_inside_read_context_service() {
        let (task, snapshot) = OwnedBatchGenerationTaskReadState::from_parts(
            build_task("running"),
            Some(build_snapshot()),
        )
        .into_parts();
        let state =
            super::build_batch_generation_stream_state_from_task_and_snapshot(task, snapshot);

        assert_eq!(state.status, "running");
        assert_eq!(state.progress, 60);
        assert_eq!(state.event_status, "processing");
        assert_eq!(
            state.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_build_terminal_stream_state_from_read_context_stream_owner() {
        let state = BatchGenerationStreamState::from_task_state(build_task("completed"), None);

        assert_eq!(state.progress, 100);
        assert_eq!(state.message, "生成完成");
        assert_eq!(state.event_status, "success");
        assert_eq!(
            state.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Completed)
        );
    }

    #[test]
    fn should_build_stream_state_with_checkpoint_fallbacks() {
        let running = BatchGenerationStreamState::from_task_state(build_task("running"), None);
        assert_eq!(running.progress, 65);
        assert_eq!(running.message, "正在生成正文...");
        assert_eq!(running.event_status, "processing");
        assert_eq!(running.terminal_kind, None);
        assert_eq!(running.analysis_task_id, None);
        assert_eq!(running.terminal_label, None);

        let completed = BatchGenerationStreamState::from_task_state(
            build_task("completed"),
            Some(&json!({
                "progress": 120,
                "last_message": "  ",
                "analysis_task_id": "analysis-task-1",
                "analysis_task_message": "第 2 章分析任务已启动",
                "analysis_task_progress": 85,
                "analysis_started_chapter_id": "chapter-2",
                "analysis_started_chapter_number": 2
            })),
        );
        assert_eq!(completed.progress, 100);
        assert_eq!(completed.message, "生成完成");
        assert_eq!(completed.event_status, "success");
        assert_eq!(
            completed.analysis_task_id.as_deref(),
            Some("analysis-task-1")
        );
        assert_eq!(
            completed.analysis_task_message.as_deref(),
            Some("第 2 章分析任务已启动")
        );
        assert_eq!(completed.analysis_task_progress, Some(85));
        assert_eq!(
            completed.analysis_started_chapter_id.as_deref(),
            Some("chapter-2")
        );
        assert_eq!(completed.analysis_started_chapter_number, Some(2));
        assert_eq!(
            completed.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Completed)
        );
        assert_eq!(completed.terminal_label, None);
    }

    #[test]
    fn should_build_stream_state_for_terminal_and_unknown_statuses() {
        let failed = BatchGenerationStreamState::from_task_state(build_task("failed"), None);
        assert_eq!(failed.progress, 100);
        assert_eq!(failed.message, "生成失败");
        assert_eq!(failed.event_status, "error");
        assert_eq!(
            failed.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Failed)
        );
        assert_eq!(failed.terminal_label, None);

        let cancelled = BatchGenerationStreamState::from_task_state(
            build_task("cancelled"),
            Some(&json!({
                "progress": -5,
                "last_message": "已停止"
            })),
        );
        assert_eq!(cancelled.progress, 0);
        assert_eq!(cancelled.message, "已停止");
        assert_eq!(cancelled.event_status, "processing");
        assert_eq!(cancelled.analysis_task_id, None);
        assert_eq!(
            cancelled.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Cancelled)
        );

        let unknown = BatchGenerationStreamState::from_task_state(build_task("queued"), None);
        assert_eq!(unknown.progress, 15);
        assert_eq!(unknown.message, "任务处理中");
        assert_eq!(unknown.event_status, "processing");
        assert_eq!(unknown.analysis_task_id, None);
        assert_eq!(unknown.terminal_kind, None);
        assert_eq!(unknown.terminal_label, None);
    }

    #[test]
    fn should_restore_active_story_repair_payload_from_runtime_state_when_quality_context_missing()
    {
        let state = BatchGenerationStreamState::from_task_state(
            build_task("running"),
            Some(&json!({
                "progress": 55,
                "active_story_repair_payload": {
                    "scope": "batch",
                    "mode": "repair"
                }
            })),
        );

        assert_eq!(
            state.active_story_repair_payload,
            Some(json!({
                "scope": "batch",
                "mode": "repair"
            }))
        );
    }

    #[test]
    fn should_resolve_stream_status_owner_contract() {
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("completed").terminal_kind(None),
            Some(BatchGenerationStreamTerminalKind::Completed)
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("failed").terminal_kind(None),
            Some(BatchGenerationStreamTerminalKind::Failed)
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("cancelled").terminal_kind(None),
            Some(BatchGenerationStreamTerminalKind::Cancelled)
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("failed")
                .terminal_kind(Some(&"自动修复后重试".to_string())),
            Some(BatchGenerationStreamTerminalKind::Failed)
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("failed").event_status(),
            "error"
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("completed").event_status(),
            "success"
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("running").event_status(),
            "processing"
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("running").terminal_kind(None),
            None
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("pending").default_progress(),
            10
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("queued").default_message(),
            "任务处理中"
        );
    }

    #[test]
    fn should_build_stream_observation_key_from_state_owner() {
        let state = BatchGenerationStreamState::from_task_state(
            build_task("completed"),
            Some(&json!({
                "progress": 100,
                "phase": "completed",
                "last_message": "生成完成",
                "analysis_task_id": "analysis-task-2",
                "analysis_task_message": "第 2 章分析任务已启动",
                "analysis_task_progress": 85,
                "analysis_started_chapter_id": "chapter-2",
                "analysis_started_chapter_number": 2,
                "quality_gate": {
                    "decision": "pass",
                    "phase": "completed"
                },
                "active_story_repair_payload": {
                    "quality_gate_decision": "pass",
                    "phase": "completed"
                }
            })),
        );

        let key = state.observation_key();

        assert_eq!(
            key,
            BatchGenerationStreamObservationKey {
                status: "completed".to_string(),
                completed: 1,
                progress: 100,
                message: "生成完成".to_string(),
                phase: "completed".to_string(),
                event_status: "success",
                current_retry_count: 0,
                max_retries: 3,
                analysis_task_id: Some("analysis-task-2".to_string()),
                analysis_task_message: Some("第 2 章分析任务已启动".to_string()),
                analysis_task_progress: Some(85),
                analysis_started_chapter_id: Some("chapter-2".to_string()),
                analysis_started_chapter_number: Some(2),
                selected_candidate_events: vec![],
                quality_gate: Some(json!({
                    "decision": "pass",
                    "phase": "completed"
                })),
                active_story_repair_payload: Some(json!({
                    "quality_gate_decision": "pass",
                    "phase": "completed"
                })),
                terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
                candidate_gateway: None,
            }
        );
    }

    #[test]
    fn should_keep_manual_review_stream_state_as_telemetry_only_from_quality_context_owner() {
        let manual_review = BatchGenerationStreamState::from_task_state_with_quality_context(
            build_task("failed"),
            None,
            Some(&BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({
                    "quality_gate": {
                        "decision": "manual_review",
                        "label": "等待人工复核"
                    }
                })),
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: None,
                quality_history_context: None,
                active_story_repair_payload: None,
            }),
        );
        assert_eq!(manual_review.message, "生成失败");
        assert_eq!(manual_review.phase, "failed");
        assert_eq!(
            manual_review.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Failed)
        );
        assert_eq!(manual_review.terminal_label.as_deref(), None);
        assert_eq!(
            manual_review
                .quality_gate
                .as_ref()
                .and_then(|gate| gate.get("decision"))
                .and_then(serde_json::Value::as_str),
            Some("manual_review")
        );

        let retry = BatchGenerationStreamState::from_task_state_with_quality_context(
            {
                let mut task = build_task("failed");
                task.current_retry_count = 1;
                task.max_retries = 3;
                task
            },
            None,
            Some(&BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "自动修复后重试"
                    }
                })),
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: None,
                quality_history_context: None,
                active_story_repair_payload: None,
            }),
        );
        assert_eq!(retry.message, "自动修复后重试");
        assert_eq!(
            retry.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Failed)
        );
        assert_eq!(retry.terminal_label.as_deref(), Some("自动修复后重试"));
    }

    #[test]
    fn should_resolve_failed_when_auto_repair_budget_is_exhausted() {
        let state = BatchGenerationStreamState::from_task_state_with_quality_context(
            {
                let mut task = build_task("failed");
                task.current_retry_count = 3;
                task.max_retries = 3;
                task
            },
            None,
            Some(&BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "自动修复预算已耗尽"
                    }
                })),
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: None,
                quality_history_context: None,
                active_story_repair_payload: None,
            }),
        );

        assert_eq!(state.message, "生成失败");
        assert_eq!(
            state.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Failed)
        );
        assert_eq!(state.terminal_label.as_deref(), None);
    }

    #[test]
    fn should_not_restore_manual_review_quality_blocked_status_from_runtime_state() {
        let manual_review = BatchGenerationStreamState::from_task_state_with_quality_context(
            build_task("failed"),
            Some(&json!({
                "phase": "quality_blocked",
                "last_message": "等待人工复核"
            })),
            Some(&BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({
                    "quality_gate": {
                        "decision": "manual_review",
                        "label": "等待人工复核"
                    }
                })),
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: None,
                quality_history_context: None,
                active_story_repair_payload: Some(json!({
                    "quality_gate_decision": "manual_review",
                    "quality_gate_label": "等待人工复核",
                    "phase": "quality_blocked"
                })),
            }),
        );
        assert_eq!(manual_review.phase, "failed");
        assert_eq!(manual_review.message, "生成失败");
        assert_eq!(manual_review.event_status, "error");
        assert_eq!(
            manual_review.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Failed)
        );
    }

    #[test]
    fn should_keep_retry_quality_gate_terminal_progress_status_running_before_error_event() {
        let retry = BatchGenerationStreamState::from_task_state_with_quality_context(
            {
                let mut task = build_task("failed");
                task.current_retry_count = 1;
                task.max_retries = 3;
                task
            },
            Some(&json!({
                "phase": "repair_pending",
                "last_message": "自动修复后重试"
            })),
            Some(&BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "自动修复后重试"
                    }
                })),
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: None,
                quality_history_context: None,
                active_story_repair_payload: Some(json!({
                    "quality_gate_decision": "auto_repair",
                    "quality_gate_label": "自动修复后重试",
                    "phase": "repair_pending"
                })),
            }),
        );

        assert_eq!(retry.event_status, "running");
    }

    #[test]
    fn should_keep_status_stream_system_event_owner_contract() {
        let task_not_found_payload = super::batch_generation_stream_task_not_found_event_payload();
        let timed_out_payload = super::batch_generation_stream_timeout_event_payload();

        assert_eq!(task_not_found_payload["error"], "批量生成任务不存在");
        assert_eq!(timed_out_payload["code"], 408);
        assert_eq!(
            super::batch_generation_stream_heartbeat_comment(),
            "heartbeat"
        );
    }

    fn build_snapshot() -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"score": 91})),
            quality_metrics_history: Some(json!([{"score": 90}])),
            quality_metrics_summary: Some(json!({"summary": "ok"})),
            workflow_runtime_state: Some(json!({
                "progress": 60,
                "active_story_repair_payload": {
                    "mode": "repair"
                }
            })),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn should_build_stream_transport_events_from_owner_contract() {
        let payload = json!({
            "type": "progress",
            "message": "处理中"
        });
        let data_event = super::batch_generation_stream_data_event(payload.clone());
        let heartbeat_event = super::batch_generation_stream_heartbeat_event();

        let data_debug = format!("{data_event:?}");
        let heartbeat_debug = format!("{heartbeat_event:?}");
        let expected_data_debug = format!("{:?}", Event::default().data(payload.to_string()));
        let expected_heartbeat_debug = format!("{:?}", Event::default().comment("heartbeat"));

        assert_eq!(data_debug, expected_data_debug);
        assert_eq!(heartbeat_debug, expected_heartbeat_debug);
        assert_eq!(
            super::batch_generation_stream_heartbeat_comment(),
            "heartbeat"
        );
    }

    #[test]
    fn should_build_stream_events_from_state_owner() {
        let state = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            phase: "completed".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            selected_candidate_events: vec![],
            quality_gate: None,
            active_story_repair_payload: None,
            candidate_gateway: None,
            terminal_label: None,
        };

        let events = state.events();

        assert_eq!(events.len(), 4);
        assert_eq!(events[0]["type"], "progress");
        assert_eq!(events[0]["status"], "success");
        assert_eq!(events[1]["type"], "analysis_started");
        assert_eq!(events[2]["type"], "result");
        assert_eq!(events[3]["type"], "done");
    }

    #[test]
    fn should_append_selected_candidate_events_from_runtime_state_owner() {
        let state = BatchGenerationStreamState::from_task_state(
            build_task("running"),
            Some(&json!({
                "progress": 70,
                "phase": "generating",
                "last_message": "候选已选定",
                "selected_candidate_events": [
                    {
                        "type": "progress",
                        "phase": "generating",
                        "message": "Selected chapter 1 candidate 1/2 (1200 chars)"
                    },
                    {
                        "type": "chunk",
                        "chapter_id": "chapter-1",
                        "content": "候选片段"
                    }
                ]
            })),
        );

        let events = state.events();

        assert_eq!(events.len(), 3);
        assert_eq!(events[0]["type"], "progress");
        assert_eq!(events[0]["message"], "候选已选定");
        assert_eq!(
            events[1]["message"],
            "Selected chapter 1 candidate 1/2 (1200 chars)"
        );
        assert_eq!(events[2]["type"], "chunk");
        assert_eq!(events[2]["content"], "候选片段");
        assert_eq!(state.observation_key().selected_candidate_events.len(), 2);
    }

    #[test]
    fn should_project_candidate_gateway_from_runtime_state_into_stream_events() {
        let candidate_gateway = json!({
            "execution_path": "rust_candidate_executor",
            "fallback_applied": false,
            "rollback_boundary": "python_candidate_executor_fallback",
            "rust_executor_enabled": true,
            "fallback_on_rust_error": true,
            "disabled_reason": null
        });
        let mut task = build_task("completed");
        task.current_chapter_id = Some("chapter-2".to_string());
        let state = BatchGenerationStreamState::from_task_state(
            task,
            Some(&json!({
                "progress": 100,
                "phase": "completed",
                "last_message": "生成完成",
                "analysis_task_id": "analysis-task-1",
                "analysis_started_chapter_id": "chapter-2",
                "analysis_started_chapter_number": 2,
                "candidate_gateway": candidate_gateway
            })),
        );

        let events = state.events();

        assert_eq!(state.candidate_gateway, Some(candidate_gateway.clone()));
        assert_eq!(
            state
                .observation_key()
                .candidate_gateway
                .as_ref()
                .and_then(|metadata| metadata.get("execution_path")),
            Some(&json!("rust_candidate_executor"))
        );
        assert_eq!(events[0]["candidate_gateway"], candidate_gateway);
        assert_eq!(
            events[1]["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(
            events[2]["candidate_gateway"]["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
    }

    #[test]
    fn should_build_terminal_batch_generation_events_from_read_context_stream_owner() {
        let completed = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            phase: "completed".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            selected_candidate_events: vec![],
            quality_gate: None,
            active_story_repair_payload: None,
            candidate_gateway: None,
            terminal_label: None,
        };
        let mut failed = BatchGenerationStreamState {
            task: build_task("failed"),
            status: "failed".to_string(),
            completed: 0,
            progress: 100,
            message: "生成失败".to_string(),
            phase: "failed".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Failed),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            selected_candidate_events: vec![],
            quality_gate: None,
            active_story_repair_payload: None,
            candidate_gateway: None,
            terminal_label: None,
        };
        failed.task.error_message = Some("boom".to_string());
        let manual_review = BatchGenerationStreamState {
            task: {
                let mut task = build_task("failed");
                task.error_message =
                    Some("第7章触发质量门禁，需人工复核: 等待人工复核".to_string());
                task
            },
            status: "failed".to_string(),
            completed: 0,
            progress: 100,
            message: "生成失败".to_string(),
            phase: "failed".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Failed),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            selected_candidate_events: vec![],
            quality_gate: Some(json!({
                "decision": "manual_review",
                "label": "等待人工复核",
                "phase": "quality_blocked"
            })),
            active_story_repair_payload: Some(json!({
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "等待人工复核",
                "phase": "quality_blocked"
            })),
            candidate_gateway: None,
            terminal_label: None,
        };
        let cancelled = BatchGenerationStreamState {
            task: build_task("cancelled"),
            status: "cancelled".to_string(),
            completed: 0,
            progress: 100,
            message: "生成已取消".to_string(),
            phase: "cancelled".to_string(),
            event_status: "processing",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Cancelled),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            selected_candidate_events: vec![],
            quality_gate: None,
            active_story_repair_payload: None,
            candidate_gateway: None,
            terminal_label: None,
        };

        let completed_events = completed.terminal_events().expect("completed events");
        assert_eq!(completed_events.len(), 2);
        assert_eq!(completed_events[0]["type"], "result");
        assert_eq!(completed_events[1]["type"], "done");

        let failed_events = failed.terminal_events().expect("failed events");
        assert_eq!(failed_events[0]["error"], "boom");
        assert_eq!(failed_events[0]["phase"], "failed");
        assert_eq!(failed_events[1]["type"], "done");

        let manual_review_events = manual_review
            .terminal_events()
            .expect("manual review events");
        assert_eq!(manual_review_events[0]["phase"], "failed");
        assert_eq!(manual_review_events[0]["code"], 500);
        assert_eq!(manual_review_events[0]["error"], "批量生成任务执行失败");
        assert_eq!(manual_review_events[1]["type"], "done");

        let cancelled_events = cancelled.terminal_events().expect("cancelled events");
        assert_eq!(cancelled_events.len(), 1);
        assert_eq!(cancelled_events[0]["type"], "done");
    }

    #[test]
    fn should_keep_manual_review_quality_gate_progress_payload_as_telemetry() {
        let manual_review_events = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 76,
            message: "正在生成正文...".to_string(),
            phase: "generating".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            selected_candidate_events: vec![],
            quality_gate: Some(json!({
                "decision": "manual_review",
                "label": "等待人工复核",
                "phase": "quality_blocked"
            })),
            active_story_repair_payload: Some(json!({
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "等待人工复核",
                "phase": "quality_blocked"
            })),
            candidate_gateway: None,
            terminal_label: None,
        }
        .events();
        let retry_events = BatchGenerationStreamState {
            task: {
                let mut task = build_task("failed");
                task.current_retry_count = 1;
                task
            },
            status: "failed".to_string(),
            completed: 0,
            progress: 76,
            message: "自动修复后重试".to_string(),
            phase: "repair_pending".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Failed),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            selected_candidate_events: vec![],
            quality_gate: Some(json!({
                "decision": "auto_repair",
                "label": "自动修复后重试",
                "phase": "repair_pending"
            })),
            active_story_repair_payload: Some(json!({
                "quality_gate_decision": "repair",
                "quality_gate_label": "自动修复后重试",
                "phase": "repair_pending"
            })),
            candidate_gateway: None,
            terminal_label: Some("自动修复后重试".to_string()),
        }
        .events();

        assert_eq!(
            manual_review_events[0]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            manual_review_events[0]["active_story_repair_payload"]["phase"],
            "quality_blocked"
        );
        assert_eq!(manual_review_events[0]["phase"], "generating");
        assert_eq!(manual_review_events[0]["status"], "processing");
        assert_eq!(retry_events[0]["current_retry_count"], 1);
        assert_eq!(retry_events[0]["quality_gate"]["decision"], "auto_repair");
        assert_eq!(
            retry_events[0]["active_story_repair_payload"]["phase"],
            "repair_pending"
        );
    }

    #[test]
    fn should_resolve_stream_event_batch_contract_inside_read_context_stream_owner() {
        let running = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 65,
            message: "处理中".to_string(),
            phase: "generating".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            selected_candidate_events: vec![],
            quality_gate: None,
            active_story_repair_payload: None,
            candidate_gateway: None,
            terminal_label: None,
        };

        let continue_batch = super::BatchGenerationStreamCursor { observation: None }
            .resolve_event_batch(&running)
            .expect("continue batch");
        match continue_batch {
            super::BatchGenerationStreamEventResolution::Continue {
                events,
                next_cursor,
            } => {
                assert_eq!(events.len(), 1);
                assert_eq!(events[0]["type"], "progress");
                assert_eq!(next_cursor.observation, Some(running.observation_key()));
            }
            super::BatchGenerationStreamEventResolution::Close { .. } => {
                panic!("expected continue resolution")
            }
        }

        let same_cursor = super::BatchGenerationStreamCursor {
            observation: Some(running.observation_key()),
        };
        assert!(same_cursor.resolve_event_batch(&running).is_none());

        let completed = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            phase: "completed".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            selected_candidate_events: vec![],
            quality_gate: None,
            active_story_repair_payload: None,
            candidate_gateway: None,
            terminal_label: None,
        };
        let close_batch = super::BatchGenerationStreamCursor { observation: None }
            .resolve_event_batch(&completed)
            .expect("close batch");
        match close_batch {
            super::BatchGenerationStreamEventResolution::Close { events } => {
                assert_eq!(events.len(), 4);
                assert_eq!(events[0]["type"], "progress");
                assert_eq!(events[1]["type"], "analysis_started");
                assert_eq!(events[2]["type"], "result");
                assert_eq!(events[3]["type"], "done");
            }
            super::BatchGenerationStreamEventResolution::Continue { .. } => {
                panic!("expected close resolution")
            }
        }
    }

    #[test]
    fn should_emit_stream_event_batch_when_phase_or_analysis_fields_change() {
        let baseline = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 76,
            message: "正在生成正文...".to_string(),
            phase: "generating".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            selected_candidate_events: vec![],
            quality_gate: Some(json!({
                "decision": "manual_review",
                "label": "等待人工复核",
                "phase": "quality_blocked"
            })),
            active_story_repair_payload: Some(json!({
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "等待人工复核",
                "phase": "quality_blocked"
            })),
            candidate_gateway: None,
            terminal_label: None,
        };
        let next_phase_state = BatchGenerationStreamState {
            phase: "repair_pending".to_string(),
            quality_gate: Some(json!({
                "decision": "auto_repair",
                "label": "自动修复后重试",
                "phase": "repair_pending"
            })),
            active_story_repair_payload: Some(json!({
                "quality_gate_decision": "repair",
                "quality_gate_label": "自动修复后重试",
                "phase": "repair_pending"
            })),
            terminal_label: Some("自动修复后重试".to_string()),
            ..baseline.clone()
        };
        let next_analysis_state = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 85,
            message: "正在分析章节".to_string(),
            phase: "parsing".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: Some("analysis-task-9".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            selected_candidate_events: vec![],
            quality_gate: None,
            active_story_repair_payload: None,
            candidate_gateway: None,
            terminal_label: None,
        };

        let phase_batch = super::BatchGenerationStreamCursor {
            observation: Some(baseline.observation_key()),
        }
        .resolve_event_batch(&next_phase_state)
        .expect("phase change batch");
        match phase_batch {
            super::BatchGenerationStreamEventResolution::Continue { events, .. } => {
                assert_eq!(events[0]["phase"], "repair_pending");
                assert_eq!(events[0]["quality_gate"]["decision"], "auto_repair");
            }
            super::BatchGenerationStreamEventResolution::Close { .. } => {
                panic!("expected continue resolution for non-terminal phase change")
            }
        }

        let analysis_baseline = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 85,
            message: "正在分析章节".to_string(),
            phase: "parsing".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            selected_candidate_events: vec![],
            quality_gate: None,
            active_story_repair_payload: None,
            candidate_gateway: None,
            terminal_label: None,
        };
        let analysis_batch = super::BatchGenerationStreamCursor {
            observation: Some(analysis_baseline.observation_key()),
        }
        .resolve_event_batch(&next_analysis_state)
        .expect("analysis change batch");
        match analysis_batch {
            super::BatchGenerationStreamEventResolution::Continue {
                events,
                next_cursor,
            } => {
                assert_eq!(events.len(), 2);
                assert_eq!(events[1]["type"], "analysis_started");
                assert_eq!(events[1]["task_id"], "analysis-task-9");
                assert_eq!(
                    next_cursor.observation,
                    Some(next_analysis_state.observation_key())
                );
            }
            super::BatchGenerationStreamEventResolution::Close { .. } => {
                panic!("expected continue resolution for running analysis state")
            }
        }
    }
}
