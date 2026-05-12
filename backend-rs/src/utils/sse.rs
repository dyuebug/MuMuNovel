use axum::response::sse::Event;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
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

/// Format any serializable payload into an SSE event string
pub fn format_sse(data: &impl Serialize) -> String {
    let json = serde_json::to_string(data)
        .unwrap_or_else(|_| r#"{"type":"error","error":"serialize failed"}"#.into());
    format!("data: {}\n\n", json)
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

/// Create a result event (matching Python SSEResponse.send_result)
pub fn sse_result(data: &Value) -> Event {
    let payload = ResultPayload {
        event_type: "result".into(),
        data: data.clone(),
    };
    Event::default().data(serde_json::to_string(&payload).unwrap_or_default())
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

    pub fn progress(&mut self, message: &str, progress: u32) -> Event {
        self.current_progress = progress.clamp(0, 100);
        sse_progress(message, self.current_progress, "processing")
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

    pub fn parsing(&mut self, message: Option<&str>, progress: u32) -> Event {
        self.current_progress = progress;
        sse_progress(
            message.unwrap_or(&format!("解析{}数据...", self.task_name)),
            progress,
            "processing",
        )
    }

    pub fn saving(&mut self, message: Option<&str>) -> Event {
        let progress = self.current_progress.max(85).min(95);
        self.current_progress = progress;
        sse_progress(
            message.unwrap_or(&format!("保存{}到数据库...", self.task_name)),
            progress,
            "processing",
        )
    }

    pub fn complete(&mut self, message: Option<&str>) -> Event {
        self.current_progress = 100;
        sse_progress(
            message.unwrap_or(&format!("{}生成完成!", self.task_name)),
            100,
            "success",
        )
    }

    pub fn retry(&mut self, retry_count: u32, max_retries: u32, reason: &str) -> Event {
        sse_progress(
            &format!("⚠ {}... ({}/{})", reason, retry_count, max_retries),
            self.current_progress,
            "processing",
        )
    }

    pub fn warning(&mut self, message: &str) -> Event {
        sse_progress(
            &format!("⚠ {}", message),
            self.current_progress,
            "processing",
        )
    }

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

#[derive(Debug, Clone, Default)]
pub struct SseTaskCapture {
    pub message: Option<String>,
    pub progress: Option<u32>,
    pub status: Option<String>,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub done: bool,
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
        self.send(sse_chunk(content)).await;
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

/// Convenience: shared progress tracker for multi-step operations
pub type SharedProgress = Arc<Mutex<SseProgress>>;

pub fn shared_progress(task_name: &str) -> SharedProgress {
    Arc::new(Mutex::new(SseProgress::new(task_name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_sse_progress() {
        let payload = ProgressPayload {
            event_type: "progress".into(),
            message: "测试消息".into(),
            progress: 50,
            status: "processing".into(),
        };
        let data = format_sse(&payload);
        assert!(data.contains("\"type\":\"progress\""));
        assert!(data.contains("测试消息"));
        assert!(data.contains("\"progress\":50"));
        assert!(data.starts_with("data: "));
    }

    #[test]
    fn test_format_sse_done() {
        let payload = DonePayload {
            event_type: "done".into(),
        };
        let data = format_sse(&payload);
        assert!(data.contains("\"type\":\"done\""));
    }

    #[test]
    fn test_format_sse_error() {
        let payload = ErrorPayload {
            event_type: "error".into(),
            error: "错误信息".into(),
            code: 500,
        };
        let data = format_sse(&payload);
        assert!(data.contains("\"type\":\"error\""));
        assert!(data.contains("错误信息"));
        assert!(data.contains("\"code\":500"));
    }

    #[test]
    fn test_progress_tracker_state() {
        let mut tracker = SseProgress::new("世界观");
        let _start = tracker.start();
        assert_eq!(tracker.current_progress(), 0);

        let _prog = tracker.progress("生成中", 50);
        assert_eq!(tracker.current_progress(), 50);

        let _done = tracker.complete(None);
        assert_eq!(tracker.current_progress(), 100);
    }

    #[test]
    fn test_sse_event_functions_dont_panic() {
        sse_progress("test", 0, "processing");
        sse_chunk("hello");
        sse_result(&serde_json::json!({"key": "value"}));
        sse_error("error", 500);
        sse_done();
    }
}
