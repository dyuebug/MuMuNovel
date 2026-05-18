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
