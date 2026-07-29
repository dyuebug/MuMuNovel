use axum::response::sse::{Event, KeepAlive};
use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Structured SSE event builder — replicates Python SSEResponse + WizardProgressTracker
pub struct SseProgress {
    current_progress: u32,
    task_name: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProgressPayload {
    #[serde(rename = "type")]
    event_type: String,
    message: String,
    progress: u32,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
struct ResultPayload {
    #[serde(rename = "type")]
    event_type: String,
    data: Value,
}

#[derive(Debug, Clone, Serialize)]
struct ChunkPayload {
    #[serde(rename = "type")]
    event_type: String,
    content: String,
}

#[derive(Debug, Clone, Serialize)]
struct ErrorPayload {
    #[serde(rename = "type")]
    event_type: String,
    error: String,
    code: u16,
}

#[derive(Debug, Clone, Serialize)]
struct DonePayload {
    #[serde(rename = "type")]
    event_type: String,
}

/// Create a progress event (matching Python SSEResponse.send_progress)
pub fn sse_progress(message: &str, progress: u32, status: &str) -> Event {
    let payload = ProgressPayload {
        event_type: "progress".into(),
        message: message.to_string(),
        progress: progress.clamp(0, 100),
        status: status.to_string(),
    };
    Event::default().data(serde_json::to_string(&payload).unwrap_or_default())
}

/// Create a chunk event for streaming content (matching Python SSEResponse.send_chunk)
pub fn sse_chunk(content: &str) -> Event {
    let payload = ChunkPayload {
        event_type: "chunk".into(),
        content: content.to_string(),
    };
    Event::default().data(serde_json::to_string(&payload).unwrap_or_default())
}

/// Create a reasoning chunk event from Provider-explicit reasoning/thinking output.
pub fn sse_reasoning_chunk(content: &str) -> Event {
    let payload = ChunkPayload {
        event_type: "reasoning_chunk".into(),
        content: content.to_string(),
    };
    Event::default().data(serde_json::to_string(&payload).unwrap_or_default())
}

/// Create a result event (matching Python SSEResponse.send_result)
pub fn sse_result(data: &Value) -> Event {
    let payload = ResultPayload {
        event_type: "result".into(),
        data: data.clone(),
    };
    Event::default().data(serde_json::to_string(&payload).unwrap_or_default())
}

/// Create a custom JSON event payload for non-standard stream contracts.
pub fn sse_json(data: &Value) -> Event {
    Event::default().data(serde_json::to_string(data).unwrap_or_default())
}

/// Create an error event (matching Python SSEResponse.send_error)
pub fn sse_error(error: &str, code: u16) -> Event {
    let payload = ErrorPayload {
        event_type: "error".into(),
        error: error.to_string(),
        code,
    };
    Event::default().data(serde_json::to_string(&payload).unwrap_or_default())
}

/// Create a done event (matching Python SSEResponse.send_done)
pub fn sse_done() -> Event {
    let payload = DonePayload {
        event_type: "done".into(),
    };
    Event::default().data(serde_json::to_string(&payload).unwrap_or_default())
}

pub fn default_sse_keep_alive() -> KeepAlive {
    KeepAlive::new().interval(Duration::from_secs(10))
}

pub fn named_sse_keep_alive(text: &'static str) -> KeepAlive {
    default_sse_keep_alive().text(text)
}

impl SseProgress {
    pub fn new(task_name: &str) -> Self {
        Self {
            current_progress: 0,
            task_name: task_name.to_string(),
        }
    }

    pub fn start(&mut self) -> Event {
        self.current_progress = 0;
        sse_progress(&format!("开始生成{}...", self.task_name), 0, "processing")
    }

    pub fn preparing(&mut self, message: Option<&str>) -> Event {
        let progress = self.current_progress.max(15).min(20);
        self.current_progress = progress;
        let msg = message.unwrap_or("准备中...");
        sse_progress(msg, progress, "processing")
    }

    pub fn generating(
        &mut self,
        message: Option<&str>,
        progress_range: (u32, u32),
        char_count: usize,
        retry_count: Option<u32>,
    ) -> Event {
        let base = progress_range.0;
        let range = progress_range.1.saturating_sub(base);
        let char_bonus = (char_count as f64 / 2000.0 * range as f64) as u32;
        let progress = (base + char_bonus).clamp(base, progress_range.1);
        self.current_progress = progress;

        let mut msg = message
            .unwrap_or(&format!("生成{}中...", self.task_name))
            .to_string();
        if char_count > 0 {
            msg = format!("生成{}中... ({}字符)", self.task_name, char_count);
        }
        if let Some(n) = retry_count {
            msg.push_str(&format!(" (重试 {}/{})", n, 3));
        }

        sse_progress(&msg, progress, "processing")
    }

    pub fn complete(&mut self, message: Option<&str>) -> Event {
        self.current_progress = 100;
        sse_progress(
            message.unwrap_or(&format!("{}生成完成!", self.task_name)),
            100,
            "success",
        )
    }

    #[cfg(test)]
    pub fn current_progress(&self) -> u32 {
        self.current_progress
    }
}

/// Helper for building SSE streams via mpsc channel
pub struct SseChannel {
    tx: tokio::sync::mpsc::Sender<Result<Event, std::convert::Infallible>>,
    result_capture: Option<Arc<Mutex<Option<Value>>>>,
    state_capture: Option<Arc<Mutex<SseTaskCapture>>>,
}

const MAX_PENDING_TASK_OUTPUT_EVENTS: usize = 256;
const MAX_PENDING_TASK_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_MERGED_TASK_OUTPUT_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SseTaskOutputKind {
    Content,
    Reasoning,
}

impl SseTaskOutputKind {
    pub fn event_type(self) -> &'static str {
        match self {
            Self::Content => "chunk",
            Self::Reasoning => "reasoning_chunk",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseTaskOutputEvent {
    kind: SseTaskOutputKind,
    content: String,
}

impl SseTaskOutputEvent {
    pub fn event_type(&self) -> &'static str {
        self.kind.event_type()
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

#[derive(Debug, Clone, Default)]
pub struct SseTaskCapture {
    pub message: Option<String>,
    pub progress: Option<u32>,
    pub status: Option<String>,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub done: bool,
    pending_output_events: VecDeque<SseTaskOutputEvent>,
    pending_output_bytes: usize,
}

impl SseTaskCapture {
    fn capture_output(&mut self, kind: SseTaskOutputKind, content: &str) {
        if content.is_empty() {
            return;
        }

        let content = utf8_tail(content, MAX_PENDING_TASK_OUTPUT_BYTES);
        if let Some(last) = self.pending_output_events.back_mut() {
            if last.kind == kind
                && last.content.len().saturating_add(content.len()) <= MAX_MERGED_TASK_OUTPUT_BYTES
            {
                last.content.push_str(content);
                self.pending_output_bytes = self.pending_output_bytes.saturating_add(content.len());
                return;
            }
        }

        while self.pending_output_events.len() >= MAX_PENDING_TASK_OUTPUT_EVENTS
            || self.pending_output_bytes.saturating_add(content.len())
                > MAX_PENDING_TASK_OUTPUT_BYTES
        {
            let Some(removed) = self.pending_output_events.pop_front() else {
                break;
            };
            self.pending_output_bytes = self
                .pending_output_bytes
                .saturating_sub(removed.content.len());
        }

        self.pending_output_bytes = self.pending_output_bytes.saturating_add(content.len());
        self.pending_output_events.push_back(SseTaskOutputEvent {
            kind,
            content: content.to_string(),
        });
    }

    pub fn drain_output_events(&mut self) -> Vec<SseTaskOutputEvent> {
        self.pending_output_bytes = 0;
        self.pending_output_events.drain(..).collect()
    }
}

fn utf8_tail(content: &str, max_bytes: usize) -> &str {
    if content.len() <= max_bytes {
        return content;
    }

    let mut start = content.len().saturating_sub(max_bytes);
    while start < content.len() && !content.is_char_boundary(start) {
        start += 1;
    }
    &content[start..]
}

impl SseChannel {
    pub fn new(tx: tokio::sync::mpsc::Sender<Result<Event, std::convert::Infallible>>) -> Self {
        Self {
            tx,
            result_capture: None,
            state_capture: None,
        }
    }

    pub fn with_result_capture(
        tx: tokio::sync::mpsc::Sender<Result<Event, std::convert::Infallible>>,
        result_capture: Arc<Mutex<Option<Value>>>,
    ) -> Self {
        Self {
            tx,
            result_capture: Some(result_capture),
            state_capture: None,
        }
    }

    pub fn with_captures(
        tx: tokio::sync::mpsc::Sender<Result<Event, std::convert::Infallible>>,
        result_capture: Arc<Mutex<Option<Value>>>,
        state_capture: Arc<Mutex<SseTaskCapture>>,
    ) -> Self {
        Self {
            tx,
            result_capture: Some(result_capture),
            state_capture: Some(state_capture),
        }
    }

    pub async fn send(&self, event: Event) {
        let _ = self.tx.send(Ok(event)).await;
    }

    pub async fn progress(&self, message: &str, progress: u32, status: &str) {
        if let Some(capture) = &self.state_capture {
            let mut state = capture.lock().await;
            state.message = Some(message.to_string());
            state.progress = Some(progress.clamp(0, 100));
            state.status = Some(status.to_string());
        }
        self.send(sse_progress(message, progress, status)).await;
    }

    pub async fn chunk(&self, content: &str) {
        if let Some(capture) = &self.state_capture {
            capture
                .lock()
                .await
                .capture_output(SseTaskOutputKind::Content, content);
        }
        self.send(sse_chunk(content)).await;
    }

    pub async fn reasoning_chunk(&self, content: &str) {
        if let Some(capture) = &self.state_capture {
            capture
                .lock()
                .await
                .capture_output(SseTaskOutputKind::Reasoning, content);
        }
        self.send(sse_reasoning_chunk(content)).await;
    }

    pub async fn result(&self, data: &Value) {
        if let Some(capture) = &self.result_capture {
            *capture.lock().await = Some(data.clone());
        }
        if let Some(capture) = &self.state_capture {
            let mut state = capture.lock().await;
            state.result = Some(data.clone());
        }
        self.send(sse_result(data)).await;
    }

    pub async fn error(&self, error: &str, code: u16) {
        if let Some(capture) = &self.state_capture {
            let mut state = capture.lock().await;
            state.error = Some(error.to_string());
            state.status = Some("error".to_string());
        }
        self.send(sse_error(error, code)).await;
    }

    pub async fn done(&self) {
        if let Some(capture) = &self.state_capture {
            capture.lock().await.done = true;
        }
        self.send(sse_done()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn task_capture_keeps_reasoning_and_content_separate_and_ordered() {
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let result_capture = Arc::new(Mutex::new(None));
        let state_capture = Arc::new(Mutex::new(SseTaskCapture::default()));
        let channel = SseChannel::with_captures(tx, result_capture, state_capture.clone());

        channel.reasoning_chunk("先分析").await;
        channel.reasoning_chunk("再判断").await;
        channel.chunk("最终正文").await;

        let events = state_capture.lock().await.drain_output_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type(), "reasoning_chunk");
        assert_eq!(events[0].content(), "先分析再判断");
        assert_eq!(events[1].event_type(), "chunk");
        assert_eq!(events[1].content(), "最终正文");
    }

    #[test]
    fn task_capture_bounds_transient_output_memory_on_utf8_boundaries() {
        let mut capture = SseTaskCapture::default();
        let oversized = "思".repeat(MAX_PENDING_TASK_OUTPUT_BYTES);
        capture.capture_output(SseTaskOutputKind::Reasoning, &oversized);

        let events = capture.drain_output_events();
        assert_eq!(events.len(), 1);
        assert!(events[0].content().len() <= MAX_PENDING_TASK_OUTPUT_BYTES);
        assert!(std::str::from_utf8(events[0].content().as_bytes()).is_ok());
    }

    #[test]
    fn test_progress_tracker_state() {
        let mut tracker = SseProgress::new("世界观");
        let _start = tracker.start();
        assert_eq!(tracker.current_progress(), 0);

        let _preparing = tracker.preparing(None);
        assert_eq!(tracker.current_progress(), 15);

        let _generating = tracker.generating(Some("生成中"), (20, 90), 0, None);
        assert_eq!(tracker.current_progress(), 20);

        let _done = tracker.complete(None);
        assert_eq!(tracker.current_progress(), 100);
    }

    #[test]
    fn test_sse_event_functions_dont_panic() {
        let _ = sse_progress("test", 0, "processing");
        let _ = sse_chunk("hello");
        let _ = sse_reasoning_chunk("thinking");
        let _ = sse_json(&serde_json::json!({"type": "analysis_started", "task_id": "task-1"}));
        let _ = sse_result(&serde_json::json!({"key": "value"}));
        let _ = sse_error("error", 500);
        let _ = sse_done();
    }
}
