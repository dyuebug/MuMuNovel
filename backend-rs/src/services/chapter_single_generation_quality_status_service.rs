use serde_json::Value;

use crate::models::batch_generation_snapshot;
use crate::services::chapter_generation_quality_runtime_context_service::{
    resolve_generation_quality_runtime_context_from_persisted_sources,
    GenerationQualityRuntimeContext,
};
use crate::services::chapter_generation_request_runtime_state_service::active_story_repair_payload_from_runtime_state;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SingleGenerationQualityStatusContext {
    pub(crate) latest_quality_metrics: Option<Value>,
    pub(crate) quality_metrics_history: Option<Value>,
    pub(crate) quality_metrics_summary_state: Option<Value>,
    pub(crate) quality_metrics_summary: Option<Value>,
    pub(crate) quality_history_context: Option<Value>,
    pub(crate) active_story_repair_payload: Option<Value>,
}

impl SingleGenerationQualityStatusContext {
    pub(crate) fn from_snapshot_and_runtime_state(
        snapshot: Option<&batch_generation_snapshot::Model>,
        workflow_runtime_state: Option<&Value>,
    ) -> Self {
        let active_story_repair_payload =
            active_story_repair_payload_from_runtime_state(workflow_runtime_state);
        let quality_runtime_context =
            resolve_generation_quality_runtime_context_from_persisted_sources(
                "chapter",
                snapshot.and_then(|item| item.latest_quality_metrics.as_ref()),
                snapshot.and_then(|item| item.quality_metrics_history.as_ref()),
                workflow_runtime_state
                    .and_then(Value::as_object)
                    .and_then(|state| state.get("quality_metrics_summary_state")),
                snapshot.and_then(|item| item.quality_metrics_summary.as_ref()),
            );

        Self::from_runtime_quality_context_and_active_payload(
            &quality_runtime_context,
            active_story_repair_payload.as_ref(),
        )
    }

    pub(crate) fn insert_into_payload(&self, payload: &mut serde_json::Map<String, Value>) {
        payload.insert(
            "latest_quality_metrics".to_string(),
            serde_json::json!(self.latest_quality_metrics),
        );
        payload.insert(
            "quality_metrics_history".to_string(),
            serde_json::json!(self.quality_metrics_history),
        );
        payload.insert(
            "quality_metrics_summary_state".to_string(),
            serde_json::json!(self.quality_metrics_summary_state),
        );
        payload.insert(
            "quality_metrics_summary".to_string(),
            serde_json::json!(self.quality_metrics_summary),
        );
        payload.insert(
            "quality_history_context".to_string(),
            serde_json::json!(self.quality_history_context),
        );
        payload.insert(
            "active_story_repair_payload".to_string(),
            serde_json::json!(self.active_story_repair_payload),
        );
    }

    pub(crate) fn from_runtime_quality_context_and_active_payload(
        quality_runtime_context: &GenerationQualityRuntimeContext,
        active_story_repair_payload: Option<&Value>,
    ) -> Self {
        Self {
            latest_quality_metrics: quality_runtime_context.latest_quality_metrics.clone(),
            quality_metrics_history: quality_runtime_context.quality_metrics_history.clone(),
            quality_metrics_summary_state: quality_runtime_context
                .quality_metrics_summary_state
                .clone(),
            quality_metrics_summary: quality_runtime_context.quality_metrics_summary.clone(),
            quality_history_context: quality_runtime_context.quality_history_context.clone(),
            active_story_repair_payload: active_story_repair_payload.cloned(),
        }
    }
}

pub(crate) fn manual_review_label_from_single_generation_quality_context(
    active_story_repair_payload: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Option<String> {
    manual_review_label_from_payload(active_story_repair_payload)
        .or_else(|| manual_review_label_from_payload(quality_metrics_summary))
        .or_else(|| {
            quality_metrics_summary
                .and_then(|summary| summary.get("quality_gate"))
                .and_then(|payload| manual_review_label_from_payload(Some(payload)))
        })
        .or_else(|| manual_review_label_from_payload(latest_quality_metrics))
        .or_else(|| {
            latest_quality_metrics
                .and_then(|metrics| metrics.get("quality_gate"))
                .and_then(|payload| manual_review_label_from_payload(Some(payload)))
        })
}

fn manual_review_label_from_payload(value: Option<&Value>) -> Option<String> {
    let payload = value?.as_object()?;
    let failure_phase = payload
        .get("phase")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let decision = payload
        .get("quality_gate_decision")
        .and_then(Value::as_str)
        .or_else(|| payload.get("decision").and_then(Value::as_str));
    let is_manual_review = decision
        .map(str::trim)
        .is_some_and(|value| value == "manual_review")
        || failure_phase.is_some_and(|value| value == "quality_blocked");
    if !is_manual_review {
        return None;
    }

    payload
        .get("quality_gate_label")
        .or_else(|| payload.get("label"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| Some("需人工复核".to_string()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        manual_review_label_from_single_generation_quality_context,
        SingleGenerationQualityStatusContext,
    };
    use crate::models::batch_generation_snapshot;

    #[test]
    fn should_build_single_generation_quality_status_context_from_snapshot_and_runtime_state() {
        let snapshot = batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"overall_score": 91})),
            quality_metrics_history: Some(json!([
                {"overall_score": 84},
                {"overall_score": 91}
            ])),
            quality_metrics_summary: Some(json!({
                "quality_gate": {"decision": "pass"},
                "chapter_count": 2
            })),
            workflow_runtime_state: None,
            created_at: None,
            updated_at: None,
        };
        let runtime_state = json!({
            "quality_metrics_summary_state": {
                "scope": "chapter",
                "chapter_count": 2
            },
            "active_story_repair_payload": {
                "summary": "沿用修复建议"
            }
        });

        let context = SingleGenerationQualityStatusContext::from_snapshot_and_runtime_state(
            Some(&snapshot),
            Some(&runtime_state),
        );

        assert_eq!(
            context.latest_quality_metrics,
            Some(json!({"overall_score": 91}))
        );
        assert_eq!(
            context.quality_metrics_summary_state,
            Some(json!({"scope": "chapter", "chapter_count": 2}))
        );
        assert_eq!(
            context.active_story_repair_payload,
            Some(json!({"summary": "沿用修复建议"}))
        );
    }

    #[test]
    fn should_resolve_manual_review_label_from_single_generation_quality_context() {
        let label = manual_review_label_from_single_generation_quality_context(
            None,
            Some(&json!({
                "quality_gate": {
                    "decision": "manual_review",
                    "label": "连续性需人工复核"
                }
            })),
            None,
        );

        assert_eq!(label.as_deref(), Some("连续性需人工复核"));
    }
}
