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

pub fn manual_review_label(failed_chapters: Option<&Value>) -> Option<String> {
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
    let latest_quality_metrics = snapshot.and_then(|item| item.latest_quality_metrics.clone());
    let quality_metrics_summary = snapshot.and_then(|item| item.quality_metrics_summary.clone());

    BatchGenerationQualityStatusContext {
        latest_quality_metrics,
        quality_metrics_summary,
        active_story_repair_payload,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::models::{batch_generation_snapshot, batch_generation_task};

    use super::{
        active_story_repair_payload_from_runtime_state, build_quality_status_context,
        manual_review_label, terminal_semantics,
    };

    fn snapshot_with_quality_fields() -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"score": 91})),
            quality_metrics_history: Some(json!([{"score": 90}])),
            quality_metrics_summary: Some(json!({"summary": "ok"})),
            workflow_runtime_state: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn should_extract_active_story_repair_payload_from_runtime_state() {
        let runtime_state = json!({
            "active_story_repair_payload": {
                "mode": "repair",
                "attempt": 2
            }
        });

        let payload = active_story_repair_payload_from_runtime_state(Some(&runtime_state));

        assert_eq!(payload, Some(json!({"mode": "repair", "attempt": 2})));
    }

    #[test]
    fn should_ignore_non_object_active_story_repair_payload() {
        let runtime_state = json!({
            "active_story_repair_payload": "not-an-object"
        });

        assert_eq!(
            active_story_repair_payload_from_runtime_state(Some(&runtime_state)),
            None
        );
        assert_eq!(active_story_repair_payload_from_runtime_state(None), None);
    }

    #[test]
    fn should_build_quality_status_context_from_snapshot_and_runtime_state() {
        let snapshot = snapshot_with_quality_fields();
        let runtime_state = json!({
            "active_story_repair_payload": {
                "mode": "repair"
            }
        });

        let context = build_quality_status_context(Some(&snapshot), Some(&runtime_state));

        assert_eq!(context.latest_quality_metrics, Some(json!({"score": 91})));
        assert_eq!(
            context.quality_metrics_summary,
            Some(json!({"summary": "ok"}))
        );
        assert_eq!(
            context.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_build_empty_quality_status_context_without_snapshot_or_runtime_state() {
        let context = build_quality_status_context(None, None);

        assert_eq!(context.latest_quality_metrics, None);
        assert_eq!(context.quality_metrics_summary, None);
        assert_eq!(context.active_story_repair_payload, None);
    }

    #[test]
    fn should_resolve_manual_review_label_with_fallback_and_custom_label() {
        let default_failed_chapters = json!([
            {
                "quality_gate_decision": "manual_review"
            }
        ]);
        let custom_failed_chapters = json!([
            {
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "需人工处理"
            }
        ]);
        let blank_label_failed_chapters = json!([
            {
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "   "
            }
        ]);
        let ignored_failed_chapters = json!([
            {
                "quality_gate_decision": "auto_pass",
                "quality_gate_label": "should-not-appear"
            }
        ]);

        assert_eq!(
            manual_review_label(Some(&default_failed_chapters)),
            Some("需人工复核".to_string())
        );
        assert_eq!(
            manual_review_label(Some(&custom_failed_chapters)),
            Some("需人工处理".to_string())
        );
        assert_eq!(
            manual_review_label(Some(&blank_label_failed_chapters)),
            Some("需人工复核".to_string())
        );
        assert_eq!(manual_review_label(Some(&ignored_failed_chapters)), None);
        assert_eq!(manual_review_label(None), None);
    }

    #[test]
    fn should_resolve_terminal_semantics_for_manual_review_failed_task() {
        let task = batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", "chapter-2"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: "failed".to_string(),
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: json!([
                {
                    "quality_gate_decision": "manual_review",
                    "quality_gate_label": "待补充"
                }
            ]),
            current_chapter_id: None,
            current_chapter_number: None,
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };

        let (reason, label, review_required, can_resume) =
            terminal_semantics(&task, Some(&task.failed_chapters));

        assert_eq!(reason, Some("manual_review"));
        assert_eq!(label, Some("待补充".to_string()));
        assert!(review_required);
        assert!(!can_resume);
    }
}
