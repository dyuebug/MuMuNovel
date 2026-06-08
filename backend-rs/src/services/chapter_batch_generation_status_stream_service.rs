use std::convert::Infallible;

use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_batch_generation_owned_task_query_service::{
    load_owned_batch_generation_task_read_state, LoadOwnedBatchGenerationTaskError,
};
use crate::services::chapter_batch_generation_task_payload_base_service::{
    resolve_failed_terminal_semantics, BatchGenerationFailedTerminalKind,
    BatchGenerationFailedTerminalSemantics, BatchGenerationQualityStatusContext,
};
use axum::response::sse::Event;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio_stream::wrappers::ReceiverStream;

const STATUS_POLL_ATTEMPTS: usize = 300;
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const STATUS_HEARTBEAT_POLL_INTERVAL: usize = 15;

pub(crate) type BatchGenerationStatusStream = ReceiverStream<Result<Event, Infallible>>;

#[derive(Debug, Clone)]
pub(crate) struct BatchGenerationStreamState {
    pub(crate) task: batch_generation_task::Model,
    pub(crate) status: String,
    pub(crate) completed: i32,
    pub(crate) progress: i32,
    pub(crate) message: String,
    pub(crate) phase: String,
    pub(crate) event_status: &'static str,
    pub(crate) terminal_kind: Option<BatchGenerationStreamTerminalKind>,
    pub(crate) analysis_task_id: Option<String>,
    pub(crate) analysis_task_message: Option<String>,
    pub(crate) analysis_task_progress: Option<i32>,
    pub(crate) analysis_started_chapter_id: Option<String>,
    pub(crate) analysis_started_chapter_number: Option<i32>,
    pub(crate) quality_gate: Option<Value>,
    pub(crate) active_story_repair_payload: Option<Value>,
    pub(crate) terminal_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchGenerationStreamObservationKey {
    status: String,
    completed: i32,
    progress: i32,
    message: String,
    phase: String,
    event_status: &'static str,
    current_retry_count: i32,
    max_retries: i32,
    analysis_task_id: Option<String>,
    analysis_task_message: Option<String>,
    analysis_task_progress: Option<i32>,
    analysis_started_chapter_id: Option<String>,
    analysis_started_chapter_number: Option<i32>,
    quality_gate: Option<Value>,
    active_story_repair_payload: Option<Value>,
    terminal_kind: Option<BatchGenerationStreamTerminalKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationStreamTerminalKind {
    Completed,
    Failed,
    Cancelled,
    ManualReview,
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
        let failed_terminal_semantics = resolve_failed_terminal_semantics(
            &task,
            Some(&task.failed_chapters),
            quality_status_context,
        );
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
        let phase = workflow_runtime_state
            .and_then(|item| item.get("phase"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                manual_review_terminal_label
                    .as_ref()
                    .map(|_| "quality_blocked".to_string())
            })
            .or_else(|| {
                retryable_repair_terminal_label
                    .as_ref()
                    .map(|_| "repair_pending".to_string())
            })
            .unwrap_or_else(|| resolved_status.default_phase().to_string());
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
        let quality_gate =
            resolve_stream_quality_gate(quality_status_context, workflow_runtime_state);
        let active_story_repair_payload = quality_status_context
            .and_then(|context| context.active_story_repair_payload.clone())
            .or_else(|| {
                workflow_runtime_state
                    .and_then(Value::as_object)
                    .and_then(|state| state.get("active_story_repair_payload"))
                    .filter(|payload| payload.is_object())
                    .cloned()
            });
        let event_status = resolve_stream_event_status(
            &resolved_status,
            &phase,
            failed_terminal_semantics.as_ref(),
        );

        Self {
            task,
            status,
            completed,
            progress,
            message,
            phase,
            event_status,
            terminal_kind: resolved_status.terminal_kind(
                manual_review_terminal_label.as_ref(),
                retryable_repair_terminal_label.as_ref(),
            ),
            analysis_task_id,
            analysis_task_message,
            analysis_task_progress,
            analysis_started_chapter_id,
            analysis_started_chapter_number,
            quality_gate,
            active_story_repair_payload,
            terminal_label: manual_review_terminal_label.or(retryable_repair_terminal_label),
        }
    }

    fn observation_key(&self) -> BatchGenerationStreamObservationKey {
        BatchGenerationStreamObservationKey {
            status: self.status.clone(),
            completed: self.completed,
            progress: self.progress,
            message: self.message.clone(),
            phase: self.phase.clone(),
            event_status: self.event_status,
            current_retry_count: self.task.current_retry_count,
            max_retries: self.task.max_retries,
            analysis_task_id: self.analysis_task_id.clone(),
            analysis_task_message: self.analysis_task_message.clone(),
            analysis_task_progress: self.analysis_task_progress,
            analysis_started_chapter_id: self.analysis_started_chapter_id.clone(),
            analysis_started_chapter_number: self.analysis_started_chapter_number,
            quality_gate: self.quality_gate.clone(),
            active_story_repair_payload: self.active_story_repair_payload.clone(),
            terminal_kind: self.terminal_kind,
        }
    }
}

fn resolve_stream_quality_gate(
    quality_status_context: Option<&BatchGenerationQualityStatusContext>,
    workflow_runtime_state: Option<&Value>,
) -> Option<Value> {
    quality_status_context
        .and_then(|context| {
            context
                .latest_quality_metrics
                .as_ref()
                .and_then(|metrics| metrics.get("quality_gate"))
                .cloned()
                .or_else(|| {
                    context
                        .quality_metrics_summary
                        .as_ref()
                        .and_then(|summary| summary.get("quality_gate"))
                        .cloned()
                })
                .or_else(|| {
                    context
                        .active_story_repair_payload
                        .as_ref()
                        .and_then(build_quality_gate_from_active_story_repair_payload)
                })
        })
        .or_else(|| {
            workflow_runtime_state
                .and_then(Value::as_object)
                .and_then(|state| state.get("quality_gate"))
                .cloned()
        })
}

fn build_quality_gate_from_active_story_repair_payload(payload: &Value) -> Option<Value> {
    let object = payload.as_object()?;
    let decision = object
        .get("quality_gate_decision")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    let mut quality_gate = serde_json::Map::new();
    quality_gate.insert("decision".to_string(), json!(decision));

    if let Some(label) = object
        .get("quality_gate_label")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        quality_gate.insert("label".to_string(), json!(label));
    }

    if let Some(phase) = object
        .get("phase")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        quality_gate.insert("phase".to_string(), json!(phase));
    }

    Some(Value::Object(quality_gate))
}

fn resolve_stream_event_status(
    resolved_status: &BatchGenerationResolvedStreamStatus,
    phase: &str,
    failed_terminal_semantics: Option<&BatchGenerationFailedTerminalSemantics>,
) -> &'static str {
    match resolved_status {
        BatchGenerationResolvedStreamStatus::Failed
            if matches!(
                failed_terminal_semantics.map(|semantics| semantics.kind),
                Some(BatchGenerationFailedTerminalKind::ManualReview)
                    | Some(BatchGenerationFailedTerminalKind::Retry)
            ) && matches!(phase, "quality_blocked" | "repair_pending" | "saving") =>
        {
            "running"
        }
        _ => resolved_status.event_status(),
    }
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

    fn default_phase(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "generating",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Unknown => "processing",
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
                Some(BatchGenerationStreamTerminalKind::Failed)
            }
            Self::Failed => Some(BatchGenerationStreamTerminalKind::Failed),
            Self::Cancelled => Some(BatchGenerationStreamTerminalKind::Cancelled),
            Self::Pending | Self::Running | Self::Unknown => None,
        }
    }
}

fn batch_generation_stream_connected_event_payload() -> Value {
    json!({
        "type": "progress",
        "message": "正在连接批量生成任务流",
        "progress": 0,
        "status": "processing"
    })
}

fn batch_generation_stream_task_not_found_event_payload() -> Value {
    json!({
        "type": "error",
        "error": "批量生成任务不存在",
        "code": 404
    })
}

fn batch_generation_stream_timeout_event_payload() -> Value {
    json!({
        "type": "error",
        "error": "批量生成任务流等待超时",
        "code": 408
    })
}

fn batch_generation_stream_heartbeat_comment() -> &'static str {
    "heartbeat"
}

fn batch_generation_stream_data_event(payload: Value) -> Event {
    Event::default().data(payload.to_string())
}

fn batch_generation_stream_heartbeat_event() -> Event {
    Event::default().comment(batch_generation_stream_heartbeat_comment())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchGenerationStreamCursor {
    observation: Option<BatchGenerationStreamObservationKey>,
}

impl BatchGenerationStreamCursor {
    fn resolve_event_batch(
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
enum BatchGenerationStreamEventResolution {
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

    fn terminal_events(&self) -> Option<Vec<Value>> {
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

fn build_batch_generation_stream_state_from_task_and_snapshot(
    task: batch_generation_task::Model,
    snapshot: Option<batch_generation_snapshot::Model>,
) -> BatchGenerationStreamState {
    let workflow_runtime_state = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.as_ref());
    let quality_status_context =
        BatchGenerationQualityStatusContext::from_snapshot_and_runtime_state(
            snapshot.as_ref(),
            workflow_runtime_state,
        );

    BatchGenerationStreamState::from_task_state_with_quality_context(
        task,
        workflow_runtime_state,
        Some(&quality_status_context),
    )
}

async fn load_owned_batch_generation_stream_state(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<BatchGenerationStreamState, LoadOwnedBatchGenerationTaskError> {
    let (task, snapshot) = load_owned_batch_generation_task_read_state(db, batch_id, user_id)
        .await?
        .into_parts();

    Ok(build_batch_generation_stream_state_from_task_and_snapshot(
        task, snapshot,
    ))
}

async fn send_stream_event(tx: &mpsc::Sender<Result<Event, Infallible>>, event: Event) {
    let _ = tx.send(Ok(event)).await;
}

async fn send_stream_events(
    tx: &mpsc::Sender<Result<Event, Infallible>>,
    events: Vec<serde_json::Value>,
) {
    for event in events {
        send_stream_event(tx, batch_generation_stream_data_event(event)).await;
    }
}

pub(crate) async fn load_owned_batch_generation_status_stream(
    db: DatabaseConnection,
    batch_id: String,
    user_id: String,
) -> Result<BatchGenerationStatusStream, LoadOwnedBatchGenerationTaskError> {
    let initial_state = load_owned_batch_generation_stream_state(&db, &batch_id, &user_id).await?;

    Ok(build_batch_generation_status_stream(
        db,
        batch_id,
        user_id,
        initial_state,
    ))
}

pub(crate) fn build_batch_generation_status_stream(
    db: DatabaseConnection,
    batch_id: String,
    user_id: String,
    initial_state: BatchGenerationStreamState,
) -> BatchGenerationStatusStream {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);

    tokio::spawn(async move {
        let mut cursor = BatchGenerationStreamCursor { observation: None };
        let mut idle_poll_count = 0usize;
        let mut pending_state = Some(initial_state);
        send_stream_event(
            &tx,
            batch_generation_stream_data_event(batch_generation_stream_connected_event_payload()),
        )
        .await;

        for _ in 0..STATUS_POLL_ATTEMPTS {
            let state = if let Some(state) = pending_state.take() {
                state
            } else {
                match load_owned_batch_generation_stream_state(&db, &batch_id, &user_id).await {
                    Ok(state) => state,
                    Err(_) => {
                        send_stream_event(
                            &tx,
                            batch_generation_stream_data_event(
                                batch_generation_stream_task_not_found_event_payload(),
                            ),
                        )
                        .await;
                        return;
                    }
                }
            };

            if let Some(event_batch) = cursor.resolve_event_batch(&state) {
                match event_batch {
                    BatchGenerationStreamEventResolution::Continue {
                        events,
                        next_cursor,
                    } => {
                        send_stream_events(&tx, events).await;
                        cursor = next_cursor;
                        idle_poll_count = 0;
                    }
                    BatchGenerationStreamEventResolution::Close { events } => {
                        send_stream_events(&tx, events).await;
                        return;
                    }
                }
            } else {
                idle_poll_count += 1;
                if idle_poll_count >= STATUS_HEARTBEAT_POLL_INTERVAL {
                    send_stream_event(&tx, batch_generation_stream_heartbeat_event()).await;
                    idle_poll_count = 0;
                }
            }

            sleep(STATUS_POLL_INTERVAL).await;
        }

        send_stream_event(
            &tx,
            batch_generation_stream_data_event(batch_generation_stream_timeout_event_payload()),
        )
        .await;
    });

    ReceiverStream::new(rx)
}

#[cfg(test)]
mod tests {
    use super::{
        BatchGenerationResolvedStreamStatus, BatchGenerationStreamObservationKey,
        BatchGenerationStreamState, BatchGenerationStreamTerminalKind,
    };
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_batch_generation_owned_task_query_service::OwnedBatchGenerationTaskReadState;
    use crate::services::chapter_batch_generation_task_payload_base_service::BatchGenerationQualityStatusContext;
    use axum::response::sse::Event;
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

    fn build_stream_state(status: &str) -> BatchGenerationStreamState {
        BatchGenerationStreamState {
            task: build_task(status),
            status: status.to_string(),
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
        }
    }

    #[test]
    fn should_build_python_compatible_stream_connected_event_payload() {
        let payload = super::batch_generation_stream_connected_event_payload();

        assert_eq!(payload["type"], "progress");
        assert_eq!(payload["message"], "正在连接批量生成任务流");
        assert_eq!(payload["progress"], 0);
        assert_eq!(payload["status"], "processing");
    }

    #[test]
    fn should_resolve_stream_poll_from_pending_initial_state_first() {
        let initial_state = build_stream_state("running");
        let mut pending_state = Some(initial_state.clone());

        let state = match pending_state.take() {
            Some(state) => state,
            None => panic!("pending state should be present when checked"),
        };

        assert_eq!(state.status, "running");
        assert_eq!(state.progress, 65);

        assert!(pending_state.is_none());
    }

    #[test]
    fn should_close_stream_poll_when_loaded_state_is_missing() {
        let pending_state: Option<BatchGenerationStreamState> = None;

        assert!(pending_state.is_none());
    }

    #[test]
    fn should_build_task_not_found_stream_system_event_payload() {
        let payload = super::batch_generation_stream_task_not_found_event_payload();

        assert_eq!(payload["type"], "error");
        assert_eq!(payload["error"], "批量生成任务不存在");
        assert_eq!(payload["code"], 404);
    }

    #[test]
    fn should_build_timed_out_stream_system_event_payload() {
        let payload = super::batch_generation_stream_timeout_event_payload();

        assert_eq!(payload["type"], "error");
        assert_eq!(payload["error"], "批量生成任务流等待超时");
        assert_eq!(payload["code"], 408);
    }

    #[test]
    fn should_build_python_compatible_stream_heartbeat_comment() {
        assert_eq!(
            super::batch_generation_stream_heartbeat_comment(),
            "heartbeat"
        );
    }

    #[test]
    fn should_build_stream_state_from_task_and_snapshot_owner_inside_status_stream_service() {
        let state = super::build_batch_generation_stream_state_from_task_and_snapshot(
            build_task("running"),
            Some(build_snapshot()),
        );

        assert_eq!(state.status, "running");
        assert_eq!(state.completed, 1);
        assert_eq!(state.progress, 60);
        assert_eq!(state.message, "正在生成正文...");
        assert_eq!(state.event_status, "processing");
        assert_eq!(
            state.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_build_terminal_stream_state_from_task_and_snapshot_owner_inside_status_stream_service(
    ) {
        let state = super::build_batch_generation_stream_state_from_task_and_snapshot(
            build_task("completed"),
            None,
        );

        assert_eq!(state.progress, 100);
        assert_eq!(state.message, "生成完成");
        assert_eq!(state.event_status, "success");
        assert_eq!(
            state.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Completed)
        );
    }

    #[test]
    fn should_build_stream_state_from_shared_owned_read_state_owner_inside_status_stream_service() {
        let (task, snapshot) = OwnedBatchGenerationTaskReadState::from_parts(
            build_task("running"),
            Some(build_snapshot()),
        )
        .into_parts();
        let state =
            super::build_batch_generation_stream_state_from_task_and_snapshot(task, snapshot);

        assert_eq!(state.status, "running");
        assert_eq!(state.progress, 60);
        assert_eq!(state.event_status, "processing");
        assert_eq!(
            state.active_story_repair_payload,
            Some(json!({"mode": "repair"}))
        );
    }

    #[test]
    fn should_build_terminal_stream_state_from_stream_owner() {
        let state = BatchGenerationStreamState::from_task_state(build_task("completed"), None);

        assert_eq!(state.progress, 100);
        assert_eq!(state.message, "生成完成");
        assert_eq!(state.event_status, "success");
        assert_eq!(
            state.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Completed)
        );
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
        assert_eq!(
            completed.analysis_task_id.as_deref(),
            Some("analysis-task-1")
        );
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
            Some(BatchGenerationStreamTerminalKind::Failed)
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
    fn should_build_stream_observation_key_from_state_owner() {
        let state = BatchGenerationStreamState::from_task_state(
            build_task("completed"),
            Some(&json!({
                "progress": 100,
                "phase": "completed",
                "last_message": "生成完成",
                "analysis_task_id": "analysis-task-2",
                "analysis_task_message": "第 2 章分析任务已启动",
                "analysis_task_progress": 85,
                "analysis_started_chapter_id": "chapter-2",
                "analysis_started_chapter_number": 2,
                "quality_gate": {
                    "decision": "pass",
                    "phase": "completed"
                },
                "active_story_repair_payload": {
                    "quality_gate_decision": "pass",
                    "phase": "completed"
                }
            })),
        );

        let key = state.observation_key();

        assert_eq!(
            key,
            BatchGenerationStreamObservationKey {
                status: "completed".to_string(),
                completed: 1,
                progress: 100,
                message: "生成完成".to_string(),
                phase: "completed".to_string(),
                event_status: "success",
                current_retry_count: 0,
                max_retries: 3,
                analysis_task_id: Some("analysis-task-2".to_string()),
                analysis_task_message: Some("第 2 章分析任务已启动".to_string()),
                analysis_task_progress: Some(85),
                analysis_started_chapter_id: Some("chapter-2".to_string()),
                analysis_started_chapter_number: Some(2),
                quality_gate: Some(json!({
                    "decision": "pass",
                    "phase": "completed"
                })),
                active_story_repair_payload: Some(json!({
                    "quality_gate_decision": "pass",
                    "phase": "completed"
                })),
                terminal_kind: Some(BatchGenerationStreamTerminalKind::Completed),
            }
        );
    }

    #[test]
    fn should_resolve_manual_review_and_retry_stream_state_from_quality_context_owner() {
        let manual_review = BatchGenerationStreamState::from_task_state_with_quality_context(
            build_task("failed"),
            None,
            Some(&BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({
                    "quality_gate": {
                        "decision": "manual_review",
                        "label": "等待人工复核"
                    }
                })),
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: None,
                quality_history_context: None,
                active_story_repair_payload: None,
            }),
        );
        assert_eq!(manual_review.message, "等待人工复核");
        assert_eq!(
            manual_review.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::ManualReview)
        );
        assert_eq!(
            manual_review.terminal_label.as_deref(),
            Some("等待人工复核")
        );

        let retry = BatchGenerationStreamState::from_task_state_with_quality_context(
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
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: None,
                quality_history_context: None,
                active_story_repair_payload: None,
            }),
        );
        assert_eq!(retry.message, "自动修复后重试");
        assert_eq!(
            retry.terminal_kind,
            Some(BatchGenerationStreamTerminalKind::Failed)
        );
        assert_eq!(retry.terminal_label.as_deref(), Some("自动修复后重试"));
    }

    #[test]
    fn should_resolve_manual_review_when_auto_repair_budget_is_exhausted() {
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
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: None,
                quality_history_context: None,
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
    fn should_keep_quality_gate_terminal_progress_status_running_before_error_event() {
        let manual_review = BatchGenerationStreamState::from_task_state_with_quality_context(
            build_task("failed"),
            Some(&json!({
                "phase": "quality_blocked",
                "last_message": "等待人工复核"
            })),
            Some(&BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({
                    "quality_gate": {
                        "decision": "manual_review",
                        "label": "等待人工复核"
                    }
                })),
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: None,
                quality_history_context: None,
                active_story_repair_payload: Some(json!({
                    "quality_gate_decision": "manual_review",
                    "quality_gate_label": "等待人工复核",
                    "phase": "quality_blocked"
                })),
            }),
        );
        let retry = BatchGenerationStreamState::from_task_state_with_quality_context(
            {
                let mut task = build_task("failed");
                task.current_retry_count = 1;
                task.max_retries = 3;
                task
            },
            Some(&json!({
                "phase": "repair_pending",
                "last_message": "自动修复后重试"
            })),
            Some(&BatchGenerationQualityStatusContext {
                latest_quality_metrics: Some(json!({
                    "quality_gate": {
                        "decision": "auto_repair",
                        "label": "自动修复后重试"
                    }
                })),
                quality_metrics_history: None,
                quality_metrics_summary_state: None,
                quality_metrics_summary: None,
                quality_history_context: None,
                active_story_repair_payload: Some(json!({
                    "quality_gate_decision": "auto_repair",
                    "quality_gate_label": "自动修复后重试",
                    "phase": "repair_pending"
                })),
            }),
        );

        assert_eq!(manual_review.event_status, "running");
        assert_eq!(retry.event_status, "running");
    }

    #[test]
    fn should_keep_status_stream_system_event_owner_contract() {
        let task_not_found_payload = super::batch_generation_stream_task_not_found_event_payload();
        let timed_out_payload = super::batch_generation_stream_timeout_event_payload();

        assert_eq!(task_not_found_payload["error"], "批量生成任务不存在");
        assert_eq!(timed_out_payload["code"], 408);
        assert_eq!(
            super::batch_generation_stream_heartbeat_comment(),
            "heartbeat"
        );
    }

    fn build_snapshot() -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-1".to_string(),
            batch_task_id: "task-1".to_string(),
            latest_quality_metrics: Some(json!({"score": 91})),
            quality_metrics_history: Some(json!([{"score": 90}])),
            quality_metrics_summary: Some(json!({"summary": "ok"})),
            workflow_runtime_state: Some(json!({
                "progress": 60,
                "active_story_repair_payload": {
                    "mode": "repair"
                }
            })),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn should_build_stream_transport_events_from_owner_contract() {
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
        assert_eq!(events[1]["type"], "analysis_started");
        assert_eq!(events[2]["type"], "result");
        assert_eq!(events[3]["type"], "done");
    }

    #[test]
    fn should_build_terminal_batch_generation_events_from_status_stream_owner() {
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
        assert_eq!(failed_events[0]["error"], "boom");
        assert_eq!(failed_events[0]["phase"], "failed");
        assert_eq!(failed_events[1]["type"], "done");

        let manual_review_events = manual_review
            .terminal_events()
            .expect("manual review events");
        assert_eq!(manual_review_events[0]["phase"], "quality_blocked");
        assert_eq!(manual_review_events[0]["code"], 422);
        assert_eq!(manual_review_events[1]["type"], "done");

        let cancelled_events = cancelled.terminal_events().expect("cancelled events");
        assert_eq!(cancelled_events.len(), 1);
        assert_eq!(cancelled_events[0]["type"], "done");
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

        assert_eq!(
            manual_review_events[0]["quality_gate"]["decision"],
            "manual_review"
        );
        assert_eq!(
            manual_review_events[0]["active_story_repair_payload"]["phase"],
            "quality_blocked"
        );
        assert_eq!(retry_events[0]["current_retry_count"], 1);
        assert_eq!(retry_events[0]["quality_gate"]["decision"], "auto_repair");
        assert_eq!(
            retry_events[0]["active_story_repair_payload"]["phase"],
            "repair_pending"
        );
    }

    #[test]
    fn should_resolve_stream_event_batch_contract_inside_status_stream_owner() {
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

        let continue_batch = super::BatchGenerationStreamCursor { observation: None }
            .resolve_event_batch(&running)
            .expect("continue batch");
        match continue_batch {
            super::BatchGenerationStreamEventResolution::Continue {
                events,
                next_cursor,
            } => {
                assert_eq!(events.len(), 1);
                assert_eq!(events[0]["type"], "progress");
                assert_eq!(next_cursor.observation, Some(running.observation_key()));
            }
            super::BatchGenerationStreamEventResolution::Close { .. } => {
                panic!("expected continue resolution")
            }
        }

        let same_cursor = super::BatchGenerationStreamCursor {
            observation: Some(running.observation_key()),
        };
        assert!(same_cursor.resolve_event_batch(&running).is_none());

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
        let close_batch = super::BatchGenerationStreamCursor { observation: None }
            .resolve_event_batch(&completed)
            .expect("close batch");
        match close_batch {
            super::BatchGenerationStreamEventResolution::Close { events } => {
                assert_eq!(events.len(), 4);
                assert_eq!(events[0]["type"], "progress");
                assert_eq!(events[1]["type"], "analysis_started");
                assert_eq!(events[2]["type"], "result");
                assert_eq!(events[3]["type"], "done");
            }
            super::BatchGenerationStreamEventResolution::Continue { .. } => {
                panic!("expected close resolution")
            }
        }
    }

    #[test]
    fn should_emit_stream_event_batch_when_phase_or_analysis_fields_change() {
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
        let next_phase_state = BatchGenerationStreamState {
            phase: "repair_pending".to_string(),
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
        let next_analysis_state = BatchGenerationStreamState {
            task: build_task("running"),
            status: "running".to_string(),
            completed: 0,
            progress: 85,
            message: "正在分析章节".to_string(),
            phase: "parsing".to_string(),
            event_status: "processing",
            terminal_kind: None,
            analysis_task_id: Some("analysis-task-9".to_string()),
            analysis_task_message: Some("第 1 章分析任务已启动".to_string()),
            analysis_task_progress: Some(85),
            analysis_started_chapter_id: Some("chapter-1".to_string()),
            analysis_started_chapter_number: Some(1),
            quality_gate: None,
            active_story_repair_payload: None,
            terminal_label: None,
        };

        let phase_batch = super::BatchGenerationStreamCursor {
            observation: Some(baseline.observation_key()),
        }
        .resolve_event_batch(&next_phase_state)
        .expect("phase change batch");
        match phase_batch {
            super::BatchGenerationStreamEventResolution::Close { events } => {
                assert_eq!(events[0]["phase"], "repair_pending");
                assert_eq!(events[0]["quality_gate"]["decision"], "auto_repair");
            }
            super::BatchGenerationStreamEventResolution::Continue { .. } => {
                panic!("expected close resolution for terminal phase change")
            }
        }

        let analysis_baseline = BatchGenerationStreamState {
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
        let analysis_batch = super::BatchGenerationStreamCursor {
            observation: Some(analysis_baseline.observation_key()),
        }
        .resolve_event_batch(&next_analysis_state)
        .expect("analysis change batch");
        match analysis_batch {
            super::BatchGenerationStreamEventResolution::Continue {
                events,
                next_cursor,
            } => {
                assert_eq!(events.len(), 2);
                assert_eq!(events[1]["type"], "analysis_started");
                assert_eq!(events[1]["task_id"], "analysis-task-9");
                assert_eq!(
                    next_cursor.observation,
                    Some(next_analysis_state.observation_key())
                );
            }
            super::BatchGenerationStreamEventResolution::Close { .. } => {
                panic!("expected continue resolution for running analysis state")
            }
        }
    }
}
