use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{batch_generation_task, chapter};
use crate::services::chapter_batch_generation_command_payload_adapter_service::{
    build_batch_generation_create_response_payload,
    build_cancel_batch_generation_response_payload,
    build_resume_batch_generation_response_payload,
    build_single_generation_background_create_response_payload,
};
use crate::services::chapter_batch_generation_chapter_payload_service::single_task_chapter_payload;
use crate::services::chapter_batch_generation_quality_status_service::manual_review_label;
use crate::services::chapter_batch_generation_runtime_state_service::{
    finalize_batch_generation_cancelled, persist_new_batch_generation_task_snapshot,
    persist_new_single_generation_task_snapshot, reset_batch_generation_task_for_resume,
};
use crate::services::chapter_batch_generation_status_semantics_service::task_type;

fn parse_batch_task_chapter_ids(task: &batch_generation_task::Model) -> Vec<String> {
    task.chapter_ids
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.as_str()
                .map(str::to_string)
                .or_else(|| item.get("id").and_then(Value::as_str).map(str::to_string))
        })
        .collect()
}

pub(crate) struct BatchGenerationCreatePlan {
    pub(crate) created_task_id: String,
    pub(crate) chapter_ids: Vec<String>,
    pub(crate) target_word_count: i32,
    pub(crate) response_payload: Value,
}

pub(crate) async fn create_batch_generation_task_plan(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    start_chapter_number: i32,
    chapters_to_generate: &[chapter::Model],
    style_id: Option<i32>,
    target_word_count: i32,
    enable_analysis: bool,
    max_retries: i32,
) -> Result<BatchGenerationCreatePlan, String> {
    let chapter_id_values: Vec<Value> = chapters_to_generate
        .iter()
        .map(|chapter_model| json!(chapter_model.id))
        .collect();
    let chapter_ids: Vec<String> = chapters_to_generate
        .iter()
        .map(|chapter_model| chapter_model.id.clone())
        .collect();
    let total_chapters = chapters_to_generate.len() as i32;
    let now = Utc::now().naive_utc();
    let task = batch_generation_task::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(project_id.to_string()),
        user_id: Set(user_id.to_string()),
        start_chapter_number: Set(start_chapter_number),
        chapter_count: Set(total_chapters),
        chapter_ids: Set(Value::Array(chapter_id_values)),
        style_id: Set(style_id),
        target_word_count: Set(target_word_count),
        enable_analysis: Set(enable_analysis),
        status: Set("pending".to_string()),
        total_chapters: Set(total_chapters),
        completed_chapters: Set(0),
        failed_chapters: Set(json!([])),
        current_chapter_id: Set(None),
        current_chapter_number: Set(None),
        current_retry_count: Set(0),
        max_retries: Set(max_retries),
        created_at: Set(Some(now)),
        started_at: Set(None),
        completed_at: Set(None),
        error_message: Set(None),
    };
    let created_task = task.insert(db).await.map_err(|error| error.to_string())?;
    persist_new_batch_generation_task_snapshot(db, &created_task.id, total_chapters).await?;

    Ok(BatchGenerationCreatePlan {
        created_task_id: created_task.id.clone(),
        chapter_ids,
        target_word_count,
        response_payload: build_batch_generation_create_response_payload(
            &created_task,
            chapters_to_generate,
        ),
    })
}

pub(crate) struct SingleGenerationBackgroundCreatePlan {
    pub(crate) created_task_id: String,
    pub(crate) target_word_count: i32,
    pub(crate) response_payload: Value,
}

pub(crate) async fn create_single_generation_background_task_plan(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_model: &chapter::Model,
    target_word_count: i32,
) -> Result<SingleGenerationBackgroundCreatePlan, String> {
    let now = Utc::now().naive_utc();
    let task_id = Uuid::new_v4().to_string();
    let task = batch_generation_task::ActiveModel {
        id: Set(task_id),
        project_id: Set(chapter_model.project_id.clone()),
        user_id: Set(user_id.to_string()),
        start_chapter_number: Set(chapter_model.chapter_number),
        chapter_count: Set(1),
        chapter_ids: Set(single_task_chapter_payload(chapter_model)),
        style_id: Set(None),
        target_word_count: Set(target_word_count),
        enable_analysis: Set(false),
        status: Set("pending".to_string()),
        total_chapters: Set(1),
        completed_chapters: Set(0),
        failed_chapters: Set(json!([])),
        current_chapter_id: Set(Some(chapter_model.id.clone())),
        current_chapter_number: Set(Some(chapter_model.chapter_number)),
        current_retry_count: Set(0),
        max_retries: Set(0),
        created_at: Set(Some(now)),
        started_at: Set(None),
        completed_at: Set(None),
        error_message: Set(None),
    };
    let created_task = task.insert(db).await.map_err(|error| error.to_string())?;
    persist_new_single_generation_task_snapshot(
        db,
        &created_task.id,
        &chapter_model.id,
        chapter_model.chapter_number,
    )
    .await?;

    Ok(SingleGenerationBackgroundCreatePlan {
        created_task_id: created_task.id.clone(),
        response_payload: build_single_generation_background_create_response_payload(
            &created_task,
            chapter_model,
        ),
        target_word_count,
    })
}

pub(crate) async fn cancel_batch_generation_task(
    db: &DatabaseConnection,
    task: batch_generation_task::Model,
) -> Result<Value, String> {
    if matches!(task.status.as_str(), "completed" | "failed" | "cancelled") {
        return Err(format!("Cannot cancel task in status {}", task.status));
    }

    finalize_batch_generation_cancelled(db, &task.id, task.completed_chapters, task.total_chapters)
        .await?;

    Ok(build_cancel_batch_generation_response_payload(&task))
}

pub(crate) struct ResumeBatchGenerationPlan {
    pub(crate) response_payload: Value,
    pub(crate) execution: ResumeExecutionPlan,
}

pub(crate) enum ResumeExecutionPlan {
    SingleChapter {
        chapter_id: String,
        target_word_count: i32,
        user_id: String,
    },
    Batch {
        chapter_ids: Vec<String>,
        target_word_count: i32,
        user_id: String,
    },
}

pub(crate) async fn prepare_batch_generation_resume(
    db: &DatabaseConnection,
    task: batch_generation_task::Model,
    user_id: &str,
) -> Result<ResumeBatchGenerationPlan, String> {
    if !matches!(task.status.as_str(), "failed" | "cancelled") {
        return Err("Only failed or cancelled tasks can be resumed".to_string());
    }

    if manual_review_label(Some(&task.failed_chapters)).is_some() {
        return Err("Manual review blocked tasks cannot be resumed".to_string());
    }

    let execution = if task_type(&task) == "chapter_single_generate" {
        let Some(chapter_id) = task.current_chapter_id.clone() else {
            return Err("Batch generation task has no chapters to resume".to_string());
        };
        ResumeExecutionPlan::SingleChapter {
            chapter_id,
            target_word_count: task.target_word_count.max(1),
            user_id: user_id.to_string(),
        }
    } else {
        let chapter_ids = parse_batch_task_chapter_ids(&task);
        if chapter_ids.is_empty() {
            return Err("Batch generation task has no chapters to resume".to_string());
        }
        ResumeExecutionPlan::Batch {
            chapter_ids,
            target_word_count: task.target_word_count.max(1),
            user_id: user_id.to_string(),
        }
    };

    let updated_task = reset_batch_generation_task_for_resume(db, &task).await?;

    Ok(ResumeBatchGenerationPlan {
        response_payload: build_resume_batch_generation_response_payload(&updated_task),
        execution,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_batch_task_chapter_ids;
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_quality_status_service::manual_review_label;
    use crate::services::chapter_batch_generation_runtime_state_service::build_pending_batch_generation_runtime_checkpoint;
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

    #[test]
    fn should_parse_batch_task_chapter_ids_from_strings_and_objects() {
        let mut task = build_task("pending");
        task.chapter_ids = json!(["chapter-1", {"id": "chapter-2"}, {"name": "ignored"}]);

        let chapter_ids = parse_batch_task_chapter_ids(&task);

        assert_eq!(
            chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
    }

    #[test]
    fn should_detect_manual_review_resume_blocker_from_shared_quality_semantics() {
        assert_eq!(
            manual_review_label(Some(&json!([{
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "needs review"
            }]))),
            Some("needs review".to_string())
        );
        assert_eq!(
            manual_review_label(Some(&json!([{
                "quality_gate_decision": "manual_review"
            }]))),
            Some("需人工复核".to_string())
        );
        assert!(manual_review_label(Some(&json!([{
            "quality_gate_decision": "passed"
        }])))
        .is_none());
    }

    #[test]
    fn should_build_pending_runtime_checkpoint_for_queued_batch_task() {
        let checkpoint = build_pending_batch_generation_runtime_checkpoint(
            "queued",
            "批量生成任务已创建，等待开始...",
            None,
            None,
            Some((0, 4)),
        );

        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(checkpoint["progress"], 0);
        assert_eq!(checkpoint["status"], "pending");
        assert_eq!(checkpoint["last_event"], "queued");
        assert_eq!(checkpoint["completed"], 0);
        assert_eq!(checkpoint["total"], 4);
        assert!(checkpoint["chapter_id"].is_null());
    }
}
