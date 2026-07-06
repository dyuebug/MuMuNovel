use std::convert::Infallible;

use axum::response::sse::Event;
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tokio_stream::wrappers::ReceiverStream;

use super::{load_owned_batch_generation_task_read_state, LoadOwnedBatchGenerationTaskError};
use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_batch_generation_read_context_service::stream_progress_owner::{
    build_batch_generation_stream_progress_event, BatchGenerationStreamProgressEventInput,
};
use crate::services::chapter_batch_generation_task_payload_base_service::{
    resolve_failed_terminal_semantics, BatchGenerationFailedTerminalKind,
    BatchGenerationFailedTerminalSemantics, BatchGenerationQualityStatusContext,
};
use crate::services::chapter_generation_execution_contract_service::active_story_repair_payload_from_runtime_state;

const STATUS_POLL_ATTEMPTS: usize = 300;
const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const STATUS_HEARTBEAT_POLL_INTERVAL: usize = 15;

pub(crate) type BatchGenerationStatusStream = ReceiverStream<Result<Event, Infallible>>;

pub(crate) fn build_batch_generation_stream_state_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_read_context_service::stream_state_owner",
        "scope": "batch_generation_stream_state_projection_cursor_and_transport",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_read_context_service.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service/stream_state_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_read_context_service/stream_progress_owner.rs",
            "backend-rs/src/api/chapter_batch_generation.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "state_projection": [
                "BatchGenerationStreamState::from_task_state",
                "BatchGenerationStreamState::from_task_state_with_quality_context",
                "build_batch_generation_stream_state_from_task_and_snapshot"
            ],
            "event_projection": [
                "BatchGenerationStreamState::events",
                "BatchGenerationStreamState::observation_key",
                "BatchGenerationStreamCursor::resolve_event_batch"
            ],
            "stream_transport": [
                "load_owned_batch_generation_status_stream",
                "build_batch_generation_status_stream",
                "batch_generation_stream_connected_event_payload",
                "batch_generation_stream_task_not_found_event_payload",
                "batch_generation_stream_timeout_event_payload",
                "batch_generation_stream_heartbeat_event"
            ],
            "transport_retry_policy": {
                "poll_attempts": STATUS_POLL_ATTEMPTS,
                "heartbeat_poll_interval": STATUS_HEARTBEAT_POLL_INTERVAL,
                "poll_interval_seconds": STATUS_POLL_INTERVAL.as_secs()
            }
        },
        "active_consumers": [
            "chapter_batch_generation_read_context_service",
            "chapter_batch_generation",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "validation_boundary": [
            "cargo test chapter_batch_generation_read_context_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-batch-generation-owner",
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 11,
            "python_fallback_probe_count": 0,
            "stream_state_owner": "BatchGenerationStreamState::from_task_state_with_quality_context",
            "stream_event_owner": "BatchGenerationStreamState::events",
            "stream_transport_owner": "build_batch_generation_status_stream",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "batch-generation read-context source-map package deleted; surviving Python closeout work is now limited to separate shared runtime/projection source-map packages",
            "status": "rust_batch_generation_stream_state_owner_source_map_deleted"
        },
        "rollback_boundary": {
            "source_map_policy": "batch_generation_read_context_owner_is_rust_only_and_surviving_python_query_status_surfaces_are_tracked_by_external_shared_runtime_projection_contracts",
            "runtime_state_keys": [
                "progress",
                "phase",
                "last_message",
                "selected_candidate_events",
                "active_story_repair_payload",
                "quality_gate",
                "candidate_gateway"
            ],
            "transport_system_events": [
                "connected",
                "task_not_found",
                "heartbeat",
                "timeout"
            ]
        }
    })
}

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
    pub(crate) selected_candidate_events: Vec<Value>,
    pub(crate) quality_gate: Option<Value>,
    pub(crate) active_story_repair_payload: Option<Value>,
    pub(crate) candidate_gateway: Option<Value>,
    #[cfg(test)]
    pub(crate) terminal_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationStreamObservationKey {
    pub(crate) status: String,
    pub(crate) completed: i32,
    pub(crate) progress: i32,
    pub(crate) message: String,
    pub(crate) phase: String,
    pub(crate) event_status: &'static str,
    pub(crate) current_retry_count: i32,
    pub(crate) max_retries: i32,
    pub(crate) analysis_task_id: Option<String>,
    pub(crate) analysis_task_message: Option<String>,
    pub(crate) analysis_task_progress: Option<i32>,
    pub(crate) analysis_started_chapter_id: Option<String>,
    pub(crate) analysis_started_chapter_number: Option<i32>,
    pub(crate) selected_candidate_events: Vec<Value>,
    pub(crate) quality_gate: Option<Value>,
    pub(crate) active_story_repair_payload: Option<Value>,
    pub(crate) candidate_gateway: Option<Value>,
    pub(crate) terminal_kind: Option<BatchGenerationStreamTerminalKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationStreamTerminalKind {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationResolvedStreamStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Unknown,
}

impl BatchGenerationStreamState {
    #[cfg(test)]
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
        let retryable_repair_terminal_label = failed_terminal_semantics
            .as_ref()
            .filter(|semantics| semantics.kind == BatchGenerationFailedTerminalKind::Retry)
            .map(|semantics| semantics.label.clone());
        let quality_gate =
            resolve_stream_quality_gate(quality_status_context, workflow_runtime_state);
        let active_story_repair_payload = quality_status_context
            .and_then(|context| context.active_story_repair_payload.clone())
            .or_else(|| active_story_repair_payload_from_runtime_state(workflow_runtime_state));
        let manual_review_is_telemetry_only =
            is_manual_review_telemetry(&quality_gate, &active_story_repair_payload);
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
            .filter(|value| {
                !(manual_review_is_telemetry_only && matches!(*value, "quality_blocked"))
            })
            .map(str::to_string)
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
            .filter(|value| {
                !(manual_review_is_telemetry_only && looks_like_manual_review_message(value))
            })
            .map(str::to_string)
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
        let selected_candidate_events = workflow_runtime_state
            .and_then(|item| item.get("selected_candidate_events"))
            .and_then(Value::as_array)
            .map(|events| events.to_vec())
            .unwrap_or_default();
        let candidate_gateway = resolve_stream_candidate_gateway(workflow_runtime_state);
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
            terminal_kind: resolved_status.terminal_kind(retryable_repair_terminal_label.as_ref()),
            analysis_task_id,
            analysis_task_message,
            analysis_task_progress,
            analysis_started_chapter_id,
            analysis_started_chapter_number,
            selected_candidate_events,
            quality_gate,
            active_story_repair_payload,
            candidate_gateway,
            #[cfg(test)]
            terminal_label: retryable_repair_terminal_label,
        }
    }

    pub(crate) fn observation_key(&self) -> BatchGenerationStreamObservationKey {
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
            selected_candidate_events: self.selected_candidate_events.clone(),
            quality_gate: self.quality_gate.clone(),
            active_story_repair_payload: self.active_story_repair_payload.clone(),
            candidate_gateway: self.candidate_gateway.clone(),
            terminal_kind: self.terminal_kind,
        }
    }

    pub(crate) fn events(&self) -> Vec<Value> {
        let mut progress_event =
            build_batch_generation_stream_progress_event(BatchGenerationStreamProgressEventInput {
                message: self.message.clone(),
                progress: self.progress,
                status: self.event_status,
                phase: self.phase.clone(),
                current_retry_count: self.task.current_retry_count,
                max_retries: self.task.max_retries,
                candidate_gateway: self.candidate_gateway.clone(),
            });
        if let Some(quality_gate) = self.quality_gate.as_ref() {
            progress_event["quality_gate"] = quality_gate.clone();
        }
        if let Some(active_story_repair_payload) = self.active_story_repair_payload.as_ref() {
            progress_event["active_story_repair_payload"] = active_story_repair_payload.clone();
        }
        let mut events = vec![progress_event];
        events.extend(self.selected_candidate_events.iter().cloned());

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
        insert_stream_candidate_gateway(&mut event, self.candidate_gateway.as_ref());
        Some(event)
    }

    pub(crate) fn terminal_events(&self) -> Option<Vec<Value>> {
        self.terminal_kind.map(|kind| match kind {
            BatchGenerationStreamTerminalKind::Completed => {
                let mut result_event = json!({
                    "type": "result",
                    "data": {
                        "generation_task_id": self.task.id,
                        "chapter_id": self.task.current_chapter_id,
                        "content_source": "chapter",
                        "analysis_task_id": self.analysis_task_id,
                    }
                });
                insert_stream_candidate_gateway(&mut result_event, self.candidate_gateway.as_ref());
                vec![result_event, json!({"type":"done"})]
            }
            BatchGenerationStreamTerminalKind::Failed => {
                let error_message = self
                    .task
                    .error_message
                    .clone()
                    .filter(|message| {
                        !(is_manual_review_telemetry(
                            &self.quality_gate,
                            &self.active_story_repair_payload,
                        ) && looks_like_manual_review_message(message))
                    })
                    .unwrap_or_else(|| "批量生成任务执行失败".to_string());
                let mut error_event = json!({
                    "type": "error",
                    "error": error_message,
                    "code": 500,
                    "phase": "failed"
                });
                insert_stream_candidate_gateway(&mut error_event, self.candidate_gateway.as_ref());
                vec![error_event, json!({"type":"done"})]
            }
            BatchGenerationStreamTerminalKind::Cancelled => vec![json!({"type":"done"})],
        })
    }
}

fn is_manual_review_telemetry(
    quality_gate: &Option<Value>,
    active_payload: &Option<Value>,
) -> bool {
    quality_gate.as_ref().is_some_and(is_manual_review_payload)
        || active_payload
            .as_ref()
            .is_some_and(is_manual_review_payload)
}

fn is_manual_review_payload(payload: &Value) -> bool {
    let Some(object) = payload.as_object() else {
        return false;
    };

    object
        .get("decision")
        .or_else(|| object.get("quality_gate_decision"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| value == "manual_review")
        || object
            .get("quality_gate")
            .is_some_and(is_manual_review_payload)
}

fn looks_like_manual_review_message(message: &str) -> bool {
    let value = message.trim();
    value.contains("人工复核") || value.contains("需复核")
}

pub(crate) fn insert_stream_candidate_gateway(
    event: &mut Value,
    candidate_gateway: Option<&Value>,
) {
    if let Some(candidate_gateway) = candidate_gateway {
        event["candidate_gateway"] = candidate_gateway.clone();
    }
}

fn resolve_stream_candidate_gateway(workflow_runtime_state: Option<&Value>) -> Option<Value> {
    workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("candidate_gateway"))
        .filter(|metadata| metadata.is_object())
        .cloned()
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
                Some(BatchGenerationFailedTerminalKind::Retry)
            ) && matches!(phase, "quality_blocked" | "repair_pending" | "saving") =>
        {
            "running"
        }
        _ => resolved_status.event_status(),
    }
}

impl BatchGenerationResolvedStreamStatus {
    pub(crate) fn from_status(status: &str) -> Self {
        match status {
            "pending" => Self::Pending,
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn default_progress(self) -> i32 {
        match self {
            Self::Pending => 10,
            Self::Running => 65,
            Self::Completed | Self::Failed | Self::Cancelled => 100,
            Self::Unknown => 15,
        }
    }

    pub(crate) fn default_message(self) -> &'static str {
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

    pub(crate) fn event_status(self) -> &'static str {
        match self {
            Self::Failed => "error",
            Self::Completed => "success",
            Self::Pending | Self::Running | Self::Cancelled | Self::Unknown => "processing",
        }
    }

    pub(crate) fn terminal_kind(
        self,
        retryable_repair_terminal_label: Option<&String>,
    ) -> Option<BatchGenerationStreamTerminalKind> {
        match self {
            Self::Completed => Some(BatchGenerationStreamTerminalKind::Completed),
            Self::Failed if retryable_repair_terminal_label.is_some() => {
                Some(BatchGenerationStreamTerminalKind::Failed)
            }
            Self::Failed => Some(BatchGenerationStreamTerminalKind::Failed),
            Self::Cancelled => Some(BatchGenerationStreamTerminalKind::Cancelled),
            Self::Pending | Self::Running | Self::Unknown => None,
        }
    }
}

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

pub(crate) fn build_batch_generation_stream_state_from_task_and_snapshot(
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

pub(crate) async fn load_owned_batch_generation_stream_state(
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

async fn send_stream_events(tx: &mpsc::Sender<Result<Event, Infallible>>, events: Vec<Value>) {
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
