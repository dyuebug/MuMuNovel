use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationFailureKind {
    MissingChapter,
    LoadChapterError,
    GenerationError,
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
        "updated_at": chrono::Utc::now().to_rfc3339(),
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
        BatchGenerationFailureKind::QualityGateBlocked => "批量生成失败：质量门禁阻断，需人工复核",
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
        let build_runtime_checkpoint = |phase: &str,
                                        progress: i32,
                                        status: &str,
                                        last_event: &str,
                                        last_message: &str| {
            let mut checkpoint = json!({
                "phase": phase,
                "progress": progress.clamp(0, 100),
                "status": status,
                "last_event": last_event,
                "last_message": last_message,
                "chapter_id": chapter_id,
                "current_chapter_id": chapter_id,
                "current_chapter_number": current_chapter_number,
                "updated_at": chrono::Utc::now().to_rfc3339(),
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

#[cfg(test)]
mod tests {
    use super::{
        build_batch_generation_runtime_checkpoint_for_stage,
        build_pending_batch_generation_runtime_checkpoint,
        checkpoint_message_for_batch_generation_failure, compute_batch_running_progress,
        BatchGenerationFailureKind, BatchGenerationSnapshotStage,
    };

    #[test]
    fn should_compute_batch_running_progress_with_floor_and_clamp() {
        assert_eq!(compute_batch_running_progress(0, 0), 15);
        assert_eq!(compute_batch_running_progress(2, 5), 55);
        assert_eq!(compute_batch_running_progress(5, 5), 95);
        assert_eq!(compute_batch_running_progress(7, 5), 95);
    }

    #[test]
    fn should_build_batch_generation_runtime_checkpoint_for_started_stage() {
        let checkpoint = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::ChapterStarted,
            Some("chapter-3"),
            Some(3),
            2,
            5,
        );

        assert_eq!(checkpoint["phase"], "generating");
        assert_eq!(checkpoint["progress"], 55);
        assert_eq!(checkpoint["status"], "running");
        assert_eq!(checkpoint["last_event"], "chapter_start");
        assert_eq!(checkpoint["last_message"], "正在生成第 3 章...");
        assert_eq!(checkpoint["chapter_id"], "chapter-3");
        assert_eq!(checkpoint["current_chapter_id"], "chapter-3");
        assert_eq!(checkpoint["current_chapter_number"], 3);
        assert_eq!(checkpoint["completed"], 2);
        assert_eq!(checkpoint["total"], 5);
        assert!(checkpoint["analysis_task_id"].is_null());
        assert!(checkpoint["analysis_started_chapter_id"].is_null());
    }

    #[test]
    fn should_build_pending_batch_generation_runtime_checkpoint_for_queue_and_resume() {
        let queued = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Queued,
            None,
            None,
            0,
            4,
        );
        assert_eq!(queued["phase"], "pending");
        assert_eq!(queued["progress"], 0);
        assert_eq!(queued["status"], "pending");
        assert_eq!(queued["last_event"], "queued");
        assert_eq!(queued["last_message"], "批量生成任务已创建，等待开始...");
        assert_eq!(queued["completed"], 0);
        assert_eq!(queued["total"], 4);
        assert!(queued["analysis_task_id"].is_null());

        let resumed = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Resumed {
                include_progress_totals: false,
            },
            Some("chapter-3"),
            Some(3),
            0,
            5,
        );
        assert_eq!(resumed["phase"], "pending");
        assert_eq!(resumed["status"], "pending");
        assert_eq!(resumed["last_event"], "resume");
        assert_eq!(
            resumed["last_message"],
            "批量生成任务已恢复，等待重新开始..."
        );
        assert!(resumed["analysis_task_id"].is_null());
        assert!(resumed.get("completed").is_none());
        assert!(resumed.get("total").is_none());
    }

    #[test]
    fn should_build_pending_batch_generation_runtime_checkpoint_with_progress_totals() {
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
        assert!(checkpoint.get("analysis_task_id").is_none());
        assert!(checkpoint["chapter_id"].is_null());
    }

    #[test]
    fn should_build_cancelled_and_failed_runtime_checkpoints() {
        let cancelled = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Cancelled,
            None,
            None,
            2,
            5,
        );
        assert_eq!(cancelled["phase"], "cancelled");
        assert_eq!(cancelled["progress"], 100);
        assert_eq!(cancelled["status"], "cancelled");
        assert_eq!(cancelled["last_event"], "cancelled");
        assert_eq!(cancelled["last_message"], "批量生成已取消");
        assert!(cancelled["analysis_task_id"].is_null());

        let failed = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::Failed(BatchGenerationFailureKind::LoadChapterError),
            Some("chapter-2"),
            Some(2),
            1,
            5,
        );
        assert_eq!(failed["phase"], "failed");
        assert_eq!(failed["progress"], 100);
        assert_eq!(failed["status"], "failed");
        assert_eq!(failed["last_event"], "error");
        assert_eq!(failed["last_message"], "批量生成失败：加载章节异常");
        assert!(failed["analysis_task_id"].is_null());
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
