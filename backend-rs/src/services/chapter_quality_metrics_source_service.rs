use chrono::NaiveDateTime;
use serde_json::Value;

use crate::models::{chapter_draft_attempt, generation_history};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityMetricsSourceKind {
    Candidate,
    History,
    None,
}

#[derive(Debug, Clone)]
pub struct ResolvedQualityMetricsSource {
    pub source_kind: QualityMetricsSourceKind,
    pub metrics: Option<Value>,
    pub source_history_id: Option<String>,
    pub generated_at: Option<NaiveDateTime>,
}

fn latest_history_metrics(
    histories: &[generation_history::Model],
) -> Option<(&generation_history::Model, Value)> {
    histories.iter().find_map(|history| {
        history.generated_content.as_ref().and_then(|content| {
            serde_json::from_str::<Value>(content)
                .ok()
                .and_then(|payload| payload.get("quality_metrics").cloned())
                .map(|metrics| (history, metrics))
        })
    })
}

pub fn resolve_quality_metrics_source(
    histories: &[generation_history::Model],
    candidate_attempt: Option<&chapter_draft_attempt::Model>,
) -> ResolvedQualityMetricsSource {
    if let Some(attempt) = candidate_attempt {
        if let Some(metrics) = attempt.quality_metrics.clone() {
            return ResolvedQualityMetricsSource {
                source_kind: QualityMetricsSourceKind::Candidate,
                metrics: Some(metrics),
                // 保持现有兼容语义：candidate 分支仍复用 history_id 字段返回 attempt.id
                source_history_id: Some(attempt.id.clone()),
                generated_at: attempt.created_at,
            };
        }
    }

    if let Some((history, metrics)) = latest_history_metrics(histories) {
        return ResolvedQualityMetricsSource {
            source_kind: QualityMetricsSourceKind::History,
            metrics: Some(metrics),
            source_history_id: Some(history.id.clone()),
            generated_at: history.created_at,
        };
    }

    ResolvedQualityMetricsSource {
        source_kind: QualityMetricsSourceKind::None,
        metrics: None,
        source_history_id: None,
        generated_at: None,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use super::{resolve_quality_metrics_source, QualityMetricsSourceKind};
    use crate::models::{chapter_draft_attempt, generation_history};

    fn build_attempt(metrics: Option<Value>) -> chapter_draft_attempt::Model {
        chapter_draft_attempt::Model {
            id: "attempt-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            batch_task_id: None,
            source: "candidate".to_string(),
            attempt_state: "generated".to_string(),
            quality_gate_action: None,
            quality_gate_decision: None,
            word_count: 3000,
            summary_preview: None,
            content_preview: None,
            quality_metrics: metrics,
            repair_payload: None,
            created_at: Some(Utc::now().naive_utc()),
        }
    }

    fn build_history(id: &str, metrics: Option<Value>) -> generation_history::Model {
        generation_history::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            prompt: None,
            generated_content: metrics.map(|value| json!({ "quality_metrics": value }).to_string()),
            model: None,
            tokens_used: None,
            generation_time: None,
            created_at: Some(Utc::now().naive_utc()),
        }
    }

    #[test]
    fn should_prefer_candidate_attempt_metrics_over_history() {
        let attempt_metrics = json!({ "score": 91 });
        let history_metrics = json!({ "score": 80 });
        let histories = vec![build_history("history-1", Some(history_metrics.clone()))];

        let resolved =
            resolve_quality_metrics_source(&histories, Some(&build_attempt(Some(attempt_metrics.clone()))));

        assert_eq!(resolved.source_kind, QualityMetricsSourceKind::Candidate);
        assert_eq!(resolved.metrics, Some(attempt_metrics));
        assert_eq!(resolved.source_history_id.as_deref(), Some("attempt-1"));
    }

    #[test]
    fn should_fallback_to_latest_history_metrics_when_candidate_missing() {
        let histories = vec![
            build_history("history-1", None),
            build_history("history-2", Some(json!({ "score": 87 }))),
        ];

        let resolved = resolve_quality_metrics_source(&histories, None);

        assert_eq!(resolved.source_kind, QualityMetricsSourceKind::History);
        assert_eq!(resolved.metrics, Some(json!({ "score": 87 })));
        assert_eq!(resolved.source_history_id.as_deref(), Some("history-2"));
    }

    #[test]
    fn should_return_none_when_no_metrics_source_available() {
        let histories = vec![build_history("history-1", None)];

        let resolved = resolve_quality_metrics_source(&histories, Some(&build_attempt(None)));

        assert_eq!(resolved.source_kind, QualityMetricsSourceKind::None);
        assert!(resolved.metrics.is_none());
        assert!(resolved.source_history_id.is_none());
        assert!(resolved.generated_at.is_none());
    }
}
