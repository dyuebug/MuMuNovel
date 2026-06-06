use crate::services::chapter_batch_generation_quality_runtime_context_service::{
    resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state,
    BatchGenerationQualityRuntimeContext,
};
use crate::services::chapter_batch_generation_runtime_checkpoint_service::{
    build_batch_generation_runtime_checkpoint_for_stage, BatchGenerationSnapshotStage,
};
use crate::services::chapter_generation_request_runtime_state_service::active_story_repair_payload_from_runtime_state;
use crate::services::chapter_generation_snapshot_persistence_service::{
    merge_chapter_generation_runtime_state, persist_chapter_generation_runtime_snapshot,
    upsert_chapter_generation_runtime_snapshot, ChapterGenerationSnapshotWriteMode,
};
use crate::services::chapter_generation_snapshot_query_service::load_chapter_generation_snapshot;
use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationQueuedSnapshotPlan {
    runtime_state: Value,
    quality_runtime_context: BatchGenerationQualityRuntimeContext,
    active_story_repair_payload: Option<Value>,
    quality_history_context: Option<Value>,
}

impl BatchGenerationQueuedSnapshotPlan {
    pub(crate) fn from_runtime_state_seed(
        total_chapters: i32,
        runtime_state_seed: Option<Value>,
    ) -> Self {
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

        let quality_runtime_context =
            resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state(
                None,
                Some(&runtime_state),
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

    pub(crate) fn quality_runtime_context(&self) -> BatchGenerationQualityRuntimeContext {
        self.quality_runtime_context.clone()
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
        upsert_batch_generation_runtime_snapshot(db, task_id, self.runtime_state).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationResumeSnapshotPlan {
    runtime_state: Value,
}

impl BatchGenerationResumeSnapshotPlan {
    pub(crate) fn from_resume_checkpoint(
        existing_workflow_runtime_state: Option<Value>,
        resume_checkpoint: Value,
    ) -> Self {
        Self {
            runtime_state: merge_batch_generation_runtime_state(
                existing_workflow_runtime_state,
                resume_checkpoint,
            ),
        }
    }

    #[cfg(test)]
    pub(crate) fn runtime_state(&self) -> &Value {
        &self.runtime_state
    }

    pub(crate) async fn persist_replace(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> Result<(), String> {
        persist_chapter_generation_runtime_snapshot(
            db,
            task_id,
            self.runtime_state,
            ChapterGenerationSnapshotWriteMode::ReplaceRuntimeState,
            Utc::now().naive_utc(),
        )
        .await
    }
}

fn merge_batch_generation_runtime_state(
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
    db: &DatabaseConnection,
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

pub(crate) async fn persist_new_batch_generation_task_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    total_chapters: i32,
    runtime_state_seed: Option<Value>,
) -> Result<(), String> {
    BatchGenerationQueuedSnapshotPlan::from_runtime_state_seed(total_chapters, runtime_state_seed)
        .persist(db, task_id)
        .await
}

pub(crate) async fn replace_batch_generation_runtime_snapshot_for_resume(
    db: &DatabaseConnection,
    task_id: &str,
    workflow_runtime_state: Value,
) -> Result<(), String> {
    BatchGenerationResumeSnapshotPlan::from_resume_checkpoint(
        load_chapter_generation_snapshot(db, task_id)
            .await?
            .and_then(|snapshot| snapshot.workflow_runtime_state),
        workflow_runtime_state,
    )
    .persist_replace(db, task_id)
    .await
}

#[cfg(test)]
mod tests {
    use crate::models::batch_generation_snapshot;
    use crate::services::chapter_generation_snapshot_persistence_service::{
        apply_optional_quality_field, backfill_missing_quality_snapshot_fields_from_runtime_state,
        ChapterGenerationSnapshotWriteMode,
    };
    use chrono::NaiveDate;
    use sea_orm::Set;
    use serde_json::json;

    use super::{
        merge_batch_generation_runtime_state, BatchGenerationQueuedSnapshotPlan,
        BatchGenerationResumeSnapshotPlan,
    };

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
    fn should_build_batch_generation_queued_snapshot_plan_from_runtime_seed() {
        let plan = BatchGenerationQueuedSnapshotPlan::from_runtime_state_seed(
            4,
            Some(json!({
                "quality_metrics_summary": {"chapter_count": 2},
                "active_story_repair_payload": {"summary": "沿用修复建议"}
            })),
        );

        assert_eq!(plan.runtime_state()["phase"], "pending");
        assert_eq!(plan.runtime_state()["last_event"], "queued");
        assert_eq!(plan.runtime_state()["total"], 4);
        assert_eq!(
            plan.runtime_state()["quality_metrics_summary"]["chapter_count"],
            2
        );
        assert_eq!(
            plan.runtime_state()["active_story_repair_payload"]["summary"],
            "沿用修复建议"
        );
    }

    #[test]
    fn should_expose_response_ready_quality_contract_from_batch_generation_queued_snapshot_plan() {
        let plan = BatchGenerationQueuedSnapshotPlan::from_runtime_state_seed(
            2,
            Some(json!({
                "quality_metrics_summary": {
                    "chapter_count": 2,
                    "overall_score": 86.0,
                    "quality_runtime_context": {
                        "recent_metrics": [
                            {"overall_score": 86}
                        ],
                        "history_scope": "batch"
                    }
                },
                "quality_metrics_summary_state": {
                    "scope": "batch",
                    "chapter_count": 2,
                    "first_overall_score": 82.0,
                    "last_overall_score": 86.0
                },
                "quality_metrics_history": [
                    {"overall_score": 82},
                    {"overall_score": 86}
                ],
                "latest_quality_metrics": {
                    "overall_score": 86,
                    "quality_gate": {
                        "decision": "repair"
                    }
                },
                "quality_history_context": {
                    "scope": "batch",
                    "source": "queued_snapshot_test"
                },
                "active_story_repair_payload": {
                    "summary": "沿用批量修复建议",
                    "repair_targets": ["压缩说明"],
                    "source": "recent_history_summary",
                    "scope": "batch"
                }
            })),
        );

        let quality_runtime_context = plan.quality_runtime_context();

        assert_eq!(
            quality_runtime_context
                .quality_metrics_summary
                .as_ref()
                .and_then(|summary| summary.get("chapter_count")),
            Some(&json!(2))
        );
        assert_eq!(
            quality_runtime_context
                .latest_quality_metrics
                .as_ref()
                .and_then(|metrics| metrics.get("overall_score")),
            Some(&json!(86))
        );
        assert_eq!(
            plan.quality_history_context()
                .as_ref()
                .and_then(|context| context.get("source")),
            Some(&json!("queued_snapshot_test"))
        );
        assert_eq!(
            plan.active_story_repair_payload()
                .as_ref()
                .and_then(|payload| payload.get("summary")),
            Some(&json!("沿用批量修复建议"))
        );
    }

    #[test]
    fn should_build_batch_generation_resume_snapshot_plan_from_existing_runtime_state() {
        let plan = BatchGenerationResumeSnapshotPlan::from_resume_checkpoint(
            Some(json!({
                "phase": "failed",
                "last_event": "error",
                "quality_metrics_history": [{"overall_score": 79}]
            })),
            json!({
                "phase": "pending",
                "last_event": "resume",
                "current_chapter_id": "chapter-2"
            }),
        );

        assert_eq!(plan.runtime_state()["phase"], "pending");
        assert_eq!(plan.runtime_state()["last_event"], "resume");
        assert_eq!(plan.runtime_state()["current_chapter_id"], "chapter-2");
        assert_eq!(
            plan.runtime_state()["quality_metrics_history"][0]["overall_score"],
            79
        );
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

        ChapterGenerationSnapshotWriteMode::ReplaceRuntimeState.apply_to_active_model(
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

        ChapterGenerationSnapshotWriteMode::MergeRuntimeState.apply_to_active_model(
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

        ChapterGenerationSnapshotWriteMode::MergeRuntimeState.apply_to_active_model(
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

        ChapterGenerationSnapshotWriteMode::ReplaceRuntimeState.apply_to_active_model(
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
    fn should_backfill_missing_quality_snapshot_fields_from_history_only_runtime_state() {
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
    fn should_sync_and_backfill_quality_columns_from_replace_runtime_state_history_only_payload() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 20)
            .expect("valid date")
            .and_hms_opt(22, 49, 0)
            .expect("valid time");
        let mut active = batch_generation_snapshot::ActiveModel {
            latest_quality_metrics: Set(Some(json!({"score": 95}))),
            quality_metrics_history: Set(Some(json!([{"score": 95}]))),
            quality_metrics_summary: Set(Some(json!({"avg": 95}))),
            workflow_runtime_state: Set(Some(json!({"phase": "generating"}))),
            ..Default::default()
        };

        ChapterGenerationSnapshotWriteMode::ReplaceRuntimeState.apply_to_active_model(
            &mut active,
            json!({
                "phase": "pending",
                "quality_metrics_history": [
                    {
                        "overall_score": 90,
                        "quality_gate": {
                            "status": "passed",
                            "decision": "continue",
                            "label": "通过"
                        }
                    },
                    {
                        "overall_score": 82,
                        "quality_gate": {
                            "status": "warning",
                            "decision": "auto_repair",
                            "label": "建议修复"
                        }
                    }
                ]
            }),
            now,
        );

        assert_eq!(
            active.quality_metrics_history,
            Set(Some(json!([
                {
                    "overall_score": 90,
                    "quality_gate": {
                        "status": "passed",
                        "decision": "continue",
                        "label": "通过"
                    }
                },
                {
                    "overall_score": 82,
                    "quality_gate": {
                        "status": "warning",
                        "decision": "auto_repair",
                        "label": "建议修复"
                    }
                }
            ])))
        );
        assert_eq!(
            active.latest_quality_metrics,
            Set(Some(json!({
                "overall_score": 82,
                "quality_gate": {
                    "status": "warning",
                    "decision": "auto_repair",
                    "label": "建议修复"
                }
            })))
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
