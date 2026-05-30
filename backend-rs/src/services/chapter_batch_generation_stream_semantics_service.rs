use serde_json::Value;

use crate::models::batch_generation_task;
use crate::services::chapter_batch_generation_quality_status_service::{
    resolve_failed_terminal_semantics, BatchGenerationFailedTerminalKind,
    BatchGenerationQualityStatusContext,
};

#[derive(Debug, Clone)]
pub(crate) struct BatchGenerationStreamState {
    pub(crate) task: batch_generation_task::Model,
    pub(crate) status: String,
    pub(crate) completed: i32,
    pub(crate) progress: i32,
    pub(crate) message: String,
    pub(crate) event_status: &'static str,
    pub(crate) terminal_kind: Option<BatchGenerationStreamTerminalKind>,
    pub(crate) analysis_task_id: Option<String>,
    pub(crate) analysis_task_message: Option<String>,
    pub(crate) analysis_task_progress: Option<i32>,
    pub(crate) analysis_started_chapter_id: Option<String>,
    pub(crate) analysis_started_chapter_number: Option<i32>,
    pub(crate) terminal_label: Option<String>,
}

impl BatchGenerationStreamState {
    pub(crate) fn from_task_state(
        task: batch_generation_task::Model,
        workflow_runtime_state: Option<&Value>,
    ) -> Self {
        Self::from_task_state_with_quality_context(task, workflow_runtime_state, None)
    }

    pub(crate) fn from_task_state_with_quality_context(
        task: batch_generation_task::Model,
        workflow_runtime_state: Option<&Value>,
        quality_status_context: Option<&BatchGenerationQualityStatusContext>,
    ) -> Self {
        let status = task.status.clone();
        let completed = task.completed_chapters;
        let resolved_status = BatchGenerationResolvedStreamStatus::from_status(&status);
        let failed_terminal_semantics =
            resolve_failed_terminal_semantics(&task, Some(&task.failed_chapters), quality_status_context);
        let manual_review_terminal_label = failed_terminal_semantics
            .as_ref()
            .filter(|semantics| semantics.kind == BatchGenerationFailedTerminalKind::ManualReview)
            .map(|semantics| semantics.label.clone());
        let retryable_repair_terminal_label = failed_terminal_semantics
            .as_ref()
            .filter(|semantics| semantics.kind == BatchGenerationFailedTerminalKind::Retry)
            .map(|semantics| semantics.label.clone());
        let progress = workflow_runtime_state
            .and_then(|item| item.get("progress"))
            .and_then(Value::as_i64)
            .map(|value| value.clamp(0, 100) as i32)
            .unwrap_or_else(|| resolved_status.default_progress());
        let message = workflow_runtime_state
            .and_then(|item| item.get("last_message"))
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .or_else(|| manual_review_terminal_label.clone())
            .or_else(|| retryable_repair_terminal_label.clone())
            .unwrap_or_else(|| resolved_status.default_message().to_string())
            .to_string();
        let analysis_task_id = workflow_runtime_state
            .and_then(|item| item.get("analysis_task_id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty());
        let analysis_task_message = workflow_runtime_state
            .and_then(|item| item.get("analysis_task_message"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty());
        let analysis_task_progress = workflow_runtime_state
            .and_then(|item| item.get("analysis_task_progress"))
            .and_then(Value::as_i64)
            .map(|value| value.clamp(0, 100) as i32);
        let analysis_started_chapter_id = workflow_runtime_state
            .and_then(|item| item.get("analysis_started_chapter_id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty());
        let analysis_started_chapter_number = workflow_runtime_state
            .and_then(|item| item.get("analysis_started_chapter_number"))
            .and_then(Value::as_i64)
            .map(|value| value as i32);

        Self {
            task,
            status,
            completed,
            progress,
            message,
            event_status: resolved_status.event_status(),
            terminal_kind: resolved_status.terminal_kind(
                manual_review_terminal_label.as_ref(),
                retryable_repair_terminal_label.as_ref(),
            ),
            analysis_task_id,
            analysis_task_message,
            analysis_task_progress,
            analysis_started_chapter_id,
            analysis_started_chapter_number,
            terminal_label: manual_review_terminal_label.or(retryable_repair_terminal_label),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchGenerationResolvedStreamStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationStreamTerminalKind {
    Completed,
    Failed,
    Cancelled,
    ManualReview,
    Retry,
}

impl BatchGenerationResolvedStreamStatus {
    fn from_status(status: &str) -> Self {
        match status {
            "pending" => Self::Pending,
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Unknown,
        }
    }

    fn default_progress(self) -> i32 {
        match self {
            Self::Pending => 10,
            Self::Running => 65,
            Self::Completed | Self::Failed | Self::Cancelled => 100,
            Self::Unknown => 15,
        }
    }

    fn default_message(self) -> &'static str {
        match self {
            Self::Pending => "等待开始生成...",
            Self::Running => "正在生成正文...",
            Self::Completed => "生成完成",
            Self::Failed => "生成失败",
            Self::Cancelled => "生成已取消",
            Self::Unknown => "任务处理中",
        }
    }

    fn event_status(self) -> &'static str {
        match self {
            Self::Failed => "error",
            Self::Completed => "success",
            Self::Pending | Self::Running | Self::Cancelled | Self::Unknown => "processing",
        }
    }

    fn terminal_kind(
        self,
        manual_review_terminal_label: Option<&String>,
        retryable_repair_terminal_label: Option<&String>,
    ) -> Option<BatchGenerationStreamTerminalKind> {
        match self {
            Self::Completed => Some(BatchGenerationStreamTerminalKind::Completed),
            Self::Failed if manual_review_terminal_label.is_some() => {
                Some(BatchGenerationStreamTerminalKind::ManualReview)
            }
            Self::Failed if retryable_repair_terminal_label.is_some() => {
                Some(BatchGenerationStreamTerminalKind::Retry)
            }
            Self::Failed => Some(BatchGenerationStreamTerminalKind::Failed),
            Self::Cancelled => Some(BatchGenerationStreamTerminalKind::Cancelled),
            Self::Pending | Self::Running | Self::Unknown => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_quality_status_service::BatchGenerationQualityStatusContext;

    use super::{
        BatchGenerationResolvedStreamStatus, BatchGenerationStreamState,
        BatchGenerationStreamTerminalKind,
    };

    fn build_task(status: &str) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", "chapter-2"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-2".to_string()),
            current_chapter_number: Some(2),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    #[test]
    fn should_build_stream_state_with_checkpoint_fallbacks() {
        let running = BatchGenerationStreamState::from_task_state(build_task("running"), None);
        assert_eq!(running.progress, 65);
        assert_eq!(running.message, "正在生成正文...");
        assert_eq!(running.event_status, "processing");
        assert_eq!(running.terminal_kind, None);
        assert_eq!(running.analysis_task_id, None);
        assert_eq!(running.terminal_label, None);

        let completed = BatchGenerationStreamState::from_task_state(
            build_task("completed"),
            Some(&json!({
                "progress": 120,
                "last_message": "  ",
                "analysis_task_id": "analysis-task-1",
                "analysis_task_message": "第 2 章分析任务已启动",
                "analysis_task_progress": 85,
                "analysis_started_chapter_id": "chapter-2",
                "analysis_started_chapter_number": 2
            })),
        );
        assert_eq!(completed.progress, 100);
        assert_eq!(completed.message, "生成完成");
        assert_eq!(completed.event_status, "success");
        assert_eq!(completed.analysis_task_id.as_deref(), Some("analysis-task-1"));
        assert_eq!(
            completed.analysis_task_message.as_deref(),
            Some("第 2 章分析任务已启动")
        );
        assert_eq!(completed.analysis_task_progress, Some(85));
        assert_eq!(
            completed.analysis_started_chapter_id.as_deref(),
            Some("chapter-2")
        );
        assert_eq!(completed.analysis_started_chapter_number, Some(2));
        assert_eq!(
            completed.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Completed)
        );
        assert_eq!(completed.terminal_label, None);
    }

    #[test]
    fn should_build_stream_state_for_terminal_and_unknown_statuses() {
        let failed = BatchGenerationStreamState::from_task_state(build_task("failed"), None);
        assert_eq!(failed.progress, 100);
        assert_eq!(failed.message, "生成失败");
        assert_eq!(failed.event_status, "error");
        assert_eq!(
            failed.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Failed)
        );
        assert_eq!(failed.terminal_label, None);

        let cancelled = BatchGenerationStreamState::from_task_state(
            build_task("cancelled"),
            Some(&json!({
                "progress": -5,
                "last_message": "已停止"
            })),
        );
        assert_eq!(cancelled.progress, 0);
        assert_eq!(cancelled.message, "已停止");
        assert_eq!(cancelled.event_status, "processing");
        assert_eq!(cancelled.analysis_task_id, None);
        assert_eq!(
            cancelled.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Cancelled)
        );

        let unknown = BatchGenerationStreamState::from_task_state(build_task("queued"), None);
        assert_eq!(unknown.progress, 15);
        assert_eq!(unknown.message, "任务处理中");
        assert_eq!(unknown.event_status, "processing");
        assert_eq!(unknown.analysis_task_id, None);
        assert_eq!(unknown.terminal_kind, None);
        assert_eq!(unknown.terminal_label, None);
    }

    #[test]
    fn should_resolve_stream_status_owner_contract() {
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("completed").terminal_kind(None, None),
            Some(BatchGenerationStreamTerminalKind::Completed)
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("failed").terminal_kind(None, None),
            Some(BatchGenerationStreamTerminalKind::Failed)
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("cancelled").terminal_kind(None, None),
            Some(BatchGenerationStreamTerminalKind::Cancelled)
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("failed")
                .terminal_kind(Some(&"等待人工复核".to_string()), None),
            Some(BatchGenerationStreamTerminalKind::ManualReview)
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("failed")
                .terminal_kind(None, Some(&"自动修复后重试".to_string())),
            Some(BatchGenerationStreamTerminalKind::Retry)
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("failed").event_status(),
            "error"
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("completed").event_status(),
            "success"
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("running").event_status(),
            "processing"
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("running").terminal_kind(None, None),
            None
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("pending").default_progress(),
            10
        );
        assert_eq!(
            BatchGenerationResolvedStreamStatus::from_status("queued").default_message(),
            "任务处理中"
        );
    }

    #[test]
    fn should_build_stream_state_from_task_state_owner() {
        let state = BatchGenerationStreamState::from_task_state(
            build_task("running"),
            Some(&json!({"progress": 40, "last_message": "处理中"})),
        );

        assert_eq!(state.status, "running");
        assert_eq!(state.completed, 1);
        assert_eq!(state.progress, 40);
        assert_eq!(state.message, "处理中");
        assert_eq!(state.event_status, "processing");
        assert_eq!(state.analysis_task_id, None);
        assert_eq!(state.terminal_kind, None);
        assert_eq!(state.terminal_label, None);
    }

    #[test]
    fn should_resolve_manual_review_stream_state_from_quality_context_owner() {
        let state = BatchGenerationStreamState::from_task_state_with_quality_context(
            build_task("failed"),
            None,
            Some(&BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({
                    "quality_gate": {
                        "decision": "manual_review",
                        "label": "等待人工复核"
                    }
                })),
                quality_metrics_summary: None,
                active_story_repair_payload: None,
            }),
        );

        assert_eq!(state.message, "等待人工复核");
        assert_eq!(
            state.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::ManualReview)
        );
        assert_eq!(state.terminal_label.as_deref(), Some("等待人工复核"));
    }

    #[test]
    fn should_resolve_manual_review_stream_state_when_auto_repair_budget_is_exhausted() {
        let state = BatchGenerationStreamState::from_task_state_with_quality_context(
            {
                let mut task = build_task("failed");
                task.current_retry_count = 3;
                task.max_retries = 3;
                task
            },
            None,
            Some(&BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "自动修复预算已耗尽"
                    }
                })),
                quality_metrics_summary: None,
                active_story_repair_payload: None,
            }),
        );

        assert_eq!(state.message, "自动修复预算已耗尽");
        assert_eq!(
            state.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::ManualReview)
        );
        assert_eq!(state.terminal_label.as_deref(), Some("自动修复预算已耗尽"));
    }

    #[test]
    fn should_resolve_retry_stream_state_when_auto_repair_budget_is_available() {
        let state = BatchGenerationStreamState::from_task_state_with_quality_context(
            {
                let mut task = build_task("failed");
                task.current_retry_count = 1;
                task.max_retries = 3;
                task
            },
            None,
            Some(&BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "自动修复后重试"
                    }
                })),
                quality_metrics_summary: None,
                active_story_repair_payload: None,
            }),
        );

        assert_eq!(state.message, "自动修复后重试");
        assert_eq!(
            state.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Retry)
        );
        assert_eq!(state.terminal_label.as_deref(), Some("自动修复后重试"));
    }
}
