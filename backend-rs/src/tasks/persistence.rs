use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::tasks::registry::TaskRegistry;
use crate::tasks::types::TaskRecord;

const SNAPSHOT_VERSION: u32 = 1;
const SNAPSHOT_DIR: &str = "data/runtime";
const SNAPSHOT_FILE: &str = "background_tasks.json";
const SNAPSHOT_BACKUP_FILE: &str = "background_tasks.json.bak";
const SNAPSHOT_TEMP_FILE: &str = "background_tasks.json.tmp";

static SNAPSHOT_WRITE_LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Debug, Serialize, Deserialize)]
struct Snapshot {
    version: u32,
    updated_at: String,
    items: Vec<TaskRecord>,
}

#[derive(Debug, Clone)]
struct SnapshotPaths {
    dir: PathBuf,
    primary: PathBuf,
    backup: PathBuf,
    temporary: PathBuf,
}

impl SnapshotPaths {
    fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            primary: dir.join(SNAPSHOT_FILE),
            backup: dir.join(SNAPSHOT_BACKUP_FILE),
            temporary: dir.join(SNAPSHOT_TEMP_FILE),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SnapshotSource {
    Primary,
    Backup,
    Temporary,
}

impl SnapshotSource {
    fn label(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Backup => "backup",
            Self::Temporary => "temporary",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadOutcome {
    Loaded {
        source: SnapshotSource,
        item_count: usize,
    },
    Empty,
}

#[derive(Debug)]
enum SnapshotPersistenceError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Serialize(serde_json::Error),
    InvalidSnapshot {
        path: PathBuf,
        reason: String,
    },
}

impl SnapshotPersistenceError {
    fn io(operation: &'static str, path: &Path, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.to_path_buf(),
            source,
        }
    }
}

impl fmt::Display for SnapshotPersistenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "{} background task snapshot at {} failed: {}",
                operation,
                path.display(),
                source
            ),
            Self::Serialize(source) => {
                write!(
                    formatter,
                    "serialize background task snapshot failed: {source}"
                )
            }
            Self::InvalidSnapshot { path, reason } => write!(
                formatter,
                "background task snapshot at {} is invalid: {}",
                path.display(),
                reason
            ),
        }
    }
}

impl std::error::Error for SnapshotPersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Serialize(source) => Some(source),
            Self::InvalidSnapshot { .. } => None,
        }
    }
}

pub async fn load_from_disk(registry: &TaskRegistry) {
    let outcome = load_from_dir(registry, Path::new(SNAPSHOT_DIR)).await;
    if outcome == LoadOutcome::Empty {
        info!("No valid background tasks snapshot found, starting fresh");
    }
}

async fn load_from_dir(registry: &TaskRegistry, dir: &Path) -> LoadOutcome {
    let paths = SnapshotPaths::new(dir);
    let candidates = [
        (SnapshotSource::Primary, &paths.primary),
        (SnapshotSource::Backup, &paths.backup),
        (SnapshotSource::Temporary, &paths.temporary),
    ];

    for (source, path) in candidates {
        match read_snapshot(path).await {
            Ok(Some(snapshot)) => {
                let item_count = snapshot.items.len();
                registry.load_records(snapshot.items).await;
                info!(
                    source = source.label(),
                    item_count, "Loaded background tasks from disk snapshot"
                );
                return LoadOutcome::Loaded { source, item_count };
            }
            Ok(None) => {}
            Err(error @ SnapshotPersistenceError::InvalidSnapshot { .. }) => {
                error!(
                    source = source.label(),
                    error = %error,
                    "Rejected invalid background tasks snapshot"
                );
                if let Err(quarantine_error) = quarantine_corrupt_candidate(path).await {
                    error!(
                        source = source.label(),
                        error = %quarantine_error,
                        "Failed to quarantine invalid background tasks snapshot"
                    );
                }
            }
            Err(error) => {
                error!(
                    source = source.label(),
                    error = %error,
                    "Failed to read background tasks snapshot candidate"
                );
            }
        }
    }

    LoadOutcome::Empty
}

pub async fn save_to_disk(registry: &TaskRegistry) {
    if let Err(error) = save_to_dir(registry, Path::new(SNAPSHOT_DIR)).await {
        error!(error = %error, "Failed to persist background tasks snapshot");
    }
}

async fn save_to_dir(registry: &TaskRegistry, dir: &Path) -> Result<(), SnapshotPersistenceError> {
    let _write_guard = SNAPSHOT_WRITE_LOCK.lock().await;

    let mut records = registry.all_records().await;
    records.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));

    let snapshot = Snapshot {
        version: SNAPSHOT_VERSION,
        updated_at: Utc::now().to_rfc3339(),
        items: records,
    };
    let serialized = serde_json::to_vec(&snapshot).map_err(SnapshotPersistenceError::Serialize)?;

    let paths = SnapshotPaths::new(dir);
    tokio::fs::create_dir_all(&paths.dir)
        .await
        .map_err(|error| SnapshotPersistenceError::io("create directory for", &paths.dir, error))?;

    write_synced_temporary_snapshot(&paths.temporary, &serialized).await?;
    commit_temporary_snapshot(&paths).await?;

    if let Err(error) = sync_snapshot_directory(&paths.dir).await {
        warn!(
            path = %paths.dir.display(),
            error = %error,
            "Background task snapshot committed but directory sync was unavailable"
        );
    }

    Ok(())
}

async fn write_synced_temporary_snapshot(
    path: &Path,
    serialized: &[u8],
) -> Result<(), SnapshotPersistenceError> {
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .await
        .map_err(|error| SnapshotPersistenceError::io("open temporary", path, error))?;

    file.write_all(serialized)
        .await
        .map_err(|error| SnapshotPersistenceError::io("write temporary", path, error))?;
    file.flush()
        .await
        .map_err(|error| SnapshotPersistenceError::io("flush temporary", path, error))?;
    file.sync_all()
        .await
        .map_err(|error| SnapshotPersistenceError::io("sync temporary", path, error))?;
    drop(file);

    decode_snapshot(serialized, path).map(|_| ())
}

async fn commit_temporary_snapshot(paths: &SnapshotPaths) -> Result<(), SnapshotPersistenceError> {
    match read_snapshot(&paths.primary).await {
        Ok(Some(_)) => rotate_primary_to_backup(paths).await?,
        Ok(None) => {}
        Err(SnapshotPersistenceError::InvalidSnapshot { .. }) => {
            quarantine_corrupt_candidate(&paths.primary).await?;
        }
        Err(error) => return Err(error),
    }

    if let Err(commit_error) = tokio::fs::rename(&paths.temporary, &paths.primary).await {
        let primary_missing = !path_exists(&paths.primary).await?;
        let backup_exists = path_exists(&paths.backup).await?;
        if primary_missing && backup_exists {
            if let Err(rollback_error) = tokio::fs::rename(&paths.backup, &paths.primary).await {
                error!(
                    error = %rollback_error,
                    "Failed to roll back background task snapshot after commit failure"
                );
            }
        }
        return Err(SnapshotPersistenceError::io(
            "commit temporary",
            &paths.primary,
            commit_error,
        ));
    }

    Ok(())
}

async fn rotate_primary_to_backup(paths: &SnapshotPaths) -> Result<(), SnapshotPersistenceError> {
    if path_exists(&paths.backup).await? {
        tokio::fs::remove_file(&paths.backup)
            .await
            .map_err(|error| {
                SnapshotPersistenceError::io("remove stale backup", &paths.backup, error)
            })?;
    }

    tokio::fs::rename(&paths.primary, &paths.backup)
        .await
        .map_err(|error| {
            SnapshotPersistenceError::io("rotate primary snapshot to backup", &paths.primary, error)
        })
}

async fn read_snapshot(path: &Path) -> Result<Option<Snapshot>, SnapshotPersistenceError> {
    let serialized = match tokio::fs::read(path).await {
        Ok(serialized) => serialized,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(SnapshotPersistenceError::io("read", path, error)),
    };

    decode_snapshot(&serialized, path).map(Some)
}

fn decode_snapshot(serialized: &[u8], path: &Path) -> Result<Snapshot, SnapshotPersistenceError> {
    let snapshot = serde_json::from_slice::<Snapshot>(serialized).map_err(|error| {
        SnapshotPersistenceError::InvalidSnapshot {
            path: path.to_path_buf(),
            reason: error.to_string(),
        }
    })?;

    if snapshot.version != SNAPSHOT_VERSION {
        return Err(SnapshotPersistenceError::InvalidSnapshot {
            path: path.to_path_buf(),
            reason: format!(
                "unsupported version {}, expected {}",
                snapshot.version, SNAPSHOT_VERSION
            ),
        });
    }

    Ok(snapshot)
}

async fn quarantine_corrupt_candidate(path: &Path) -> Result<PathBuf, SnapshotPersistenceError> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(SNAPSHOT_FILE);
    let quarantine_path = path.with_file_name(format!(
        "{}.corrupt-{}-{}",
        file_name,
        Utc::now().format("%Y%m%dT%H%M%S%.3fZ"),
        Uuid::new_v4()
    ));

    tokio::fs::rename(path, &quarantine_path)
        .await
        .map_err(|error| SnapshotPersistenceError::io("quarantine", path, error))?;
    warn!(
        source = %path.display(),
        quarantine = %quarantine_path.display(),
        "Quarantined invalid background tasks snapshot"
    );
    Ok(quarantine_path)
}

async fn path_exists(path: &Path) -> Result<bool, SnapshotPersistenceError> {
    match tokio::fs::metadata(path).await {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(SnapshotPersistenceError::io("inspect", path, error)),
    }
}

#[cfg(unix)]
async fn sync_snapshot_directory(dir: &Path) -> io::Result<()> {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || std::fs::File::open(dir)?.sync_all())
        .await
        .map_err(|error| io::Error::other(format!("directory sync task failed: {error}")))?
}

#[cfg(not(unix))]
async fn sync_snapshot_directory(_dir: &Path) -> io::Result<()> {
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{
        load_from_dir, read_snapshot, save_to_dir, LoadOutcome, SnapshotPaths, SnapshotSource,
        SNAPSHOT_FILE,
    };
    use crate::tasks::registry::TaskRegistry;
    use crate::tasks::types::TaskRecord;
    use serde_json::json;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    struct TestSnapshotDir {
        path: PathBuf,
    }

    impl TestSnapshotDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "mumunovel-background-task-snapshot-{}",
                Uuid::new_v4()
            ));
            std::fs::create_dir_all(&path).expect("create snapshot test directory");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestSnapshotDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    fn build_record(task_id: &str) -> TaskRecord {
        TaskRecord::new(
            task_id.to_string(),
            "chapter_generate".to_string(),
            "user-1".to_string(),
            "project-1".to_string(),
            "interactive".to_string(),
        )
    }

    async fn registry_with(task_ids: &[&str]) -> TaskRegistry {
        let registry = TaskRegistry::new();
        for task_id in task_ids {
            registry.insert(build_record(task_id)).await;
        }
        registry
    }

    async fn snapshot_task_ids(path: &Path) -> Vec<String> {
        let snapshot = read_snapshot(path)
            .await
            .expect("read snapshot")
            .expect("snapshot exists");
        let mut task_ids = snapshot
            .items
            .into_iter()
            .map(|record| record.task_id)
            .collect::<Vec<_>>();
        task_ids.sort();
        task_ids
    }

    async fn corrupt_files(dir: &Path) -> Vec<PathBuf> {
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .expect("read snapshot directory");
        let mut corrupt = Vec::new();
        while let Some(entry) = entries.next_entry().await.expect("read entry") {
            if entry.file_name().to_string_lossy().contains(".corrupt-") {
                corrupt.push(entry.path());
            }
        }
        corrupt
    }

    #[tokio::test]
    async fn first_save_commits_parseable_primary_snapshot() {
        let dir = TestSnapshotDir::new();
        let registry = registry_with(&["task-1"]).await;
        let paths = SnapshotPaths::new(dir.path());

        save_to_dir(&registry, dir.path())
            .await
            .expect("save first snapshot");

        assert_eq!(snapshot_task_ids(&paths.primary).await, vec!["task-1"]);
        assert!(!paths.temporary.exists());
        assert!(!paths.backup.exists());
    }

    #[tokio::test]
    async fn second_save_keeps_previous_primary_as_backup() {
        let dir = TestSnapshotDir::new();
        let registry = registry_with(&["task-1"]).await;
        let paths = SnapshotPaths::new(dir.path());
        save_to_dir(&registry, dir.path())
            .await
            .expect("save first snapshot");

        registry.insert(build_record("task-2")).await;
        save_to_dir(&registry, dir.path())
            .await
            .expect("save second snapshot");

        assert_eq!(
            snapshot_task_ids(&paths.primary).await,
            vec!["task-1", "task-2"]
        );
        assert_eq!(snapshot_task_ids(&paths.backup).await, vec!["task-1"]);
        assert!(!paths.temporary.exists());
    }

    #[tokio::test]
    async fn corrupted_primary_is_quarantined_and_backup_is_loaded() {
        let dir = TestSnapshotDir::new();
        let registry = registry_with(&["task-1"]).await;
        let paths = SnapshotPaths::new(dir.path());
        save_to_dir(&registry, dir.path())
            .await
            .expect("save first snapshot");
        registry.insert(build_record("task-2")).await;
        save_to_dir(&registry, dir.path())
            .await
            .expect("save second snapshot");
        tokio::fs::write(&paths.primary, b"{partial")
            .await
            .expect("corrupt primary");

        let restored = TaskRegistry::new();
        let outcome = load_from_dir(&restored, dir.path()).await;

        assert_eq!(
            outcome,
            LoadOutcome::Loaded {
                source: SnapshotSource::Backup,
                item_count: 1
            }
        );
        assert!(restored.get("task-1").await.is_some());
        assert!(restored.get("task-2").await.is_none());
        assert!(!paths.primary.exists());
        assert_eq!(corrupt_files(dir.path()).await.len(), 1);
    }

    #[tokio::test]
    async fn missing_primary_falls_back_to_backup() {
        let dir = TestSnapshotDir::new();
        let registry = registry_with(&["task-1"]).await;
        let paths = SnapshotPaths::new(dir.path());
        save_to_dir(&registry, dir.path())
            .await
            .expect("save first snapshot");
        registry.insert(build_record("task-2")).await;
        save_to_dir(&registry, dir.path())
            .await
            .expect("save second snapshot");
        tokio::fs::remove_file(&paths.primary)
            .await
            .expect("remove primary");

        let restored = TaskRegistry::new();
        let outcome = load_from_dir(&restored, dir.path()).await;

        assert_eq!(
            outcome,
            LoadOutcome::Loaded {
                source: SnapshotSource::Backup,
                item_count: 1
            }
        );
        assert!(restored.get("task-1").await.is_some());
    }

    #[tokio::test]
    async fn complete_temporary_snapshot_is_last_resort_fallback() {
        let dir = TestSnapshotDir::new();
        let registry = registry_with(&["task-1"]).await;
        let paths = SnapshotPaths::new(dir.path());
        save_to_dir(&registry, dir.path())
            .await
            .expect("save first snapshot");
        tokio::fs::rename(&paths.primary, &paths.temporary)
            .await
            .expect("move primary to temporary");

        let restored = TaskRegistry::new();
        let outcome = load_from_dir(&restored, dir.path()).await;

        assert_eq!(
            outcome,
            LoadOutcome::Loaded {
                source: SnapshotSource::Temporary,
                item_count: 1
            }
        );
        assert!(restored.get("task-1").await.is_some());
    }

    #[tokio::test]
    async fn unsupported_version_is_quarantined_before_backup_fallback() {
        let dir = TestSnapshotDir::new();
        let registry = registry_with(&["task-1"]).await;
        let paths = SnapshotPaths::new(dir.path());
        save_to_dir(&registry, dir.path())
            .await
            .expect("save first snapshot");
        tokio::fs::rename(&paths.primary, &paths.backup)
            .await
            .expect("create backup");
        tokio::fs::write(
            &paths.primary,
            serde_json::to_vec(&json!({
                "version": 999,
                "updated_at": "2026-07-12T00:00:00Z",
                "items": []
            }))
            .expect("serialize unsupported snapshot"),
        )
        .await
        .expect("write unsupported primary");

        let restored = TaskRegistry::new();
        let outcome = load_from_dir(&restored, dir.path()).await;

        assert_eq!(
            outcome,
            LoadOutcome::Loaded {
                source: SnapshotSource::Backup,
                item_count: 1
            }
        );
        assert_eq!(corrupt_files(dir.path()).await.len(), 1);
    }

    #[tokio::test]
    async fn temporary_open_failure_preserves_existing_primary() {
        let dir = TestSnapshotDir::new();
        let registry = registry_with(&["task-1"]).await;
        let paths = SnapshotPaths::new(dir.path());
        save_to_dir(&registry, dir.path())
            .await
            .expect("save first snapshot");
        tokio::fs::create_dir(&paths.temporary)
            .await
            .expect("block temporary file with directory");
        registry.insert(build_record("task-2")).await;

        let error = save_to_dir(&registry, dir.path())
            .await
            .expect_err("temporary open must fail");

        assert!(error.to_string().contains("open temporary"));
        assert_eq!(snapshot_task_ids(&paths.primary).await, vec!["task-1"]);
    }

    #[tokio::test]
    async fn concurrent_saves_leave_primary_and_backup_parseable() {
        let dir = TestSnapshotDir::new();
        let paths = SnapshotPaths::new(dir.path());
        let mut handles = Vec::new();

        for index in 0..8 {
            let registry = registry_with(&[&format!("task-{index}")]).await;
            let save_dir = dir.path().to_path_buf();
            handles.push(tokio::spawn(async move {
                save_to_dir(&registry, &save_dir).await
            }));
        }

        for handle in handles {
            handle
                .await
                .expect("join concurrent save")
                .expect("concurrent save succeeds");
        }

        assert_eq!(snapshot_task_ids(&paths.primary).await.len(), 1);
        assert_eq!(snapshot_task_ids(&paths.backup).await.len(), 1);
        assert!(!paths.temporary.exists());
        assert!(corrupt_files(dir.path()).await.is_empty());
    }

    #[test]
    fn production_snapshot_file_name_remains_backward_compatible() {
        assert_eq!(SNAPSHOT_FILE, "background_tasks.json");
    }
}
