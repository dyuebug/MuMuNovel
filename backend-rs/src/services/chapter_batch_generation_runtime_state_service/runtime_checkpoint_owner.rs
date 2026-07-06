use chrono::Utc;
use serde_json::{json, Value};

pub(crate) fn build_batch_generation_runtime_checkpoint_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::runtime_checkpoint_projection",
        "scope": "queued_resumed_running_cancelled_failed_checkpoint_stage_projection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_checkpoint_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_persistence_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/retry_routing_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/startup_and_command_projection_owner.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "checkpoint_entrypoints": [
                "build_pending_batch_generation_runtime_checkpoint",
                "build_batch_generation_runtime_checkpoint_for_stage",
                "BatchGenerationSnapshotStage::build_checkpoint"
            ],
            "checkpoint_helpers": [
                "compute_batch_running_progress",
                "checkpoint_message_for_batch_generation_failure",
                "clear_batch_analysis_runtime_metadata"
            ],
            "checkpoint_stages": [
                "queued",
                "resumed",
                "preparing",
                "chapter_started",
                "chapter_succeeded",
                "cancelled",
                "failed"
            ],
            "projection_fields": [
                "phase",
                "progress",
                "status",
                "last_event",
                "last_message",
                "chapter_id",
                "current_chapter_id",
                "current_chapter_number",
                "completed",
                "total",
                "updated_at"
            ],
            "analysis_runtime_fields_cleared": [
                "analysis_task_id",
                "analysis_task_message",
                "analysis_task_progress",
                "analysis_started_chapter_id",
                "analysis_started_chapter_number",
                "analysis_started_at"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_runtime_state_service::runtime_persistence_owner",
            "chapter_batch_generation_runtime_state_service::retry_routing_owner",
            "chapter_batch_generation_runtime_state_service::startup_cancel_resume_task_payload_projection",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "batch_generation_runtime_checkpoint_owner_is_rust_only_and_surviving_checkpoint_projection_surfaces_are_tracked_by_external_runtime_contracts",
            "runtime_state_keys": [
                "phase",
                "progress",
                "status",
                "last_event",
                "last_message",
                "chapter_id",
                "current_chapter_id",
                "current_chapter_number",
                "completed",
                "total",
                "analysis_task_id",
                "analysis_task_message",
                "analysis_task_progress"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_batch_runtime_checkpoint_smoke"
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationFailureKind {
    MissingChapter,
    LoadChapterError,
    GenerationError,
    #[allow(dead_code)]
    QualityGateBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationSnapshotStage {
    Queued,
    Resumed { include_progress_totals: bool },
    Preparing,
    ChapterStarted,
    ChapterSucceeded,
    Cancelled,
    Failed(BatchGenerationFailureKind),
}

pub(crate) fn build_pending_batch_generation_runtime_checkpoint(
    last_event: &str,
    last_message: &str,
    chapter_id: Option<&str>,
    current_chapter_number: Option<i32>,
    progress_totals: Option<(i32, i32)>,
) -> Value {
    let mut checkpoint = json!({
        "phase": "pending",
        "progress": 0,
        "status": "pending",
        "last_event": last_event,
        "last_message": last_message,
        "chapter_id": chapter_id,
        "current_chapter_id": chapter_id,
        "current_chapter_number": current_chapter_number,
        "updated_at": Utc::now().to_rfc3339(),
    });
    if let Some((completed, total)) = progress_totals {
        if let Some(object) = checkpoint.as_object_mut() {
            object.insert("completed".to_string(), json!(completed.max(0)));
            object.insert("total".to_string(), json!(total.max(0)));
        }
    }
    checkpoint
}

pub(crate) fn compute_batch_running_progress(completed_chapters: i32, total_chapters: i32) -> i32 {
    if total_chapters <= 0 {
        return 15;
    }

    let base_progress = ((completed_chapters * 100) / total_chapters).clamp(0, 100);
    (base_progress + 15).clamp(15, 95)
}

pub(crate) fn checkpoint_message_for_batch_generation_failure(
    kind: BatchGenerationFailureKind,
) -> &'static str {
    match kind {
        BatchGenerationFailureKind::MissingChapter => "批量生成失败：章节不存在",
        BatchGenerationFailureKind::LoadChapterError => "批量生成失败：加载章节异常",
        BatchGenerationFailureKind::GenerationError => "批量生成失败",
        BatchGenerationFailureKind::QualityGateBlocked => "批量生成失败：质量门禁未通过",
    }
}

fn clear_batch_analysis_runtime_metadata(checkpoint: &mut Value) {
    if let Some(object) = checkpoint.as_object_mut() {
        object.insert("analysis_task_id".to_string(), Value::Null);
        object.insert("analysis_task_message".to_string(), Value::Null);
        object.insert("analysis_task_progress".to_string(), Value::Null);
        object.insert("analysis_started_chapter_id".to_string(), Value::Null);
        object.insert("analysis_started_chapter_number".to_string(), Value::Null);
        object.insert("analysis_started_at".to_string(), Value::Null);
    }
}

impl BatchGenerationSnapshotStage {
    fn build_checkpoint(
        self,
        chapter_id: Option<&str>,
        completed_chapters: i32,
        total_chapters: i32,
        current_chapter_number: Option<i32>,
    ) -> Value {
        let build_runtime_checkpoint =
            |phase: &str, progress: i32, status: &str, last_event: &str, last_message: &str| {
                let mut checkpoint = json!({
                    "phase": phase,
                    "progress": progress.clamp(0, 100),
                    "status": status,
                    "last_event": last_event,
                    "last_message": last_message,
                    "chapter_id": chapter_id,
                    "current_chapter_id": chapter_id,
                    "current_chapter_number": current_chapter_number,
                    "updated_at": Utc::now().to_rfc3339(),
                });
                if let Some(object) = checkpoint.as_object_mut() {
                    object.insert("completed".to_string(), json!(completed_chapters.max(0)));
                    object.insert("total".to_string(), json!(total_chapters.max(0)));
                }
                checkpoint
            };

        match self {
            BatchGenerationSnapshotStage::Queued => {
                let mut checkpoint = build_pending_batch_generation_runtime_checkpoint(
                    "queued",
                    "批量生成任务已创建，等待开始...",
                    chapter_id,
                    current_chapter_number,
                    Some((0, total_chapters)),
                );
                clear_batch_analysis_runtime_metadata(&mut checkpoint);
                checkpoint
            }
            BatchGenerationSnapshotStage::Resumed {
                include_progress_totals,
            } => {
                let mut checkpoint = build_pending_batch_generation_runtime_checkpoint(
                    "resume",
                    "批量生成任务已恢复，等待重新开始...",
                    chapter_id,
                    current_chapter_number,
                    include_progress_totals.then_some((0, total_chapters)),
                );
                clear_batch_analysis_runtime_metadata(&mut checkpoint);
                checkpoint
            }
            BatchGenerationSnapshotStage::Preparing => {
                let mut checkpoint = build_runtime_checkpoint(
                    "generating",
                    10,
                    "running",
                    "progress",
                    "正在准备批量生成...",
                );
                clear_batch_analysis_runtime_metadata(&mut checkpoint);
                checkpoint
            }
            BatchGenerationSnapshotStage::ChapterStarted => {
                let mut checkpoint = build_runtime_checkpoint(
                    "generating",
                    compute_batch_running_progress(completed_chapters, total_chapters),
                    "running",
                    "chapter_start",
                    &match current_chapter_number {
                        Some(chapter_number) => format!("正在生成第 {} 章...", chapter_number),
                        None => "正在生成章节...".to_string(),
                    },
                );
                clear_batch_analysis_runtime_metadata(&mut checkpoint);
                checkpoint
            }
            BatchGenerationSnapshotStage::ChapterSucceeded => {
                if completed_chapters >= total_chapters {
                    return build_runtime_checkpoint(
                        "completed",
                        100,
                        "completed",
                        "done",
                        "批量生成完成",
                    );
                }

                let completed_progress = if total_chapters <= 0 {
                    100
                } else {
                    ((completed_chapters * 100) / total_chapters).clamp(0, 100)
                };

                build_runtime_checkpoint(
                    "generating",
                    completed_progress,
                    "running",
                    "progress",
                    "当前章节生成完成，继续下一章...",
                )
            }
            BatchGenerationSnapshotStage::Cancelled => {
                let mut checkpoint = build_runtime_checkpoint(
                    "cancelled",
                    100,
                    "cancelled",
                    "cancelled",
                    "批量生成已取消",
                );
                clear_batch_analysis_runtime_metadata(&mut checkpoint);
                checkpoint
            }
            BatchGenerationSnapshotStage::Failed(failure_kind) => {
                let mut checkpoint = build_runtime_checkpoint(
                    "failed",
                    100,
                    "failed",
                    "error",
                    checkpoint_message_for_batch_generation_failure(failure_kind),
                );
                clear_batch_analysis_runtime_metadata(&mut checkpoint);
                checkpoint
            }
        }
    }
}

pub(crate) fn build_batch_generation_runtime_checkpoint_for_stage(
    stage: BatchGenerationSnapshotStage,
    chapter_id: Option<&str>,
    current_chapter_number: Option<i32>,
    completed_chapters: i32,
    total_chapters: i32,
) -> Value {
    stage.build_checkpoint(
        chapter_id,
        completed_chapters,
        total_chapters,
        current_chapter_number,
    )
}
