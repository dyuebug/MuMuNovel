use chrono::{Duration as ChronoDuration, NaiveDateTime, Utc};
use sea_orm::{ActiveModelTrait, DatabaseConnection};
use serde_json::{json, Value};

use crate::models::batch_generation_task;

const RUNNING_TASK_AUTO_RECOVERY_TIMEOUT_MINUTES: i64 = 15;
const PENDING_TASK_AUTO_RECOVERY_TIMEOUT_MINUTES: i64 = 3;

pub(crate) fn build_batch_generation_task_recovery_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_read_context_service::task_recovery_owner",
        "scope": "task_recovery_owner",
        "python_source_map": [
            "backend/app/services/batch_generation/query_service.py",
            "backend/app/services/batch_generation_orchestration_service.py",
            "backend/app/models/batch_generation_task.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service/task_recovery_owner.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "resolve_generation_task_auto_recovery_error",
                "recover_generation_task_if_needed"
            ],
            "running_timeout_minutes": RUNNING_TASK_AUTO_RECOVERY_TIMEOUT_MINUTES,
            "pending_timeout_minutes": PENDING_TASK_AUTO_RECOVERY_TIMEOUT_MINUTES,
            "mutated_task_fields": [
                "status=failed",
                "error_message",
                "completed_at"
            ],
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
            "physical_python_closeout_completed": false,
            "remaining_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
            "status": "rust_batch_generation_task_recovery_owner_ready_for_source_map_closeout_review"
        },
        "rollback_boundary": "batch_generation_package_query_source_map"
    })
}

pub(crate) fn resolve_generation_task_auto_recovery_error(
    task: &batch_generation_task::Model,
    now: NaiveDateTime,
) -> Option<String> {
    if task.status == "running" {
        if let Some(started_at) = task.started_at {
            if now - started_at
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

pub(crate) async fn recover_generation_task_if_needed(
    db: &DatabaseConnection,
    task: batch_generation_task::Model,
) -> Result<(batch_generation_task::Model, bool), String> {
    let now = Utc::now().naive_utc();
    let Some(error_message) = resolve_generation_task_auto_recovery_error(&task, now) else {
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
