use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::tasks::types::TaskEvent;

const DEFAULT_CAPACITY: usize = 200;

#[derive(Clone)]
pub struct TaskStreamHub {
    senders: Arc<RwLock<HashMap<String, broadcast::Sender<String>>>>,
}

impl TaskStreamHub {
    pub fn new() -> Self {
        Self {
            senders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn subscribe(&self, task_id: &str) -> broadcast::Receiver<String> {
        let mut senders = self.senders.write().await;
        if let Some(sender) = senders.get(task_id) {
            return sender.subscribe();
        }

        let (tx, rx) = broadcast::channel(DEFAULT_CAPACITY);
        senders.insert(task_id.to_string(), tx);
        rx
    }

    pub async fn fanout(&self, task_id: &str, event: &TaskEvent) {
        let sender = self.senders.read().await.get(task_id).cloned();
        Self::send(sender, event);
    }

    pub async fn fanout_terminal(&self, task_id: &str, event: &TaskEvent) {
        let sender = self.senders.write().await.remove(task_id);
        Self::send(sender, event);
    }

    fn send(sender: Option<broadcast::Sender<String>>, event: &TaskEvent) {
        if let Some(sender) = sender {
            if let Ok(json) = serde_json::to_string(event) {
                let _ = sender.send(json);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use serde_json::json;
    use tokio::sync::Barrier;
    use tokio::time::timeout;

    use super::TaskStreamHub;
    use crate::tasks::types::TaskEvent;

    fn progress_event() -> TaskEvent {
        TaskEvent {
            event_type: "progress".to_string(),
            task_id: Some("task-1".to_string()),
            message: Some("running".to_string()),
            progress: Some(42),
            status: Some("running".to_string()),
            data: None,
            error: None,
        }
    }

    #[tokio::test]
    async fn fanout_waits_for_sender_map_lock_instead_of_dropping_event() {
        let hub = TaskStreamHub::new();
        let mut receiver = hub.subscribe("task-1").await;
        let guard = hub.senders.write().await;
        let fanout_hub = hub.clone();
        let fanout = tokio::spawn(async move {
            fanout_hub.fanout("task-1", &progress_event()).await;
        });

        tokio::task::yield_now().await;
        drop(guard);
        fanout.await.expect("fanout task should complete");

        let payload = timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("fanout should not be dropped while the sender map is locked")
            .expect("receiver should remain connected");
        let event: TaskEvent = serde_json::from_str(&payload).expect("event should be valid JSON");
        assert_eq!(event.event_type, "progress");
        assert_eq!(event.progress, Some(42));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_first_subscribers_share_one_broadcast_channel() {
        let hub = TaskStreamHub::new();
        let barrier = Arc::new(Barrier::new(3));

        let first_hub = hub.clone();
        let first_barrier = Arc::clone(&barrier);
        let first = tokio::spawn(async move {
            first_barrier.wait().await;
            first_hub.subscribe("task-1").await
        });
        let second_hub = hub.clone();
        let second_barrier = Arc::clone(&barrier);
        let second = tokio::spawn(async move {
            second_barrier.wait().await;
            second_hub.subscribe("task-1").await
        });

        barrier.wait().await;
        let mut first_receiver = first.await.expect("first subscription should complete");
        let mut second_receiver = second.await.expect("second subscription should complete");
        hub.fanout("task-1", &progress_event()).await;

        for receiver in [&mut first_receiver, &mut second_receiver] {
            let payload = timeout(Duration::from_secs(1), receiver.recv())
                .await
                .expect("both first subscribers should receive the event")
                .expect("subscriber should remain connected");
            let event: serde_json::Value =
                serde_json::from_str(&payload).expect("event should be valid JSON");
            assert_eq!(event["type"], json!("progress"));
        }
    }

    #[tokio::test]
    async fn terminal_fanout_delivers_then_releases_sender_for_reconnect() {
        let hub = TaskStreamHub::new();
        let mut terminal_receiver = hub.subscribe("task-1").await;
        let terminal_event = TaskEvent {
            event_type: "done".to_string(),
            task_id: Some("task-1".to_string()),
            message: Some("completed".to_string()),
            progress: Some(100),
            status: Some("completed".to_string()),
            data: None,
            error: None,
        };

        hub.fanout_terminal("task-1", &terminal_event).await;

        let payload = terminal_receiver
            .recv()
            .await
            .expect("existing subscriber should receive the terminal event");
        let event: TaskEvent =
            serde_json::from_str(&payload).expect("terminal event should be valid JSON");
        assert_eq!(event.event_type, "done");
        assert!(matches!(
            terminal_receiver.recv().await,
            Err(tokio::sync::broadcast::error::RecvError::Closed)
        ));
        assert!(!hub.senders.read().await.contains_key("task-1"));

        let mut reconnected_receiver = hub.subscribe("task-1").await;
        hub.fanout("task-1", &progress_event()).await;
        let payload = reconnected_receiver
            .recv()
            .await
            .expect("reconnected subscriber should use a fresh channel");
        let event: TaskEvent =
            serde_json::from_str(&payload).expect("reconnected event should be valid JSON");
        assert_eq!(event.event_type, "progress");
    }
}
