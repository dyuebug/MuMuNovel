use serde_json::Value;

use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_batch_generation_quality_runtime_context_service::{
    resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state,
    BatchGenerationQualityRuntimeContext,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BatchGenerationQualityStatusContext {
    pub latest_quality_metrics: Option<Value>,
    pub quality_metrics_history: Option<Value>,
    pub quality_metrics_summary_state: Option<Value>,
    pub quality_metrics_summary: Option<Value>,
    pub quality_history_context: Option<Value>,
    pub active_story_repair_payload: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationFailedTerminalKind {
    ManualReview,
    Retry,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationFailedTerminalSemantics {
    pub(crate) kind: BatchGenerationFailedTerminalKind,
    pub(crate) reason: &'static str,
    pub(crate) label: String,
    pub(crate) review_required: bool,
    pub(crate) can_resume: bool,
}

impl BatchGenerationQualityStatusContext {
    pub fn from_snapshot_and_runtime_state(
        snapshot: Option<&batch_generation_snapshot::Model>,
        workflow_runtime_state: Option<&Value>,
    ) -> Self {
        let active_story_repair_payload =
            Self::active_story_repair_payload_from_runtime_state(workflow_runtime_state);
        let quality_runtime_context =
            resolve_batch_quality_runtime_context_from_snapshot_and_runtime_state(
                snapshot,
                workflow_runtime_state,
            );

        Self::from_runtime_quality_context_and_active_payload(
            &quality_runtime_context,
            active_story_repair_payload.as_ref(),
        )
    }

    pub fn insert_into_payload(&self, payload: &mut serde_json::Map<String, Value>) {
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

    pub fn from_runtime_quality_context_and_active_payload(
        quality_runtime_context: &BatchGenerationQualityRuntimeContext,
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

    fn active_story_repair_payload_from_runtime_state(
        workflow_runtime_state: Option<&Value>,
    ) -> Option<Value> {
        workflow_runtime_state
            .and_then(Value::as_object)
            .and_then(|state| state.get("active_story_repair_payload"))
            .filter(|payload| payload.is_object())
            .cloned()
    }
}

pub fn insert_batch_generation_terminal_status_payload(
    payload: &mut serde_json::Map<String, Value>,
    task: &batch_generation_task::Model,
    failed_chapters: Option<&Value>,
    quality_status_context: Option<&BatchGenerationQualityStatusContext>,
) {
    let (terminal_reason, terminal_label, review_required, can_resume) = if task.status == "failed"
    {
        resolve_failed_terminal_semantics(task, failed_chapters, quality_status_context)
            .map(|semantics| match semantics.kind {
                BatchGenerationFailedTerminalKind::ManualReview => (
                    Some(semantics.reason),
                    Some(semantics.label),
                    semantics.review_required,
                    semantics.can_resume,
                ),
                BatchGenerationFailedTerminalKind::Retry
                | BatchGenerationFailedTerminalKind::Error => {
                    (Some("error"), Some("执行失败".to_string()), false, true)
                }
            })
            .unwrap_or((Some("error"), Some("执行失败".to_string()), false, true))
    } else {
        match task.status.as_str() {
            "completed" => (Some("completed"), Some("已完成".to_string()), false, false),
            "cancelled" => (Some("cancelled"), Some("已取消".to_string()), false, true),
            _ => (None, None, false, false),
        }
    };

    payload.insert(
        "terminal_reason".to_string(),
        serde_json::json!(terminal_reason),
    );
    payload.insert(
        "terminal_label".to_string(),
        serde_json::json!(terminal_label),
    );
    payload.insert(
        "review_required".to_string(),
        serde_json::json!(review_required),
    );
    payload.insert("can_resume".to_string(), serde_json::json!(can_resume));
}

pub(crate) fn resolve_failed_terminal_semantics(
    task: &batch_generation_task::Model,
    failed_chapters: Option<&Value>,
    quality_status_context: Option<&BatchGenerationQualityStatusContext>,
) -> Option<BatchGenerationFailedTerminalSemantics> {
    resolve_failed_terminal_semantics_from_sources(
        failed_chapters,
        quality_status_context,
        task.current_retry_count,
        task.max_retries,
    )
}

pub(crate) fn resolve_failed_terminal_semantics_from_sources(
    failed_chapters: Option<&Value>,
    quality_status_context: Option<&BatchGenerationQualityStatusContext>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<BatchGenerationFailedTerminalSemantics> {
    if let Some(label) = manual_review_label(failed_chapters).or_else(|| {
        quality_status_context.and_then(|context| {
            manual_review_label_from_quality_context_with_retry_budget(
                context.active_story_repair_payload.as_ref(),
                context.quality_metrics_summary.as_ref(),
                context.latest_quality_metrics.as_ref(),
                current_retry_count,
                max_retries,
            )
        })
    }) {
        return Some(BatchGenerationFailedTerminalSemantics {
            kind: BatchGenerationFailedTerminalKind::ManualReview,
            reason: "manual_review",
            label,
            review_required: true,
            can_resume: false,
        });
    }

    if let Some(label) = retryable_repair_label(failed_chapters, current_retry_count, max_retries)
        .or_else(|| {
            quality_status_context.and_then(|context| {
                retryable_repair_label_from_quality_context_with_retry_budget(
                    context.active_story_repair_payload.as_ref(),
                    context.quality_metrics_summary.as_ref(),
                    context.latest_quality_metrics.as_ref(),
                    current_retry_count,
                    max_retries,
                )
            })
        })
    {
        return Some(BatchGenerationFailedTerminalSemantics {
            kind: BatchGenerationFailedTerminalKind::Retry,
            reason: "retry",
            label,
            review_required: false,
            can_resume: true,
        });
    }

    Some(BatchGenerationFailedTerminalSemantics {
        kind: BatchGenerationFailedTerminalKind::Error,
        reason: "error",
        label: "执行失败".to_string(),
        review_required: false,
        can_resume: true,
    })
}

pub fn manual_review_label(failed_chapters: Option<&Value>) -> Option<String> {
    failed_chapters.and_then(latest_failed_chapter_manual_review_label)
}

pub fn retryable_repair_label(
    failed_chapters: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    failed_chapters.and_then(|items| {
        latest_failed_chapter_retryable_repair_label(items, current_retry_count, max_retries)
    })
}

pub fn manual_review_label_from_quality_context(
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

pub fn manual_review_label_from_quality_context_with_retry_budget(
    active_story_repair_payload: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    manual_review_label_from_quality_context(
        active_story_repair_payload,
        quality_metrics_summary,
        latest_quality_metrics,
    )
    .or_else(|| {
        exhausted_auto_repair_label_from_quality_context(
            active_story_repair_payload,
            quality_metrics_summary,
            latest_quality_metrics,
            current_retry_count,
            max_retries,
        )
    })
}

pub fn retryable_repair_label_from_quality_context_with_retry_budget(
    active_story_repair_payload: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    retryable_repair_label_from_payload(
        active_story_repair_payload,
        current_retry_count,
        max_retries,
    )
    .or_else(|| {
        retryable_repair_label_from_payload(
            quality_metrics_summary,
            current_retry_count,
            max_retries,
        )
    })
    .or_else(|| {
        quality_metrics_summary
            .and_then(|summary| summary.get("quality_gate"))
            .and_then(|payload| {
                retryable_repair_label_from_payload(Some(payload), current_retry_count, max_retries)
            })
    })
    .or_else(|| {
        retryable_repair_label_from_payload(
            latest_quality_metrics,
            current_retry_count,
            max_retries,
        )
    })
    .or_else(|| {
        latest_quality_metrics
            .and_then(|metrics| metrics.get("quality_gate"))
            .and_then(|payload| {
                retryable_repair_label_from_payload(Some(payload), current_retry_count, max_retries)
            })
    })
}

fn latest_failed_chapter_manual_review_label(failed_chapters: &Value) -> Option<String> {
    let items = failed_chapters.as_array()?;
    let latest = items.iter().rev().find(|item| item.is_object())?;
    manual_review_label_from_payload(Some(latest))
}

fn latest_failed_chapter_retryable_repair_label(
    failed_chapters: &Value,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    let items = failed_chapters.as_array()?;
    let latest = items.iter().rev().find(|item| item.is_object())?;
    retryable_repair_label_from_payload(Some(latest), current_retry_count, max_retries)
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

fn retryable_repair_label_from_payload(
    value: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    if max_retries < 0 || current_retry_count >= max_retries {
        return None;
    }

    let payload = value?.as_object()?;
    let decision = payload
        .get("quality_gate_decision")
        .and_then(Value::as_str)
        .or_else(|| payload.get("decision").and_then(Value::as_str))
        .map(str::trim)?;
    if decision != "auto_repair" && decision != "repair" {
        return None;
    }

    payload
        .get("quality_gate_label")
        .or_else(|| payload.get("label"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| Some("可自动修复后重试".to_string()))
}

fn exhausted_auto_repair_label_from_quality_context(
    active_story_repair_payload: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    exhausted_auto_repair_label_from_payload(
        active_story_repair_payload,
        current_retry_count,
        max_retries,
    )
    .or_else(|| {
        exhausted_auto_repair_label_from_payload(
            quality_metrics_summary,
            current_retry_count,
            max_retries,
        )
    })
    .or_else(|| {
        quality_metrics_summary
            .and_then(|summary| summary.get("quality_gate"))
            .and_then(|payload| {
                exhausted_auto_repair_label_from_payload(
                    Some(payload),
                    current_retry_count,
                    max_retries,
                )
            })
    })
    .or_else(|| {
        exhausted_auto_repair_label_from_payload(
            latest_quality_metrics,
            current_retry_count,
            max_retries,
        )
    })
    .or_else(|| {
        latest_quality_metrics
            .and_then(|metrics| metrics.get("quality_gate"))
            .and_then(|payload| {
                exhausted_auto_repair_label_from_payload(
                    Some(payload),
                    current_retry_count,
                    max_retries,
                )
            })
    })
}

fn exhausted_auto_repair_label_from_payload(
    value: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    if max_retries < 0 || current_retry_count < max_retries {
        return None;
    }

    let payload = value?.as_object()?;
    let decision = payload
        .get("quality_gate_decision")
        .and_then(Value::as_str)
        .or_else(|| payload.get("decision").and_then(Value::as_str))
        .map(str::trim)?;
    if decision != "auto_repair" && decision != "repair" {
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
    use serde_json::{json, Value};

    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_batch_generation_quality_runtime_context_service::BatchGenerationQualityRuntimeContext;

    use super::{
        insert_batch_generation_terminal_status_payload, manual_review_label,
        manual_review_label_from_quality_context,
        manual_review_label_from_quality_context_with_retry_budget,
        resolve_failed_terminal_semantics, resolve_failed_terminal_semantics_from_sources,
        retryable_repair_label, retryable_repair_label_from_quality_context_with_retry_budget,
        BatchGenerationFailedTerminalKind, BatchGenerationQualityStatusContext,
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

        let payload =
            BatchGenerationQualityStatusContext::active_story_repair_payload_from_runtime_state(
                Some(&runtime_state),
            );

        assert_eq!(payload, Some(json!({"mode": "repair", "attempt": 2})));
    }

    #[test]
    fn should_ignore_non_object_active_story_repair_payload() {
        let runtime_state = json!({
            "active_story_repair_payload": "not-an-object"
        });

        assert_eq!(
            BatchGenerationQualityStatusContext::active_story_repair_payload_from_runtime_state(
                Some(&runtime_state),
            ),
            None
        );
        assert_eq!(
            BatchGenerationQualityStatusContext::active_story_repair_payload_from_runtime_state(
                None
            ),
            None
        );
    }

    #[test]
    fn should_build_quality_status_context_from_snapshot_and_runtime_state() {
        let snapshot = snapshot_with_quality_fields();
        let runtime_state = json!({
            "active_story_repair_payload": {
                "mode": "repair"
            }
        });

        let context = BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
            Some(&snapshot),
            Some(&runtime_state),
        );

        assert_eq!(context.latest_quality_metrics, Some(json!({"score": 91})));
        assert_eq!(
            context.quality_metrics_history,
            Some(json!([{"score": 90}]))
        );
        assert_eq!(
            context.quality_metrics_summary,
            Some(json!({"summary": "ok"}))
        );
        assert_eq!(
            context
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("scope")),
            Some(&json!("batch"))
        );
        assert_eq!(
            context
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("chapter_count")),
            Some(&json!(1))
        );
        assert_eq!(context.quality_history_context, None);
        assert_eq!(
            context.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_fallback_to_runtime_state_quality_fields_when_snapshot_missing_them() {
        let mut snapshot = snapshot_with_quality_fields();
        snapshot.latest_quality_metrics = None;
        snapshot.quality_metrics_summary = None;
        let runtime_state = json!({
            "latest_quality_metrics": {
                "score": 87
            },
            "quality_metrics_summary": {
                "summary": "runtime"
            },
            "active_story_repair_payload": {
                "mode": "repair"
            }
        });

        let context = BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
            Some(&snapshot),
            Some(&runtime_state),
        );

        assert_eq!(context.latest_quality_metrics, Some(json!({"score": 87})));
        assert_eq!(
            context.quality_metrics_history,
            Some(json!([{"score": 90}]))
        );
        assert_eq!(
            context.quality_metrics_summary,
            Some(json!({"summary": "runtime"}))
        );
        assert_eq!(
            context
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("scope")),
            Some(&json!("batch"))
        );
        assert_eq!(
            context
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("chapter_count")),
            Some(&json!(1))
        );
        assert_eq!(context.quality_history_context, None);
        assert_eq!(
            context.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_build_empty_quality_status_context_without_snapshot_or_runtime_state() {
        let context =
            BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(None, None);

        assert_eq!(context.latest_quality_metrics, None);
        assert_eq!(context.quality_metrics_history, None);
        assert_eq!(context.quality_metrics_summary_state, None);
        assert_eq!(context.quality_metrics_summary, None);
        assert_eq!(context.quality_history_context, None);
        assert_eq!(context.active_story_repair_payload, None);
    }

    #[test]
    fn should_insert_quality_status_context_fields_into_payload() {
        let context = BatchGenerationQualityStatusContext {
            latest_quality_metrics: Some(json!({"score": 91})),
            quality_metrics_history: Some(json!([{"score": 90}])),
            quality_metrics_summary_state: Some(json!({"chapter_count": 1})),
            quality_metrics_summary: Some(json!({"summary": "ok"})),
            quality_history_context: Some(json!({"scope": "batch"})),
            active_story_repair_payload: Some(json!({"mode": "repair"})),
        };
        let mut payload = serde_json::Map::new();

        context.insert_into_payload(&mut payload);

        assert_eq!(payload["latest_quality_metrics"]["score"], 91);
        assert_eq!(payload["quality_metrics_history"][0]["score"], 90);
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 1);
        assert_eq!(payload["quality_metrics_summary"]["summary"], "ok");
        assert_eq!(payload["quality_history_context"]["scope"], "batch");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }

    #[test]
    fn should_build_quality_status_context_from_runtime_quality_context_and_active_payload() {
        let runtime_quality_context = BatchGenerationQualityRuntimeContext {
            latest_quality_metrics: Some(json!({"score": 91})),
            quality_metrics_history: Some(json!([{"score": 90}])),
            quality_metrics_summary_state: Some(json!({"chapter_count": 1})),
            quality_metrics_summary: Some(json!({"summary": "ok"})),
            quality_history_context: Some(json!({"scope": "batch"})),
        };
        let context =
            BatchGenerationQualityStatusContext::from_runtime_quality_context_and_active_payload(
                &runtime_quality_context,
                Some(&json!({"mode": "repair"})),
            );

        assert_eq!(context.latest_quality_metrics, Some(json!({"score": 91})));
        assert_eq!(
            context.quality_metrics_history,
            Some(json!([{"score": 90}]))
        );
        assert_eq!(
            context.quality_metrics_summary_state,
            Some(json!({"chapter_count": 1}))
        );
        assert_eq!(
            context.quality_metrics_summary,
            Some(json!({"summary": "ok"}))
        );
        assert_eq!(
            context.quality_history_context,
            Some(json!({"scope": "batch"}))
        );
        assert_eq!(
            context.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_insert_terminal_status_fields_into_payload() {
        let mut payload = serde_json::Map::new();
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
            failed_chapters: json!([{
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "待补充"
            }]),
            current_chapter_id: None,
            current_chapter_number: None,
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };

        insert_batch_generation_terminal_status_payload(
            &mut payload,
            &task,
            Some(&task.failed_chapters),
            None,
        );

        assert_eq!(payload["terminal_reason"], "manual_review");
        assert_eq!(payload["terminal_label"], "待补充");
        assert_eq!(payload["review_required"], true);
        assert_eq!(payload["can_resume"], false);
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
        let phase_only_failed_chapters = json!([
            {
                "phase": "quality_blocked"
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
        assert_eq!(
            manual_review_label(Some(&phase_only_failed_chapters)),
            Some("需人工复核".to_string())
        );
        assert_eq!(manual_review_label(Some(&ignored_failed_chapters)), None);
        assert_eq!(manual_review_label(None), None);
    }

    #[test]
    fn should_prefer_latest_failed_chapter_for_manual_review_label() {
        let failed_chapters = json!([
            {
                "quality_gate_decision": "auto_repair",
                "quality_gate_label": "旧的自动修复建议"
            },
            {
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "新的人工复核标签"
            }
        ]);

        assert_eq!(
            manual_review_label(Some(&failed_chapters)),
            Some("新的人工复核标签".to_string())
        );
    }

    #[test]
    fn should_prefer_latest_failed_chapter_for_retryable_repair_label() {
        let failed_chapters = json!([
            {
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "旧的人工复核标签"
            },
            {
                "quality_gate_decision": "auto_repair",
                "quality_gate_label": "新的自动修复建议"
            }
        ]);

        assert_eq!(
            retryable_repair_label(Some(&failed_chapters), 1, 3),
            Some("新的自动修复建议".to_string())
        );
    }

    #[test]
    fn should_resolve_manual_review_label_from_quality_context_sources() {
        assert_eq!(
            manual_review_label_from_quality_context(
                Some(&json!({
                    "quality_gate_decision": "manual_review",
                    "quality_gate_label": "来自 active payload"
                })),
                None,
                None,
            ),
            Some("来自 active payload".to_string())
        );

        assert_eq!(
            manual_review_label_from_quality_context(
                None,
                Some(&json!({
                    "quality_gate": {
                        "decision": "manual_review",
                        "label": "来自 summary gate"
                    }
                })),
                None,
            ),
            Some("来自 summary gate".to_string())
        );

        assert_eq!(
            manual_review_label_from_quality_context(
                None,
                None,
                Some(&json!({
                    "quality_gate": {
                        "decision": "manual_review"
                    }
                })),
            ),
            Some("需人工复核".to_string())
        );

        assert_eq!(
            manual_review_label_from_quality_context(
                None,
                Some(&json!({
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "不应命中"
                    }
                })),
                None,
            ),
            None
        );
    }

    #[test]
    fn should_resolve_manual_review_label_when_auto_repair_retry_budget_is_exhausted() {
        assert_eq!(
            manual_review_label_from_quality_context_with_retry_budget(
                None,
                Some(&json!({
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "自动修复预算已耗尽"
                    }
                })),
                None,
                3,
                3,
            ),
            Some("自动修复预算已耗尽".to_string())
        );

        assert_eq!(
            manual_review_label_from_quality_context_with_retry_budget(
                None,
                Some(&json!({
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "不应命中"
                    }
                })),
                None,
                2,
                3,
            ),
            None
        );
    }

    #[test]
    fn should_resolve_retryable_repair_label_when_auto_repair_budget_is_available() {
        assert_eq!(
            retryable_repair_label_from_quality_context_with_retry_budget(
                None,
                Some(&json!({
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "自动修复后重试"
                    }
                })),
                None,
                1,
                3,
            ),
            Some("自动修复后重试".to_string())
        );

        assert_eq!(
            retryable_repair_label(
                Some(&json!([{
                    "quality_gate_decision": "repair",
                    "quality_gate_label": "继续修复"
                }])),
                0,
                2,
            ),
            Some("继续修复".to_string())
        );
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

        let mut payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(
            &mut payload,
            &task,
            Some(&task.failed_chapters),
            None,
        );

        assert_eq!(payload["terminal_reason"], "manual_review");
        assert_eq!(payload["terminal_label"], "待补充");
        assert_eq!(payload["review_required"], true);
        assert_eq!(payload["can_resume"], false);
    }

    #[test]
    fn should_resolve_terminal_semantics_for_quality_blocked_failed_task() {
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
                    "phase": "quality_blocked"
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

        let mut payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(
            &mut payload,
            &task,
            Some(&task.failed_chapters),
            None,
        );

        assert_eq!(payload["terminal_reason"], "manual_review");
        assert_eq!(payload["terminal_label"], "需人工复核");
        assert_eq!(payload["review_required"], true);
        assert_eq!(payload["can_resume"], false);
    }

    #[test]
    fn should_build_terminal_status_payload_for_completed_cancelled_and_default_tasks() {
        let mut completed = batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", "chapter-2"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: "completed".to_string(),
            total_chapters: 2,
            completed_chapters: 2,
            failed_chapters: json!([]),
            current_chapter_id: None,
            current_chapter_number: None,
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };
        let cancelled = batch_generation_task::Model {
            status: "cancelled".to_string(),
            ..completed.clone()
        };
        let pending = batch_generation_task::Model {
            status: "pending".to_string(),
            ..completed.clone()
        };

        let mut completed_payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(
            &mut completed_payload,
            &completed,
            None,
            None,
        );
        assert_eq!(completed_payload["terminal_reason"], "completed");
        assert_eq!(completed_payload["terminal_label"], "已完成");
        assert_eq!(completed_payload["review_required"], false);
        assert_eq!(completed_payload["can_resume"], false);

        let mut cancelled_payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(
            &mut cancelled_payload,
            &cancelled,
            None,
            None,
        );
        assert_eq!(cancelled_payload["terminal_reason"], "cancelled");
        assert_eq!(cancelled_payload["terminal_label"], "已取消");
        assert_eq!(cancelled_payload["review_required"], false);
        assert_eq!(cancelled_payload["can_resume"], true);

        let mut pending_payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(&mut pending_payload, &pending, None, None);
        assert_eq!(pending_payload["terminal_reason"], Value::Null);
        assert_eq!(pending_payload["terminal_label"], Value::Null);
        assert_eq!(pending_payload["review_required"], false);
        assert_eq!(pending_payload["can_resume"], false);

        completed.status = "failed".to_string();
        completed.failed_chapters = json!([{
            "quality_gate_decision": "manual_review",
            "quality_gate_label": "待补充"
        }]);
        let mut manual_review_payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(
            &mut manual_review_payload,
            &completed,
            Some(&completed.failed_chapters),
            None,
        );
        assert_eq!(manual_review_payload["terminal_reason"], "manual_review");
        assert_eq!(manual_review_payload["terminal_label"], "待补充");
        assert_eq!(manual_review_payload["review_required"], true);
        assert_eq!(manual_review_payload["can_resume"], false);
    }

    #[test]
    fn should_resolve_terminal_semantics_from_quality_context_when_failed_chapters_missing_label() {
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
            failed_chapters: json!([]),
            current_chapter_id: None,
            current_chapter_number: None,
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };
        let quality_status_context = BatchGenerationQualityStatusContext {
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
        };

        let mut payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(
            &mut payload,
            &task,
            Some(&task.failed_chapters),
            Some(&quality_status_context),
        );

        assert_eq!(payload["terminal_reason"], "manual_review");
        assert_eq!(payload["terminal_label"], "等待人工复核");
        assert_eq!(payload["review_required"], true);
        assert_eq!(payload["can_resume"], false);
    }

    #[test]
    fn should_resolve_terminal_semantics_for_exhausted_auto_repair_failed_task() {
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
            failed_chapters: json!([]),
            current_chapter_id: None,
            current_chapter_number: None,
            current_retry_count: 3,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };
        let quality_status_context = BatchGenerationQualityStatusContext {
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
        };

        let mut payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(
            &mut payload,
            &task,
            Some(&task.failed_chapters),
            Some(&quality_status_context),
        );

        assert_eq!(payload["terminal_reason"], "manual_review");
        assert_eq!(payload["terminal_label"], "自动修复预算已耗尽");
        assert_eq!(payload["review_required"], true);
        assert_eq!(payload["can_resume"], false);
    }

    #[test]
    fn should_resolve_terminal_semantics_for_retryable_auto_repair_failed_task() {
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
            failed_chapters: json!([]),
            current_chapter_id: None,
            current_chapter_number: None,
            current_retry_count: 1,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };
        let quality_status_context = BatchGenerationQualityStatusContext {
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
        };

        let mut payload = serde_json::Map::new();
        insert_batch_generation_terminal_status_payload(
            &mut payload,
            &task,
            Some(&task.failed_chapters),
            Some(&quality_status_context),
        );

        assert_eq!(payload["terminal_reason"], "error");
        assert_eq!(payload["terminal_label"], "执行失败");
        assert_eq!(payload["review_required"], false);
        assert_eq!(payload["can_resume"], true);
    }

    #[test]
    fn should_resolve_shared_failed_terminal_semantics_owner() {
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
            failed_chapters: json!([]),
            current_chapter_id: None,
            current_chapter_number: None,
            current_retry_count: 1,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };
        let quality_status_context = BatchGenerationQualityStatusContext {
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
        };

        let semantics = resolve_failed_terminal_semantics(
            &task,
            Some(&task.failed_chapters),
            Some(&quality_status_context),
        )
        .expect("failed terminal semantics");

        assert_eq!(semantics.kind, BatchGenerationFailedTerminalKind::Retry);
        assert_eq!(semantics.reason, "retry");
        assert_eq!(semantics.label, "自动修复后重试");
        assert!(!semantics.review_required);
        assert!(semantics.can_resume);
    }

    #[test]
    fn should_resolve_shared_failed_terminal_semantics_from_sources_without_task_wrapper() {
        let quality_status_context = BatchGenerationQualityStatusContext {
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
        };

        let semantics = resolve_failed_terminal_semantics_from_sources(
            Some(&json!([])),
            Some(&quality_status_context),
            0,
            3,
        )
        .expect("failed terminal semantics");

        assert_eq!(
            semantics.kind,
            BatchGenerationFailedTerminalKind::ManualReview
        );
        assert_eq!(semantics.reason, "manual_review");
        assert_eq!(semantics.label, "等待人工复核");
        assert!(semantics.review_required);
        assert!(!semantics.can_resume);
    }
}
