use serde_json::{json, Value};

use crate::services::chapter_batch_generation_stream_semantics_service::{
    BatchGenerationStreamState, BatchGenerationStreamTerminalKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationStreamCursor {
    pub(crate) status: String,
    pub(crate) completed: i32,
    pub(crate) progress: i32,
    pub(crate) message: String,
}

impl BatchGenerationStreamCursor {
    pub(crate) fn resolve_event_batch(
        &self,
        state: &BatchGenerationStreamState,
    ) -> Option<BatchGenerationStreamEventResolution> {
        if self.status == state.status
            && self.completed == state.completed
            && self.progress == state.progress
            && self.message == state.message
        {
            return None;
        }

        let events = state.events();

        Some(if state.terminal_kind.is_some() {
            BatchGenerationStreamEventResolution::Close { events }
        } else {
            BatchGenerationStreamEventResolution::Continue {
                events,
                next_cursor: Self {
                    status: state.status.clone(),
                    completed: state.completed,
                    progress: state.progress,
                    message: state.message.clone(),
                },
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BatchGenerationStreamEventResolution {
    Continue {
        events: Vec<Value>,
        next_cursor: BatchGenerationStreamCursor,
    },
    Close {
        events: Vec<Value>,
    },
}

impl BatchGenerationStreamState {
    fn events(&self) -> Vec<Value> {
        let mut events = vec![json!({
            "type": "progress",
            "message": self.message,
            "progress": self.progress,
            "status": self.event_status,
        })];

        if let Some(analysis_started_event) = self.analysis_started_event() {
            events.push(analysis_started_event);
        }

        if let Some(terminal_events) = self.terminal_events() {
            events.extend(terminal_events);
        }

        events
    }

    fn analysis_started_event(&self) -> Option<Value> {
        self.analysis_task_id.as_ref().map(|task_id| {
            json!({
                "type": "analysis_started",
                "task_id": task_id,
                "chapter_id": self.analysis_started_chapter_id,
                "chapter_number": self.analysis_started_chapter_number,
                "message": self
                    .analysis_task_message
                    .clone()
                    .unwrap_or_else(|| "章节分析任务已启动".to_string()),
                "progress": self.analysis_task_progress.unwrap_or(85),
            })
        })
    }

    pub(crate) fn terminal_events(&self) -> Option<Vec<Value>> {
        self.terminal_kind.map(|kind| match kind {
            BatchGenerationStreamTerminalKind::Completed => vec![
                json!({
                    "type": "result",
                    "data": {
                        "generation_task_id": self.task.id,
                        "chapter_id": self.task.current_chapter_id,
                        "content_source": "chapter",
                        "analysis_task_id": self.analysis_task_id,
                    }
                }),
                json!({"type":"done"}),
            ],
            BatchGenerationStreamTerminalKind::Failed => vec![json!({
                "type": "error",
                "error": self
                    .task
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "Generation task failed.".to_string()),
                "code": 500
            })],
            BatchGenerationStreamTerminalKind::ManualReview => vec![json!({
                "type": "error",
                "error": self
                    .terminal_label
                    .clone()
                    .unwrap_or_else(|| "需人工复核".to_string()),
                "code": 500,
                "phase": "quality_blocked",
                "terminal_reason": "manual_review",
                "terminal_label": self
                    .terminal_label
                    .clone()
                    .unwrap_or_else(|| "需人工复核".to_string()),
                "review_required": true,
                "can_resume": false
            })],
            BatchGenerationStreamTerminalKind::Retry => vec![json!({
                "type": "error",
                "error": self
                    .terminal_label
                    .clone()
                    .unwrap_or_else(|| "可自动修复后重试".to_string()),
                "code": 409,
                "phase": "repair_pending",
                "terminal_reason": "retry",
                "terminal_label": self
                    .terminal_label
                    .clone()
                    .unwrap_or_else(|| "可自动修复后重试".to_string()),
                "review_required": false,
                "can_resume": true
            })],
            BatchGenerationStreamTerminalKind::Cancelled => vec![json!({
                "type": "error",
                "error": "Generation task was cancelled.",
                "code": 499
            })],
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_stream_semantics_service::{
        BatchGenerationStreamState, BatchGenerationStreamTerminalKind,
    };
    use serde_json::json;

    use super::{BatchGenerationStreamCursor, BatchGenerationStreamEventResolution};

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
    fn should_build_batch_generation_stream_events() {
        let state = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            terminal_label: None,
        };

        let progress_event = BatchGenerationStreamCursor {
            status: String::new(),
            completed: -1,
            progress: -1,
            message: String::new(),
        }
        .resolve_event_batch(&state)
        .map(|batch| match batch {
            BatchGenerationStreamEventResolution::Continue { events, .. }
            | BatchGenerationStreamEventResolution::Close { events } => events
                .into_iter()
                .next()
                .expect("progress event"),
        })
        .expect("event batch");
        let result_event = state
            .terminal_events()
            .expect("completed terminal events")
            .into_iter()
            .find(|event| event["type"] == "result")
            .expect("result event");
        let failed_event = BatchGenerationStreamState {
            task: {
                let mut task = build_task("failed");
                task.error_message = Some("boom".to_string());
                task
            },
            status: "failed".to_string(),
            completed: 0,
            progress: 100,
            message: "生成失败".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Failed),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            terminal_label: None,
        }
        .terminal_events()
        .expect("failed terminal events")
        .into_iter()
        .next()
        .expect("failed event");

        assert_eq!(progress_event["type"], "progress");
        assert_eq!(progress_event["status"], "success");
        assert_eq!(result_event["type"], "result");
        assert_eq!(result_event["data"]["content_source"], "chapter");
        assert_eq!(result_event["data"]["analysis_task_id"], "analysis-task-1");
        assert_eq!(failed_event["error"], "boom");
    }

    #[test]
    fn should_build_stream_events_from_state_owner() {
        let state = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            terminal_label: None,
        };

        let events = state.events();

        assert_eq!(events.len(), 4);
        assert_eq!(events[0]["type"], "progress");
        assert_eq!(events[0]["status"], "success");
        assert_eq!(events[1]["type"], "analysis_started");
        assert_eq!(events[1]["task_id"], "analysis-task-1");
        assert_eq!(events[2]["type"], "result");
        assert_eq!(events[3]["type"], "done");
        assert!(state.terminal_kind.is_some());
    }

    #[test]
    fn should_build_terminal_batch_generation_events() {
        let completed = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            terminal_label: None,
        };
        let mut failed = BatchGenerationStreamState {
            task: build_task("failed"),
            status: "failed".to_string(),
            completed: 0,
            progress: 100,
            message: "生成失败".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Failed),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            terminal_label: None,
        };
        failed.task.error_message = Some("boom".to_string());
        let manual_review = BatchGenerationStreamState {
            task: build_task("failed"),
            status: "failed".to_string(),
            completed: 0,
            progress: 100,
            message: "等待人工复核".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::ManualReview),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            terminal_label: Some("等待人工复核".to_string()),
        };
        let retry = BatchGenerationStreamState {
            task: build_task("failed"),
            status: "failed".to_string(),
            completed: 0,
            progress: 100,
            message: "自动修复后重试".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Retry),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            terminal_label: Some("自动修复后重试".to_string()),
        };
        let cancelled = BatchGenerationStreamState {
            task: build_task("cancelled"),
            status: "cancelled".to_string(),
            completed: 0,
            progress: 100,
            message: "生成已取消".to_string(),
            event_status: "processing",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Cancelled),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            terminal_label: None,
        };

        let completed_events = completed.terminal_events().expect("completed events");
        assert_eq!(completed_events.len(), 2);
        assert_eq!(completed_events[0]["type"], "result");
        assert_eq!(completed_events[1]["type"], "done");

        let failed_events = failed.terminal_events().expect("failed events");
        assert_eq!(failed_events.len(), 1);
        assert_eq!(failed_events[0]["type"], "error");
        assert_eq!(failed_events[0]["error"], "boom");

        let manual_review_events = manual_review
            .terminal_events()
            .expect("manual review events");
        assert_eq!(manual_review_events.len(), 1);
        assert_eq!(manual_review_events[0]["type"], "error");
        assert_eq!(manual_review_events[0]["phase"], "quality_blocked");
        assert_eq!(manual_review_events[0]["terminal_reason"], "manual_review");
        assert_eq!(manual_review_events[0]["terminal_label"], "等待人工复核");
        assert_eq!(manual_review_events[0]["review_required"], true);
        assert_eq!(manual_review_events[0]["can_resume"], false);

        let retry_events = retry.terminal_events().expect("retry events");
        assert_eq!(retry_events.len(), 1);
        assert_eq!(retry_events[0]["type"], "error");
        assert_eq!(retry_events[0]["phase"], "repair_pending");
        assert_eq!(retry_events[0]["terminal_reason"], "retry");
        assert_eq!(retry_events[0]["terminal_label"], "自动修复后重试");
        assert_eq!(retry_events[0]["review_required"], false);
        assert_eq!(retry_events[0]["can_resume"], true);
        assert_eq!(retry_events[0]["code"], 409);

        let cancelled_events = cancelled.terminal_events().expect("cancelled events");
        assert_eq!(cancelled_events.len(), 1);
        assert_eq!(cancelled_events[0]["code"], 499);

        let running = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 65,
            message: "处理中".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            terminal_label: None,
        };
        assert!(running.terminal_events().is_none());
    }

    #[test]
    fn should_resolve_non_terminal_stream_event_batch_and_next_cursor() {
        let cursor = BatchGenerationStreamCursor {
            status: String::new(),
            completed: -1,
            progress: -1,
            message: String::new(),
        };
        let state = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 65,
            message: "处理中".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            terminal_label: None,
        };

        let batch = cursor.resolve_event_batch(&state).expect("event batch");

        match batch {
            BatchGenerationStreamEventResolution::Continue {
                events,
                next_cursor,
            } => {
                assert_eq!(events.len(), 1);
                assert_eq!(events[0]["type"], "progress");
                assert_eq!(events[0]["status"], "processing");
                assert_eq!(next_cursor.status, "running");
            }
            BatchGenerationStreamEventResolution::Close { .. } => {
                panic!("expected continue resolution")
            }
        }
    }

    #[test]
    fn should_skip_stream_event_batch_when_cursor_is_unchanged() {
        let state = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 1,
            progress: 65,
            message: "处理中".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            terminal_label: None,
        };
        let cursor = BatchGenerationStreamCursor {
            status: "running".to_string(),
            completed: 1,
            progress: 65,
            message: "处理中".to_string(),
        };

        assert!(cursor.resolve_event_batch(&state).is_none());
    }

    #[test]
    fn should_resolve_terminal_stream_event_batch_without_next_cursor() {
        let cursor = BatchGenerationStreamCursor {
            status: String::new(),
            completed: -1,
            progress: -1,
            message: String::new(),
        };
        let state = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            terminal_label: None,
        };

        let batch = cursor.resolve_event_batch(&state).expect("event batch");

        match batch {
            BatchGenerationStreamEventResolution::Close { events } => {
                assert_eq!(events.len(), 4);
                assert_eq!(events[0]["type"], "progress");
                assert_eq!(events[1]["type"], "analysis_started");
                assert_eq!(events[2]["type"], "result");
                assert_eq!(events[3]["type"], "done");
            }
            BatchGenerationStreamEventResolution::Continue { .. } => {
                panic!("expected close resolution")
            }
        }
    }

    #[test]
    fn should_keep_batch_generation_stream_close_contract_on_terminal_kind() {
        let completed = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            terminal_label: None,
        };
        let running = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 65,
            message: "处理中".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            terminal_label: None,
        };

        assert!(completed.terminal_kind.is_some());
        assert!(running.terminal_kind.is_none());
    }
}
