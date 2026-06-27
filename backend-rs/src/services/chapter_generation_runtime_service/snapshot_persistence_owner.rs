use std::collections::HashMap;

use chrono::NaiveDateTime;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::Value;
use uuid::Uuid;

use crate::models::batch_generation_snapshot;
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::resolve_generation_quality_runtime_context_from_persisted_sources;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChapterGenerationSnapshotWriteMode {
    MergeRuntimeState,
    ReplaceRuntimeState,
}

pub(crate) fn build_chapter_generation_snapshot_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_generation_runtime_service::snapshot_persistence_owner",
        "scope": "chapter_generation_runtime_snapshot_persistence",
        "python_source_map": [
            "backend/migrator_app/models/batch_generation_snapshot.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs"
        ],
        "behavior_contract": {
            "read_functions": [
                "load_chapter_generation_snapshot",
                "load_chapter_generation_snapshot_map"
            ],
            "write_functions": [
                "persist_chapter_generation_runtime_snapshot",
                "upsert_chapter_generation_runtime_snapshot"
            ],
            "write_modes": [
                "MergeRuntimeState",
                "ReplaceRuntimeState"
            ],
            "quality_fields": [
                "latest_quality_metrics",
                "quality_metrics_history",
                "quality_metrics_summary"
            ],
            "runtime_state_policy": [
                "object_payloads_merge_keywise",
                "non_object_payloads_replace_existing_state",
                "replace_mode_clears_quality_fields_before_runtime_backfill",
                "missing_quality_fields_backfill_from_persisted_runtime_sources"
            ]
        },
        "source_map_closeout_status": {
            "compat_shell_status": "physically_deleted",
            "default_python_module_consumers": [],
            "dedicated_python_regression_surfaces": [],
            "shared_test_support_consumers_removed": true,
            "shared_import_guard_consumers_removed": true,
            "physical_python_closeout_completed": true,
            "shared_schema_hold_status": {
                "batch_generation_snapshot_model": "shared_python_runtime_database_and_api_test_reference",
                "default_python_module_consumers": [
                    "backend/tests/test_support/database_test_support.py",
                    "backend/tests/test_support/task_system/snapshot_runtime_persistence.py"
                ],
                "dedicated_python_regression_surfaces": [
                    "backend/tests/test_api/test_chapters.py",
                    "backend/tests/test_api/test_chapters_batch_status_resume.py"
                ],
                "physical_closeout_ready": false
            }
        },
        "validation_boundary": [
            "cargo test services::chapter_generation_runtime_service::snapshot_persistence_owner",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "rollback_boundary": "batch_generation_snapshot_python_source_map"
    })
}

pub(crate) async fn load_chapter_generation_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<Option<batch_generation_snapshot::Model>, String> {
    batch_generation_snapshot::Entity::find()
        .filter(batch_generation_snapshot::Column::BatchTaskId.eq(task_id))
        .one(db)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn load_chapter_generation_snapshot_map(
    db: &DatabaseConnection,
    task_ids: &[String],
) -> Result<HashMap<String, batch_generation_snapshot::Model>, String> {
    if task_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let snapshots = batch_generation_snapshot::Entity::find()
        .filter(batch_generation_snapshot::Column::BatchTaskId.is_in(task_ids.iter().cloned()))
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    Ok(snapshots
        .into_iter()
        .map(|snapshot| (snapshot.batch_task_id.clone(), snapshot))
        .collect())
}

pub(crate) fn merge_chapter_generation_runtime_state(
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

pub(crate) fn apply_optional_quality_field(
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

fn runtime_state_contains_key(workflow_runtime_state: &Value, key: &str) -> bool {
    workflow_runtime_state
        .as_object()
        .is_some_and(|object| object.contains_key(key))
}

pub(crate) fn backfill_missing_quality_snapshot_fields_from_runtime_state(
    active: &mut batch_generation_snapshot::ActiveModel,
    workflow_runtime_state: &Value,
) {
    let latest_quality_metrics_present =
        runtime_state_contains_key(workflow_runtime_state, "latest_quality_metrics");
    let quality_metrics_history_present =
        runtime_state_contains_key(workflow_runtime_state, "quality_metrics_history");
    let quality_metrics_summary_present =
        runtime_state_contains_key(workflow_runtime_state, "quality_metrics_summary");
    let quality_metrics_summary_state = workflow_runtime_state
        .as_object()
        .and_then(|state| state.get("quality_metrics_summary_state"));

    let current_latest_quality_metrics = active.latest_quality_metrics.clone().take().flatten();
    let current_quality_metrics_history = active.quality_metrics_history.clone().take().flatten();
    let current_quality_metrics_summary = active.quality_metrics_summary.clone().take().flatten();
    let resolved_quality_context =
        resolve_generation_quality_runtime_context_from_persisted_sources(
            "batch",
            current_latest_quality_metrics.as_ref(),
            current_quality_metrics_history.as_ref(),
            quality_metrics_summary_state,
            current_quality_metrics_summary.as_ref(),
        );

    if !latest_quality_metrics_present && current_latest_quality_metrics.is_none() {
        active.latest_quality_metrics = Set(resolved_quality_context.latest_quality_metrics);
    }
    if !quality_metrics_history_present && current_quality_metrics_history.is_none() {
        active.quality_metrics_history = Set(resolved_quality_context.quality_metrics_history);
    }
    if !quality_metrics_summary_present && current_quality_metrics_summary.is_none() {
        active.quality_metrics_summary = Set(resolved_quality_context.quality_metrics_summary);
    }
}

impl ChapterGenerationSnapshotWriteMode {
    pub(crate) fn apply_to_active_model(
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
        backfill_missing_quality_snapshot_fields_from_runtime_state(
            active,
            &workflow_runtime_state,
        );

        active.workflow_runtime_state = Set(Some(match self {
            Self::MergeRuntimeState => merge_chapter_generation_runtime_state(
                active.workflow_runtime_state.clone().take().flatten(),
                workflow_runtime_state,
            ),
            Self::ReplaceRuntimeState => workflow_runtime_state,
        }));
        active.updated_at = Set(Some(now));
    }
}

pub(crate) async fn persist_chapter_generation_runtime_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    workflow_runtime_state: Value,
    write_mode: ChapterGenerationSnapshotWriteMode,
    now: NaiveDateTime,
) -> Result<(), String> {
    let existing = load_chapter_generation_snapshot(db, task_id).await?;

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
            ChapterGenerationSnapshotWriteMode::MergeRuntimeState => {
                merge_chapter_generation_runtime_state(None, workflow_runtime_state)
            }
            ChapterGenerationSnapshotWriteMode::ReplaceRuntimeState => workflow_runtime_state,
        })),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
    };
    active.insert(db).await.map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) async fn upsert_chapter_generation_runtime_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    workflow_runtime_state: Value,
    now: NaiveDateTime,
) -> Result<(), String> {
    persist_chapter_generation_runtime_snapshot(
        db,
        task_id,
        workflow_runtime_state,
        ChapterGenerationSnapshotWriteMode::MergeRuntimeState,
        now,
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
        apply_optional_quality_field, backfill_missing_quality_snapshot_fields_from_runtime_state,
        build_chapter_generation_snapshot_owner_contract, merge_chapter_generation_runtime_state,
        ChapterGenerationSnapshotWriteMode,
    };

    #[test]
    fn should_publish_chapter_generation_snapshot_owner_contract() {
        let contract = build_chapter_generation_snapshot_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_generation_runtime_service::snapshot_persistence_owner"
        );
        assert_eq!(
            contract["scope"],
            "chapter_generation_runtime_snapshot_persistence"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/migrator_app/models/batch_generation_snapshot.py"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .expect("python source map")
                .len(),
            1
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["read_functions"][1],
            "load_chapter_generation_snapshot_map"
        );
        assert_eq!(
            contract["behavior_contract"]["write_functions"][0],
            "persist_chapter_generation_runtime_snapshot"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_state_policy"][0],
            "object_payloads_merge_keywise"
        );
        assert_eq!(
            contract["source_map_closeout_status"]["compat_shell_status"],
            "physically_deleted"
        );
        assert_eq!(
            contract["source_map_closeout_status"]["default_python_module_consumers"],
            json!([])
        );
        assert_eq!(
            contract["source_map_closeout_status"]["dedicated_python_regression_surfaces"],
            json!([])
        );
        assert_eq!(
            contract["source_map_closeout_status"]["shared_test_support_consumers_removed"],
            json!(true)
        );
        assert_eq!(
            contract["source_map_closeout_status"]["shared_import_guard_consumers_removed"],
            json!(true)
        );
        assert_eq!(
            contract["source_map_closeout_status"]["physical_python_closeout_completed"],
            json!(true)
        );
        assert_eq!(
            contract["source_map_closeout_status"]["shared_schema_hold_status"]
                ["batch_generation_snapshot_model"],
            "shared_python_runtime_database_and_api_test_reference"
        );
        assert_eq!(
            contract["source_map_closeout_status"]["shared_schema_hold_status"]
                ["default_python_module_consumers"],
            json!([
                "backend/tests/test_support/database_test_support.py",
                "backend/tests/test_support/task_system/snapshot_runtime_persistence.py"
            ])
        );
        assert_eq!(
            contract["source_map_closeout_status"]["shared_schema_hold_status"]
                ["dedicated_python_regression_surfaces"],
            json!([
                "backend/tests/test_api/test_chapters.py",
                "backend/tests/test_api/test_chapters_batch_status_resume.py"
            ])
        );
        assert_eq!(
            contract["source_map_closeout_status"]["shared_schema_hold_status"]
                ["physical_closeout_ready"],
            json!(false)
        );
        assert_eq!(
            contract["rollback_boundary"],
            "batch_generation_snapshot_python_source_map"
        );
        assert!(!contract["python_source_map"]
            .as_array()
            .expect("python source map")
            .iter()
            .any(|item| item == "backend/app/api/chapters.py"));
    }

    #[test]
    fn should_apply_empty_quality_fields_to_existing_snapshot_active_model() {
        let mut active = batch_generation_snapshot::ActiveModel {
            latest_quality_metrics: Set(Some(json!({"score": 95}))),
            quality_metrics_history: Set(Some(json!([{"score": 95}]))),
            quality_metrics_summary: Set(Some(json!({"avg": 95}))),
            ..Default::default()
        };

        ChapterGenerationSnapshotWriteMode::ReplaceRuntimeState.apply_to_active_model(
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
    fn should_apply_optional_quality_field_when_runtime_state_carries_value() {
        let mut active = batch_generation_snapshot::ActiveModel {
            latest_quality_metrics: Set(None),
            ..Default::default()
        };

        apply_optional_quality_field(
            &mut active.latest_quality_metrics,
            &json!({"latest_quality_metrics": {"score": 95}}),
            "latest_quality_metrics",
        );

        assert_eq!(
            active.latest_quality_metrics,
            Set(Some(json!({"score": 95})))
        );
    }

    #[test]
    fn should_backfill_missing_quality_snapshot_fields_from_runtime_history_only_state() {
        let mut active = batch_generation_snapshot::ActiveModel {
            latest_quality_metrics: Set(None),
            quality_metrics_history: Set(Some(json!([
                {
                    "overall_score": 88,
                    "quality_gate": {
                        "status": "passed",
                        "decision": "continue",
                        "label": "通过"
                    }
                },
                {
                    "overall_score": 84,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议修复"
                    }
                }
            ]))),
            quality_metrics_summary: Set(None),
            ..Default::default()
        };

        backfill_missing_quality_snapshot_fields_from_runtime_state(
            &mut active,
            &json!({
                "quality_metrics_history": [
                    {
                        "overall_score": 88,
                        "quality_gate": {
                            "status": "passed",
                            "decision": "continue",
                            "label": "通过"
                        }
                    },
                    {
                        "overall_score": 84,
                        "quality_gate": {
                            "status": "warning",
                            "decision": "auto_repair",
                            "label": "建议修复"
                        }
                    }
                ]
            }),
        );

        assert_eq!(
            active.latest_quality_metrics,
            Set(Some(json!({
                "overall_score": 84,
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议修复"
                }
            })))
        );
        assert_eq!(
            active.quality_metrics_history,
            Set(Some(json!([
                {
                    "overall_score": 88,
                    "quality_gate": {
                        "status": "passed",
                        "decision": "continue",
                        "label": "通过"
                    }
                },
                {
                    "overall_score": 84,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议修复"
                    }
                }
            ])))
        );
        assert_eq!(
            active
                .quality_metrics_summary
                .clone()
                .take()
                .flatten()
                .and_then(|summary| summary.get("chapter_count").cloned()),
            Some(json!(2))
        );
    }

    #[test]
    fn should_merge_chapter_generation_runtime_state_for_object_payloads() {
        let merged = merge_chapter_generation_runtime_state(
            Some(json!({
                "phase": "pending",
                "progress": 15,
                "checkpoint": {"chapter_id": "chapter-1"}
            })),
            json!({
                "progress": 65,
                "last_event": "progress"
            }),
        );

        assert_eq!(merged["phase"], "pending");
        assert_eq!(merged["progress"], 65);
        assert_eq!(merged["checkpoint"]["chapter_id"], "chapter-1");
        assert_eq!(merged["last_event"], "progress");
    }

    #[test]
    fn should_replace_chapter_generation_runtime_state_when_payload_is_not_object() {
        let merged = merge_chapter_generation_runtime_state(
            Some(json!({"phase": "pending"})),
            json!(["non-object"]),
        );

        assert_eq!(merged, json!(["non-object"]));
    }
}
