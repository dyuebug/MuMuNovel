use chrono::NaiveDateTime;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::models::{chapter, chapter_draft_attempt, generation_history};
use crate::services::chapter_access_service::load_accessible_chapter;
use crate::services::chapter_analysis_read_context_service::load_chapter_analysis_read_context;
use crate::services::chapter_analysis_service::ChapterAnalysisQueryContextError;

pub type LoadChapterQualityMetricsPayloadError = ChapterAnalysisQueryContextError;

pub struct ChapterAnalysisQualityFragments {
    pub quality_metrics: Option<Value>,
    pub quality_metrics_summary: Option<Value>,
}

impl ChapterAnalysisQualityFragments {
    fn from_resolved_source(resolved_source: ResolvedQualityMetricsSource) -> Self {
        let quality_metrics = resolved_source.metrics;
        let quality_metrics_summary = quality_metrics
            .as_ref()
            .map(|metrics| ResolvedQualityMetricsSource::build_summary(metrics, false));

        Self {
            quality_metrics,
            quality_metrics_summary,
        }
    }
}

pub struct ChapterQualityMetricsFragments {
    pub latest_quality_metrics: Option<Value>,
    pub history_id: Option<String>,
    pub generated_at: Option<String>,
    pub quality_metrics_summary: Option<Value>,
}

impl ChapterQualityMetricsFragments {
    fn from_resolved_source(resolved_source: ResolvedQualityMetricsSource) -> Self {
        let generated_at = resolved_source.formatted_generated_at();
        let latest_quality_metrics = resolved_source.metrics;
        let history_id = resolved_source.source_history_id;
        let quality_metrics_summary = latest_quality_metrics
            .as_ref()
            .map(|metrics| ResolvedQualityMetricsSource::build_summary(metrics, true));

        Self {
            latest_quality_metrics,
            history_id,
            generated_at,
            quality_metrics_summary,
        }
    }

    fn into_payload(self, chapter_id: &str) -> Value {
        let ChapterQualityMetricsFragments {
            latest_quality_metrics,
            history_id,
            generated_at,
            quality_metrics_summary,
        } = self;

        json!({
            "chapter_id": chapter_id,
            "has_metrics": latest_quality_metrics.is_some(),
            "latest_metrics": latest_quality_metrics.clone(),
            "history_id": history_id,
            "generated_at": generated_at,
            "latest_quality_metrics": latest_quality_metrics,
            "quality_metrics_summary": quality_metrics_summary,
            "quality_profile_summary": Value::Null,
        })
    }
}

#[derive(Debug, Clone)]
struct ResolvedQualityMetricsSource {
    metrics: Option<Value>,
    source_history_id: Option<String>,
    generated_at: Option<NaiveDateTime>,
}

impl ResolvedQualityMetricsSource {
    fn resolve(
        histories: &[generation_history::Model],
        candidate_attempt: Option<&chapter_draft_attempt::Model>,
    ) -> Self {
        if let Some(attempt) = candidate_attempt {
            if let Some(metrics) = attempt.quality_metrics.clone() {
                return Self {
                    metrics: Some(metrics),
                    source_history_id: Some(attempt.id.clone()),
                    generated_at: attempt.created_at,
                };
            }
        }

        if let Some((history, metrics)) = Self::latest_history_metrics(histories) {
            return Self {
                metrics: Some(metrics),
                source_history_id: Some(history.id.clone()),
                generated_at: history.created_at,
            };
        }

        Self {
            metrics: None,
            source_history_id: None,
            generated_at: None,
        }
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

    fn build_summary(metrics: &Value, include_runtime_context: bool) -> Value {
        let mut payload = serde_json::Map::new();
        payload.insert(
            "repair_guidance".to_string(),
            metrics
                .get("repair_guidance")
                .cloned()
                .unwrap_or(Value::Null),
        );
        payload.insert(
            "quality_gate".to_string(),
            metrics.get("quality_gate").cloned().unwrap_or(Value::Null),
        );
        if include_runtime_context {
            payload.insert(
                "quality_runtime_context".to_string(),
                metrics
                    .get("quality_runtime_context")
                    .cloned()
                    .unwrap_or(Value::Null),
            );
        }
        payload.insert("raw".to_string(), metrics.clone());
        Value::Object(payload)
    }

    fn formatted_generated_at(&self) -> Option<String> {
        self.generated_at
            .map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string())
    }
}

pub fn build_chapter_analysis_quality_fragments(
    histories: &[generation_history::Model],
    candidate_attempt: Option<&chapter_draft_attempt::Model>,
) -> ChapterAnalysisQualityFragments {
    ChapterAnalysisQualityFragments::from_resolved_source(ResolvedQualityMetricsSource::resolve(
        histories,
        candidate_attempt,
    ))
}

pub fn build_chapter_quality_metrics_fragments(
    histories: &[generation_history::Model],
    candidate_attempt: Option<&chapter_draft_attempt::Model>,
) -> ChapterQualityMetricsFragments {
    ChapterQualityMetricsFragments::from_resolved_source(ResolvedQualityMetricsSource::resolve(
        histories,
        candidate_attempt,
    ))
}

pub async fn load_chapter_quality_metrics_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
) -> Result<Value, String> {
    let chapter_id = chapter.id.clone();
    let read_context = load_chapter_analysis_read_context(db, &chapter_id).await?;
    let quality_fragments = build_chapter_quality_metrics_fragments(
        &read_context.histories,
        read_context.candidate_attempt.as_ref(),
    );

    Ok(quality_fragments.into_payload(&chapter_id))
}

pub async fn load_owned_chapter_quality_metrics_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadChapterQualityMetricsPayloadError> {
    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(LoadChapterQualityMetricsPayloadError::Chapter)?;

    load_chapter_quality_metrics_payload(db, &chapter)
        .await
        .map_err(LoadChapterQualityMetricsPayloadError::Internal)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use serde_json::{json, Value};

    use crate::models::{chapter_draft_attempt, generation_history};
    use crate::services::chapter_access_service::LoadAccessibleChapterError;
    use crate::services::chapter_analysis_service::ChapterAnalysisQueryContextError;

    use super::{
        build_chapter_analysis_quality_fragments, build_chapter_quality_metrics_fragments,
        ChapterQualityMetricsFragments, LoadChapterQualityMetricsPayloadError,
        ResolvedQualityMetricsSource,
    };

    fn history(id: &str, metrics: Option<Value>) -> generation_history::Model {
        let generated_content = metrics.map(|metrics| {
            json!({
                "quality_metrics": metrics
            })
            .to_string()
        });

        generation_history::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            prompt: None,
            generated_content,
            model: None,
            tokens_used: None,
            generation_time: None,
            created_at: Some(
                NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
                    .expect("test datetime should parse"),
            ),
        }
    }

    fn quality_metrics() -> Value {
        json!({
            "quality_gate": {
                "decision": "pass"
            },
            "repair_guidance": {
                "summary": "ok"
            },
            "quality_runtime_context": {
                "task_id": "task-1"
            },
            "score": 91
        })
    }

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
            created_at: Some(
                NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
                    .expect("test datetime should parse"),
            ),
        }
    }

    #[test]
    fn should_build_chapter_analysis_quality_fragments_without_runtime_context() {
        let histories = vec![history("history-1", Some(quality_metrics()))];

        let fragments = build_chapter_analysis_quality_fragments(&histories, None);

        assert_eq!(fragments.quality_metrics, Some(quality_metrics()));
        assert_eq!(
            fragments
                .quality_metrics_summary
                .as_ref()
                .and_then(|value| value.get("quality_gate")),
            Some(&json!({"decision": "pass"}))
        );
        assert_eq!(
            fragments
                .quality_metrics_summary
                .as_ref()
                .and_then(|value| value.get("repair_guidance")),
            Some(&json!({"summary": "ok"}))
        );
        assert!(fragments
            .quality_metrics_summary
            .as_ref()
            .and_then(|value| value.get("quality_runtime_context"))
            .is_none());
    }

    #[test]
    fn should_build_chapter_quality_metrics_fragments_with_runtime_context() {
        let histories = vec![history("history-1", Some(quality_metrics()))];

        let fragments = build_chapter_quality_metrics_fragments(&histories, None);

        assert_eq!(fragments.latest_quality_metrics, Some(quality_metrics()));
        assert_eq!(fragments.history_id, Some("history-1".to_string()));
        assert_eq!(
            fragments.generated_at,
            Some("2026-05-17T12:30:45".to_string())
        );
        assert_eq!(
            fragments
                .quality_metrics_summary
                .as_ref()
                .and_then(|value| value.get("quality_runtime_context")),
            Some(&json!({"task_id": "task-1"}))
        );
        assert_eq!(
            fragments
                .quality_metrics_summary
                .as_ref()
                .and_then(|value| value.get("raw")),
            Some(&quality_metrics())
        );
    }

    #[test]
    fn should_build_empty_quality_fragments_without_metrics() {
        let histories = vec![history("history-1", None)];

        let analysis_fragments = build_chapter_analysis_quality_fragments(&histories, None);
        let metrics_fragments = build_chapter_quality_metrics_fragments(&histories, None);

        assert_eq!(analysis_fragments.quality_metrics, None);
        assert_eq!(analysis_fragments.quality_metrics_summary, None);
        assert_eq!(metrics_fragments.latest_quality_metrics, None);
        assert_eq!(metrics_fragments.history_id, None);
        assert_eq!(metrics_fragments.generated_at, None);
        assert_eq!(metrics_fragments.quality_metrics_summary, None);
    }

    #[test]
    fn quality_metrics_error_alias_keeps_shared_analysis_query_context_owner() {
        let chapter_error = LoadChapterQualityMetricsPayloadError::Chapter(
            LoadAccessibleChapterError::NotFoundOrAccessDenied,
        );
        let internal_error = LoadChapterQualityMetricsPayloadError::Internal("boom".to_string());

        assert!(matches!(
            chapter_error,
            ChapterAnalysisQueryContextError::Chapter(
                LoadAccessibleChapterError::NotFoundOrAccessDenied
            )
        ));
        assert!(matches!(
            internal_error,
            ChapterAnalysisQueryContextError::Internal(detail) if detail == "boom"
        ));
    }

    #[test]
    fn should_build_chapter_quality_metrics_payload_with_metrics() {
        let payload = ChapterQualityMetricsFragments {
            latest_quality_metrics: Some(json!({"score": 91})),
            history_id: Some("history-1".to_string()),
            generated_at: Some("2026-05-17T12:30:45".to_string()),
            quality_metrics_summary: Some(json!({"summary": "ok"})),
        }
        .into_payload("chapter-1");

        assert_eq!(payload["chapter_id"], "chapter-1");
        assert_eq!(payload["has_metrics"], true);
        assert_eq!(payload["latest_metrics"], json!({"score": 91}));
        assert_eq!(payload["latest_quality_metrics"], json!({"score": 91}));
        assert_eq!(payload["history_id"], "history-1");
        assert_eq!(payload["generated_at"], "2026-05-17T12:30:45");
        assert_eq!(payload["quality_metrics_summary"], json!({"summary": "ok"}));
        assert!(payload["quality_profile_summary"].is_null());
    }

    #[test]
    fn should_build_chapter_quality_metrics_payload_without_metrics() {
        let payload = ChapterQualityMetricsFragments {
            latest_quality_metrics: None,
            history_id: None,
            generated_at: None,
            quality_metrics_summary: None,
        }
        .into_payload("chapter-1");

        assert_eq!(payload["chapter_id"], "chapter-1");
        assert_eq!(payload["has_metrics"], false);
        assert!(payload["latest_metrics"].is_null());
        assert!(payload["latest_quality_metrics"].is_null());
        assert!(payload["history_id"].is_null());
        assert!(payload["generated_at"].is_null());
        assert!(payload["quality_metrics_summary"].is_null());
        assert!(payload["quality_profile_summary"].is_null());
    }

    #[test]
    fn should_prefer_candidate_attempt_metrics_over_history() {
        let attempt_metrics = json!({ "score": 91 });
        let history_metrics = json!({ "score": 80 });
        let histories = vec![history("history-1", Some(history_metrics.clone()))];

        let resolved = ResolvedQualityMetricsSource::resolve(
            &histories,
            Some(&build_attempt(Some(attempt_metrics.clone()))),
        );

        assert_eq!(resolved.metrics, Some(attempt_metrics));
        assert_eq!(resolved.source_history_id.as_deref(), Some("attempt-1"));
    }

    #[test]
    fn should_fallback_to_latest_history_metrics_when_candidate_missing() {
        let histories = vec![
            history("history-1", None),
            history("history-2", Some(json!({ "score": 87 }))),
        ];

        let resolved = ResolvedQualityMetricsSource::resolve(&histories, None);

        assert_eq!(resolved.metrics, Some(json!({ "score": 87 })));
        assert_eq!(resolved.source_history_id.as_deref(), Some("history-2"));
    }

    #[test]
    fn should_return_none_when_no_metrics_source_available() {
        let histories = vec![history("history-1", None)];

        let resolved =
            ResolvedQualityMetricsSource::resolve(&histories, Some(&build_attempt(None)));

        assert!(resolved.metrics.is_none());
        assert!(resolved.source_history_id.is_none());
        assert!(resolved.generated_at.is_none());
    }
}
