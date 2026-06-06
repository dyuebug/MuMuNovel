use chrono::NaiveDateTime;
use sea_orm::Set;
use serde_json::{json, Value};

use crate::models::batch_generation_task;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationTaskPersistenceSeed {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) user_id: String,
    pub(crate) start_chapter_number: i32,
    pub(crate) chapter_count: i32,
    pub(crate) chapter_ids: Value,
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: i32,
    pub(crate) enable_analysis: bool,
    pub(crate) total_chapters: i32,
    pub(crate) current_chapter_id: Option<String>,
    pub(crate) current_chapter_number: Option<i32>,
    pub(crate) max_retries: i32,
}

impl BatchGenerationTaskPersistenceSeed {
    pub(crate) fn into_active_model(
        self,
        now: NaiveDateTime,
    ) -> batch_generation_task::ActiveModel {
        batch_generation_task::ActiveModel {
            id: Set(self.id),
            project_id: Set(self.project_id),
            user_id: Set(self.user_id),
            start_chapter_number: Set(self.start_chapter_number),
            chapter_count: Set(self.chapter_count),
            chapter_ids: Set(self.chapter_ids),
            style_id: Set(self.style_id),
            target_word_count: Set(self.target_word_count),
            enable_analysis: Set(self.enable_analysis),
            status: Set("pending".to_string()),
            total_chapters: Set(self.total_chapters),
            completed_chapters: Set(0),
            failed_chapters: Set(json!([])),
            current_chapter_id: Set(self.current_chapter_id),
            current_chapter_number: Set(self.current_chapter_number),
            current_retry_count: Set(0),
            max_retries: Set(self.max_retries),
            created_at: Set(Some(now)),
            started_at: Set(None),
            completed_at: Set(None),
            error_message: Set(None),
        }
    }
}

pub(crate) fn build_batch_generation_task_active_model(
    id: String,
    project_id: String,
    user_id: String,
    start_chapter_number: i32,
    chapter_count: i32,
    chapter_ids: Value,
    style_id: Option<i32>,
    target_word_count: i32,
    enable_analysis: bool,
    total_chapters: i32,
    current_chapter_id: Option<String>,
    current_chapter_number: Option<i32>,
    max_retries: i32,
    now: NaiveDateTime,
) -> batch_generation_task::ActiveModel {
    BatchGenerationTaskPersistenceSeed {
        id,
        project_id,
        user_id,
        start_chapter_number,
        chapter_count,
        chapter_ids,
        style_id,
        target_word_count,
        enable_analysis,
        total_chapters,
        current_chapter_id,
        current_chapter_number,
        max_retries,
    }
    .into_active_model(now)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::Set;
    use serde_json::json;

    use super::build_batch_generation_task_active_model;
    use crate::services::chapter_batch_generation_write_workflow_service::BatchGenerationCreateChapterTarget;

    fn build_chapter_target(
        id: &str,
        project_id: &str,
        chapter_number: i32,
    ) -> BatchGenerationCreateChapterTarget {
        let _ = project_id;
        BatchGenerationCreateChapterTarget {
            id: id.to_string(),
            chapter_number,
            title: format!("Chapter {chapter_number}"),
        }
    }

    #[test]
    fn should_build_pending_batch_generation_task_active_model_with_shared_defaults() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(2, 0, 0)
            .expect("valid time");
        let active = build_batch_generation_task_active_model(
            "task-9".to_string(),
            "project-9".to_string(),
            "user-9".to_string(),
            3,
            2,
            json!(["chapter-1", "chapter-2"]),
            Some(7),
            4200,
            true,
            2,
            Some("chapter-1".to_string()),
            Some(1),
            4,
            now,
        );

        assert_eq!(active.status, Set("pending".to_string()));
        assert_eq!(active.completed_chapters, Set(0));
        assert_eq!(active.failed_chapters, Set(json!([])));
        assert_eq!(active.current_retry_count, Set(0));
        assert_eq!(active.created_at, Set(Some(now)));
        assert_eq!(active.started_at, Set(None));
        assert_eq!(active.completed_at, Set(None));
        assert_eq!(active.error_message, Set(None));
        assert_eq!(active.total_chapters, Set(2));
        assert_eq!(active.max_retries, Set(4));
        assert_eq!(
            active.current_chapter_id,
            Set(Some("chapter-1".to_string()))
        );
        assert_eq!(active.current_chapter_number, Set(Some(1)));
    }

    #[test]
    fn should_build_batch_generation_task_active_model_from_create_request() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(2, 45, 0)
            .expect("valid time");
        let chapters_to_generate = vec![
            build_chapter_target("chapter-3", "project-9", 3),
            build_chapter_target("chapter-4", "project-9", 4),
        ];

        let chapter_id_values: Vec<serde_json::Value> = chapters_to_generate
            .iter()
            .map(|chapter_target| json!(chapter_target.id))
            .collect();
        let total_chapters = chapters_to_generate.len() as i32;
        let request = crate::services::chapter_batch_generation_write_workflow_service::BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 3,
            count: 2,
            style_id: Some(7),
            target_word_count: Some(2800),
            enable_analysis: true,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            max_retries: 4,
            model_override: Some("gpt-4.1".to_string()),
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: Vec::new(),
            story_preserve_strengths: Vec::new(),
        };

        let active = build_batch_generation_task_active_model(
            "task-10".to_string(),
            "project-9".to_string(),
            "user-9".to_string(),
            request.start_chapter_number,
            total_chapters,
            serde_json::Value::Array(chapter_id_values),
            request.style_id,
            2800,
            request.enable_analysis,
            total_chapters,
            None,
            None,
            request.max_retries,
            now,
        );

        assert_eq!(active.id, Set("task-10".to_string()));
        assert_eq!(active.start_chapter_number, Set(3));
        assert_eq!(active.target_word_count, Set(2800));
        assert_eq!(active.enable_analysis, Set(true));
        assert_eq!(active.total_chapters, Set(2));
        assert_eq!(active.max_retries, Set(4));
        assert_eq!(active.chapter_ids, Set(json!(["chapter-3", "chapter-4"])));
    }
}
