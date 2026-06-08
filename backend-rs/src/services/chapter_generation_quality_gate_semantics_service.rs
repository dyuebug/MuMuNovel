use serde_json::Value;

pub(crate) fn manual_review_label(failed_chapters: Option<&Value>) -> Option<String> {
    failed_chapters.and_then(latest_failed_chapter_manual_review_label)
}

pub(crate) fn retryable_repair_label(
    failed_chapters: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    failed_chapters.and_then(|items| {
        latest_failed_chapter_retryable_repair_label(items, current_retry_count, max_retries)
    })
}

pub(crate) fn manual_review_label_from_quality_context(
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

pub(crate) fn manual_review_label_from_quality_context_with_retry_budget(
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

pub(crate) fn retryable_repair_label_from_quality_context_with_retry_budget(
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
    use serde_json::json;

    use super::{
        manual_review_label, manual_review_label_from_quality_context,
        manual_review_label_from_quality_context_with_retry_budget, retryable_repair_label,
        retryable_repair_label_from_quality_context_with_retry_budget,
    };

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
}
