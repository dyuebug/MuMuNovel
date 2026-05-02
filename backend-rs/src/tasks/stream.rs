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
        let senders = self.senders.read().await;
        if let Some(sender) = senders.get(task_id) {
            return sender.subscribe();
        }
        drop(senders);
        let (tx, rx) = broadcast::channel(DEFAULT_CAPACITY);
        self.senders.write().await.insert(task_id.to_string(), tx);
        rx
    }

    pub async fn unsubscribe(&self, task_id: &str) {
        self.senders.write().await.remove(task_id);
    }

    pub fn fanout(&self, task_id: &str, event: &TaskEvent) {
        if let Ok(senders) = self.senders.try_read() {
            if let Some(sender) = senders.get(task_id) {
                if let Ok(json) = serde_json::to_string(event) {
                    let _ = sender.send(json);
                }
            }
        }
    }

    pub async fn fanout_async(&self, task_id: &str, event: &TaskEvent) {
        let senders = self.senders.read().await;
        if let Some(sender) = senders.get(task_id) {
            if let Ok(json) = serde_json::to_string(event) {
                let _ = sender.send(format!("data: {}\n\n", json));
            }
        }
    }

    pub async fn has_subscribers(&self, task_id: &str) -> bool {
        self.senders
            .read()
            .await
            .get(task_id)
            .map(|s| s.receiver_count() > 0)
            .unwrap_or(false)
    }

    pub async fn cleanup_stale(&self) {
        self.senders
            .write()
            .await
            .retain(|_, sender| sender.receiver_count() > 0);
    }
}
