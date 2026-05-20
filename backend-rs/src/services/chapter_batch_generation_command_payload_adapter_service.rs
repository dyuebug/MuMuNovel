use serde_json::{json, Value};

use crate::models::{batch_generation_task, chapter};
use crate::services::chapter_batch_generation_status_payload_adapter_service::{
    checkpoint_with_runtime_metadata, to_iso,
};
use crate::services::chapter_batch_generation_status_semantics_service::{
    task_execution_mode, task_stage_code, task_type,
};

fn estimate_batch_generation_task_minutes(total_chapters: i32) -> i32 {
    total_chapters.max(1) * 2
}

pub(crate) fn build_batch_generation_create_response_payload(
    created_task: &batch_generation_task::Model,
    chapters_to_generate: &[chapter::Model],
) -> Value {
    json!({
        "batch_id": created_task.id,
        "message": "Batch generation task created",
        "chapters_to_generate": chapters_to_generate.iter().map(|chapter_model| json!({
            "id": chapter_model.id,
            "chapter_number": chapter_model.chapter_number,
            "title": chapter_model.title,
        })).collect::<Vec<_>>(),
        "estimated_time_minutes": estimate_batch_generation_task_minutes(created_task.total_chapters),
    })
}

pub(crate) fn build_single_generation_background_create_response_payload(
    created_task: &batch_generation_task::Model,
    chapter_model: &chapter::Model,
) -> Value {
    json!({
        "task_id": created_task.id,
        "chapter_id": chapter_model.id,
        "status": "pending",
        "message": "单章后台生成任务已创建",
        "estimated_time_minutes": estimate_batch_generation_task_minutes(1),
        "active_story_repair_payload": null,
    })
}

pub(crate) fn build_cancel_batch_generation_response_payload(
    task: &batch_generation_task::Model,
) -> Value {
    json!({
        "message": "Batch generation cancelled",
        "batch_id": task.id,
        "completed_chapters": task.completed_chapters,
        "total_chapters": task.total_chapters,
    })
}

pub(crate) fn build_resume_batch_generation_response_payload(
    task: &batch_generation_task::Model,
) -> Value {
    let stage_code = task_stage_code(task);
    let execution_mode = task_execution_mode(task);
    let mut checkpoint = checkpoint_with_runtime_metadata(None, stage_code, execution_mode);
    checkpoint.insert(
        "chapter_id".to_string(),
        json!(task.current_chapter_id.clone()),
    );

    json!({
        "message": "Batch generation resumed",
        "batch_id": task.id,
        "project_id": task.project_id,
        "task_type": task_type(task),
        "status": task.status,
        "stage_code": stage_code,
        "execution_mode": execution_mode,
        "current_chapter_id": task.current_chapter_id,
        "checkpoint": checkpoint,
        "total_chapters": task.total_chapters,
        "completed_chapters": task.completed_chapters,
        "created_at": to_iso(task.created_at),
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{
        build_batch_generation_create_response_payload,
        build_cancel_batch_generation_response_payload,
        build_resume_batch_generation_response_payload,
        build_single_generation_background_create_response_payload,
        estimate_batch_generation_task_minutes,
    };
    use crate::models::{batch_generation_task, chapter};
    use serde_json::json;

    fn build_task(status: &str) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 1,
            chapter_ids: json!(["chapter-1"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(1),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn build_chapter(id: &str, chapter_number: i32, title: &str) -> chapter::Model {
        chapter::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_number,
            title: title.to_string(),
            content: Some("content".to_string()),
            summary: None,
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    #[test]
    fn should_build_resume_batch_generation_response_payload_with_shared_status_metadata() {
        let mut task = build_task("pending");
        task.project_id = "project-9".to_string();
        task.total_chapters = 3;
        task.completed_chapters = 0;
        task.current_chapter_id = Some("chapter-2".to_string());

        let payload = build_resume_batch_generation_response_payload(&task);

        assert_eq!(payload["message"], "Batch generation resumed");
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["project_id"], "project-9");
        assert_eq!(payload["task_type"], "chapter_single_generate");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["stage_code"], "6.writing.pending");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.pending");
        assert_eq!(payload["checkpoint"]["execution_mode"], "interactive");
        assert_eq!(payload["checkpoint"]["chapter_id"], "chapter-2");
        assert_eq!(payload["completed_chapters"], 0);
        assert_eq!(payload["total_chapters"], 3);
    }

    #[test]
    fn should_estimate_batch_generation_task_minutes_with_minimum_floor() {
        assert_eq!(estimate_batch_generation_task_minutes(0), 2);
        assert_eq!(estimate_batch_generation_task_minutes(1), 2);
        assert_eq!(estimate_batch_generation_task_minutes(3), 6);
    }

    #[test]
    fn should_build_batch_generation_create_response_payload() {
        let mut task = build_task("pending");
        task.total_chapters = 2;
        let chapters = vec![
            build_chapter("chapter-1", 1, "First"),
            build_chapter("chapter-2", 2, "Second"),
        ];

        let payload = build_batch_generation_create_response_payload(&task, &chapters);

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["message"], "Batch generation task created");
        assert_eq!(payload["chapters_to_generate"][0]["id"], "chapter-1");
        assert_eq!(payload["chapters_to_generate"][1]["title"], "Second");
        assert_eq!(payload["estimated_time_minutes"], 4);
    }

    #[test]
    fn should_build_single_generation_background_create_response_payload() {
        let task = build_task("pending");
        let chapter = build_chapter("chapter-7", 7, "Seven");

        let payload =
            build_single_generation_background_create_response_payload(&task, &chapter);

        assert_eq!(payload["task_id"], "task-1");
        assert_eq!(payload["chapter_id"], "chapter-7");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["estimated_time_minutes"], 2);
        assert!(payload["active_story_repair_payload"].is_null());
    }

    #[test]
    fn should_build_cancel_batch_generation_response_payload() {
        let mut task = build_task("running");
        task.completed_chapters = 2;
        task.total_chapters = 5;

        let payload = build_cancel_batch_generation_response_payload(&task);

        assert_eq!(payload["message"], "Batch generation cancelled");
        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["completed_chapters"], 2);
        assert_eq!(payload["total_chapters"], 5);
    }
}
