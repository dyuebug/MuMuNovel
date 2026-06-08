use serde_json::Value;

use crate::models::batch_generation_task;

const ACTIVE_BATCH_GENERATION_STATUSES: [&str; 2] = ["pending", "running"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationTaskKind {
    SingleChapter,
    Batch,
}

pub(crate) fn active_batch_generation_statuses() -> [&'static str; 2] {
    ACTIVE_BATCH_GENERATION_STATUSES
}

pub(crate) fn batch_generation_task_kind(
    chapter_count: i32,
    chapter_ids: &Value,
) -> BatchGenerationTaskKind {
    if chapter_count == 1 && chapter_ids.as_array().is_some_and(|items| items.len() == 1) {
        BatchGenerationTaskKind::SingleChapter
    } else {
        BatchGenerationTaskKind::Batch
    }
}

pub(crate) fn task_kind(task: &batch_generation_task::Model) -> BatchGenerationTaskKind {
    batch_generation_task_kind(task.chapter_count, &task.chapter_ids)
}

pub(crate) fn batch_generation_task_type(kind: BatchGenerationTaskKind) -> &'static str {
    match kind {
        BatchGenerationTaskKind::SingleChapter => "chapter_single_generate",
        BatchGenerationTaskKind::Batch => "chapters_batch_generate",
    }
}

pub(crate) fn task_type(task: &batch_generation_task::Model) -> &'static str {
    batch_generation_task_type(task_kind(task))
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        active_batch_generation_statuses, batch_generation_task_kind, batch_generation_task_type,
        task_kind, task_type, BatchGenerationTaskKind,
    };
    use crate::models::batch_generation_task;

    fn build_task_shape(
        status: &str,
        chapter_count: i32,
        chapter_ids: Value,
        total_chapters: i32,
    ) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count,
            chapter_ids,
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters,
            completed_chapters: 1,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(1),
            current_retry_count: 2,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    #[test]
    fn should_resolve_batch_generation_task_type() {
        let single = build_task_shape("pending", 1, json!(["chapter-1"]), 1);
        let batch = build_task_shape("pending", 2, json!(["chapter-1", "chapter-2"]), 2);
        let malformed_single =
            build_task_shape("pending", 1, json!({"chapter_id": "chapter-1"}), 1);

        assert_eq!(
            batch_generation_task_type(BatchGenerationTaskKind::SingleChapter),
            "chapter_single_generate"
        );
        assert_eq!(
            batch_generation_task_type(BatchGenerationTaskKind::Batch),
            "chapters_batch_generate"
        );
        assert_eq!(
            batch_generation_task_kind(1, &json!(["chapter-1"])),
            BatchGenerationTaskKind::SingleChapter
        );
        assert_eq!(
            batch_generation_task_kind(2, &json!(["chapter-1", "chapter-2"])),
            BatchGenerationTaskKind::Batch
        );
        assert_eq!(task_kind(&single), BatchGenerationTaskKind::SingleChapter);
        assert_eq!(task_kind(&batch), BatchGenerationTaskKind::Batch);
        assert_eq!(task_kind(&malformed_single), BatchGenerationTaskKind::Batch);
        assert_eq!(task_type(&single), "chapter_single_generate");
        assert_eq!(task_type(&batch), "chapters_batch_generate");
        assert_eq!(task_type(&malformed_single), "chapters_batch_generate");
    }

    #[test]
    fn should_expose_active_batch_generation_statuses() {
        assert_eq!(active_batch_generation_statuses(), ["pending", "running"]);
        assert!(active_batch_generation_statuses().contains(&"pending"));
        assert!(active_batch_generation_statuses().contains(&"running"));
        assert!(!active_batch_generation_statuses().contains(&"completed"));
        assert!(!active_batch_generation_statuses().contains(&"cancelled"));
    }
}
