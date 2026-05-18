use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{batch_generation_snapshot, batch_generation_task, chapter};
use crate::services::chapter_batch_generation_chapter_payload_service::single_task_chapter_payload;

fn to_iso(value: Option<chrono::NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.and_utc().to_rfc3339())
}

fn task_type(task: &batch_generation_task::Model) -> &'static str {
    if task.chapter_count == 1
        && task
            .chapter_ids
            .as_array()
            .is_some_and(|items| items.len() == 1)
    {
        "chapter_single_generate"
    } else {
        "chapters_batch_generate"
    }
}

fn task_stage_code(task: &batch_generation_task::Model) -> &'static str {
    match task_type(task) {
        "chapter_single_generate" => match task.status.as_str() {
            "completed" => "6.writing.completed",
            "failed" => "6.writing.failed",
            "cancelled" => "6.writing.cancelled",
            "running" => "6.writing.generating",
            _ => "6.writing.pending",
        },
        _ => match task.status.as_str() {
            "completed" => "6.writing.completed",
            "failed" => "6.writing.failed",
            "cancelled" => "6.writing.cancelled",
            "running" => "6.writing.generating",
            _ => "6.writing.pending",
        },
    }
}

fn task_execution_mode(task: &batch_generation_task::Model) -> &'static str {
    match task_type(task) {
        "chapter_single_generate" => "interactive",
        _ => "interactive",
    }
}

fn manual_review_label(failed_chapters: Option<&Value>) -> Option<String> {
    let items = failed_chapters?.as_array()?;
    let first = items.first()?.as_object()?;
    let decision = first.get("quality_gate_decision")?.as_str()?;
    if decision != "manual_review" {
        return None;
    }

    first
        .get("quality_gate_label")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| Some("需人工复核".to_string()))
}

async fn load_batch_generation_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<Option<batch_generation_snapshot::Model>, String> {
    batch_generation_snapshot::Entity::find()
        .filter(batch_generation_snapshot::Column::BatchTaskId.eq(task_id))
        .one(db)
        .await
        .map_err(|error| error.to_string())
}

fn build_runtime_checkpoint(
    phase: &str,
    progress: i32,
    status: &str,
    last_event: &str,
    last_message: &str,
    chapter_id: Option<&str>,
    current_chapter_number: Option<i32>,
) -> Value {
    json!({
        "phase": phase,
        "progress": progress.clamp(0, 100),
        "status": status,
        "last_event": last_event,
        "last_message": last_message,
        "chapter_id": chapter_id,
        "current_chapter_id": chapter_id,
        "current_chapter_number": current_chapter_number,
        "updated_at": Utc::now().to_rfc3339(),
    })
}

fn build_resume_runtime_checkpoint(
    current_chapter_id: Option<&str>,
    current_chapter_number: Option<i32>,
) -> Value {
    build_runtime_checkpoint(
        "pending",
        0,
        "pending",
        "resume",
        "批量生成任务已恢复，等待重新开始...",
        current_chapter_id,
        current_chapter_number,
    )
}

async fn upsert_batch_generation_runtime_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    workflow_runtime_state: Value,
) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let existing = load_batch_generation_snapshot(db, task_id).await?;

    if let Some(snapshot) = existing {
        let mut active: batch_generation_snapshot::ActiveModel = snapshot.into();
        let merged_runtime_state =
            match (active.workflow_runtime_state.clone().take(), workflow_runtime_state) {
                (Some(Some(Value::Object(mut current))), Value::Object(incoming)) => {
                    for (key, value) in incoming {
                        current.insert(key, value);
                    }
                    Value::Object(current)
                }
                (_, incoming) => incoming,
            };
        active.workflow_runtime_state = Set(Some(merged_runtime_state));
        active.updated_at = Set(Some(now));
        active.update(db).await.map_err(|error| error.to_string())?;
        return Ok(());
    }

    let active = batch_generation_snapshot::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        batch_task_id: Set(task_id.to_string()),
        latest_quality_metrics: Set(None),
        quality_metrics_history: Set(None),
        quality_metrics_summary: Set(None),
        workflow_runtime_state: Set(Some(workflow_runtime_state)),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
    };
    active.insert(db).await.map_err(|error| error.to_string())?;
    Ok(())
}

async fn persist_new_batch_generation_task_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    total_chapters: i32,
) -> Result<(), String> {
    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        json!({
            "phase": "pending",
            "progress": 0,
            "status": "pending",
            "last_event": "queued",
            "last_message": "批量生成任务已创建，等待开始...",
            "chapter_id": null,
            "current_chapter_id": null,
            "current_chapter_number": null,
            "completed": 0,
            "total": total_chapters.max(0),
            "updated_at": Utc::now().to_rfc3339(),
        }),
    )
    .await
}

async fn persist_new_single_generation_task_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_id: &str,
    chapter_number: i32,
) -> Result<(), String> {
    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        json!({
            "phase": "pending",
            "progress": 0,
            "status": "pending",
            "last_event": "queued",
            "last_message": "单章生成任务已创建，等待开始...",
            "chapter_id": chapter_id,
            "current_chapter_id": chapter_id,
            "current_chapter_number": chapter_number,
            "updated_at": Utc::now().to_rfc3339(),
        }),
    )
    .await
}

pub fn parse_batch_task_chapter_ids(task: &batch_generation_task::Model) -> Vec<String> {
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

pub struct BatchGenerationCreatePlan {
    pub created_task: batch_generation_task::Model,
    pub chapter_ids: Vec<String>,
    pub target_word_count: i32,
    pub response_payload: Value,
}

pub async fn create_batch_generation_task_plan(
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
        chapter_ids,
        target_word_count,
        response_payload: json!({
            "batch_id": created_task.id,
            "message": "Batch generation task created",
            "chapters_to_generate": chapters_to_generate.iter().map(|chapter_model| json!({
                "id": chapter_model.id,
                "chapter_number": chapter_model.chapter_number,
                "title": chapter_model.title,
            })).collect::<Vec<_>>(),
            "estimated_time_minutes": total_chapters.max(1) * 2,
        }),
        created_task,
    })
}

pub struct SingleGenerationBackgroundCreatePlan {
    pub created_task: batch_generation_task::Model,
    pub target_word_count: i32,
    pub response_payload: Value,
}

pub async fn create_single_generation_background_task_plan(
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
        response_payload: json!({
            "task_id": created_task.id,
            "chapter_id": chapter_model.id,
            "status": "pending",
            "message": "单章后台生成任务已创建",
            "estimated_time_minutes": 2,
            "active_story_repair_payload": null,
        }),
        target_word_count,
        created_task,
    })
}

pub struct CancelBatchGenerationResult {
    pub response_payload: Value,
}

pub async fn cancel_batch_generation_task(
    db: &DatabaseConnection,
    task: batch_generation_task::Model,
) -> Result<CancelBatchGenerationResult, String> {
    if matches!(task.status.as_str(), "completed" | "failed" | "cancelled") {
        return Err(format!("Cannot cancel task in status {}", task.status));
    }

    let mut active: batch_generation_task::ActiveModel = task.clone().into();
    active.status = Set("cancelled".to_string());
    active.completed_at = Set(Some(Utc::now().naive_utc()));
    active
        .update(db)
        .await
        .map_err(|error| error.to_string())?;

    Ok(CancelBatchGenerationResult {
        response_payload: json!({
            "message": "Batch generation cancelled",
            "batch_id": task.id,
            "completed_chapters": task.completed_chapters,
            "total_chapters": task.total_chapters,
        }),
    })
}

pub struct ResumeBatchGenerationPlan {
    pub updated_task: batch_generation_task::Model,
    pub response_payload: Value,
    pub execution: ResumeExecutionPlan,
}

pub enum ResumeExecutionPlan {
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

pub async fn prepare_batch_generation_resume(
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

    let mut active: batch_generation_task::ActiveModel = task.clone().into();
    active.status = Set("pending".to_string());
    active.error_message = Set(None);
    active.completed_at = Set(None);
    active.started_at = Set(None);
    active.completed_chapters = Set(0);
    active.current_retry_count = Set(0);
    let updated_task = active.update(db).await.map_err(|error| error.to_string())?;
    let resume_checkpoint = build_resume_runtime_checkpoint(
        updated_task.current_chapter_id.as_deref(),
        updated_task.current_chapter_number,
    );
    upsert_batch_generation_runtime_snapshot(db, &updated_task.id, resume_checkpoint).await?;

    Ok(ResumeBatchGenerationPlan {
        response_payload: json!({
            "message": "Batch generation resumed",
            "batch_id": task.id,
            "project_id": updated_task.project_id,
            "task_type": task_type(&updated_task),
            "status": "pending",
            "stage_code": task_stage_code(&updated_task),
            "execution_mode": task_execution_mode(&updated_task),
            "current_chapter_id": updated_task.current_chapter_id,
            "checkpoint": {
                "stage_code": task_stage_code(&updated_task),
                "execution_mode": task_execution_mode(&updated_task),
                "chapter_id": updated_task.current_chapter_id,
            },
            "total_chapters": updated_task.total_chapters,
            "completed_chapters": 0,
            "created_at": to_iso(updated_task.created_at),
        }),
        updated_task,
        execution,
    })
}

#[cfg(test)]
mod tests {
    use super::{build_resume_runtime_checkpoint, parse_batch_task_chapter_ids};
    use crate::models::batch_generation_task;
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
    fn should_build_resume_runtime_checkpoint_with_and_without_chapter_id() {
        let with_chapter = build_resume_runtime_checkpoint(Some("chapter-1"), Some(3));
        assert_eq!(with_chapter["phase"], "pending");
        assert_eq!(with_chapter["progress"], 0);
        assert_eq!(with_chapter["status"], "pending");
        assert_eq!(with_chapter["last_event"], "resume");
        assert_eq!(with_chapter["last_message"], "批量生成任务已恢复，等待重新开始...");
        assert_eq!(with_chapter["chapter_id"], "chapter-1");
        assert_eq!(with_chapter["current_chapter_id"], "chapter-1");
        assert_eq!(with_chapter["current_chapter_number"], 3);

        let without_chapter = build_resume_runtime_checkpoint(None, Some(4));
        assert_eq!(without_chapter["phase"], "pending");
        assert_eq!(without_chapter["progress"], 0);
        assert_eq!(without_chapter["status"], "pending");
        assert_eq!(without_chapter["last_event"], "resume");
        assert_eq!(without_chapter["last_message"], "批量生成任务已恢复，等待重新开始...");
        assert!(without_chapter["chapter_id"].is_null());
        assert!(without_chapter["current_chapter_id"].is_null());
        assert_eq!(without_chapter["current_chapter_number"], 4);
    }
}
