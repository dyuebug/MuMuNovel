use chrono::NaiveDateTime;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};

use crate::models::{chapter, generation_history, plot_analysis, story_memory};
use crate::services::chapter_access_service::load_accessible_chapter;
use crate::services::chapter_analysis_read_context_service::load_chapter_analysis_read_context;
use crate::services::chapter_analysis_service::ChapterAnalysisQueryContextError;
use crate::services::chapter_draft_view_payload_service::build_chapter_draft_analysis_view_fragments;
use crate::services::chapter_draft_view_payload_service::ChapterDraftAnalysisViewFragments;
use crate::services::chapter_quality_metrics_query_service::build_chapter_analysis_quality_fragments;
use crate::services::chapter_quality_metrics_query_service::ChapterAnalysisQualityFragments;

pub enum LoadChapterAnalysisViewPayloadError {
    Context(ChapterAnalysisQueryContextError),
    AnalysisNotFound,
}

pub struct ChapterAnalysisCheckerFragments {
    pub checker_result: Option<Value>,
    pub checker_created_at: Option<String>,
}

impl ChapterAnalysisCheckerFragments {
    fn from_histories(histories: &[generation_history::Model]) -> Self {
        let checker_result = histories.iter().find_map(Self::parse_result);
        let checker_created_at = histories.iter().find_map(|history| {
            Self::parse_result(history)?;
            format_datetime(history.created_at)
        });

        Self {
            checker_result,
            checker_created_at,
        }
    }

    fn parse_result(history: &generation_history::Model) -> Option<Value> {
        history.generated_content.as_ref().and_then(|content| {
            serde_json::from_str::<Value>(content)
                .ok()
                .and_then(|payload| {
                    if payload.get("log_type").and_then(Value::as_str)
                        == Some("chapter_text_checker_v1")
                    {
                        payload.get("checker_result").cloned()
                    } else {
                        None
                    }
                })
        })
    }
}

fn format_datetime(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string())
}

fn value_or_null(value: Option<Value>) -> Value {
    value.unwrap_or(Value::Null)
}

fn build_chapter_analysis_view_payload(
    chapter: &chapter::Model,
    analysis: plot_analysis::Model,
    memories: Vec<story_memory::Model>,
    checker_fragments: ChapterAnalysisCheckerFragments,
    draft_fragments: ChapterDraftAnalysisViewFragments,
    quality_fragments: ChapterAnalysisQualityFragments,
    created_at: String,
    analysis_created_at: Option<String>,
) -> Value {
    json!({
        "chapter_id": chapter.id,
        "analysis": {
            "id": analysis.id,
            "project_id": analysis.project_id,
            "chapter_id": analysis.chapter_id,
            "plot_stage": analysis.plot_stage,
            "conflict_level": analysis.conflict_level,
            "conflict_types": value_or_null(analysis.conflict_types),
            "emotional_tone": analysis.emotional_tone,
            "emotional_intensity": analysis.emotional_intensity,
            "emotional_curve": value_or_null(analysis.emotional_curve),
            "hooks": value_or_null(analysis.hooks),
            "hooks_count": analysis.hooks_count,
            "hooks_avg_strength": analysis.hooks_avg_strength,
            "foreshadows": value_or_null(analysis.foreshadows),
            "foreshadows_planted": analysis.foreshadows_planted,
            "foreshadows_resolved": analysis.foreshadows_resolved,
            "plot_points": value_or_null(analysis.plot_points),
            "plot_points_count": analysis.plot_points_count,
            "character_states": value_or_null(analysis.character_states),
            "scenes": value_or_null(analysis.scenes),
            "pacing": analysis.pacing,
            "overall_quality_score": analysis.overall_quality_score,
            "pacing_score": analysis.pacing_score,
            "engagement_score": analysis.engagement_score,
            "coherence_score": analysis.coherence_score,
            "analysis_report": analysis.analysis_report,
            "suggestions": value_or_null(analysis.suggestions),
            "word_count": analysis.word_count,
            "dialogue_ratio": analysis.dialogue_ratio,
            "description_ratio": analysis.description_ratio,
            "created_at": analysis_created_at,
        },
        "memories": memories.into_iter().map(|memory| json!({
            "id": memory.id,
            "type": memory.memory_type,
            "title": memory.title,
            "content": memory.content,
            "importance": memory.importance_score,
            "tags": value_or_null(memory.tags),
            "is_foreshadow": memory.is_foreshadow != 0,
            "position": memory.chapter_position,
            "related_characters": value_or_null(memory.related_characters),
        })).collect::<Vec<_>>(),
        "checker_result": checker_fragments.checker_result,
        "checker_created_at": checker_fragments.checker_created_at,
        "auto_revision_draft": draft_fragments.auto_revision_draft,
        "candidate_draft": draft_fragments.candidate_draft,
        "quality_metrics": quality_fragments.quality_metrics,
        "quality_metrics_summary": quality_fragments.quality_metrics_summary,
        "created_at": created_at,
    })
}

pub async fn load_chapter_analysis_view_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
) -> Result<Value, LoadChapterAnalysisViewPayloadError> {
    let chapter_id = chapter.id.clone();

    let analysis = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ChapterId.eq(&chapter_id))
        .order_by_desc(plot_analysis::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| {
            LoadChapterAnalysisViewPayloadError::Context(
                ChapterAnalysisQueryContextError::Internal(error.to_string()),
            )
        })?
        .ok_or(LoadChapterAnalysisViewPayloadError::AnalysisNotFound)?;

    let memories = story_memory::Entity::find()
        .filter(story_memory::Column::ChapterId.eq(Some(chapter_id.clone())))
        .order_by_desc(story_memory::Column::ImportanceScore)
        .all(db)
        .await
        .map_err(|error| {
            LoadChapterAnalysisViewPayloadError::Context(
                ChapterAnalysisQueryContextError::Internal(error.to_string()),
            )
        })?;
    let read_context = load_chapter_analysis_read_context(db, &chapter_id)
        .await
        .map_err(|error| {
            LoadChapterAnalysisViewPayloadError::Context(
                ChapterAnalysisQueryContextError::Internal(error),
            )
        })?;

    let checker_fragments =
        ChapterAnalysisCheckerFragments::from_histories(&read_context.histories);
    let draft_fragments = build_chapter_draft_analysis_view_fragments(
        &read_context.histories,
        read_context.candidate_attempt.as_ref(),
        chapter.updated_at,
    );
    let quality_fragments = build_chapter_analysis_quality_fragments(
        &read_context.histories,
        read_context.candidate_attempt.as_ref(),
    );
    let analysis_created_at = format_datetime(analysis.created_at);
    let created_at = analysis_created_at
        .clone()
        .or_else(|| format_datetime(chapter.updated_at))
        .unwrap_or_default();

    Ok(build_chapter_analysis_view_payload(
        chapter,
        analysis,
        memories,
        checker_fragments,
        draft_fragments,
        quality_fragments,
        created_at,
        analysis_created_at,
    ))
}

pub async fn load_owned_chapter_analysis_view_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadChapterAnalysisViewPayloadError> {
    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(|error| {
            LoadChapterAnalysisViewPayloadError::Context(ChapterAnalysisQueryContextError::Chapter(
                error,
            ))
        })?;

    load_chapter_analysis_view_payload(db, &chapter).await
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use serde_json::{json, Value};

    use crate::models::{chapter, generation_history, plot_analysis, story_memory};
    use crate::services::chapter_draft_view_payload_service::ChapterDraftAnalysisViewFragments;
    use crate::services::chapter_quality_metrics_query_service::ChapterAnalysisQualityFragments;

    use super::{
        build_chapter_analysis_view_payload, value_or_null, ChapterAnalysisCheckerFragments,
    };

    fn test_datetime() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse")
    }

    fn chapter_model() -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 1,
            title: "测试章节".to_string(),
            content: Some("正文".to_string()),
            summary: None,
            word_count: 2,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: test_datetime(),
            updated_at: Some(test_datetime()),
        }
    }

    fn plot_analysis_model() -> plot_analysis::Model {
        plot_analysis::Model {
            id: "analysis-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: "chapter-1".to_string(),
            plot_stage: Some("climax".to_string()),
            conflict_level: Some(4),
            conflict_types: Some(json!(["inner"])),
            emotional_tone: Some("tense".to_string()),
            emotional_intensity: Some(0.8),
            emotional_curve: None,
            hooks: None,
            hooks_count: 1,
            hooks_avg_strength: Some(0.7),
            foreshadows: None,
            foreshadows_planted: 2,
            foreshadows_resolved: 1,
            plot_points: Some(json!(["point"])),
            plot_points_count: 1,
            character_states: None,
            scenes: None,
            pacing: Some("fast".to_string()),
            overall_quality_score: Some(88.0),
            pacing_score: Some(86.0),
            engagement_score: Some(87.0),
            coherence_score: Some(89.0),
            analysis_report: Some("report".to_string()),
            suggestions: Some(json!(["suggestion"])),
            word_count: Some(1200),
            dialogue_ratio: Some(0.3),
            description_ratio: Some(0.7),
            created_at: Some(test_datetime()),
        }
    }

    fn memory_model() -> story_memory::Model {
        story_memory::Model {
            id: "memory-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            memory_type: "plot".to_string(),
            title: Some("伏笔".to_string()),
            content: "记忆内容".to_string(),
            full_context: None,
            related_characters: None,
            related_locations: None,
            tags: Some(json!(["tag"])),
            importance_score: Some(0.9),
            story_timeline: 1,
            chapter_position: 20,
            text_length: 4,
            is_foreshadow: 1,
            foreshadow_resolved_at: None,
            foreshadow_strength: None,
            vector_id: None,
            embedding_model: None,
            created_at: Some(test_datetime()),
            updated_at: Some(test_datetime()),
        }
    }

    fn history(id: &str, generated_content: Option<String>) -> generation_history::Model {
        generation_history::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            prompt: None,
            generated_content,
            model: None,
            tokens_used: None,
            generation_time: None,
            created_at: Some(test_datetime()),
        }
    }

    #[test]
    fn should_convert_optional_value_to_json_null() {
        assert_eq!(value_or_null(Some(json!({"k": "v"}))), json!({"k": "v"}));
        assert_eq!(value_or_null(None), Value::Null);
    }

    #[test]
    fn should_build_chapter_analysis_view_payload_with_fragments_and_memories() {
        let chapter = chapter_model();
        let payload = build_chapter_analysis_view_payload(
            &chapter,
            plot_analysis_model(),
            vec![memory_model()],
            ChapterAnalysisCheckerFragments {
                checker_result: Some(json!({"score": 91})),
                checker_created_at: Some("2026-05-17T12:30:45".to_string()),
            },
            ChapterDraftAnalysisViewFragments {
                auto_revision_draft: Some(json!({"history_id": "history-1"})),
                candidate_draft: None,
            },
            ChapterAnalysisQualityFragments {
                quality_metrics: Some(json!({"score": 88})),
                quality_metrics_summary: Some(json!({"summary": "ok"})),
            },
            "2026-05-17T12:30:45".to_string(),
            Some("2026-05-17T12:30:45".to_string()),
        );

        assert_eq!(payload["chapter_id"], "chapter-1");
        assert_eq!(payload["analysis"]["id"], "analysis-1");
        assert_eq!(payload["analysis"]["conflict_types"], json!(["inner"]));
        assert!(payload["analysis"]["emotional_curve"].is_null());
        assert_eq!(payload["memories"][0]["id"], "memory-1");
        assert_eq!(payload["memories"][0]["is_foreshadow"], true);
        assert!(payload["memories"][0]["related_characters"].is_null());
        assert_eq!(payload["checker_result"], json!({"score": 91}));
        assert_eq!(payload["auto_revision_draft"]["history_id"], "history-1");
        assert!(payload["candidate_draft"].is_null());
        assert_eq!(payload["quality_metrics"], json!({"score": 88}));
        assert_eq!(payload["quality_metrics_summary"], json!({"summary": "ok"}));
        assert_eq!(payload["created_at"], "2026-05-17T12:30:45");
    }

    #[test]
    fn should_build_checker_fragments_from_first_matching_history() {
        let histories = vec![
            history(
                "unrelated",
                Some(json!({"log_type": "other", "checker_result": {"score": 1}}).to_string()),
            ),
            history(
                "checker",
                Some(
                    json!({
                        "log_type": "chapter_text_checker_v1",
                        "checker_result": {
                            "score": 91,
                            "status": "passed"
                        }
                    })
                    .to_string(),
                ),
            ),
        ];

        let fragments = ChapterAnalysisCheckerFragments::from_histories(&histories);

        assert_eq!(
            fragments.checker_result,
            Some(json!({"score": 91, "status": "passed"}))
        );
        assert_eq!(
            fragments.checker_created_at,
            Some("2026-05-17T12:30:45".to_string())
        );
    }

    #[test]
    fn should_ignore_invalid_or_non_checker_histories() {
        let histories = vec![
            history("invalid-json", Some("{not-json".to_string())),
            history(
                "missing-result",
                Some(json!({"log_type": "chapter_text_checker_v1"}).to_string()),
            ),
        ];

        let fragments = ChapterAnalysisCheckerFragments::from_histories(&histories);

        assert_eq!(fragments.checker_result, None);
        assert_eq!(fragments.checker_created_at, None);
    }

    #[test]
    fn should_skip_checker_created_at_when_matching_history_has_no_created_at() {
        let mut item = history(
            "checker",
            Some(
                json!({
                    "log_type": "chapter_text_checker_v1",
                    "checker_result": {
                        "score": 88
                    }
                })
                .to_string(),
            ),
        );
        item.created_at = None;

        let fragments = ChapterAnalysisCheckerFragments::from_histories(&[item]);

        assert_eq!(fragments.checker_result, Some(json!({"score": 88})));
        assert_eq!(fragments.checker_created_at, None);
    }
}
