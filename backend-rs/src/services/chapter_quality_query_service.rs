use chrono::NaiveDateTime;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::models::{chapter, chapter_draft_attempt, generation_history};
use crate::services::chapter_service::ChapterService;
use crate::services::chapter_quality_metrics_payload_adapter_service::{
    build_chapter_quality_metrics_payload, ChapterQualityMetricsFragments,
};
use crate::services::chapter_quality_metrics_query_context_service::load_chapter_quality_metrics_query_context;
use crate::services::chapter_quality_metrics_source_service::resolve_quality_metrics_source;

pub enum LoadQualityTrendPayloadError {
    NotFound,
    Internal(String),
}

pub struct ChapterAnalysisQualityFragments {
    pub quality_metrics: Option<Value>,
    pub quality_metrics_summary: Option<Value>,
}

fn format_datetime(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string())
}

fn build_quality_metrics_summary(metrics: &Value, include_runtime_context: bool) -> Value {
    let mut payload = serde_json::Map::new();
    payload.insert(
        "repair_guidance".to_string(),
        metrics.get("repair_guidance").cloned().unwrap_or(Value::Null),
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

pub async fn load_quality_trend_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, LoadQualityTrendPayloadError> {
    match ChapterService::list_by_project(db, project_id, user_id).await {
        Ok(Some(chapters)) => Ok(json!(
            chapters
                .iter()
                .map(|chapter| json!({
                    "chapter_id": chapter.id,
                    "chapter_number": chapter.chapter_number,
                    "title": chapter.title,
                    "word_count": chapter.word_count,
                    "status": chapter.status,
                    "created_at": chapter.created_at.and_utc().to_rfc3339(),
                }))
                .collect::<Vec<Value>>()
        )),
        Ok(None) => Err(LoadQualityTrendPayloadError::NotFound),
        Err(error) => Err(LoadQualityTrendPayloadError::Internal(error)),
    }
}

pub fn build_chapter_analysis_quality_fragments(
    histories: &[generation_history::Model],
    candidate_attempt: Option<&chapter_draft_attempt::Model>,
) -> ChapterAnalysisQualityFragments {
    let quality_metrics = resolve_quality_metrics_source(histories, candidate_attempt).metrics;

    let quality_metrics_summary = quality_metrics
        .as_ref()
        .map(|metrics| build_quality_metrics_summary(metrics, false));

    ChapterAnalysisQualityFragments {
        quality_metrics,
        quality_metrics_summary,
    }
}

pub fn build_chapter_quality_metrics_fragments(
    histories: &[generation_history::Model],
    candidate_attempt: Option<&chapter_draft_attempt::Model>,
) -> ChapterQualityMetricsFragments {
    let resolved_source = resolve_quality_metrics_source(histories, candidate_attempt);
    let latest_quality_metrics = resolved_source.metrics;
    let history_id = resolved_source.source_history_id;
    let generated_at = format_datetime(resolved_source.generated_at);

    let quality_metrics_summary = latest_quality_metrics
        .as_ref()
        .map(|metrics| build_quality_metrics_summary(metrics, true));

    ChapterQualityMetricsFragments {
        latest_quality_metrics,
        history_id,
        generated_at,
        quality_metrics_summary,
    }
}

pub async fn load_chapter_quality_metrics_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
) -> Result<Value, String> {
    let chapter_id = chapter.id.clone();
    let query_context = load_chapter_quality_metrics_query_context(db, &chapter_id).await?;
    let quality_fragments = build_chapter_quality_metrics_fragments(
        &query_context.histories,
        query_context.candidate_attempt.as_ref(),
    );

    Ok(build_chapter_quality_metrics_payload(
        &chapter_id,
        quality_fragments,
    ))
}
