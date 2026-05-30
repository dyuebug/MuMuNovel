use chrono::{NaiveDateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::Value;
use uuid::Uuid;

use crate::models::batch_generation_snapshot;
use crate::services::chapter_batch_generation_runtime_checkpoint_service::{
    build_batch_generation_runtime_checkpoint_for_stage, BatchGenerationSnapshotStage,
};

pub(crate) async fn load_batch_generation_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<Option<batch_generation_snapshot::Model>, String> {
    batch_generation_snapshot::Entity::find()
        .filter(batch_generation_snapshot::Column::BatchTaskId.eq(task_id))
        .one(db)
        .await
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchGenerationSnapshotWriteMode {
    MergeRuntimeState,
    ReplaceRuntimeState,
}

fn merge_batch_generation_runtime_state(
    current_workflow_runtime_state: Option<Value>,
    incoming_workflow_runtime_state: Value,
) -> Value {
    match (
        current_workflow_runtime_state,
        incoming_workflow_runtime_state,
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

fn apply_optional_quality_field(
    active_field: &mut sea_orm::ActiveValue<Option<Value>>,
    incoming_workflow_runtime_state: &Value,
    key: &str,
) {
    let Some(object) = incoming_workflow_runtime_state.as_object() else {
        return;
    };
    let Some(value) = object.get(key) else {
        return;
    };

    *active_field = Set(match value {
        Value::Null => None,
        other => Some(other.clone()),
    });
}

impl BatchGenerationSnapshotWriteMode {
    fn apply_to_active_model(
        self,
        active: &mut batch_generation_snapshot::ActiveModel,
        workflow_runtime_state: Value,
        now: NaiveDateTime,
    ) {
        if self == Self::ReplaceRuntimeState {
            active.latest_quality_metrics = Set(None);
            active.quality_metrics_history = Set(None);
            active.quality_metrics_summary = Set(None);
        }

        apply_optional_quality_field(
            &mut active.latest_quality_metrics,
            &workflow_runtime_state,
            "latest_quality_metrics",
        );
        apply_optional_quality_field(
            &mut active.quality_metrics_history,
            &workflow_runtime_state,
            "quality_metrics_history",
        );
        apply_optional_quality_field(
            &mut active.quality_metrics_summary,
            &workflow_runtime_state,
            "quality_metrics_summary",
        );

        active.workflow_runtime_state = Set(Some(match self {
            Self::MergeRuntimeState => merge_batch_generation_runtime_state(
                active.workflow_runtime_state.clone().take().flatten(),
                workflow_runtime_state,
            ),
            Self::ReplaceRuntimeState => workflow_runtime_state,
        }));
        active.updated_at = Set(Some(now));
    }
}

async fn write_batch_generation_runtime_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    workflow_runtime_state: Value,
    write_mode: BatchGenerationSnapshotWriteMode,
) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let existing = load_batch_generation_snapshot(db, task_id).await?;

    if let Some(snapshot) = existing {
        let mut active: batch_generation_snapshot::ActiveModel = snapshot.into();
        write_mode.apply_to_active_model(&mut active, workflow_runtime_state, now);
        active.update(db).await.map_err(|error| error.to_string())?;
        return Ok(());
    }

    let active = batch_generation_snapshot::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        batch_task_id: Set(task_id.to_string()),
        latest_quality_metrics: Set(None),
        quality_metrics_history: Set(None),
        quality_metrics_summary: Set(None),
        workflow_runtime_state: Set(Some(match write_mode {
            BatchGenerationSnapshotWriteMode::MergeRuntimeState => {
                merge_batch_generation_runtime_state(None, workflow_runtime_state)
            }
            BatchGenerationSnapshotWriteMode::ReplaceRuntimeState => workflow_runtime_state,
        })),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
    };
    active.insert(db).await.map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) async fn upsert_batch_generation_runtime_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    workflow_runtime_state: Value,
) -> Result<(), String> {
    write_batch_generation_runtime_snapshot(
        db,
        task_id,
        workflow_runtime_state,
        BatchGenerationSnapshotWriteMode::MergeRuntimeState,
    )
    .await
}

pub(crate) async fn persist_new_batch_generation_task_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    total_chapters: i32,
    runtime_state_seed: Option<Value>,
) -> Result<(), String> {
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
    upsert_batch_generation_runtime_snapshot(db, task_id, runtime_state).await
}

pub(crate) async fn replace_batch_generation_runtime_snapshot_for_resume(
    db: &DatabaseConnection,
    task_id: &str,
    workflow_runtime_state: Value,
) -> Result<(), String> {
    let preserved_runtime_state = merge_batch_generation_runtime_state(
        load_batch_generation_snapshot(db, task_id)
            .await?
            .and_then(|snapshot| snapshot.workflow_runtime_state),
        workflow_runtime_state,
    );
    write_batch_generation_runtime_snapshot(
        db,
        task_id,
        preserved_runtime_state,
        BatchGenerationSnapshotWriteMode::ReplaceRuntimeState,
    )
    .await
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::Set;
    use serde_json::json;

    use crate::models::batch_generation_snapshot;

    use super::{
        apply_optional_quality_field, merge_batch_generation_runtime_state,
        BatchGenerationSnapshotWriteMode,
    };

    #[test]
    fn should_apply_empty_quality_fields_to_existing_snapshot_active_model() {
        let mut active = batch_generation_snapshot::ActiveModel {
            latest_quality_metrics: Set(Some(json!({"score": 95}))),
            quality_metrics_history: Set(Some(json!([{"score": 95}]))),
            quality_metrics_summary: Set(Some(json!({"avg": 95}))),
            ..Default::default()
        };

        BatchGenerationSnapshotWriteMode::ReplaceRuntimeState.apply_to_active_model(
            &mut active,
            json!({"phase": "pending", "status": "pending"}),
            NaiveDate::from_ymd_opt(2026, 5, 20)
                .expect("valid date")
                .and_hms_opt(22, 0, 0)
                .expect("valid time"),
        );

        assert_eq!(active.latest_quality_metrics, Set(None));
        assert_eq!(active.quality_metrics_history, Set(None));
        assert_eq!(active.quality_metrics_summary, Set(None));
    }

    #[test]
    fn should_keep_new_snapshot_active_model_contract_with_empty_quality_fields() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 20)
            .expect("valid date")
            .and_hms_opt(22, 30, 0)
            .expect("valid time");
        let workflow_runtime_state = json!({"phase": "pending", "status": "pending"});

        let active = batch_generation_snapshot::ActiveModel {
            id: Set("snapshot-1".to_string()),
            batch_task_id: Set("task-1".to_string()),
            latest_quality_metrics: Set(None),
            quality_metrics_history: Set(None),
            quality_metrics_summary: Set(None),
            workflow_runtime_state: Set(Some(workflow_runtime_state.clone())),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        };

        assert_eq!(active.batch_task_id, Set("task-1".to_string()));
        assert_eq!(active.latest_quality_metrics, Set(None));
        assert_eq!(active.quality_metrics_history, Set(None));
        assert_eq!(active.quality_metrics_summary, Set(None));
        assert_eq!(
            active.workflow_runtime_state,
            Set(Some(workflow_runtime_state))
        );
        assert_eq!(active.created_at, Set(Some(now)));
        assert_eq!(active.updated_at, Set(Some(now)));
    }

    #[test]
    fn should_build_new_batch_generation_task_runtime_snapshot_for_queue() {
        let snapshot =
            crate::services::chapter_batch_generation_runtime_checkpoint_service::build_batch_generation_runtime_checkpoint_for_stage(
                crate::services::chapter_batch_generation_runtime_checkpoint_service::BatchGenerationSnapshotStage::Queued,
                None,
                None,
                0,
                4,
            );

        assert_eq!(snapshot["phase"], "pending");
        assert_eq!(snapshot["progress"], 0);
        assert_eq!(snapshot["status"], "pending");
        assert_eq!(snapshot["last_event"], "queued");
        assert_eq!(snapshot["last_message"], "批量生成任务已创建，等待开始...");
        assert_eq!(snapshot["completed"], 0);
        assert_eq!(snapshot["total"], 4);
    }

    #[test]
    fn should_merge_runtime_state_for_merge_write_mode() {
        let resolved = merge_batch_generation_runtime_state(
            Some(json!({"phase": "generating", "progress": 45})),
            json!({"progress": 60, "last_event": "progress"}),
        );

        assert_eq!(resolved["phase"], "generating");
        assert_eq!(resolved["progress"], 60);
        assert_eq!(resolved["last_event"], "progress");
    }

    #[test]
    fn should_keep_incoming_runtime_state_for_replace_write_mode() {
        let resolved = json!({"phase": "pending"});

        assert_eq!(resolved, json!({"phase": "pending"}));
    }

    #[test]
    fn should_apply_replace_write_mode_and_clear_quality_fields() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 20)
            .expect("valid date")
            .and_hms_opt(22, 45, 0)
            .expect("valid time");
        let mut active = batch_generation_snapshot::ActiveModel {
            latest_quality_metrics: Set(Some(json!({"score": 95}))),
            quality_metrics_history: Set(Some(json!([{"score": 95}]))),
            quality_metrics_summary: Set(Some(json!({"avg": 95}))),
            workflow_runtime_state: Set(Some(json!({"phase": "generating"}))),
            ..Default::default()
        };

        BatchGenerationSnapshotWriteMode::ReplaceRuntimeState.apply_to_active_model(
            &mut active,
            json!({"phase": "pending"}),
            now,
        );

        assert_eq!(active.latest_quality_metrics, Set(None));
        assert_eq!(active.quality_metrics_history, Set(None));
        assert_eq!(active.quality_metrics_summary, Set(None));
        assert_eq!(
            active.workflow_runtime_state,
            Set(Some(json!({"phase": "pending"})))
        );
        assert_eq!(active.updated_at, Set(Some(now)));
    }

    #[test]
    fn should_apply_merge_write_mode_without_clearing_quality_fields() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 20)
            .expect("valid date")
            .and_hms_opt(22, 46, 0)
            .expect("valid time");
        let mut active = batch_generation_snapshot::ActiveModel {
            latest_quality_metrics: Set(Some(json!({"score": 95}))),
            quality_metrics_history: Set(Some(json!([{"score": 95}]))),
            quality_metrics_summary: Set(Some(json!({"avg": 95}))),
            workflow_runtime_state: Set(Some(json!({"phase": "generating", "progress": 45}))),
            ..Default::default()
        };

        BatchGenerationSnapshotWriteMode::MergeRuntimeState.apply_to_active_model(
            &mut active,
            json!({"progress": 60}),
            now,
        );

        assert_eq!(
            active.latest_quality_metrics,
            Set(Some(json!({"score": 95})))
        );
        assert_eq!(
            active.quality_metrics_history,
            Set(Some(json!([{"score": 95}])))
        );
        assert_eq!(
            active.quality_metrics_summary,
            Set(Some(json!({"avg": 95})))
        );
        assert_eq!(
            active.workflow_runtime_state,
            Set(Some(json!({"phase": "generating", "progress": 60})))
        );
        assert_eq!(active.updated_at, Set(Some(now)));
    }

    #[test]
    fn should_sync_quality_columns_from_merge_runtime_state_payload() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 20)
            .expect("valid date")
            .and_hms_opt(22, 47, 0)
            .expect("valid time");
        let mut active = batch_generation_snapshot::ActiveModel {
            latest_quality_metrics: Set(Some(json!({"score": 95}))),
            quality_metrics_summary: Set(Some(json!({
                "quality_gate": {"decision": "auto_repair"}
            }))),
            workflow_runtime_state: Set(Some(json!({"phase": "repair_pending"}))),
            ..Default::default()
        };

        BatchGenerationSnapshotWriteMode::MergeRuntimeState.apply_to_active_model(
            &mut active,
            json!({
                "quality_metrics_summary": {
                    "quality_gate": {"decision": "manual_review"}
                },
                "latest_quality_metrics": {
                    "quality_gate": {"decision": "manual_review"}
                }
            }),
            now,
        );

        assert_eq!(
            active.quality_metrics_summary,
            Set(Some(json!({
                "quality_gate": {"decision": "manual_review"}
            })))
        );
        assert_eq!(
            active.latest_quality_metrics,
            Set(Some(json!({
                "quality_gate": {"decision": "manual_review"}
            })))
        );
    }

    #[test]
    fn should_sync_quality_columns_from_replace_runtime_state_payload() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 20)
            .expect("valid date")
            .and_hms_opt(22, 48, 0)
            .expect("valid time");
        let mut active = batch_generation_snapshot::ActiveModel {
            latest_quality_metrics: Set(Some(json!({"score": 95}))),
            quality_metrics_history: Set(Some(json!([{"score": 95}]))),
            quality_metrics_summary: Set(Some(json!({"avg": 95}))),
            workflow_runtime_state: Set(Some(json!({"phase": "generating"}))),
            ..Default::default()
        };

        BatchGenerationSnapshotWriteMode::ReplaceRuntimeState.apply_to_active_model(
            &mut active,
            json!({
                "phase": "pending",
                "quality_metrics_summary": {
                    "quality_gate": {"decision": "manual_review"}
                }
            }),
            now,
        );

        assert_eq!(active.latest_quality_metrics, Set(None));
        assert_eq!(active.quality_metrics_history, Set(None));
        assert_eq!(
            active.quality_metrics_summary,
            Set(Some(json!({
                "quality_gate": {"decision": "manual_review"}
            })))
        );
    }

    #[test]
    fn should_apply_optional_quality_field_from_runtime_state() {
        let mut field = Set(Some(json!({"score": 91})));
        apply_optional_quality_field(
            &mut field,
            &json!({"latest_quality_metrics": null}),
            "latest_quality_metrics",
        );

        assert_eq!(field, Set(None));
    }

    #[test]
    fn should_merge_object_runtime_state_updates_into_existing_snapshot_state() {
        let merged = merge_batch_generation_runtime_state(
            Some(json!({
                "phase": "generating",
                "progress": 45,
                "checkpoint": {"completed": 1, "total": 3}
            })),
            json!({
                "progress": 60,
                "last_event": "progress"
            }),
        );

        assert_eq!(merged["phase"], "generating");
        assert_eq!(merged["progress"], 60);
        assert_eq!(merged["checkpoint"]["completed"], 1);
        assert_eq!(merged["checkpoint"]["total"], 3);
        assert_eq!(merged["last_event"], "progress");
    }

    #[test]
    fn should_replace_runtime_state_when_existing_snapshot_state_is_not_object() {
        let merged = merge_batch_generation_runtime_state(
            Some(json!(["stale-array-state"])),
            json!({"phase": "pending"}),
        );

        assert_eq!(merged, json!({"phase": "pending"}));
    }

    #[test]
    fn should_replace_runtime_state_when_incoming_snapshot_state_is_not_object() {
        let merged = merge_batch_generation_runtime_state(
            Some(json!({"phase": "generating", "progress": 45})),
            json!(["terminal-array-state"]),
        );

        assert_eq!(merged, json!(["terminal-array-state"]));
    }
}
