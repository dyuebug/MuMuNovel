use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use crate::tasks::{stream::TaskStreamHub, types::TaskEvent};

/// Best-effort runtime-only output bridge for one durable autopilot background task.
///
/// Model output is intentionally never persisted to the Run/Step records. Subscribers may
/// disconnect without affecting generation, and providers that do not expose reasoning simply
/// leave the reasoning channel empty.
#[derive(Clone)]
pub(crate) struct NovelAutopilotOutputObserver {
    stream_hub: TaskStreamHub,
    task_id: String,
    estimated_tokens: Arc<AtomicU64>,
}

impl NovelAutopilotOutputObserver {
    pub(crate) fn new(stream_hub: TaskStreamHub, task_id: impl Into<String>) -> Self {
        Self {
            stream_hub,
            task_id: task_id.into(),
            estimated_tokens: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(crate) async fn content(&self, content: impl Into<String>) {
        self.emit("chunk", content.into()).await;
    }

    pub(crate) async fn reasoning(&self, content: impl Into<String>) {
        self.emit("reasoning_chunk", content.into()).await;
    }

    /// Returns and clears the conservative output-token estimate observed in this tick.
    ///
    /// The provider abstraction does not expose authoritative usage. Counting one Unicode scalar
    /// as one token deliberately overestimates most Latin output while remaining understandable
    /// for Chinese output. The value is an operational safety budget, not a billing statement.
    pub(crate) fn take_estimated_tokens(&self) -> u64 {
        self.estimated_tokens.swap(0, Ordering::AcqRel)
    }

    pub(crate) fn reset_estimated_tokens(&self) {
        self.estimated_tokens.store(0, Ordering::Release);
    }

    async fn emit(&self, event_type: &str, content: String) {
        if content.is_empty() {
            return;
        }
        let estimated = u64::try_from(content.chars().count()).unwrap_or(u64::MAX);
        let _ =
            self.estimated_tokens
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    Some(current.saturating_add(estimated))
                });
        self.stream_hub
            .fanout(
                &self.task_id,
                &TaskEvent {
                    event_type: event_type.to_string(),
                    task_id: Some(self.task_id.clone()),
                    message: None,
                    progress: None,
                    status: Some("running".to_string()),
                    content: Some(content),
                    data: None,
                    error: None,
                },
            )
            .await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::NovelAutopilotOutputObserver;
    use crate::tasks::{stream::TaskStreamHub, types::TaskEvent};

    async fn receive(receiver: &mut tokio::sync::broadcast::Receiver<String>) -> TaskEvent {
        let payload = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("observer event should arrive")
            .expect("observer receiver should remain connected");
        serde_json::from_str(&payload).expect("observer event should be valid JSON")
    }

    #[tokio::test]
    async fn emits_content_and_provider_reasoning_on_separate_channels() {
        let hub = TaskStreamHub::new();
        let mut receiver = hub.subscribe("task-1").await;
        let observer = NovelAutopilotOutputObserver::new(hub, "task-1");

        observer.content("正文").await;
        observer.reasoning("provider reasoning").await;

        let content = receive(&mut receiver).await;
        assert_eq!(content.event_type, "chunk");
        assert_eq!(content.content.as_deref(), Some("正文"));
        let reasoning = receive(&mut receiver).await;
        assert_eq!(reasoning.event_type, "reasoning_chunk");
        assert_eq!(reasoning.content.as_deref(), Some("provider reasoning"));
    }

    #[tokio::test]
    async fn no_subscriber_and_empty_chunks_do_not_fail_generation() {
        let hub = TaskStreamHub::new();
        let observer = NovelAutopilotOutputObserver::new(hub, "task-without-subscriber");

        observer.content("").await;
        observer.reasoning("not persisted").await;
        assert_eq!(observer.take_estimated_tokens(), 13);
        assert_eq!(observer.take_estimated_tokens(), 0);
    }

    #[tokio::test]
    async fn estimates_content_and_reasoning_without_persisting_payloads() {
        let hub = TaskStreamHub::new();
        let observer = NovelAutopilotOutputObserver::new(hub, "task-estimate");

        observer.content("正文").await;
        observer.reasoning("abc").await;
        assert_eq!(observer.take_estimated_tokens(), 5);

        observer.content("reset").await;
        observer.reset_estimated_tokens();
        assert_eq!(observer.take_estimated_tokens(), 0);
    }
}
