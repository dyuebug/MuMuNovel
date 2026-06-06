use axum::response::sse::Event;
use serde_json::{json, Value};

use crate::services::chapter_batch_generation_stream_semantics_service::{
    BatchGenerationStreamObservationKey, BatchGenerationStreamState,
    BatchGenerationStreamTerminalKind,
};

pub(crate) fn batch_generation_stream_connected_event_payload() -> Value {
    json!({
        "type": "progress",
        "message": "正在连接批量生成任务流",
        "progress": 0,
        "status": "processing"
    })
}

pub(crate) fn batch_generation_stream_task_not_found_event_payload() -> Value {
    json!({
        "type": "error",
        "error": "批量生成任务不存在",
        "code": 404
    })
}

pub(crate) fn batch_generation_stream_timeout_event_payload() -> Value {
    json!({
        "type": "error",
        "error": "批量生成任务流等待超时",
        "code": 408
    })
}

pub(crate) fn batch_generation_stream_heartbeat_comment() -> &'static str {
    "heartbeat"
}

pub(crate) fn batch_generation_stream_data_event(payload: Value) -> Event {
    Event::default().data(payload.to_string())
}

pub(crate) fn batch_generation_stream_heartbeat_event() -> Event {
    Event::default().comment(batch_generation_stream_heartbeat_comment())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationStreamCursor {
    pub(crate) observation: Option<BatchGenerationStreamObservationKey>,
}

impl BatchGenerationStreamCursor {
    pub(crate) fn resolve_event_batch(
        &self,
        state: &BatchGenerationStreamState,
    ) -> Option<BatchGenerationStreamEventResolution> {
        let next_observation = state.observation_key();
        if self.observation.as_ref() == Some(&next_observation) {
            return None;
        }

        let events = state.events();

        Some(if state.terminal_kind.is_some() {
            BatchGenerationStreamEventResolution::Close { events }
        } else {
            BatchGenerationStreamEventResolution::Continue {
                events,
                next_cursor: Self {
                    observation: Some(next_observation),
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
        let mut progress_event = json!({
            "type": "progress",
            "message": self.message,
            "progress": self.progress,
            "status": self.event_status,
            "phase": self.phase,
            "current_retry_count": self.task.current_retry_count,
            "max_retries": self.task.max_retries,
        });
        if let Some(quality_gate) = self.quality_gate.as_ref() {
            progress_event["quality_gate"] = quality_gate.clone();
        }
        if let Some(active_story_repair_payload) = self.active_story_repair_payload.as_ref() {
            progress_event["active_story_repair_payload"] = active_story_repair_payload.clone();
        }
        let mut events = vec![progress_event];

        if let Some(analysis_started_event) = self.analysis_started_event() {
            events.push(analysis_started_event);
        }

        if let Some(terminal_events) = self.terminal_events() {
            events.extend(terminal_events);
        }

        events
    }

    fn analysis_started_event(&self) -> Option<Value> {
        let chapter_id = self.analysis_started_chapter_id.as_ref()?;
        let mut event = json!({
            "type": "analysis_started",
            "chapter_id": chapter_id,
            "chapter_number": self.analysis_started_chapter_number,
            "message": self
                .analysis_task_message
                .clone()
                .unwrap_or_else(|| "章节分析任务已启动".to_string()),
            "progress": self.analysis_task_progress.unwrap_or(85),
            "phase": "parsing",
            "current_retry_count": self.task.current_retry_count,
            "max_retries": self.task.max_retries,
        });
        if let Some(task_id) = self.analysis_task_id.as_ref() {
            event["task_id"] = json!(task_id);
        }
        Some(event)
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
            BatchGenerationStreamTerminalKind::Failed => vec![
                json!({
                    "type": "error",
                    "error": self
                        .task
                        .error_message
                        .clone()
                        .unwrap_or_else(|| "批量生成任务执行失败".to_string()),
                    "code": 500,
                    "phase": "failed"
                }),
                json!({"type":"done"}),
            ],
            BatchGenerationStreamTerminalKind::ManualReview => vec![
                json!({
                    "type": "error",
                    "error": self
                        .task
                        .error_message
                        .clone()
                        .unwrap_or_else(|| "需人工复核".to_string()),
                    "code": 422,
                    "phase": "quality_blocked"
                }),
                json!({"type":"done"}),
            ],
            BatchGenerationStreamTerminalKind::Cancelled => vec![json!({"type":"done"})],
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::models::batch_generation_task;
    use crate::services::chapter_batch_generation_stream_semantics_service::{
        BatchGenerationStreamState, BatchGenerationStreamTerminalKind,
    };
    use axum::response::sse::Event;
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
            phase: "completed".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        };

        let progress_event = BatchGenerationStreamCursor { observation: None }
            .resolve_event_batch(&state)
            .map(|batch| match batch {
                BatchGenerationStreamEventResolution::Continue { events, .. }
                | BatchGenerationStreamEventResolution::Close { events } => {
                    events.into_iter().next().expect("progress event")
                }
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
            phase: "failed".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Failed),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        }
        .terminal_events()
        .expect("failed terminal events")
        .into_iter()
        .next()
        .expect("failed event");

        assert_eq!(progress_event["type"], "progress");
        assert_eq!(progress_event["status"], "success");
        assert_eq!(progress_event["phase"], "completed");
        assert_eq!(progress_event["current_retry_count"], 0);
        assert_eq!(progress_event["max_retries"], 3);
        assert_eq!(result_event["type"], "result");
        assert_eq!(result_event["data"]["content_source"], "chapter");
        assert_eq!(result_event["data"]["analysis_task_id"], "analysis-task-1");
        assert_eq!(failed_event["error"], "boom");
    }

    #[test]
    fn should_build_status_stream_system_event_payloads_from_event_owner() {
        let connected = super::batch_generation_stream_connected_event_payload();
        let task_not_found = super::batch_generation_stream_task_not_found_event_payload();
        let timed_out = super::batch_generation_stream_timeout_event_payload();

        assert_eq!(connected["type"], "progress");
        assert_eq!(connected["message"], "正在连接批量生成任务流");
        assert_eq!(connected["progress"], 0);
        assert_eq!(connected["status"], "processing");
        assert_eq!(task_not_found["type"], "error");
        assert_eq!(task_not_found["error"], "批量生成任务不存在");
        assert_eq!(task_not_found["code"], 404);
        assert_eq!(timed_out["type"], "error");
        assert_eq!(timed_out["error"], "批量生成任务流等待超时");
        assert_eq!(timed_out["code"], 408);
    }

    #[test]
    fn should_build_status_stream_transport_events_from_event_owner() {
        let payload = json!({
            "type": "progress",
            "message": "处理中"
        });
        let data_event = super::batch_generation_stream_data_event(payload.clone());
        let heartbeat_event = super::batch_generation_stream_heartbeat_event();

        let data_debug = format!("{data_event:?}");
        let heartbeat_debug = format!("{heartbeat_event:?}");
        let expected_data_debug = format!("{:?}", Event::default().data(payload.to_string()));
        let expected_heartbeat_debug = format!("{:?}", Event::default().comment("heartbeat"));

        assert_eq!(data_debug, expected_data_debug);
        assert_eq!(heartbeat_debug, expected_heartbeat_debug);
        assert_eq!(
            super::batch_generation_stream_heartbeat_comment(),
            "heartbeat"
        );
    }

    #[test]
    fn should_build_stream_events_from_state_owner() {
        let state = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            phase: "completed".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        };

        let events = state.events();

        assert_eq!(events.len(), 4);
        assert_eq!(events[0]["type"], "progress");
        assert_eq!(events[0]["status"], "success");
        assert_eq!(events[0]["phase"], "completed");
        assert_eq!(events[0]["current_retry_count"], 0);
        assert_eq!(events[0]["max_retries"], 3);
        assert_eq!(events[1]["type"], "analysis_started");
        assert_eq!(events[1]["task_id"], "analysis-task-1");
        assert_eq!(events[1]["phase"], "parsing");
        assert_eq!(events[1]["current_retry_count"], 0);
        assert_eq!(events[1]["max_retries"], 3);
        assert_eq!(events[2]["type"], "result");
        assert_eq!(events[3]["type"], "done");
        assert!(state.terminal_kind.is_some());
    }

    #[test]
    fn should_build_analysis_started_event_without_task_id_for_fallback_lane() {
        let state = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 85,
            message: "正在分析章节".to_string(),
            phase: "parsing".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        };

        let events = state.events();

        assert_eq!(events.len(), 2);
        assert_eq!(events[1]["type"], "analysis_started");
        assert_eq!(events[1]["chapter_id"], "chapter-1");
        assert_eq!(events[1]["chapter_number"], 1);
        assert_eq!(events[1]["message"], "第 1 章分析任务已启动");
        assert_eq!(events[1]["progress"], 85);
        assert_eq!(events[1]["phase"], "parsing");
        assert_eq!(events[1]["current_retry_count"], 0);
        assert_eq!(events[1]["max_retries"], 3);
        assert!(events[1].get("task_id").is_none());
    }

    #[test]
    fn should_build_terminal_batch_generation_events() {
        let completed = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            phase: "completed".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        };
        let mut failed = BatchGenerationStreamState {
            task: build_task("failed"),
            status: "failed".to_string(),
            completed: 0,
            progress: 100,
            message: "生成失败".to_string(),
            phase: "failed".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Failed),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        };
        failed.task.error_message = Some("boom".to_string());
        let manual_review = BatchGenerationStreamState {
            task: {
                let mut task = build_task("failed");
                task.error_message =
                    Some("第7章触发质量门禁，需人工复核: 等待人工复核".to_string());
                task
            },
            status: "failed".to_string(),
            completed: 0,
            progress: 100,
            message: "等待人工复核".to_string(),
            phase: "quality_blocked".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::ManualReview),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: Some(json!({
                "decision": "manual_review",
                "label": "等待人工复核",
                "phase": "quality_blocked"
            })),
            active_story_repair_payload: Some(json!({
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "等待人工复核",
                "phase": "quality_blocked"
            })),
            terminal_label: Some("等待人工复核".to_string()),
        };
        let retry = BatchGenerationStreamState {
            task: build_task("failed"),
            status: "failed".to_string(),
            completed: 0,
            progress: 100,
            message: "自动修复后重试".to_string(),
            phase: "repair_pending".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Failed),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: Some(json!({
                "decision": "auto_repair",
                "label": "自动修复后重试",
                "phase": "repair_pending"
            })),
            active_story_repair_payload: Some(json!({
                "quality_gate_decision": "repair",
                "quality_gate_label": "自动修复后重试",
                "phase": "repair_pending"
            })),
            terminal_label: Some("自动修复后重试".to_string()),
        };
        let cancelled = BatchGenerationStreamState {
            task: build_task("cancelled"),
            status: "cancelled".to_string(),
            completed: 0,
            progress: 100,
            message: "生成已取消".to_string(),
            phase: "cancelled".to_string(),
            event_status: "processing",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Cancelled),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        };

        let completed_events = completed.terminal_events().expect("completed events");
        assert_eq!(completed_events.len(), 2);
        assert_eq!(completed_events[0]["type"], "result");
        assert_eq!(completed_events[1]["type"], "done");

        let failed_events = failed.terminal_events().expect("failed events");
        assert_eq!(failed_events.len(), 2);
        assert_eq!(failed_events[0]["type"], "error");
        assert_eq!(failed_events[0]["error"], "boom");
        assert_eq!(failed_events[0]["phase"], "failed");
        assert_eq!(failed_events[1]["type"], "done");

        let failed_without_message = BatchGenerationStreamState {
            task: build_task("failed"),
            status: "failed".to_string(),
            completed: 0,
            progress: 100,
            message: "生成失败".to_string(),
            phase: "failed".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Failed),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        }
        .terminal_events()
        .expect("failed fallback events");
        assert_eq!(failed_without_message[0]["error"], "批量生成任务执行失败");
        assert_eq!(failed_without_message[0]["phase"], "failed");
        assert_eq!(failed_without_message[1]["type"], "done");

        let manual_review_events = manual_review
            .terminal_events()
            .expect("manual review events");
        assert_eq!(manual_review_events.len(), 2);
        assert_eq!(manual_review_events[0]["type"], "error");
        assert_eq!(manual_review_events[0]["phase"], "quality_blocked");
        assert_eq!(
            manual_review_events[0]["error"],
            "第7章触发质量门禁，需人工复核: 等待人工复核"
        );
        assert_eq!(manual_review_events[0]["code"], 422);
        assert_eq!(manual_review_events[1]["type"], "done");
        assert!(manual_review_events[0].get("terminal_reason").is_none());
        assert!(manual_review_events[0].get("terminal_label").is_none());
        assert!(manual_review_events[0].get("review_required").is_none());
        assert!(manual_review_events[0].get("can_resume").is_none());

        let retry_events = retry.terminal_events().expect("retry events");
        assert_eq!(retry_events.len(), 2);
        assert_eq!(retry_events[0]["type"], "error");
        assert_eq!(retry_events[0]["error"], "批量生成任务执行失败");
        assert!(retry_events[0].get("terminal_reason").is_none());
        assert_eq!(retry_events[0]["phase"], "failed");
        assert_eq!(retry_events[1]["type"], "done");

        let cancelled_events = cancelled.terminal_events().expect("cancelled events");
        assert_eq!(cancelled_events.len(), 1);
        assert_eq!(cancelled_events[0]["type"], "done");

        let running = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 65,
            message: "处理中".to_string(),
            phase: "generating".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        };
        assert!(running.terminal_events().is_none());
    }

    #[test]
    fn should_build_quality_gate_progress_payload_for_manual_review_and_retry() {
        let manual_review_events = BatchGenerationStreamState {
            task: build_task("failed"),
            status: "failed".to_string(),
            completed: 0,
            progress: 76,
            message: "等待人工复核".to_string(),
            phase: "quality_blocked".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::ManualReview),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: Some(json!({
                "decision": "manual_review",
                "label": "等待人工复核",
                "phase": "quality_blocked"
            })),
            active_story_repair_payload: Some(json!({
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "等待人工复核",
                "phase": "quality_blocked"
            })),
            terminal_label: Some("等待人工复核".to_string()),
        }
        .events();
        let retry_events = BatchGenerationStreamState {
            task: {
                let mut task = build_task("failed");
                task.current_retry_count = 1;
                task
            },
            status: "failed".to_string(),
            completed: 0,
            progress: 76,
            message: "自动修复后重试".to_string(),
            phase: "repair_pending".to_string(),
            event_status: "error",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Failed),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: Some(json!({
                "decision": "auto_repair",
                "label": "自动修复后重试",
                "phase": "repair_pending"
            })),
            active_story_repair_payload: Some(json!({
                "quality_gate_decision": "repair",
                "quality_gate_label": "自动修复后重试",
                "phase": "repair_pending"
            })),
            terminal_label: Some("自动修复后重试".to_string()),
        }
        .events();

        assert_eq!(manual_review_events[0]["type"], "progress");
        assert_eq!(manual_review_events[0]["phase"], "quality_blocked");
        assert_eq!(
            manual_review_events[0]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            manual_review_events[0]["active_story_repair_payload"]["phase"],
            "quality_blocked"
        );
        assert_eq!(retry_events[0]["type"], "progress");
        assert_eq!(retry_events[0]["phase"], "repair_pending");
        assert_eq!(retry_events[0]["current_retry_count"], 1);
        assert_eq!(retry_events[0]["quality_gate"]["decision"], "auto_repair");
        assert_eq!(
            retry_events[0]["active_story_repair_payload"]["phase"],
            "repair_pending"
        );
    }

    #[test]
    fn should_resolve_non_terminal_stream_event_batch_and_next_cursor() {
        let cursor = BatchGenerationStreamCursor { observation: None };
        let state = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 65,
            message: "处理中".to_string(),
            phase: "generating".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: None,
            active_story_repair_payload: None,
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
                assert_eq!(events[0]["phase"], "generating");
                assert_eq!(next_cursor.observation, Some(state.observation_key()));
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
            phase: "generating".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        };
        let cursor = BatchGenerationStreamCursor {
            observation: Some(state.observation_key()),
        };

        assert!(cursor.resolve_event_batch(&state).is_none());
    }

    #[test]
    fn should_resolve_terminal_stream_event_batch_without_next_cursor() {
        let cursor = BatchGenerationStreamCursor { observation: None };
        let state = BatchGenerationStreamState {
            task: build_task("completed"),
            status: "completed".to_string(),
            completed: 1,
            progress: 100,
            message: "生成完成".to_string(),
            phase: "completed".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            quality_gate: None,
            active_story_repair_payload: None,
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
    fn should_emit_stream_event_batch_when_phase_changes_under_same_progress() {
        let baseline = BatchGenerationStreamState {
            task: build_task("failed"),
            status: "failed".to_string(),
            completed: 0,
            progress: 76,
            message: "等待人工复核".to_string(),
            phase: "quality_blocked".to_string(),
            event_status: "running",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::ManualReview),
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: Some(json!({
                "decision": "manual_review",
                "label": "等待人工复核",
                "phase": "quality_blocked"
            })),
            active_story_repair_payload: Some(json!({
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "等待人工复核",
                "phase": "quality_blocked"
            })),
            terminal_label: Some("等待人工复核".to_string()),
        };
        let next_state = BatchGenerationStreamState {
            phase: "repair_pending".to_string(),
            event_status: "running",
            quality_gate: Some(json!({
                "decision": "auto_repair",
                "label": "自动修复后重试",
                "phase": "repair_pending"
            })),
            active_story_repair_payload: Some(json!({
                "quality_gate_decision": "repair",
                "quality_gate_label": "自动修复后重试",
                "phase": "repair_pending"
            })),
            terminal_label: Some("自动修复后重试".to_string()),
            ..baseline.clone()
        };
        let cursor = BatchGenerationStreamCursor {
            observation: Some(baseline.observation_key()),
        };

        let batch = cursor
            .resolve_event_batch(&next_state)
            .expect("phase-only change should produce event batch");

        match batch {
            BatchGenerationStreamEventResolution::Close { events } => {
                assert_eq!(events[0]["type"], "progress");
                assert_eq!(events[0]["phase"], "repair_pending");
                assert_eq!(events[0]["quality_gate"]["decision"], "auto_repair");
                assert_eq!(
                    events[0]["active_story_repair_payload"]["phase"],
                    "repair_pending"
                );
            }
            BatchGenerationStreamEventResolution::Continue { .. } => {
                panic!("expected close resolution for terminal state change")
            }
        }
    }

    #[test]
    fn should_emit_stream_event_batch_when_analysis_started_fields_change() {
        let baseline = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 85,
            message: "正在分析章节".to_string(),
            phase: "parsing".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        };
        let next_state = BatchGenerationStreamState {
            analysis_task_id: Some("analysis-task-9".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            ..baseline.clone()
        };
        let cursor = BatchGenerationStreamCursor {
            observation: Some(baseline.observation_key()),
        };

        let batch = cursor
            .resolve_event_batch(&next_state)
            .expect("analysis-started change should produce event batch");

        match batch {
            BatchGenerationStreamEventResolution::Continue {
                events,
                next_cursor,
            } => {
                assert_eq!(events.len(), 2);
                assert_eq!(events[0]["type"], "progress");
                assert_eq!(events[1]["type"], "analysis_started");
                assert_eq!(events[1]["task_id"], "analysis-task-9");
                assert_eq!(next_cursor.observation, Some(next_state.observation_key()));
            }
            BatchGenerationStreamEventResolution::Close { .. } => {
                panic!("expected continue resolution for running analysis state")
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
            phase: "completed".to_string(),
            event_status: "success",
            terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            analysis_task_id: Some("analysis-task-1".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        };
        let running = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 65,
            message: "处理中".to_string(),
            phase: "generating".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: None,
            analysis_task_message: None,
            analysis_task_progress: None,
            analysis_started_chapter_id: None,
            analysis_started_chapter_number: None,
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        };

        assert!(completed.terminal_kind.is_some());
        assert!(running.terminal_kind.is_none());
    }
}
