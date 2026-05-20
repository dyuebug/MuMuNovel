use serde_json::{json, Value};

use crate::models::{chapter, plot_analysis, story_memory};
use crate::services::chapter_analysis_checker_query_service::ChapterAnalysisCheckerFragments;
use crate::services::chapter_draft_query_service::ChapterDraftAnalysisViewFragments;
use crate::services::chapter_quality_query_service::ChapterAnalysisQualityFragments;

pub fn value_or_null(value: Option<Value>) -> Value {
    value.unwrap_or(Value::Null)
}

pub fn build_chapter_analysis_view_payload(
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

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use serde_json::{json, Value};

    use crate::models::{chapter, plot_analysis, story_memory};
    use crate::services::chapter_analysis_checker_query_service::ChapterAnalysisCheckerFragments;
    use crate::services::chapter_draft_query_service::ChapterDraftAnalysisViewFragments;
    use crate::services::chapter_quality_query_service::ChapterAnalysisQualityFragments;

    use super::{build_chapter_analysis_view_payload, value_or_null};

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

    #[test]
    fn should_convert_optional_value_to_json_null() {
        assert_eq!(value_or_null(Some(json!({"k": "v"}))), json!({"k": "v"}));
        assert_eq!(value_or_null(None), Value::Null);
    }

    #[test]
    fn should_build_chapter_analysis_view_payload_with_fragments_and_memories() {
        let payload = build_chapter_analysis_view_payload(
            &chapter_model(),
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
}
