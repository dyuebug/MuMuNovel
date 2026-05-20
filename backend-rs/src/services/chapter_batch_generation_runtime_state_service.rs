use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ai::service::AIService;
use crate::ai::AIConfig;
use crate::models::{batch_generation_snapshot, batch_generation_task, chapter};
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
use crate::services::chapter_generation_runtime_service::generate_and_persist_chapter_content_with_provider_payload;

use super::chapter_single_generation_request_service::SingleChapterGenerationExecutionInput;

pub(crate) async fn load_batch_generation_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<Option<batch_generation_snapshot::Model>, String> {
    batch_generation_snapshot::Entity::find()
        .filter(batch_generation_snapshot::Column::BatchTaskId.eq(task_id))
        .one(db)
        .await
        .map_err(|error| error.to_string())
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
        let merged_runtime_state = match (
            active.workflow_runtime_state.clone().take(),
            workflow_runtime_state,
        ) {
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

fn build_single_generation_runtime_checkpoint(
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

fn build_batch_generation_runtime_checkpoint(
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

pub(crate) fn build_pending_batch_generation_runtime_checkpoint(
    last_event: &str,
    last_message: &str,
    chapter_id: Option<&str>,
    current_chapter_number: Option<i32>,
    progress_totals: Option<(i32, i32)>,
) -> Value {
    let mut checkpoint = build_runtime_checkpoint(
        "pending",
        0,
        "pending",
        last_event,
        last_message,
        chapter_id,
        current_chapter_number,
    );
    if let Some((completed, total)) = progress_totals {
        if let Some(object) = checkpoint.as_object_mut() {
            object.insert("completed".to_string(), json!(completed.max(0)));
            object.insert("total".to_string(), json!(total.max(0)));
        }
    }
    checkpoint
}

fn resume_runtime_position(task: &batch_generation_task::Model) -> (Option<String>, Option<i32>) {
    if task.chapter_count == 1 {
        (task.current_chapter_id.clone(), task.current_chapter_number)
    } else {
        (None, None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResumeBatchGenerationTaskResetPlan {
    current_chapter_id: Option<String>,
    current_chapter_number: Option<i32>,
    completed_chapters: i32,
    failed_chapters: Value,
    current_retry_count: i32,
}

fn resolve_resume_batch_generation_task_reset_plan(
    task: &batch_generation_task::Model,
) -> ResumeBatchGenerationTaskResetPlan {
    let (current_chapter_id, current_chapter_number) = resume_runtime_position(task);
    ResumeBatchGenerationTaskResetPlan {
        current_chapter_id,
        current_chapter_number,
        completed_chapters: 0,
        failed_chapters: json!([]),
        current_retry_count: 0,
    }
}

pub(crate) fn build_resume_batch_generation_runtime_checkpoint(
    task: &batch_generation_task::Model,
) -> Value {
    let reset_plan = resolve_resume_batch_generation_task_reset_plan(task);
    build_pending_batch_generation_runtime_checkpoint(
        "resume",
        "批量生成任务已恢复，等待重新开始...",
        reset_plan.current_chapter_id.as_deref(),
        reset_plan.current_chapter_number,
        (task.chapter_count > 1).then_some((0, task.total_chapters)),
    )
}

pub(crate) async fn reset_batch_generation_task_for_resume(
    db: &DatabaseConnection,
    task: &batch_generation_task::Model,
) -> Result<batch_generation_task::Model, String> {
    let reset_plan = resolve_resume_batch_generation_task_reset_plan(task);
    let mut active: batch_generation_task::ActiveModel = task.clone().into();
    active.status = Set("pending".to_string());
    active.error_message = Set(None);
    active.completed_at = Set(None);
    active.started_at = Set(None);
    active.completed_chapters = Set(reset_plan.completed_chapters);
    active.failed_chapters = Set(reset_plan.failed_chapters);
    active.current_retry_count = Set(reset_plan.current_retry_count);
    active.current_chapter_id = Set(reset_plan.current_chapter_id);
    active.current_chapter_number = Set(reset_plan.current_chapter_number);
    let updated_task = active.update(db).await.map_err(|error| error.to_string())?;
    let resume_checkpoint = build_resume_batch_generation_runtime_checkpoint(&updated_task);
    replace_batch_generation_runtime_snapshot_for_resume(db, &updated_task.id, resume_checkpoint)
        .await?;
    Ok(updated_task)
}

pub(crate) async fn replace_batch_generation_runtime_snapshot_for_resume(
    db: &DatabaseConnection,
    task_id: &str,
    workflow_runtime_state: Value,
) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let existing = load_batch_generation_snapshot(db, task_id).await?;

    if let Some(snapshot) = existing {
        let mut active: batch_generation_snapshot::ActiveModel = snapshot.into();
        active.latest_quality_metrics = Set(None);
        active.quality_metrics_history = Set(None);
        active.quality_metrics_summary = Set(None);
        active.workflow_runtime_state = Set(Some(workflow_runtime_state));
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

pub(crate) async fn persist_new_batch_generation_task_snapshot(
    db: &DatabaseConnection,
    task_id: &str,
    total_chapters: i32,
) -> Result<(), String> {
    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_pending_batch_generation_runtime_checkpoint(
            "queued",
            "批量生成任务已创建，等待开始...",
            None,
            None,
            Some((0, total_chapters)),
        ),
    )
    .await
}

pub(crate) async fn persist_new_single_generation_task_snapshot(
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

pub(crate) async fn prepare_single_generation_runtime(
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

pub(crate) async fn persist_single_generation_finalizing_snapshot(
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

pub(crate) async fn finalize_single_generation_success(
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

pub(crate) async fn finalize_single_generation_failure(
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

pub(crate) async fn prepare_batch_generation_runtime(
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

pub(crate) async fn finalize_batch_generation_cancelled(
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
        active.status = Set("cancelled".to_string());
        active.completed_at = Set(Some(Utc::now().naive_utc()));
        active.update(db).await.map_err(|error| error.to_string())?;
    }

    upsert_batch_generation_runtime_snapshot(
        db,
        task_id,
        build_cancelled_batch_generation_runtime_checkpoint(completed_chapters, total_chapters),
    )
    .await
}

fn build_cancelled_batch_generation_runtime_checkpoint(
    completed_chapters: i32,
    total_chapters: i32,
) -> Value {
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
    )
}

pub(crate) async fn mark_batch_generation_chapter_started(
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

pub(crate) async fn finalize_batch_generation_success(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_model: &chapter::Model,
    completed_chapters: i32,
    total_chapters: i32,
) -> Result<(), String> {
    let success_plan =
        resolve_batch_generation_success_checkpoint(completed_chapters, total_chapters);

    if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    {
        let mut active: batch_generation_task::ActiveModel = task_model.into();
        active.status = Set(success_plan.task_status.to_string());
        active.completed_chapters = Set(completed_chapters);
        active.current_chapter_id = Set(Some(chapter_model.id.clone()));
        active.current_chapter_number = Set(Some(chapter_model.chapter_number));
        if success_plan.should_complete_task {
            active.completed_at = Set(Some(Utc::now().naive_utc()));
        }
        active.error_message = Set(None);
        active.update(db).await.map_err(|error| error.to_string())?;
    }

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
    task_status: &'static str,
    should_complete_task: bool,
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
            task_status: "completed",
            should_complete_task: true,
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
        task_status: "running",
        should_complete_task: false,
        phase: "generating",
        progress: completed_progress,
        status: "running",
        last_event: "progress",
        last_message: "当前章节生成完成，继续下一章...",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationFailureKind {
    MissingChapter,
    LoadChapterError,
    GenerationError,
}

fn checkpoint_message_for_batch_generation_failure(
    kind: BatchGenerationFailureKind,
) -> &'static str {
    match kind {
        BatchGenerationFailureKind::MissingChapter => "批量生成失败：章节不存在",
        BatchGenerationFailureKind::LoadChapterError => "批量生成失败：加载章节异常",
        BatchGenerationFailureKind::GenerationError => "批量生成失败",
    }
}

pub(crate) async fn finalize_batch_generation_failure(
    db: &DatabaseConnection,
    task_id: &str,
    chapter_id: Option<&str>,
    chapter_number: Option<i32>,
    completed_chapters: i32,
    total_chapters: i32,
    failure_kind: BatchGenerationFailureKind,
    task_error_message: String,
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
            checkpoint_message_for_batch_generation_failure(failure_kind),
            chapter_id,
            chapter_number,
            completed_chapters,
            total_chapters,
        ),
    )
    .await
}

pub(crate) async fn execute_single_generation_runtime(
    db: &DatabaseConnection,
    task_id: &str,
    user_id: &str,
    execution_input: SingleChapterGenerationExecutionInput,
) {
    let SingleChapterGenerationExecutionInput {
        chapter_id,
        target_word_count,
        ai_config,
        provider_payload,
    } = execution_input;
    let _ = prepare_single_generation_runtime(db, task_id, &chapter_id).await;

    let ai_service = AIService::new(ai_config);
    let generation_result = generate_and_persist_chapter_content_with_provider_payload(
        db,
        &ai_service,
        user_id,
        &chapter_id,
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
                .unwrap_or(&chapter_id);
            let _ = persist_single_generation_finalizing_snapshot(
                db,
                task_id,
                &chapter_id,
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
            let _ = finalize_single_generation_failure(db, task_id, &chapter_id, error).await;
        }
    }
}

pub(crate) async fn execute_batch_generation_runtime(
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
        let task_model = match batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
        {
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
                    BatchGenerationFailureKind::MissingChapter,
                    format!("Chapter not found: {}", chapter_id),
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
                    BatchGenerationFailureKind::LoadChapterError,
                    error.to_string(),
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
                    BatchGenerationFailureKind::GenerationError,
                    error,
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
        build_cancelled_batch_generation_runtime_checkpoint,
        build_pending_batch_generation_runtime_checkpoint,
        build_resume_batch_generation_runtime_checkpoint,
        checkpoint_message_for_batch_generation_failure, compute_batch_running_progress,
        resolve_batch_generation_success_checkpoint,
        resolve_resume_batch_generation_task_reset_plan, BatchGenerationFailureKind,
    };
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
    fn should_compute_batch_running_progress_with_floor_and_clamp() {
        assert_eq!(compute_batch_running_progress(0, 0), 15);
        assert_eq!(compute_batch_running_progress(2, 5), 55);
        assert_eq!(compute_batch_running_progress(5, 5), 95);
        assert_eq!(compute_batch_running_progress(7, 5), 95);
    }

    #[test]
    fn should_resolve_batch_generation_success_checkpoint_for_running_and_completed_states() {
        let running = resolve_batch_generation_success_checkpoint(2, 5);
        assert_eq!(running.task_status, "running");
        assert!(!running.should_complete_task);
        assert_eq!(running.phase, "generating");
        assert_eq!(running.progress, 40);
        assert_eq!(running.status, "running");
        assert_eq!(running.last_event, "progress");
        assert_eq!(running.last_message, "当前章节生成完成，继续下一章...");

        let completed = resolve_batch_generation_success_checkpoint(5, 5);
        assert_eq!(completed.task_status, "completed");
        assert!(completed.should_complete_task);
        assert_eq!(completed.phase, "completed");
        assert_eq!(completed.progress, 100);
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.last_event, "done");
        assert_eq!(completed.last_message, "批量生成完成");
    }

    #[test]
    fn should_build_pending_batch_generation_runtime_checkpoint_for_queued_batch_task() {
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

    #[test]
    fn should_build_resume_runtime_checkpoint_for_single_generation_task() {
        let mut single_task = build_task("failed");
        single_task.chapter_count = 1;
        single_task.chapter_ids = json!(["chapter-1"]);
        single_task.current_chapter_id = Some("chapter-1".to_string());
        single_task.current_chapter_number = Some(3);

        let with_chapter = build_resume_batch_generation_runtime_checkpoint(&single_task);
        assert_eq!(with_chapter["phase"], "pending");
        assert_eq!(with_chapter["progress"], 0);
        assert_eq!(with_chapter["status"], "pending");
        assert_eq!(with_chapter["last_event"], "resume");
        assert_eq!(
            with_chapter["last_message"],
            "批量生成任务已恢复，等待重新开始..."
        );
        assert_eq!(with_chapter["chapter_id"], "chapter-1");
        assert_eq!(with_chapter["current_chapter_id"], "chapter-1");
        assert_eq!(with_chapter["current_chapter_number"], 3);
        assert!(with_chapter.get("completed").is_none());
        assert!(with_chapter.get("total").is_none());
    }

    #[test]
    fn should_resolve_resume_task_reset_plan_for_single_generation_task() {
        let mut single_task = build_task("failed");
        single_task.chapter_count = 1;
        single_task.current_chapter_id = Some("chapter-9".to_string());
        single_task.current_chapter_number = Some(9);
        single_task.completed_chapters = 1;
        single_task.failed_chapters = json!([{"chapter_id": "chapter-9"}]);
        single_task.current_retry_count = 2;

        let reset_plan = resolve_resume_batch_generation_task_reset_plan(&single_task);

        assert_eq!(reset_plan.current_chapter_id.as_deref(), Some("chapter-9"));
        assert_eq!(reset_plan.current_chapter_number, Some(9));
        assert_eq!(reset_plan.completed_chapters, 0);
        assert_eq!(reset_plan.failed_chapters, json!([]));
        assert_eq!(reset_plan.current_retry_count, 0);
    }

    #[test]
    fn should_resolve_resume_task_reset_plan_for_batch_generation_task() {
        let mut batch_task = build_task("cancelled");
        batch_task.chapter_count = 3;
        batch_task.chapter_ids = json!(["chapter-1", "chapter-2", "chapter-3"]);
        batch_task.current_chapter_id = Some("chapter-2".to_string());
        batch_task.current_chapter_number = Some(2);
        batch_task.completed_chapters = 2;
        batch_task.failed_chapters = json!([{"chapter_id": "chapter-2"}]);
        batch_task.current_retry_count = 1;

        let reset_plan = resolve_resume_batch_generation_task_reset_plan(&batch_task);

        assert!(reset_plan.current_chapter_id.is_none());
        assert!(reset_plan.current_chapter_number.is_none());
        assert_eq!(reset_plan.completed_chapters, 0);
        assert_eq!(reset_plan.failed_chapters, json!([]));
        assert_eq!(reset_plan.current_retry_count, 0);
    }

    #[test]
    fn should_clear_batch_resume_runtime_position_and_progress() {
        let mut batch_task = build_task("cancelled");
        batch_task.chapter_count = 3;
        batch_task.chapter_ids = json!(["chapter-1", "chapter-2", "chapter-3"]);
        batch_task.total_chapters = 3;
        batch_task.completed_chapters = 2;
        batch_task.current_chapter_id = Some("chapter-2".to_string());
        batch_task.current_chapter_number = Some(2);

        let reset_plan = resolve_resume_batch_generation_task_reset_plan(&batch_task);
        assert!(reset_plan.current_chapter_id.is_none());
        assert!(reset_plan.current_chapter_number.is_none());

        let checkpoint = build_resume_batch_generation_runtime_checkpoint(&batch_task);
        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(checkpoint["progress"], 0);
        assert_eq!(checkpoint["status"], "pending");
        assert_eq!(checkpoint["last_event"], "resume");
        assert_eq!(
            checkpoint["last_message"],
            "批量生成任务已恢复，等待重新开始..."
        );
        assert!(checkpoint["chapter_id"].is_null());
        assert!(checkpoint["current_chapter_id"].is_null());
        assert!(checkpoint["current_chapter_number"].is_null());
        assert_eq!(checkpoint["completed"], 0);
        assert_eq!(checkpoint["total"], 3);
    }

    #[test]
    fn should_build_cancelled_runtime_checkpoint_with_terminal_progress() {
        let checkpoint = build_cancelled_batch_generation_runtime_checkpoint(2, 5);

        assert_eq!(checkpoint["phase"], "cancelled");
        assert_eq!(checkpoint["progress"], 100);
        assert_eq!(checkpoint["status"], "cancelled");
        assert_eq!(checkpoint["last_event"], "cancelled");
        assert_eq!(checkpoint["last_message"], "批量生成已取消");
        assert_eq!(checkpoint["completed"], 2);
        assert_eq!(checkpoint["total"], 5);
        assert!(checkpoint["chapter_id"].is_null());
        assert!(checkpoint["current_chapter_id"].is_null());
    }

    #[test]
    fn should_resolve_checkpoint_message_for_batch_failure_kind() {
        assert_eq!(
            checkpoint_message_for_batch_generation_failure(
                BatchGenerationFailureKind::MissingChapter
            ),
            "批量生成失败：章节不存在"
        );
        assert_eq!(
            checkpoint_message_for_batch_generation_failure(
                BatchGenerationFailureKind::LoadChapterError
            ),
            "批量生成失败：加载章节异常"
        );
        assert_eq!(
            checkpoint_message_for_batch_generation_failure(
                BatchGenerationFailureKind::GenerationError
            ),
            "批量生成失败"
        );
    }
}
