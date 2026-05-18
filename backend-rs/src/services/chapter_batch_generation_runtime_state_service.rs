use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::ai::service::AIService;
use crate::ai::AIConfig;
use crate::models::{batch_generation_snapshot, batch_generation_task, chapter};
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
use crate::services::chapter_generation_service::generate_and_persist_chapter_content_with_provider_payload;

pub fn to_iso(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|value| value.and_utc().to_rfc3339())
}

pub fn task_type(task: &batch_generation_task::Model) -> &'static str {
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

pub fn task_stage_code(task: &batch_generation_task::Model) -> &'static str {
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

pub fn task_execution_mode(task: &batch_generation_task::Model) -> &'static str {
    match task_type(task) {
        "chapter_single_generate" => "interactive",
        _ => "interactive",
    }
}

pub fn active_story_repair_payload_from_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> Option<Value> {
    workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("active_story_repair_payload"))
        .filter(|payload| payload.is_object())
        .cloned()
}

pub async fn load_batch_generation_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<Option<batch_generation_snapshot::Model>, String> {
    batch_generation_snapshot::Entity::find()
        .filter(batch_generation_snapshot::Column::BatchTaskId.eq(task_id))
        .one(db)
        .await
        .map_err(|error| error.to_string())
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

pub fn terminal_semantics(
    task: &batch_generation_task::Model,
    failed_chapters: Option<&Value>,
) -> (Option<&'static str>, Option<String>, bool, bool) {
    if task.status == "failed" {
        if let Some(label) = manual_review_label(failed_chapters) {
            return (Some("manual_review"), Some(label), true, false);
        }
        return (Some("error"), Some("执行失败".to_string()), false, true);
    }

    match task.status.as_str() {
        "completed" => (Some("completed"), Some("已完成".to_string()), false, false),
        "cancelled" => (Some("cancelled"), Some("已取消".to_string()), false, true),
        _ => (None, None, false, false),
    }
}

pub fn checkpoint_with_runtime_metadata(
    workflow_runtime_state: Option<&Value>,
    stage_code: &str,
    execution_mode: &str,
) -> Map<String, Value> {
    let mut checkpoint = workflow_runtime_state
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    checkpoint.insert("stage_code".to_string(), json!(stage_code));
    checkpoint.insert("execution_mode".to_string(), json!(execution_mode));
    checkpoint
}

pub fn task_status_payload(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<Value>,
    latest_quality_metrics: Option<Value>,
    quality_metrics_summary: Option<Value>,
    active_story_repair_payload: Option<Value>,
) -> Value {
    let stage_code = task_stage_code(task);
    let execution_mode = task_execution_mode(task);
    let failed_chapters = task.failed_chapters.clone();
    let checkpoint = checkpoint_with_runtime_metadata(
        workflow_runtime_state.as_ref(),
        stage_code,
        execution_mode,
    );
    let (terminal_reason, terminal_label, review_required, can_resume) =
        terminal_semantics(task, Some(&failed_chapters));

    json!({
        "batch_id": task.id,
        "task_type": task_type(task),
        "project_id": task.project_id,
        "status": task.status,
        "stage_code": stage_code,
        "execution_mode": execution_mode,
        "total": task.total_chapters,
        "completed": task.completed_chapters,
        "current_chapter_id": task.current_chapter_id,
        "current_chapter_number": task.current_chapter_number,
        "current_retry_count": task.current_retry_count,
        "max_retries": task.max_retries,
        "failed_chapters": failed_chapters,
        "created_at": to_iso(task.created_at),
        "started_at": to_iso(task.started_at),
        "completed_at": to_iso(task.completed_at),
        "error_message": task.error_message,
        "checkpoint": checkpoint,
        "latest_quality_metrics": latest_quality_metrics,
        "quality_metrics_summary": quality_metrics_summary,
        "active_story_repair_payload": active_story_repair_payload,
        "terminal_reason": terminal_reason,
        "terminal_label": terminal_label,
        "review_required": review_required,
        "can_resume": can_resume,
    })
}

pub fn active_task_payload(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<Value>,
    latest_quality_metrics: Option<Value>,
    quality_metrics_summary: Option<Value>,
    active_story_repair_payload: Option<Value>,
) -> Value {
    let stage_code = task_stage_code(task);
    let execution_mode = task_execution_mode(task);
    let checkpoint = checkpoint_with_runtime_metadata(
        workflow_runtime_state.as_ref(),
        stage_code,
        execution_mode,
    );

    json!({
        "batch_id": task.id,
        "task_type": task_type(task),
        "project_id": task.project_id,
        "status": task.status,
        "stage_code": stage_code,
        "execution_mode": execution_mode,
        "total": task.total_chapters,
        "completed": task.completed_chapters,
        "current_chapter_id": task.current_chapter_id,
        "current_chapter_number": task.current_chapter_number,
        "checkpoint": checkpoint,
        "latest_quality_metrics": latest_quality_metrics,
        "quality_metrics_summary": quality_metrics_summary,
        "active_story_repair_payload": active_story_repair_payload,
        "created_at": to_iso(task.created_at),
        "started_at": to_iso(task.started_at),
        "completed_at": to_iso(task.completed_at),
        "error_message": task.error_message,
    })
}

pub async fn upsert_batch_generation_runtime_snapshot(
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

pub fn build_runtime_checkpoint(
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

pub fn build_single_generation_runtime_checkpoint(
    phase: &str,
    progress: i32,
    status: &str,
    last_event: &str,
    last_message: &str,
    chapter_id: &str,
    current_chapter_number: Option<i32>,
    word_count: Option<i32>,
) -> Value {
    let mut checkpoint = build_runtime_checkpoint(
        phase,
        progress,
        status,
        last_event,
        last_message,
        Some(chapter_id),
        current_chapter_number,
    );
    if let Some(object) = checkpoint.as_object_mut() {
        if let Some(value) = word_count {
            object.insert("word_count".to_string(), json!(value.max(0)));
        }
    }
    checkpoint
}

pub fn build_batch_generation_runtime_checkpoint(
    phase: &str,
    progress: i32,
    status: &str,
    last_event: &str,
    last_message: &str,
    chapter_id: Option<&str>,
    current_chapter_number: Option<i32>,
    completed_chapters: i32,
    total_chapters: i32,
) -> Value {
    let mut checkpoint = build_runtime_checkpoint(
        phase,
        progress,
        status,
        last_event,
        last_message,
        chapter_id,
        current_chapter_number,
    );
    if let Some(object) = checkpoint.as_object_mut() {
        object.insert("completed".to_string(), json!(completed_chapters.max(0)));
        object.insert("total".to_string(), json!(total_chapters.max(0)));
    }
    checkpoint
}

pub async fn persist_new_batch_generation_task_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    total_chapters: i32,
) -> Result<(), String> {
    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_batch_generation_runtime_checkpoint(
            "pending",
            0,
            "pending",
            "queued",
            "批量生成任务已创建，等待开始...",
            None,
            None,
            0,
            total_chapters,
        ),
    )
    .await
}

pub async fn persist_new_single_generation_task_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_id: &str,
    chapter_number: i32,
) -> Result<(), String> {
    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_single_generation_runtime_checkpoint(
            "pending",
            0,
            "pending",
            "queued",
            "单章生成任务已创建，等待开始...",
            chapter_id,
            Some(chapter_number),
            None,
        ),
    )
    .await
}

pub async fn prepare_single_generation_runtime(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_id: &str,
) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    {
        let mut active: batch_generation_task::ActiveModel = task_model.into();
        active.status = Set("running".to_string());
        active.started_at = Set(Some(now));
        active.completed_at = Set(None);
        active.error_message = Set(None);
        active.current_retry_count = Set(0);
        active.current_chapter_id = Set(Some(chapter_id.to_string()));
        active.update(db).await.map_err(|error| error.to_string())?;
    }

    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_single_generation_runtime_checkpoint(
            "generating",
            15,
            "running",
            "chapter_start",
            "正在准备章节生成...",
            chapter_id,
            None,
            None,
        ),
    )
    .await?;
    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_single_generation_runtime_checkpoint(
            "generating",
            65,
            "running",
            "progress",
            "正在生成正文...",
            chapter_id,
            None,
            None,
        ),
    )
    .await
}

pub async fn persist_single_generation_finalizing_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_id: &str,
    chapter_number: Option<i32>,
    word_count: Option<i32>,
) -> Result<(), String> {
    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_single_generation_runtime_checkpoint(
            "finalizing",
            95,
            "running",
            "progress",
            "正在整理生成结果...",
            chapter_id,
            chapter_number,
            word_count,
        ),
    )
    .await
}

pub async fn finalize_single_generation_success(
    db: &DatabaseConnection,
    task_id: &str,
    current_chapter_id: &str,
    chapter_number: Option<i32>,
    word_count: Option<i32>,
) -> Result<(), String> {
    if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    {
        let mut active: batch_generation_task::ActiveModel = task_model.into();
        active.status = Set("completed".to_string());
        active.completed_chapters = Set(1);
        active.completed_at = Set(Some(Utc::now().naive_utc()));
        active.error_message = Set(None);
        active.current_chapter_id = Set(Some(current_chapter_id.to_string()));
        active.current_chapter_number = Set(chapter_number);
        active.update(db).await.map_err(|error| error.to_string())?;
    }

    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_single_generation_runtime_checkpoint(
            "completed",
            100,
            "completed",
            "done",
            "章节生成完成",
            current_chapter_id,
            chapter_number,
            word_count,
        ),
    )
    .await
}

pub async fn finalize_single_generation_failure(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_id: &str,
    error_message: String,
) -> Result<(), String> {
    if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    {
        let mut active: batch_generation_task::ActiveModel = task_model.into();
        active.status = Set("failed".to_string());
        active.completed_at = Set(Some(Utc::now().naive_utc()));
        active.error_message = Set(Some(error_message));
        active.update(db).await.map_err(|error| error.to_string())?;
    }

    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_single_generation_runtime_checkpoint(
            "failed",
            100,
            "failed",
            "error",
            "章节生成失败",
            chapter_id,
            None,
            None,
        ),
    )
    .await
}

pub async fn prepare_batch_generation_runtime(
    db: &DatabaseConnection,
    task_id: &str,
    total_chapters: i32,
) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    {
        let mut active: batch_generation_task::ActiveModel = task_model.into();
        active.status = Set("running".to_string());
        active.started_at = Set(Some(now));
        active.completed_at = Set(None);
        active.error_message = Set(None);
        active.current_retry_count = Set(0);
        active.update(db).await.map_err(|error| error.to_string())?;
    }

    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_batch_generation_runtime_checkpoint(
            "generating",
            10,
            "running",
            "progress",
            "正在准备批量生成...",
            None,
            None,
            0,
            total_chapters,
        ),
    )
    .await
}

pub async fn finalize_batch_generation_cancelled(
    db: &DatabaseConnection,
    task_id: &str,
    completed_chapters: i32,
    total_chapters: i32,
) -> Result<(), String> {
    if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    {
        let mut active: batch_generation_task::ActiveModel = task_model.into();
        active.completed_at = Set(Some(Utc::now().naive_utc()));
        active.update(db).await.map_err(|error| error.to_string())?;
    }

    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_batch_generation_runtime_checkpoint(
            "cancelled",
            100,
            "cancelled",
            "cancelled",
            "批量生成已取消",
            None,
            None,
            completed_chapters,
            total_chapters,
        ),
    )
    .await
}

pub async fn mark_batch_generation_chapter_started(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_model: &chapter::Model,
    completed_chapters: i32,
    total_chapters: i32,
) -> Result<(), String> {
    if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    {
        let mut active: batch_generation_task::ActiveModel = task_model.into();
        active.status = Set("running".to_string());
        active.current_chapter_id = Set(Some(chapter_model.id.clone()));
        active.current_chapter_number = Set(Some(chapter_model.chapter_number));
        active.total_chapters = Set(total_chapters);
        active.completed_chapters = Set(completed_chapters);
        active.error_message = Set(None);
        active.update(db).await.map_err(|error| error.to_string())?;
    }

    let running_progress = compute_batch_running_progress(completed_chapters, total_chapters);
    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_batch_generation_runtime_checkpoint(
            "generating",
            running_progress,
            "running",
            "chapter_start",
            &format!("正在生成第 {} 章...", chapter_model.chapter_number),
            Some(&chapter_model.id),
            Some(chapter_model.chapter_number),
            completed_chapters,
            total_chapters,
        ),
    )
    .await
}

fn compute_batch_running_progress(completed_chapters: i32, total_chapters: i32) -> i32 {
    if total_chapters <= 0 {
        return 15;
    }

    let base_progress = ((completed_chapters * 100) / total_chapters).clamp(0, 100);
    (base_progress + 15).clamp(15, 95)
}

pub async fn finalize_batch_generation_success(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_model: &chapter::Model,
    completed_chapters: i32,
    total_chapters: i32,
) -> Result<(), String> {
    if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    {
        let mut active: batch_generation_task::ActiveModel = task_model.into();
        active.status = Set(if completed_chapters >= total_chapters {
            "completed".to_string()
        } else {
            "running".to_string()
        });
        active.completed_chapters = Set(completed_chapters);
        active.current_chapter_id = Set(Some(chapter_model.id.clone()));
        active.current_chapter_number = Set(Some(chapter_model.chapter_number));
        if completed_chapters >= total_chapters {
            active.completed_at = Set(Some(Utc::now().naive_utc()));
        }
        active.error_message = Set(None);
        active.update(db).await.map_err(|error| error.to_string())?;
    }

    let success_plan =
        resolve_batch_generation_success_checkpoint(completed_chapters, total_chapters);
    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_batch_generation_runtime_checkpoint(
            success_plan.phase,
            success_plan.progress,
            success_plan.status,
            success_plan.last_event,
            success_plan.last_message,
            Some(&chapter_model.id),
            Some(chapter_model.chapter_number),
            completed_chapters,
            total_chapters,
        ),
    )
    .await
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchGenerationSuccessCheckpointPlan {
    phase: &'static str,
    progress: i32,
    status: &'static str,
    last_event: &'static str,
    last_message: &'static str,
}

fn resolve_batch_generation_success_checkpoint(
    completed_chapters: i32,
    total_chapters: i32,
) -> BatchGenerationSuccessCheckpointPlan {
    if completed_chapters >= total_chapters {
        return BatchGenerationSuccessCheckpointPlan {
            phase: "completed",
            progress: 100,
            status: "completed",
            last_event: "done",
            last_message: "批量生成完成",
        };
    }

    let completed_progress = if total_chapters <= 0 {
        100
    } else {
        ((completed_chapters * 100) / total_chapters).clamp(0, 100)
    };

    BatchGenerationSuccessCheckpointPlan {
        phase: "generating",
        progress: completed_progress,
        status: "running",
        last_event: "progress",
        last_message: "当前章节生成完成，继续下一章...",
    }
}

pub async fn finalize_batch_generation_failure(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_id: Option<&str>,
    chapter_number: Option<i32>,
    completed_chapters: i32,
    total_chapters: i32,
    task_error_message: String,
    checkpoint_message: &str,
) -> Result<(), String> {
    if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    {
        let mut active: batch_generation_task::ActiveModel = task_model.into();
        active.status = Set("failed".to_string());
        active.completed_chapters = Set(completed_chapters);
        active.current_chapter_id = Set(chapter_id.map(str::to_string));
        active.current_chapter_number = Set(chapter_number);
        active.completed_at = Set(Some(Utc::now().naive_utc()));
        active.error_message = Set(Some(task_error_message));
        active.update(db).await.map_err(|error| error.to_string())?;
    }

    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_batch_generation_runtime_checkpoint(
            "failed",
            100,
            "failed",
            "error",
            checkpoint_message,
            chapter_id,
            chapter_number,
            completed_chapters,
            total_chapters,
        ),
    )
    .await
}

pub async fn execute_single_generation_runtime(
    db: &DatabaseConnection,
    task_id: &str,
    user_id: &str,
    chapter_id: &str,
    target_word_count: i32,
    ai_config: AIConfig,
    provider_payload: PromptContextProviderPayload,
) {
    let _ = prepare_single_generation_runtime(db, task_id, chapter_id).await;

    let ai_service = AIService::new(ai_config);
    let generation_result = generate_and_persist_chapter_content_with_provider_payload(
        db,
        &ai_service,
        user_id,
        chapter_id,
        target_word_count,
        provider_payload,
    )
    .await;

    match generation_result {
        Ok(payload) => {
            let chapter_number = payload
                .get("chapter_number")
                .and_then(Value::as_i64)
                .map(|value| value as i32);
            let word_count = payload
                .get("word_count")
                .and_then(Value::as_i64)
                .map(|value| value as i32);
            let resolved_chapter_id = payload
                .get("chapter_id")
                .and_then(Value::as_str)
                .unwrap_or(chapter_id);
            let _ = persist_single_generation_finalizing_snapshot(
                db,
                task_id,
                chapter_id,
                chapter_number,
                word_count,
            )
            .await;
            let _ = finalize_single_generation_success(
                db,
                task_id,
                resolved_chapter_id,
                chapter_number,
                word_count,
            )
            .await;
        }
        Err(error) => {
            let _ = finalize_single_generation_failure(db, task_id, chapter_id, error).await;
        }
    }
}

pub async fn execute_batch_generation_runtime(
    db: &DatabaseConnection,
    task_id: &str,
    user_id: &str,
    chapter_ids: &[String],
    target_word_count: i32,
    ai_config: AIConfig,
    provider_payload: PromptContextProviderPayload,
) {
    let ai_service = AIService::new(ai_config);
    let total = chapter_ids.len() as i32;
    let _ = prepare_batch_generation_runtime(db, task_id, total).await;
    let mut completed = 0i32;

    for chapter_id in chapter_ids {
        let task_model = match batch_generation_task::Entity::find_by_id(task_id).one(db).await {
            Ok(Some(task_model)) => task_model,
            _ => return,
        };

        if task_model.status == "cancelled" {
            let _ = finalize_batch_generation_cancelled(db, task_id, completed, total).await;
            return;
        }

        let chapter_model = match chapter::Entity::find_by_id(chapter_id).one(db).await {
            Ok(Some(chapter_model)) => chapter_model,
            Ok(None) => {
                let _ = finalize_batch_generation_failure(
                    db,
                    task_id,
                    Some(chapter_id),
                    None,
                    completed,
                    total,
                    format!("Chapter not found: {}", chapter_id),
                    "批量生成失败：章节不存在",
                )
                .await;
                return;
            }
            Err(error) => {
                let _ = finalize_batch_generation_failure(
                    db,
                    task_id,
                    Some(chapter_id),
                    None,
                    completed,
                    total,
                    error.to_string(),
                    "批量生成失败：加载章节异常",
                )
                .await;
                return;
            }
        };

        let _ =
            mark_batch_generation_chapter_started(db, task_id, &chapter_model, completed, total)
                .await;

        let generation_result = generate_and_persist_chapter_content_with_provider_payload(
            db,
            &ai_service,
            user_id,
            &chapter_model.id,
            target_word_count,
            provider_payload.clone(),
        )
        .await;

        match generation_result {
            Ok(_) => {
                completed += 1;
                let _ = finalize_batch_generation_success(
                    db,
                    task_id,
                    &chapter_model,
                    completed,
                    total,
                )
                .await;
            }
            Err(error) => {
                let _ = finalize_batch_generation_failure(
                    db,
                    task_id,
                    Some(&chapter_model.id),
                    Some(chapter_model.chapter_number),
                    completed,
                    total,
                    error,
                    "批量生成失败",
                )
                .await;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        compute_batch_running_progress, resolve_batch_generation_success_checkpoint,
    };

    #[test]
    fn should_compute_batch_running_progress_with_floor_and_clamp() {
        assert_eq!(compute_batch_running_progress(0, 0), 15);
        assert_eq!(compute_batch_running_progress(2, 5), 55);
        assert_eq!(compute_batch_running_progress(5, 5), 95);
        assert_eq!(compute_batch_running_progress(7, 5), 95);
    }

    #[test]
    fn should_resolve_batch_generation_success_checkpoint_for_running_and_completed_states() {
        let running = resolve_batch_generation_success_checkpoint(2, 5);
        assert_eq!(running.phase, "generating");
        assert_eq!(running.progress, 40);
        assert_eq!(running.status, "running");
        assert_eq!(running.last_event, "progress");
        assert_eq!(running.last_message, "当前章节生成完成，继续下一章...");

        let completed = resolve_batch_generation_success_checkpoint(5, 5);
        assert_eq!(completed.phase, "completed");
        assert_eq!(completed.progress, 100);
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.last_event, "done");
        assert_eq!(completed.last_message, "批量生成完成");
    }
}
