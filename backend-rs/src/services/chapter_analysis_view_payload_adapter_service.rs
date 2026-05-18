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
