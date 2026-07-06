use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, Utc};
use sea_orm::{ActiveModelTrait, DatabaseConnection};
use serde_json::{json, Value};

use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::load_chapter_generation_snapshot;

const RUNNING_TASK_AUTO_RECOVERY_TIMEOUT_MINUTES: i64 = 15;
const PENDING_TASK_AUTO_RECOVERY_TIMEOUT_MINUTES: i64 = 3;

pub(crate) fn build_batch_generation_task_recovery_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_read_context_service::task_recovery_owner",
        "scope": "task_recovery_owner",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service/task_recovery_owner.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "resolve_generation_task_auto_recovery_error",
                "resolve_generation_task_auto_recovery_error_with_snapshot",
                "recover_generation_task_if_needed_with_snapshot",
                "recover_generation_task_if_needed"
            ],
            "running_timeout_minutes": RUNNING_TASK_AUTO_RECOVERY_TIMEOUT_MINUTES,
            "pending_timeout_minutes": PENDING_TASK_AUTO_RECOVERY_TIMEOUT_MINUTES,
            "mutated_task_fields": [
                "status=failed",
                "error_message",
                "completed_at"
            ],
            "running_timeout_basis": "latest snapshot/runtime heartbeat, falling back to task.started_at",
            "error_messages": [
                "任务超时（超过15分钟未完成，已自动恢复）",
                "任务启动超时（超过3分钟未启动，已自动恢复）"
            ]
        },
        "validation_boundary": [
            "cargo test chapter_batch_generation_read_context_service",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 11,
            "python_fallback_probe_count": 0,
            "recovery_error_owner": "resolve_generation_task_auto_recovery_error",
            "recovery_mutation_owner": "recover_generation_task_if_needed",
            "running_timeout_minutes": RUNNING_TASK_AUTO_RECOVERY_TIMEOUT_MINUTES,
            "pending_timeout_minutes": PENDING_TASK_AUTO_RECOVERY_TIMEOUT_MINUTES,
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "batch-generation task-recovery source-map package deleted; surviving Python closeout work is now limited to shared batch-generation-task schema/runtime/API/test-support packages",
            "status": "rust_batch_generation_task_recovery_owner_source_map_deleted"
        },
        "shared_schema_hold_status": {
            "batch_generation_task_model": "shared_python_runtime_api_and_test_support_reference",
            "default_python_module_consumers": [
                "backend/tests/test_support/database_test_support.py",
                "backend/tests/test_support/task_system/snapshot_runtime_persistence.py"
            ],
            "dedicated_python_regression_surfaces": [
                "backend/tests/test_api/test_chapters.py",
                "backend/tests/test_api/test_chapters_batch_generation.py",
                "backend/tests/test_api/test_chapters_batch_status_resume.py"
            ],
            "test_support_consumers": [
                "backend/tests/test_support/batch_generation_status_read_owner_test_adapter.py",
                "backend/tests/test_support/batch_generation_orchestration_test_adapter.py",
                "backend/tests/test_support/batch_generation_route_test_adapter.py"
            ],
            "physical_closeout_ready": false
        },
        "rollback_boundary": "batch_generation_package_query_source_map"
    })
}

#[allow(dead_code)]
pub(crate) fn resolve_generation_task_auto_recovery_error(
    task: &batch_generation_task::Model,
    now: NaiveDateTime,
) -> Option<String> {
    resolve_generation_task_auto_recovery_error_with_snapshot(task, None, now)
}

pub(crate) fn resolve_generation_task_auto_recovery_error_with_snapshot(
    task: &batch_generation_task::Model,
    snapshot: Option<&batch_generation_snapshot::Model>,
    now: NaiveDateTime,
) -> Option<String> {
    if task.status == "running" {
        if let Some(last_activity_at) = resolve_running_task_last_activity_at(task, snapshot) {
            if now - last_activity_at
                > ChronoDuration::minutes(RUNNING_TASK_AUTO_RECOVERY_TIMEOUT_MINUTES)
            {
                return Some("任务超时（超过15分钟未完成，已自动恢复）".to_string());
            }
        }
    } else if task.status == "pending" {
        if let Some(created_at) = task.created_at {
            if now - created_at
                > ChronoDuration::minutes(PENDING_TASK_AUTO_RECOVERY_TIMEOUT_MINUTES)
            {
                return Some("任务启动超时（超过3分钟未启动，已自动恢复）".to_string());
            }
        }
    }

    None
}

fn resolve_running_task_last_activity_at(
    task: &batch_generation_task::Model,
    snapshot: Option<&batch_generation_snapshot::Model>,
) -> Option<NaiveDateTime> {
    [
        snapshot.and_then(|item| item.updated_at),
        snapshot
            .and_then(|item| item.workflow_runtime_state.as_ref())
            .and_then(runtime_state_updated_at),
        task.started_at,
    ]
    .into_iter()
    .flatten()
    .max()
}

fn runtime_state_updated_at(value: &Value) -> Option<NaiveDateTime> {
    value
        .get("updated_at")
        .and_then(Value::as_str)
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.naive_utc())
}

pub(crate) async fn recover_generation_task_if_needed_with_snapshot(
    db: &DatabaseConnection,
    task: batch_generation_task::Model,
    snapshot: Option<&batch_generation_snapshot::Model>,
) -> Result<(batch_generation_task::Model, bool), String> {
    let now = Utc::now().naive_utc();
    let Some(error_message) =
        resolve_generation_task_auto_recovery_error_with_snapshot(&task, snapshot, now)
    else {
        return Ok((task, false));
    };

    let mut active: batch_generation_task::ActiveModel = task.clone().into();
    active.status = sea_orm::Set("failed".to_string());
    active.error_message = sea_orm::Set(Some(error_message));
    active.completed_at = sea_orm::Set(Some(now));

    active
        .update(db)
        .await
        .map(|updated| (updated, true))
        .map_err(|error| error.to_string())
}

pub(crate) async fn recover_generation_task_if_needed(
    db: &DatabaseConnection,
    task: batch_generation_task::Model,
) -> Result<(batch_generation_task::Model, bool), String> {
    let snapshot = load_chapter_generation_snapshot(db, &task.id).await?;
    recover_generation_task_if_needed_with_snapshot(db, task, snapshot.as_ref()).await
}
