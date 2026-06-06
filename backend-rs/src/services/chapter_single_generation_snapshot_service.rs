use chrono::Utc;
use serde_json::Value;

use crate::services::chapter_generation_quality_runtime_context_service::{
    resolve_generation_quality_runtime_context_from_persisted_sources,
    GenerationQualityRuntimeContext,
};
use crate::services::chapter_generation_request_runtime_state_service::active_story_repair_payload_from_runtime_state;
use crate::services::chapter_generation_snapshot_persistence_service::upsert_chapter_generation_runtime_snapshot;

pub(crate) fn merge_single_generation_runtime_state(
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

pub(crate) async fn upsert_single_generation_runtime_snapshot(
    db: &sea_orm::DatabaseConnection,
    task_id: &str,
    runtime_state: Value,
) -> Result<(), String> {
    upsert_chapter_generation_runtime_snapshot(db, task_id, runtime_state, Utc::now().naive_utc())
        .await
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

    pub(crate) fn quality_metrics_summary_state(&self) -> Option<&Value> {
        self.quality_runtime_context
            .quality_metrics_summary_state
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
        db: &sea_orm::DatabaseConnection,
        task_id: &str,
    ) -> Result<(), String> {
        upsert_single_generation_runtime_snapshot(db, task_id, self.runtime_state).await
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{merge_single_generation_runtime_state, SingleGenerationStartupSnapshotPlan};

    #[test]
    fn should_merge_single_generation_runtime_state_for_object_payloads() {
        let merged = merge_single_generation_runtime_state(
            Some(&json!({
                "phase": "pending",
                "progress": 15,
                "checkpoint": {"chapter_id": "chapter-1"}
            })),
            &json!({
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
    fn should_replace_single_generation_runtime_state_when_payload_is_not_object() {
        let merged = merge_single_generation_runtime_state(
            Some(&json!({"phase": "pending"})),
            &json!(["non-object"]),
        );

        assert_eq!(merged, json!(["non-object"]));
    }

    #[test]
    fn should_build_single_generation_startup_snapshot_plan_from_pending_checkpoint() {
        let plan = SingleGenerationStartupSnapshotPlan::from_pending_checkpoint(
            json!({
                "phase": "pending",
                "status": "pending",
                "chapter_id": "chapter-7",
                "current_chapter_id": "chapter-7"
            }),
            json!({
                "quality_metrics_summary": {"chapter_count": 1},
                "active_story_repair_payload": {"summary": "沿用修复建议"}
            }),
        );

        assert_eq!(plan.runtime_state()["phase"], "pending");
        assert_eq!(plan.runtime_state()["chapter_id"], "chapter-7");
        assert_eq!(
            plan.runtime_state()["quality_metrics_summary"]["chapter_count"],
            1
        );
        assert_eq!(
            plan.runtime_state()["active_story_repair_payload"]["summary"],
            "沿用修复建议"
        );
    }

    #[test]
    fn should_expose_response_ready_quality_contract_from_single_generation_startup_snapshot_plan()
    {
        let plan = SingleGenerationStartupSnapshotPlan::from_pending_checkpoint(
            json!({
                "phase": "pending",
                "status": "pending",
                "chapter_id": "chapter-7",
                "current_chapter_id": "chapter-7"
            }),
            json!({
                "quality_metrics_summary": {
                    "scope": "chapter",
                    "chapter_count": 2
                },
                "quality_metrics_summary_state": {
                    "scope": "chapter",
                    "chapter_count": 2
                },
                "quality_metrics_history": [
                    {"overall_score": 84},
                    {"overall_score": 91}
                ],
                "latest_quality_metrics": {
                    "overall_score": 91,
                    "quality_gate": {
                        "decision": "pass"
                    }
                },
                "quality_history_context": {
                    "scope": "chapter",
                    "history_scope": "chapter",
                    "recent_metrics": [{"overall_score": 91}]
                },
                "active_story_repair_payload": {
                    "summary": "沿用单章修复建议",
                    "repair_targets": ["压缩说明"],
                    "scope": "chapter"
                }
            }),
        );

        let quality_runtime_context = plan.quality_runtime_context();

        assert_eq!(
            quality_runtime_context
                .latest_quality_metrics
                .as_ref()
                .and_then(|metrics| metrics.get("overall_score")),
            Some(&json!(91))
        );
        assert_eq!(
            quality_runtime_context
                .quality_metrics_history
                .as_ref()
                .and_then(serde_json::Value::as_array)
                .map(|history| history.len()),
            Some(2)
        );
        assert_eq!(
            quality_runtime_context
                .quality_metrics_summary
                .as_ref()
                .and_then(|summary| summary.get("chapter_count")),
            Some(&json!(2))
        );
        assert_eq!(
            plan.quality_history_context()
                .as_ref()
                .and_then(|context| context.get("history_scope")),
            Some(&json!("chapter"))
        );
        assert_eq!(
            plan.active_story_repair_payload()
                .as_ref()
                .and_then(|payload| payload.get("summary")),
            Some(&json!("沿用单章修复建议"))
        );
    }
}
