use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder, Set,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{chapter, plot_analysis, regeneration_task};
use crate::services::chapter_regeneration_prepare_service::FullChapterRegenerationStreamRequest;

pub(crate) fn build_chapter_regeneration_task_owner_contract() -> Value {
    json!({
        "owner": "chapter_regeneration_task_service",
        "scope": "full_regeneration_task_persistence_lifecycle_owner",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_regeneration_task_service.rs",
            "backend-rs/src/models/regeneration_task.rs",
            "backend-rs/src/services/chapter_regeneration_stream_workflow_service.rs"
        ],
        "behavior_contract": {
            "create_entrypoints": [
                "build_full_regeneration_task_seed",
                "create_full_regeneration_task"
            ],
            "terminal_entrypoints": [
                "mark_regeneration_task_completed",
                "mark_regeneration_task_failed"
            ],
            "persisted_fields": [
                "id",
                "chapter_id",
                "analysis_id",
                "user_id",
                "project_id",
                "modification_instructions",
                "original_suggestions",
                "selected_suggestion_indices",
                "custom_instructions",
                "style_id",
                "target_word_count",
                "focus_areas",
                "preserve_elements",
                "status",
                "original_content",
                "original_word_count",
                "regenerated_content",
                "regenerated_word_count",
                "version_note",
                "started_at",
                "completed_at",
                "error_message"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_regeneration_task_service",
            "cargo test services::chapter_regeneration_stream_workflow_service",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "rollback_boundary": {
            "python_source_map": "chapter_regeneration_prepare_python_source_map",
            "python_fallback_removal_ready": true,
            "approval_required": "direct regeneration prepare package already closed out; any surviving Python follow-up belongs to separate shared prompt or research owners"
        }
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FullRegenerationTaskSeed {
    pub(crate) task_id: String,
    pub(crate) chapter_id: String,
    pub(crate) analysis_id: Option<String>,
    pub(crate) user_id: String,
    pub(crate) project_id: String,
    pub(crate) modification_instructions: String,
    pub(crate) original_suggestions: Option<Value>,
    pub(crate) selected_suggestion_indices: Option<Value>,
    pub(crate) custom_instructions: Option<String>,
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: i32,
    pub(crate) focus_areas: Option<Value>,
    pub(crate) preserve_elements: Option<Value>,
    pub(crate) original_content: Option<String>,
    pub(crate) original_word_count: Option<i32>,
    pub(crate) version_note: Option<String>,
}

impl FullRegenerationTaskSeed {
    pub(crate) fn into_active_model(self, now: NaiveDateTime) -> regeneration_task::ActiveModel {
        regeneration_task::ActiveModel {
            id: Set(self.task_id),
            chapter_id: Set(self.chapter_id),
            analysis_id: Set(self.analysis_id),
            user_id: Set(self.user_id),
            project_id: Set(self.project_id),
            modification_instructions: Set(self.modification_instructions),
            original_suggestions: Set(self.original_suggestions),
            selected_suggestion_indices: Set(self.selected_suggestion_indices),
            custom_instructions: Set(self.custom_instructions),
            style_id: Set(self.style_id),
            target_word_count: Set(self.target_word_count),
            focus_areas: Set(self.focus_areas),
            preserve_elements: Set(self.preserve_elements),
            status: Set("running".to_string()),
            progress: Set(0),
            error_message: Set(None),
            original_content: Set(self.original_content),
            original_word_count: Set(self.original_word_count),
            regenerated_content: Set(None),
            regenerated_word_count: Set(None),
            version_number: Set(1),
            version_note: Set(self.version_note),
            created_at: Set(Some(now)),
            started_at: Set(Some(now)),
            completed_at: Set(None),
        }
    }
}

pub(crate) fn build_full_regeneration_task_seed(
    chapter: &chapter::Model,
    analysis: Option<&plot_analysis::Model>,
    user_id: &str,
    request: &FullChapterRegenerationStreamRequest,
    resolved_style_id: Option<i32>,
) -> FullRegenerationTaskSeed {
    let preserve_elements = json!({
        "preserve_structure": request.preserve_structure(),
        "preserve_dialogues": request.preserve_dialogues(),
        "preserve_plot_points": request.preserve_plot_points(),
        "preserve_character_traits": request.preserve_character_traits(),
    });

    FullRegenerationTaskSeed {
        task_id: Uuid::new_v4().to_string(),
        chapter_id: chapter.id.clone(),
        analysis_id: analysis.map(|item| item.id.clone()),
        user_id: user_id.to_string(),
        project_id: chapter.project_id.clone(),
        modification_instructions: String::new(),
        original_suggestions: analysis.and_then(|item| item.suggestions.clone()),
        selected_suggestion_indices: Some(json!(request.selected_suggestion_indices())),
        custom_instructions: Some(request.custom_instructions().to_string())
            .filter(|value| !value.is_empty()),
        style_id: resolved_style_id,
        target_word_count: request.target_word_count() as i32,
        focus_areas: Some(json!(request.focus_areas())),
        preserve_elements: Some(preserve_elements),
        original_content: chapter.content.clone(),
        original_word_count: Some(chapter.word_count),
        version_note: request.version_note().map(str::to_string),
    }
}

pub(crate) async fn create_full_regeneration_task(
    db: &DatabaseConnection,
    seed: FullRegenerationTaskSeed,
) -> Result<regeneration_task::Model, String> {
    let now = Utc::now().naive_utc();
    seed.into_active_model(now)
        .insert(db)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn mark_regeneration_task_completed(
    db: &DatabaseConnection,
    task_id: &str,
    regenerated_content: &str,
) -> Result<regeneration_task::Model, String> {
    let task = regeneration_task::Entity::find_by_id(task_id.to_string())
        .one(db)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("regeneration task not found: {task_id}"))?;

    let mut active = task.into_active_model();
    active.status = Set("completed".to_string());
    active.progress = Set(100);
    active.error_message = Set(None);
    active.regenerated_content = Set(Some(regenerated_content.to_string()));
    active.regenerated_word_count = Set(Some(regenerated_content.chars().count() as i32));
    active.completed_at = Set(Some(Utc::now().naive_utc()));
    active.update(db).await.map_err(|error| error.to_string())
}

pub(crate) async fn mark_regeneration_task_failed(
    db: &DatabaseConnection,
    task_id: &str,
    error_message: &str,
) -> Result<regeneration_task::Model, String> {
    let task = regeneration_task::Entity::find_by_id(task_id.to_string())
        .one(db)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("regeneration task not found: {task_id}"))?;

    let mut active = task.into_active_model();
    active.status = Set("failed".to_string());
    active.error_message = Set(Some(error_message.chars().take(500).collect()));
    active.completed_at = Set(Some(Utc::now().naive_utc()));
    active.update(db).await.map_err(|error| error.to_string())
}

pub(crate) async fn load_latest_chapter_analysis(
    db: &DatabaseConnection,
    chapter_id: &str,
    modification_source: &str,
) -> Result<Option<plot_analysis::Model>, String> {
    if !matches!(modification_source, "analysis_suggestions" | "mixed") {
        return Ok(None);
    }

    plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ChapterId.eq(chapter_id.to_string()))
        .order_by_desc(plot_analysis::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        build_chapter_regeneration_task_owner_contract, build_full_regeneration_task_seed,
    };
    use crate::models::{chapter, plot_analysis};
    use crate::services::chapter_regeneration_prepare_service::FullChapterRegenerationStreamRequest;
    use chrono::NaiveDateTime;
    use serde_json::json;

    fn chapter_model() -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            title: "测试章节".to_string(),
            chapter_number: 3,
            content: Some("原始正文".to_string()),
            summary: None,
            word_count: 4,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    fn analysis_model() -> plot_analysis::Model {
        plot_analysis::Model {
            id: "analysis-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: "chapter-1".to_string(),
            plot_stage: None,
            conflict_level: None,
            conflict_types: None,
            emotional_tone: None,
            emotional_intensity: None,
            emotional_curve: None,
            hooks: None,
            hooks_count: 0,
            hooks_avg_strength: None,
            foreshadows: None,
            foreshadows_planted: 0,
            foreshadows_resolved: 0,
            plot_points: None,
            plot_points_count: 0,
            character_states: None,
            scenes: None,
            pacing: None,
            overall_quality_score: None,
            pacing_score: None,
            engagement_score: None,
            coherence_score: None,
            analysis_report: None,
            suggestions: Some(json!(["建议A", "建议B"])),
            word_count: Some(4),
            dialogue_ratio: None,
            description_ratio: None,
            created_at: Some(NaiveDateTime::default()),
        }
    }

    #[test]
    fn should_build_full_regeneration_task_seed_like_python_contract() {
        let request = FullChapterRegenerationStreamRequest::new(
            Some(2600),
            Some("强化冲突".to_string()),
            vec!["1".to_string(), "3".to_string()],
            vec!["pacing".to_string(), "emotion".to_string()],
            Some("总控".to_string()),
            Some("质量备注".to_string()),
            Some("修复摘要".to_string()),
            Some("hook".to_string()),
            Some("advance_plot".to_string()),
            Some("climax".to_string()),
            Some("balanced".to_string()),
            Some(true),
            Some("夜航税卡".to_string()),
            true,
            vec!["对白A".to_string()],
            vec!["转折A".to_string()],
            false,
            vec!["修复目标A".to_string()],
            vec!["保留优势A".to_string()],
            Some("mixed".to_string()),
            Some(7),
            Some("版本说明".to_string()),
            false,
        );

        let seed = build_full_regeneration_task_seed(
            &chapter_model(),
            Some(&analysis_model()),
            "user-1",
            &request,
            Some(11),
        );

        assert_eq!(seed.chapter_id, "chapter-1");
        assert_eq!(seed.analysis_id.as_deref(), Some("analysis-1"));
        assert_eq!(seed.user_id, "user-1");
        assert_eq!(seed.project_id, "project-1");
        assert_eq!(seed.selected_suggestion_indices, Some(json!(["1", "3"])));
        assert_eq!(seed.focus_areas, Some(json!(["pacing", "emotion"])));
        assert_eq!(seed.style_id, Some(11));
        assert_eq!(seed.target_word_count, 2600);
        assert_eq!(seed.version_note.as_deref(), Some("版本说明"));
        assert_eq!(seed.original_word_count, Some(4));
        assert_eq!(seed.original_suggestions, Some(json!(["建议A", "建议B"])));
        assert_eq!(
            seed.preserve_elements,
            Some(json!({
                "preserve_structure": true,
                "preserve_dialogues": ["对白A"],
                "preserve_plot_points": ["转折A"],
                "preserve_character_traits": false
            }))
        );
    }

    #[test]
    fn should_publish_regeneration_task_owner_contract() {
        let contract = build_chapter_regeneration_task_owner_contract();

        assert_eq!(contract["owner"], "chapter_regeneration_task_service");
        assert_eq!(
            contract["scope"],
            "full_regeneration_task_persistence_lifecycle_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["create_entrypoints"][0],
            "build_full_regeneration_task_seed"
        );
        assert_eq!(
            contract["behavior_contract"]["terminal_entrypoints"][1],
            "mark_regeneration_task_failed"
        );
        assert!(contract["behavior_contract"]["persisted_fields"]
            .as_array()
            .expect("persisted fields")
            .contains(&json!("version_note")));
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
    }
}
