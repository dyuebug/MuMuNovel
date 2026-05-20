use serde_json::{json, Value};

pub struct ChapterQualityMetricsFragments {
    pub latest_quality_metrics: Option<Value>,
    pub history_id: Option<String>,
    pub generated_at: Option<String>,
    pub quality_metrics_summary: Option<Value>,
}

pub fn build_chapter_quality_metrics_payload(
    chapter_id: &str,
    quality_fragments: ChapterQualityMetricsFragments,
) -> Value {
    json!({
        "chapter_id": chapter_id,
        "has_metrics": quality_fragments.latest_quality_metrics.is_some(),
        "latest_metrics": quality_fragments.latest_quality_metrics,
        "history_id": quality_fragments.history_id,
        "generated_at": quality_fragments.generated_at,
        "latest_quality_metrics": quality_fragments.latest_quality_metrics,
        "quality_metrics_summary": quality_fragments.quality_metrics_summary,
        "quality_profile_summary": Value::Null,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{build_chapter_quality_metrics_payload, ChapterQualityMetricsFragments};

    #[test]
    fn should_build_chapter_quality_metrics_payload_with_metrics() {
        let payload = build_chapter_quality_metrics_payload(
            "chapter-1",
            ChapterQualityMetricsFragments {
                latest_quality_metrics: Some(json!({"score": 91})),
                history_id: Some("history-1".to_string()),
                generated_at: Some("2026-05-17T12:30:45".to_string()),
                quality_metrics_summary: Some(json!({"summary": "ok"})),
            },
        );

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
        let payload = build_chapter_quality_metrics_payload(
            "chapter-1",
            ChapterQualityMetricsFragments {
                latest_quality_metrics: None,
                history_id: None,
                generated_at: None,
                quality_metrics_summary: None,
            },
        );

        assert_eq!(payload["chapter_id"], "chapter-1");
        assert_eq!(payload["has_metrics"], false);
        assert!(payload["latest_metrics"].is_null());
        assert!(payload["latest_quality_metrics"].is_null());
        assert!(payload["history_id"].is_null());
        assert!(payload["generated_at"].is_null());
        assert!(payload["quality_metrics_summary"].is_null());
        assert!(payload["quality_profile_summary"].is_null());
    }
}
