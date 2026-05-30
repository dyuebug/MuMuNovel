use chrono::Utc;
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SingleGenerationSnapshotStage {
    Pending,
    Preparing,
    Generating,
    Finalizing,
    Completed,
    Failed,
}

impl SingleGenerationSnapshotStage {
    fn build_checkpoint(
        self,
        chapter_id: &str,
        current_chapter_number: Option<i32>,
        word_count: Option<i32>,
    ) -> Value {
        let (phase, progress, status, last_event, last_message) = match self {
            SingleGenerationSnapshotStage::Pending => (
                "pending",
                0,
                "pending",
                "queued",
                "单章生成任务已创建，等待开始...",
            ),
            SingleGenerationSnapshotStage::Preparing => (
                "generating",
                15,
                "running",
                "chapter_start",
                "正在准备章节生成...",
            ),
            SingleGenerationSnapshotStage::Generating => (
                "generating",
                65,
                "running",
                "progress",
                "正在生成正文...",
            ),
            SingleGenerationSnapshotStage::Finalizing => (
                "finalizing",
                95,
                "running",
                "progress",
                "正在整理生成结果...",
            ),
            SingleGenerationSnapshotStage::Completed => (
                "completed",
                100,
                "completed",
                "done",
                "章节生成完成",
            ),
            SingleGenerationSnapshotStage::Failed => (
                "failed",
                100,
                "failed",
                "error",
                "章节生成失败",
            ),
        };
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
            if let Some(value) = word_count {
                object.insert("word_count".to_string(), json!(value.max(0)));
            }
        }

        checkpoint
    }
}

pub(crate) fn build_single_generation_runtime_checkpoint_for_stage(
    stage: SingleGenerationSnapshotStage,
    chapter_id: &str,
    current_chapter_number: Option<i32>,
    word_count: Option<i32>,
) -> Value {
    stage.build_checkpoint(chapter_id, current_chapter_number, word_count)
}

#[cfg(test)]
mod tests {
    use super::{
        build_single_generation_runtime_checkpoint_for_stage, SingleGenerationSnapshotStage,
    };

    #[test]
    fn should_build_single_generation_runtime_checkpoint_for_pending_generating_and_completed_stages() {
        let pending = build_single_generation_runtime_checkpoint_for_stage(
            SingleGenerationSnapshotStage::Pending,
            "chapter-1",
            Some(1),
            None,
        );
        assert_eq!(pending["phase"], "pending");
        assert_eq!(pending["progress"], 0);
        assert_eq!(pending["status"], "pending");
        assert_eq!(pending["last_event"], "queued");
        assert_eq!(pending["last_message"], "单章生成任务已创建，等待开始...");

        let generating = build_single_generation_runtime_checkpoint_for_stage(
            SingleGenerationSnapshotStage::Generating,
            "chapter-1",
            Some(1),
            None,
        );
        assert_eq!(generating["phase"], "generating");
        assert_eq!(generating["progress"], 65);
        assert_eq!(generating["status"], "running");
        assert_eq!(generating["last_event"], "progress");
        assert_eq!(generating["last_message"], "正在生成正文...");

        let completed = build_single_generation_runtime_checkpoint_for_stage(
            SingleGenerationSnapshotStage::Completed,
            "chapter-1",
            Some(1),
            Some(2800),
        );
        assert_eq!(completed["phase"], "completed");
        assert_eq!(completed["progress"], 100);
        assert_eq!(completed["status"], "completed");
        assert_eq!(completed["last_event"], "done");
        assert_eq!(completed["last_message"], "章节生成完成");
        assert_eq!(completed["word_count"], 2800);
    }

    #[test]
    fn should_build_single_generation_runtime_checkpoint_for_stage() {
        let checkpoint = build_single_generation_runtime_checkpoint_for_stage(
            SingleGenerationSnapshotStage::Finalizing,
            "chapter-1",
            Some(3),
            Some(2800),
        );

        assert_eq!(checkpoint["phase"], "finalizing");
        assert_eq!(checkpoint["progress"], 95);
        assert_eq!(checkpoint["status"], "running");
        assert_eq!(checkpoint["last_event"], "progress");
        assert_eq!(checkpoint["last_message"], "正在整理生成结果...");
        assert_eq!(checkpoint["chapter_id"], "chapter-1");
        assert_eq!(checkpoint["current_chapter_id"], "chapter-1");
        assert_eq!(checkpoint["current_chapter_number"], 3);
        assert_eq!(checkpoint["word_count"], 2800);
    }
}
