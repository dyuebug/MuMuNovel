use serde_json::Value;

use crate::models::{batch_generation_snapshot, batch_generation_task};

#[derive(Debug, Clone)]
pub struct BatchGenerationQualityStatusContext {
    pub latest_quality_metrics: Option<Value>,
    pub quality_metrics_summary: Option<Value>,
    pub active_story_repair_payload: Option<Value>,
}

pub fn active_story_repair_payload_from_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> Option<Value> {
    workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("active_story_repair_payload"))
        .filter(|payload| payload.is_object())
        .cloned()
}

fn manual_review_label(failed_chapters: Option<&Value>) -> Option<String> {
    let items = failed_chapters?.as_array()?;
    let first = items.first()?.as_object()?;
    let decision = first.get("quality_gate_decision")?.as_str()?;
    if decision != "manual_review" {
        return None;
    }

    first
        .get("quality_gate_label")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| Some("需人工复核".to_string()))
}

pub fn terminal_semantics(
    task: &batch_generation_task::Model,
    failed_chapters: Option<&Value>,
) -> (Option<&'static str>, Option<String>, bool, bool) {
    if task.status == "failed" {
        if let Some(label) = manual_review_label(failed_chapters) {
            return (Some("manual_review"), Some(label), true, false);
        }
        return (Some("error"), Some("执行失败".to_string()), false, true);
    }

    match task.status.as_str() {
        "completed" => (Some("completed"), Some("已完成".to_string()), false, false),
        "cancelled" => (Some("cancelled"), Some("已取消".to_string()), false, true),
        _ => (None, None, false, false),
    }
}

pub fn build_quality_status_context(
    snapshot: Option<&batch_generation_snapshot::Model>,
    workflow_runtime_state: Option<&Value>,
) -> BatchGenerationQualityStatusContext {
    let active_story_repair_payload =
        active_story_repair_payload_from_runtime_state(workflow_runtime_state);
    let latest_quality_metrics = snapshot
        .and_then(|item| item.latest_quality_metrics.clone());
    let quality_metrics_summary = snapshot
        .and_then(|item| item.quality_metrics_summary.clone());

    BatchGenerationQualityStatusContext {
        latest_quality_metrics,
        quality_metrics_summary,
        active_story_repair_payload,
    }
}
