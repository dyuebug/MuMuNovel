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

    pub fn fanout(&self, task_id: &str, event: &TaskEvent) {
        if let Ok(senders) = self.senders.try_read() {
            if let Some(sender) = senders.get(task_id) {
                if let Ok(json) = serde_json::to_string(event) {
                    let _ = sender.send(json);
                }
            }
        }
    }
}
