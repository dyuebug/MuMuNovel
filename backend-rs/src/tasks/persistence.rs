use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{error, info};

use crate::tasks::registry::TaskRegistry;
use crate::tasks::types::TaskRecord;

const SNAPSHOT_DIR: &str = "data/runtime";
const SNAPSHOT_FILE: &str = "background_tasks.json";

#[derive(Debug, Serialize, Deserialize)]
struct Snapshot {
    version: u32,
    updated_at: String,
    items: Vec<TaskRecord>,
}

pub async fn load_from_disk(registry: &TaskRegistry) {
    let path = Path::new(SNAPSHOT_DIR).join(SNAPSHOT_FILE);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => match serde_json::from_str::<Snapshot>(&content) {
            Ok(snapshot) => {
                registry.load_records(snapshot.items).await;
                info!(
                    "Loaded {} background tasks from disk snapshot",
                    registry.all_records().await.len()
                );
            }
            Err(e) => {
                error!("Failed to parse background tasks snapshot: {}", e);
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            info!("No existing background tasks snapshot found, starting fresh");
        }
        Err(e) => {
            error!("Failed to read background tasks snapshot: {}", e);
        }
    }
}

pub async fn save_to_disk(registry: &TaskRegistry) {
    let mut records = registry.all_records().await;
    records.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

    let snapshot = Snapshot {
        version: 1,
        updated_at: Utc::now().to_rfc3339(),
        items: records,
    };

    let dir = Path::new(SNAPSHOT_DIR);
    if let Err(e) = tokio::fs::create_dir_all(dir).await {
        error!("Failed to create snapshot directory: {}", e);
        return;
    }

    let path = dir.join(SNAPSHOT_FILE);
    if let Ok(json) = serde_json::to_string(&snapshot) {
        if let Err(e) = tokio::fs::write(&path, json).await {
            error!("Failed to write background tasks snapshot: {}", e);
        }
    }
}

/// Start periodic auto-save (every 1.5 seconds, matching Python)
pub fn start_periodic_save(registry: TaskRegistry) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
            save_to_disk(&registry).await;
        }
    });
}
