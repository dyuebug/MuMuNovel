use chrono::NaiveDateTime;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use crate::models::{chapter, chapter_draft_attempt, generation_history};
use crate::services::chapter_access_service::load_accessible_chapter;
use crate::services::chapter_analysis_service::{
    load_chapter_analysis_read_context, ChapterAnalysisQueryContextError,
};
use crate::services::chapter_generation_history_payload_service::normalize_generated_history_quality_metrics;
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    merge_generation_quality_history_context, resolve_generation_quality_runtime_context_for_seed,
};
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::{
    build_quality_metrics_summary_from_state, build_quality_metrics_summary_state_from_history,
};

pub type LoadChapterQualityMetricsPayloadError = ChapterAnalysisQueryContextError;
const MAX_CHAPTER_QUALITY_METRICS_HISTORY: usize = 20;

pub struct ChapterAnalysisQualityFragments {
    pub quality_metrics: Option<Value>,
    pub quality_metrics_summary: Option<Value>,
}

impl ChapterAnalysisQualityFragments {
    fn from_resolved_source(
        resolved_source: ResolvedQualityMetricsSource,
        quality_metrics_history: Vec<Value>,
    ) -> Self {
        let quality_metrics = resolved_source.metrics;
        let quality_metrics_summary_state =
            build_quality_metrics_summary_state_from_history(&quality_metrics_history, "batch");
        let quality_metrics_summary = build_quality_metrics_summary_from_state(
            quality_metrics_summary_state.as_ref(),
            &quality_metrics_history,
            "batch",
        );

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
    pub quality_metrics_history: Option<Value>,
    pub quality_metrics_summary_state: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LatestQualityMetricRecord {
    pub chapter_id: String,
    pub latest_quality_metrics: Value,
    pub history_id: String,
    pub generated_at: Option<String>,
    pub generated_at_dt: Option<NaiveDateTime>,
}

impl ChapterQualityMetricsFragments {
    fn from_resolved_source(
        resolved_source: ResolvedQualityMetricsSource,
        quality_metrics_history: Vec<Value>,
    ) -> Self {
        let generated_at = resolved_source.formatted_generated_at();
        let latest_quality_metrics = resolved_source.metrics;
        let history_id = resolved_source.source_history_id;
        let quality_metrics_summary_state =
            build_quality_metrics_summary_state_from_history(&quality_metrics_history, "chapter");
        let quality_metrics_history_value = (!quality_metrics_history.is_empty())
            .then_some(Value::Array(quality_metrics_history.clone()));
        let fallback_quality_summary = latest_quality_metrics
            .as_ref()
            .map(|metrics| ResolvedQualityMetricsSource::build_summary(metrics, true));
        let resolved_quality_context = resolve_generation_quality_runtime_context_for_seed(
            "chapter",
            quality_metrics_summary_state.as_ref(),
            quality_metrics_history_value.as_ref(),
            latest_quality_metrics.as_ref(),
            fallback_quality_summary.as_ref(),
            MAX_CHAPTER_QUALITY_METRICS_HISTORY,
        );
        let merged_quality_metrics_summary = resolved_quality_context
            .quality_metrics_summary
            .clone()
            .map(|mut summary| {
                let merged_quality_history_context = resolved_quality_context
                    .quality_history_context
                    .as_ref()
                    .filter(|value| value.is_object())
                    .cloned()
                    .or_else(|| {
                        latest_quality_metrics
                            .as_ref()
                            .and_then(|latest_quality_metrics| {
                                let fallback_quality_summary =
                                    ResolvedQualityMetricsSource::build_summary(
                                        latest_quality_metrics,
                                        true,
                                    );
                                let merged_quality_history_context =
                                    merge_generation_quality_history_context(
                                        &summary,
                                        Some(&fallback_quality_summary),
                                    );
                                merged_quality_history_context
                                    .is_object()
                                    .then_some(merged_quality_history_context)
                            })
                    });
                if let Some(summary_object) = summary.as_object_mut() {
                    if let Some(quality_history_context) = merged_quality_history_context {
                        summary_object.insert(
                            "quality_runtime_context".to_string(),
                            quality_history_context,
                        );
                    }
                }
                summary
            });

        Self {
            latest_quality_metrics,
            history_id,
            generated_at,
            quality_metrics_summary: merged_quality_metrics_summary,
            quality_metrics_history: quality_metrics_history_value,
            quality_metrics_summary_state: resolved_quality_context.quality_metrics_summary_state,
        }
    }

    fn into_payload(self, chapter_id: &str) -> Value {
        let ChapterQualityMetricsFragments {
            latest_quality_metrics,
            history_id,
            generated_at,
            quality_metrics_summary,
            quality_metrics_history: _,
            quality_metrics_summary_state: _,
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

#[derive(Debug, Clone)]
struct HistoryQualityMetricsRecord {
    metrics: Value,
    generated_at: Option<NaiveDateTime>,
}

fn parse_history_quality_generated_at(payload: &Value) -> Option<NaiveDateTime> {
    payload
        .get("generated_at")
        .and_then(Value::as_str)
        .and_then(|value| NaiveDateTime::parse_from_str(value.trim(), "%Y-%m-%dT%H:%M:%S").ok())
}

impl ResolvedQualityMetricsSource {
    fn resolve(
        histories: &[generation_history::Model],
        candidate_attempt: Option<&chapter_draft_attempt::Model>,
    ) -> Self {
        if let Some(attempt) = candidate_attempt {
            if let Some(metrics) = attempt.quality_metrics.clone().filter(Value::is_object) {
                return Self {
                    metrics: Some(metrics),
                    source_history_id: Some(attempt.id.clone()),
                    generated_at: attempt.created_at,
                };
            }
        }

        if let Some((history, metrics_record)) = Self::latest_history_metrics(histories) {
            return Self {
                metrics: Some(metrics_record.metrics),
                source_history_id: Some(history.id.clone()),
                generated_at: metrics_record.generated_at.or(history.created_at),
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
    ) -> Option<(&generation_history::Model, HistoryQualityMetricsRecord)> {
        histories.iter().find_map(|history| {
            Self::history_metrics(history).map(|metrics_record| (history, metrics_record))
        })
    }

    fn history_metrics(history: &generation_history::Model) -> Option<HistoryQualityMetricsRecord> {
        history.generated_content.as_ref().and_then(|content| {
            serde_json::from_str::<Value>(content)
                .ok()
                .and_then(|payload| {
                    normalize_generated_history_quality_metrics(&payload).map(|metrics| {
                        HistoryQualityMetricsRecord {
                            metrics,
                            generated_at: parse_history_quality_generated_at(&payload),
                        }
                    })
                })
        })
    }

    fn collect_quality_metrics_history(
        histories: &[generation_history::Model],
        candidate_attempt: Option<&chapter_draft_attempt::Model>,
    ) -> Vec<Value> {
        let mut quality_metrics_history = histories
            .iter()
            .rev()
            .filter_map(|history| Self::history_metrics(history).map(|record| record.metrics))
            .collect::<Vec<_>>();

        if let Some(candidate_metrics) =
            candidate_attempt.and_then(|attempt| attempt.quality_metrics.clone())
        {
            quality_metrics_history.push(candidate_metrics);
        }

        if quality_metrics_history.len() > MAX_CHAPTER_QUALITY_METRICS_HISTORY {
            quality_metrics_history = quality_metrics_history
                .split_off(quality_metrics_history.len() - MAX_CHAPTER_QUALITY_METRICS_HISTORY);
        }

        quality_metrics_history
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

pub(crate) fn build_quality_metrics_summary_from_metrics(
    metrics: &Value,
    include_runtime_context: bool,
) -> Value {
    ResolvedQualityMetricsSource::build_summary(metrics, include_runtime_context)
}

pub fn build_chapter_analysis_quality_fragments(
    histories: &[generation_history::Model],
    candidate_attempt: Option<&chapter_draft_attempt::Model>,
) -> ChapterAnalysisQualityFragments {
    let quality_metrics_history =
        ResolvedQualityMetricsSource::collect_quality_metrics_history(histories, candidate_attempt);

    ChapterAnalysisQualityFragments::from_resolved_source(
        ResolvedQualityMetricsSource::resolve(histories, candidate_attempt),
        quality_metrics_history,
    )
}

pub fn build_chapter_quality_metrics_fragments(
    histories: &[generation_history::Model],
    candidate_attempt: Option<&chapter_draft_attempt::Model>,
) -> ChapterQualityMetricsFragments {
    ChapterQualityMetricsFragments::from_resolved_source(
        ResolvedQualityMetricsSource::resolve(histories, candidate_attempt),
        ResolvedQualityMetricsSource::collect_quality_metrics_history(histories, candidate_attempt),
    )
}

pub async fn load_latest_quality_metric_records_for_chapter_ids(
    db: &DatabaseConnection,
    chapter_ids: &[String],
) -> Result<HashMap<String, LatestQualityMetricRecord>, String> {
    let normalized_ids = chapter_ids
        .iter()
        .map(|chapter_id| chapter_id.trim().to_string())
        .filter(|chapter_id| !chapter_id.is_empty())
        .collect::<Vec<_>>();
    if normalized_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let histories = generation_history::Entity::find()
        .filter(generation_history::Column::ChapterId.is_in(normalized_ids.clone()))
        .order_by_desc(generation_history::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let mut records_by_chapter = HashMap::new();
    let mut seen = HashSet::new();
    for history in histories {
        let Some(chapter_id) = history.chapter_id.clone() else {
            continue;
        };
        if !seen.insert(chapter_id.clone()) {
            continue;
        }
        let Some(record) = ResolvedQualityMetricsSource::history_metrics(&history) else {
            continue;
        };
        records_by_chapter.insert(
            chapter_id.clone(),
            LatestQualityMetricRecord {
                chapter_id,
                latest_quality_metrics: record.metrics,
                history_id: history.id,
                generated_at: record
                    .generated_at
                    .or(history.created_at)
                    .map(|value| value.format("%Y-%m-%dT%H:%M:%S").to_string()),
                generated_at_dt: record.generated_at.or(history.created_at),
            },
        );
        if records_by_chapter.len() >= normalized_ids.len() {
            break;
        }
    }

    Ok(records_by_chapter)
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
fn build_chapter_quality_metrics_query_owner_contract() -> Value {
    json!({
        "owner": "chapter_quality_metrics_query_service",
        "scope": "chapter_quality_metrics_fragments_latest_records_and_owned_payload_owner",
        "python_source_map": [
            "backend/migrator_app/models/generation_history.py",
            "backend/migrator_app/models/chapter_draft_attempt.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_quality_metrics_query_service.rs",
            "backend-rs/src/services/chapter_query_service.rs",
            "backend-rs/src/services/chapter_analysis_service.rs",
            "backend-rs/src/services/chapter_generation_history_payload_service.rs",
            "backend-rs/src/api/chapter_crud_routes.rs",
            "backend-rs/src/api/chapter_analysis_routes.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_chapter_analysis_quality_fragments",
                "build_chapter_quality_metrics_fragments",
                "load_latest_quality_metric_records_for_chapter_ids",
                "load_chapter_quality_metrics_payload",
                "load_owned_chapter_quality_metrics_payload"
            ],
            "quality_summary_owner": "build_quality_metrics_summary_from_metrics",
            "history_parse_owner": "ResolvedQualityMetricsSource",
            "owned_access_boundary": "load_accessible_chapter"
        },
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-chapter-crud-owner",
                "phase5-chapter-analysis-owner"
            ],
            "chapter_crud_manifest_probe_count": 13,
            "chapter_analysis_manifest_probe_count": 8,
            "rust_manifest_probe_count": 21,
            "python_fallback_probe_count": 0,
            "aggregate_owner_package": [
                "chapter_service",
                "chapter_query_service",
                "chapter_access_service",
                "chapter_quality_metrics_query_service"
            ],
            "default_python_module_consumers": [
                "backend/tests/test_support/database_test_support.py"
            ],
            "dedicated_python_regression_surfaces": [
                "backend/tests/test_api/test_chapters.py",
                "backend/tests/test_api/test_chapters_batch_generation.py",
                "backend/tests/test_api/test_chapters_quality_views.py",
                "backend/tests/test_api/test_chapters_stream_routes.py"
            ],
            "shared_test_support_consumers": [
                "backend/tests/test_support/chapter_generation_history_test_support.py",
                "backend/tests/test_support/chapter_quality_metrics_query_test_support.py"
            ],
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "chapter quality metrics owner is rust_only and the surviving generation_history and chapter_draft_attempt python files now remain shared metadata registration and regression references only",
            "status": "rust_chapter_quality_metrics_query_service_owner_shared_model_source_maps_only"
        },
        "validation_boundary": [
            "cargo test chapter_quality_metrics_query_service --manifest-path backend-rs/Cargo.toml",
            "cargo test chapter_query_service --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-chapter-crud-owner",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-chapter-analysis-owner"
        ],
        "rollback_boundary": "chapter_quality_metrics_query_service_is_rust_owned_surviving_generation_history_and_chapter_draft_attempt_python_files_remain_shared_metadata_registration_and_regression_reference_only"
    })
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
        build_chapter_quality_metrics_query_owner_contract, ChapterQualityMetricsFragments,
        LoadChapterQualityMetricsPayloadError, ResolvedQualityMetricsSource,
    };
    use crate::services::chapter_generation_history_payload_service::build_generated_chapter_history_payload_with_quality_metrics;

    fn history_payload(id: &str, payload: Value) -> generation_history::Model {
        generation_history::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            prompt: None,
            generated_content: Some(payload.to_string()),
            model: None,
            tokens_used: None,
            generation_time: None,
            created_at: Some(
                NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
                    .expect("test datetime should parse"),
            ),
        }
    }

    #[test]
    fn should_publish_chapter_quality_metrics_query_owner_contract() {
        let contract = build_chapter_quality_metrics_query_owner_contract();

        assert_eq!(contract["owner"], "chapter_quality_metrics_query_service");
        assert_eq!(
            contract["python_source_map"][0],
            "backend/migrator_app/models/generation_history.py"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(
            contract["python_source_map"][1],
            "backend/migrator_app/models/chapter_draft_attempt.py"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][4],
            "load_owned_chapter_quality_metrics_payload"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-chapter-crud-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][1],
            "phase5-chapter-analysis-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            21
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["default_python_module_consumers"],
            json!(["backend/tests/test_support/database_test_support.py"])
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["shared_test_support_consumers"],
            json!([
                "backend/tests/test_support/chapter_generation_history_test_support.py",
                "backend/tests/test_support/chapter_quality_metrics_query_test_support.py"
            ])
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["remaining_cutover_gate"],
            "chapter quality metrics owner is rust_only and the surviving generation_history and chapter_draft_attempt python files now remain shared metadata registration and regression references only"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_chapter_quality_metrics_query_service_owner_shared_model_source_maps_only"
        );
        assert_eq!(
            contract["rollback_boundary"],
            "chapter_quality_metrics_query_service_is_rust_owned_surviving_generation_history_and_chapter_draft_attempt_python_files_remain_shared_metadata_registration_and_regression_reference_only"
        );
    }

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
            Some(&json!({
                "summary": "ok",
                "repair_targets": [],
                "preserve_strengths": [],
                "focus_areas": []
            }))
        );
        assert_eq!(
            fragments
                .quality_metrics_summary
                .as_ref()
                .and_then(|value| value.get("chapter_count")),
            Some(&json!(1))
        );
        assert_eq!(
            fragments
                .quality_metrics_summary
                .as_ref()
                .and_then(|value| value.get("quality_runtime_context"))
                .and_then(|value| value.get("scope")),
            Some(&json!("batch"))
        );
    }

    #[test]
    fn should_build_chapter_analysis_quality_summary_from_history_aggregation() {
        let histories = vec![
            history(
                "history-1",
                Some(json!({
                    "overall_score": 88.0,
                    "engagement_score": 81.0,
                    "coherence_score": 79.0,
                    "pacing_score": 8.2,
                    "repair_guidance": {
                        "summary": "保持冲突推进",
                        "focus_areas": ["conflict"]
                    },
                    "quality_gate": {
                        "status": "pass",
                        "decision": "pass"
                    }
                })),
            ),
            history(
                "history-2",
                Some(json!({
                    "overall_score": 82.0,
                    "engagement_score": 76.0,
                    "coherence_score": 74.0,
                    "pacing_score": 7.6,
                    "repair_guidance": {
                        "summary": "补强节奏衔接",
                        "repair_targets": ["压缩说明"],
                        "focus_areas": ["pacing"]
                    },
                    "quality_gate": {
                        "status": "repairable",
                        "decision": "auto_repair",
                        "failed_metrics": [{"label": "Pacing"}]
                    }
                })),
            ),
        ];

        let fragments = build_chapter_analysis_quality_fragments(&histories, None);
        let summary = fragments
            .quality_metrics_summary
            .as_ref()
            .expect("quality metrics summary");

        assert_eq!(summary["chapter_count"], 2);
        assert_eq!(summary["overall_score"], 88.0);
        assert_eq!(summary["avg_overall_score"], 85.0);
        assert_eq!(summary["avg_engagement_score"], 78.5);
        assert_eq!(summary["avg_coherence_score"], 76.5);
        assert_eq!(summary["avg_pacing_score"], 7.9);
        assert_eq!(summary["overall_score_delta"], 6.0);
        assert_eq!(summary["overall_score_trend"], "rising");
        assert_eq!(summary["quality_gate"]["decision"], "pass");
        assert_eq!(summary["quality_gate_counts"]["pass"], 1);
        assert_eq!(summary["quality_gate_counts"]["auto_repair"], 1);
        assert_eq!(summary["recent_failed_metric_counts"]["Pacing"], 1);
        assert_eq!(summary["recent_focus_areas"], json!(["conflict", "pacing"]));
        assert_eq!(summary["repair_guidance"]["summary"], "保持冲突推进");
        assert_eq!(summary["quality_runtime_context"]["scope"], "batch");
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
            Some(&json!({
                "task_id": "task-1",
                "scope": "chapter",
                "recent_metrics": [{
                    "history_index": 0,
                    "overall_score": Value::Null,
                    "repair_guidance": {
                        "summary": "ok"
                    },
                    "quality_gate": {
                        "decision": "pass"
                    }
                }]
            }))
        );
        assert_eq!(
            fragments.quality_metrics_history,
            Some(json!([quality_metrics()]))
        );
        assert_eq!(
            fragments
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("scope")),
            Some(&json!("chapter"))
        );
        assert_eq!(
            fragments
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("chapter_count")),
            Some(&json!(1))
        );
    }

    #[test]
    fn should_build_chapter_quality_metrics_summary_from_history_aggregation() {
        let histories = vec![
            history(
                "history-1",
                Some(json!({
                    "overall_score": 88.0,
                    "outline_alignment_rate": 86.0,
                    "dialogue_naturalness_rate": 78.0,
                    "pacing_score": 8.1,
                    "repair_guidance": {
                        "summary": "保持高压推进",
                        "focus_areas": ["conflict"]
                    },
                    "quality_gate": {
                        "status": "pass",
                        "decision": "pass"
                    },
                    "quality_runtime_context": {
                        "story_focus": "advance_plot",
                        "quality_preset": "immersive",
                        "current_chapter_number": 7,
                        "chapter_count": 12
                    }
                })),
            ),
            history(
                "history-2",
                Some(json!({
                    "overall_score": 82.0,
                    "outline_alignment_rate": 80.0,
                    "dialogue_naturalness_rate": 74.0,
                    "pacing_score": 7.5,
                    "repair_guidance": {
                        "summary": "补强节奏衔接",
                        "repair_targets": ["压缩说明"],
                        "focus_areas": ["pacing"]
                    },
                    "quality_gate": {
                        "status": "repairable",
                        "decision": "auto_repair",
                        "failed_metrics": [{"label": "Pacing"}]
                    },
                    "quality_runtime_context": {
                        "story_focus": "advance_plot",
                        "quality_preset": "immersive",
                        "current_chapter_number": 6,
                        "chapter_count": 12
                    }
                })),
            ),
        ];

        let fragments = build_chapter_quality_metrics_fragments(&histories, None);
        let summary = fragments
            .quality_metrics_summary
            .as_ref()
            .expect("quality metrics summary");

        assert_eq!(summary["chapter_count"], 2);
        assert_eq!(summary["overall_score"], 88.0);
        assert_eq!(summary["avg_overall_score"], 85.0);
        assert_eq!(summary["avg_pacing_score"], 7.8);
        assert_eq!(summary["overall_score_delta"], 6.0);
        assert_eq!(summary["overall_score_trend"], "rising");
        assert_eq!(summary["quality_gate"]["status"], "pass");
        assert_eq!(summary["quality_gate_counts"]["pass"], 1);
        assert_eq!(summary["quality_gate_counts"]["auto_repair"], 1);
        assert_eq!(summary["recent_failed_metric_counts"]["Pacing"], 1);
        assert_eq!(summary["recent_focus_areas"], json!(["conflict", "pacing"]));
        assert_eq!(summary["repair_guidance"]["summary"], "保持高压推进");
        assert_eq!(
            summary["quality_runtime_context"]["story_focus"],
            "advance_plot"
        );
        assert_eq!(
            summary["quality_runtime_context"]["current_chapter_number"],
            7
        );
        assert_eq!(summary["quality_runtime_context"]["scope"], "chapter");
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
            quality_metrics_history: None,
            quality_metrics_summary_state: None,
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
            quality_metrics_history: None,
            quality_metrics_summary_state: None,
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
    fn should_build_chapter_quality_metrics_fragments_with_existing_history_state() {
        let histories = vec![
            history(
                "history-1",
                Some(json!({
                    "overall_score": 81,
                    "quality_gate": {"decision": "repair"},
                    "repair_guidance": {"summary": "先压缩说明"}
                })),
            ),
            history(
                "history-2",
                Some(json!({
                    "overall_score": 86,
                    "quality_gate": {"decision": "passed"},
                    "repair_guidance": {"summary": "保持节奏"}
                })),
            ),
        ];

        let fragments = build_chapter_quality_metrics_fragments(&histories, None);

        assert_eq!(
            fragments.quality_metrics_history,
            Some(json!([
                {
                    "overall_score": 86,
                    "quality_gate": {"decision": "passed"},
                    "repair_guidance": {"summary": "保持节奏"}
                },
                {
                    "overall_score": 81,
                    "quality_gate": {"decision": "repair"},
                    "repair_guidance": {"summary": "先压缩说明"}
                }
            ]))
        );
        assert_eq!(
            fragments
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("chapter_count")),
            Some(&json!(2))
        );
        assert_eq!(
            fragments
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("first_overall_score")),
            Some(&json!(86.0))
        );
        assert_eq!(
            fragments
                .quality_metrics_summary_state
                .as_ref()
                .and_then(|value| value.get("last_overall_score")),
            Some(&json!(81.0))
        );
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

    #[test]
    fn should_ignore_history_payload_when_quality_metrics_is_null() {
        let histories = vec![history_payload(
            "history-1",
            json!({
                "log_type": "chapter_generation_quality_v1",
                "quality_metrics": Value::Null,
                "generated_at": "2026-05-20T08:30:15"
            }),
        )];

        let resolved = ResolvedQualityMetricsSource::resolve(&histories, None);

        assert!(resolved.metrics.is_none());
        assert!(resolved.source_history_id.is_none());
        assert!(resolved.generated_at.is_none());
    }

    #[test]
    fn should_merge_history_runtime_snapshot_into_quality_runtime_context() {
        let histories = vec![history_payload(
            "history-1",
            json!({
                "quality_metrics": {
                    "overall_score": 88,
                    "quality_runtime_context": {
                        "source": "plot_analysis",
                        "plot_stage": "climax"
                    },
                    "quality_gate": {"decision": "passed"},
                    "repair_guidance": {"summary": "ok"}
                },
                "story_runtime_snapshot": {
                    "creative_mode": "hook",
                    "plot_stage": "development",
                    "target_word_count": 2400
                }
            }),
        )];

        let resolved = ResolvedQualityMetricsSource::resolve(&histories, None);
        let metrics = resolved.metrics.expect("metrics payload");

        assert_eq!(metrics["quality_runtime_context"]["creative_mode"], "hook");
        assert_eq!(
            metrics["quality_runtime_context"]["target_word_count"],
            2400
        );
        assert_eq!(metrics["quality_runtime_context"]["plot_stage"], "climax");
        assert_eq!(
            metrics["quality_runtime_context"]["source"],
            "plot_analysis"
        );
    }

    #[test]
    fn should_restore_story_runtime_context_and_generated_at_from_history_contract_payload() {
        let histories = vec![history_payload(
            "history-1",
            json!({
                "quality_metrics": {
                    "overall_score": 86,
                    "quality_gate": {"decision": "passed"},
                    "repair_guidance": {"summary": "保持推进"}
                },
                "story_runtime_contract": {
                    "guidance": {
                        "story_focus": "advance_plot",
                        "quality_preset": "immersive"
                    },
                    "blueprint": {
                        "long_term_goal": "追回主线伏笔",
                        "chapter_count": 12,
                        "current_chapter_number": 7,
                        "target_word_count": 3200,
                        "character_focus_names": ["沈砚"],
                        "foreshadow_payoff_plan": ["回收旧约定"],
                        "character_state_ledger": [],
                        "relationship_state_ledger": [],
                        "foreshadow_state_ledger": [],
                        "organization_state_ledger": [],
                        "career_state_ledger": []
                    }
                },
                "generated_at": "2026-05-20T08:30:15"
            }),
        )];

        let fragments = build_chapter_quality_metrics_fragments(&histories, None);
        let metrics = fragments
            .latest_quality_metrics
            .as_ref()
            .expect("latest metrics");

        assert_eq!(
            fragments.generated_at,
            Some("2026-05-20T08:30:15".to_string())
        );
        assert_eq!(
            metrics["story_runtime_contract"]["guidance"]["story_focus"],
            "advance_plot"
        );
        assert_eq!(
            metrics["quality_runtime_context"]["story_focus"],
            "advance_plot"
        );
        assert_eq!(
            metrics["quality_runtime_context"]["quality_preset"],
            "immersive"
        );
        assert_eq!(
            metrics["quality_runtime_context"]["story_long_term_goal"],
            "追回主线伏笔"
        );
        assert_eq!(
            metrics["quality_runtime_context"]["current_chapter_number"],
            7
        );
        assert_eq!(
            fragments
                .quality_metrics_summary
                .as_ref()
                .and_then(|value| value.get("quality_runtime_context"))
                .and_then(|value| value.get("story_focus")),
            Some(&json!("advance_plot"))
        );
        assert_eq!(
            fragments
                .quality_metrics_summary
                .as_ref()
                .and_then(|value| value.get("quality_runtime_context"))
                .and_then(|value| value.get("scope")),
            Some(&json!("chapter"))
        );
    }

    #[test]
    fn should_restore_quality_runtime_context_from_shared_generated_history_owner_payload() {
        let created_at = NaiveDateTime::parse_from_str("2026-05-20T08:30:15", "%Y-%m-%dT%H:%M:%S")
            .expect("created_at");
        let generated_payload = build_generated_chapter_history_payload_with_quality_metrics(
            "正文",
            Some(&json!({
                "overall_score": 84,
                "quality_gate": {
                    "decision": "passed"
                },
                "repair_guidance": {
                    "summary": "保持推进"
                },
                "story_runtime_contract": {
                    "guidance": {
                        "creative_mode": "hook",
                        "story_focus": "advance_plot"
                    },
                    "blueprint": {
                        "long_term_goal": "追回主线伏笔",
                        "chapter_count": 10,
                        "current_chapter_number": 4,
                        "target_word_count": 2600,
                        "character_focus_names": ["沈砚"],
                        "foreshadow_payoff_plan": ["回收旧约定"],
                        "character_state_ledger": [],
                        "relationship_state_ledger": [],
                        "foreshadow_state_ledger": [],
                        "organization_state_ledger": [],
                        "career_state_ledger": []
                    }
                }
            })),
            Some(&json!({
                "execution_path": "rust_candidate_executor",
                "rollback_boundary": "python_candidate_executor_fallback"
            })),
            false,
            Some("manual_review"),
            created_at,
        );

        let histories = vec![history_payload("history-1", generated_payload)];
        let fragments = build_chapter_quality_metrics_fragments(&histories, None);
        let metrics = fragments
            .latest_quality_metrics
            .as_ref()
            .expect("latest metrics");

        assert_eq!(
            metrics["story_runtime_contract"]["guidance"]["creative_mode"],
            "hook"
        );
        assert_eq!(
            metrics["quality_runtime_context"]["story_focus"],
            "advance_plot"
        );
        assert_eq!(
            metrics["quality_runtime_context"]["current_chapter_number"],
            4
        );
        assert_eq!(
            metrics["quality_runtime_context"]["target_word_count"],
            2600
        );
        assert_eq!(
            fragments.generated_at,
            Some("2026-05-20T08:30:15".to_string())
        );
    }
}
