use sea_orm::{DatabaseConnection, EntityTrait};

use crate::models::{batch_generation_snapshot, batch_generation_task};
use crate::services::chapter_generation_snapshot_service::load_chapter_generation_snapshot;
use crate::services::chapter_generation_task_recovery_service::recover_generation_task_if_needed;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadOwnedBatchGenerationTaskError {
    TaskNotFound,
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadOwnedBatchGenerationTaskSourcesError {
    Task(LoadOwnedBatchGenerationTaskError),
    Snapshot(String),
}

#[derive(Debug, Clone)]
pub(crate) struct OwnedBatchGenerationTaskSources {
    task: batch_generation_task::Model,
    snapshot: Option<batch_generation_snapshot::Model>,
}

#[derive(Debug, Clone)]
pub(crate) struct OwnedBatchGenerationTaskReadState {
    task: batch_generation_task::Model,
    snapshot: Option<batch_generation_snapshot::Model>,
}

pub(crate) async fn load_owned_task(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Option<batch_generation_task::Model>, String> {
    let task = batch_generation_task::Entity::find_by_id(batch_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?;
    Ok(task.filter(|task| task.user_id == user_id))
}

impl OwnedBatchGenerationTaskSources {
    pub(crate) fn into_parts(
        self,
    ) -> (
        batch_generation_task::Model,
        Option<batch_generation_snapshot::Model>,
    ) {
        (self.task, self.snapshot)
    }

    #[cfg(test)]
    pub(crate) fn from_parts(
        task: batch_generation_task::Model,
        snapshot: Option<batch_generation_snapshot::Model>,
    ) -> Self {
        Self { task, snapshot }
    }

    #[cfg(test)]
    pub(crate) fn task(&self) -> &batch_generation_task::Model {
        &self.task
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Option<&batch_generation_snapshot::Model> {
        self.snapshot.as_ref()
    }
}

impl OwnedBatchGenerationTaskReadState {
    pub(crate) fn into_parts(
        self,
    ) -> (
        batch_generation_task::Model,
        Option<batch_generation_snapshot::Model>,
    ) {
        (self.task, self.snapshot)
    }

    #[cfg(test)]
    pub(crate) fn from_parts(
        task: batch_generation_task::Model,
        snapshot: Option<batch_generation_snapshot::Model>,
    ) -> Self {
        Self { task, snapshot }
    }

    #[cfg(test)]
    pub(crate) fn task(&self) -> &batch_generation_task::Model {
        &self.task
    }

    #[cfg(test)]
    pub(crate) fn snapshot(&self) -> Option<&batch_generation_snapshot::Model> {
        self.snapshot.as_ref()
    }
}

pub(crate) async fn load_owned_batch_generation_task_sources(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<OwnedBatchGenerationTaskSources, LoadOwnedBatchGenerationTaskSourcesError> {
    let task = load_owned_task(db, batch_id, user_id)
        .await
        .map_err(|error| {
            LoadOwnedBatchGenerationTaskSourcesError::Task(
                LoadOwnedBatchGenerationTaskError::Internal(error),
            )
        })?
        .ok_or(LoadOwnedBatchGenerationTaskSourcesError::Task(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
        ))?;
    let snapshot = load_chapter_generation_snapshot(db, &task.id)
        .await
        .map_err(LoadOwnedBatchGenerationTaskSourcesError::Snapshot)?;

    Ok(OwnedBatchGenerationTaskSources { task, snapshot })
}

pub(crate) async fn load_owned_batch_generation_task_read_state(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<OwnedBatchGenerationTaskReadState, LoadOwnedBatchGenerationTaskError> {
    let (task, snapshot) =
        match load_owned_batch_generation_task_sources(db, batch_id, user_id).await {
            Ok(sources) => sources.into_parts(),
            Err(LoadOwnedBatchGenerationTaskSourcesError::Task(error)) => return Err(error),
            Err(LoadOwnedBatchGenerationTaskSourcesError::Snapshot(error)) => {
                return Err(LoadOwnedBatchGenerationTaskError::Internal(error))
            }
        };
    let (task, _) = recover_generation_task_if_needed(db, task)
        .await
        .map_err(LoadOwnedBatchGenerationTaskError::Internal)?;

    Ok(OwnedBatchGenerationTaskReadState { task, snapshot })
}

#[cfg(test)]
mod tests {
    use super::{
        LoadOwnedBatchGenerationTaskError, LoadOwnedBatchGenerationTaskSourcesError,
        OwnedBatchGenerationTaskReadState, OwnedBatchGenerationTaskSources,
    };
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use serde_json::json;

    fn build_task() -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-owned-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", "chapter-2"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: "running".to_string(),
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

    fn build_snapshot() -> batch_generation_snapshot::Model {
        batch_generation_snapshot::Model {
            id: "snapshot-owned-1".to_string(),
            batch_task_id: "task-owned-1".to_string(),
            latest_quality_metrics: None,
            quality_metrics_history: None,
            quality_metrics_summary: None,
            workflow_runtime_state: Some(json!({"progress": 55})),
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn should_keep_owned_task_read_state_owner_contract() {
        let state =
            OwnedBatchGenerationTaskReadState::from_parts(build_task(), Some(build_snapshot()));

        assert_eq!(state.task().id, "task-owned-1");
        assert_eq!(
            state
                .snapshot()
                .and_then(|snapshot| snapshot.workflow_runtime_state.as_ref())
                .and_then(|state| state.get("progress"))
                .and_then(|value| value.as_i64()),
            Some(55)
        );
    }

    #[test]
    fn should_keep_owned_task_sources_owner_contract() {
        let sources =
            OwnedBatchGenerationTaskSources::from_parts(build_task(), Some(build_snapshot()));

        assert_eq!(sources.task().id, "task-owned-1");
        assert_eq!(
            sources
                .snapshot()
                .and_then(|snapshot| snapshot.workflow_runtime_state.as_ref())
                .and_then(|state| state.get("progress"))
                .and_then(|value| value.as_i64()),
            Some(55)
        );
    }

    #[test]
    fn should_keep_owned_task_read_state_error_contract() {
        let missing = LoadOwnedBatchGenerationTaskError::TaskNotFound;
        let internal = LoadOwnedBatchGenerationTaskError::Internal("boom".to_string());

        assert_eq!(missing, LoadOwnedBatchGenerationTaskError::TaskNotFound);
        assert_eq!(
            internal,
            LoadOwnedBatchGenerationTaskError::Internal("boom".to_string())
        );
    }

    #[test]
    fn should_keep_owned_task_sources_error_contract() {
        let missing = LoadOwnedBatchGenerationTaskSourcesError::Task(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
        );
        let snapshot = LoadOwnedBatchGenerationTaskSourcesError::Snapshot("boom".to_string());

        assert_eq!(
            missing,
            LoadOwnedBatchGenerationTaskSourcesError::Task(
                LoadOwnedBatchGenerationTaskError::TaskNotFound
            )
        );
        assert_eq!(
            snapshot,
            LoadOwnedBatchGenerationTaskSourcesError::Snapshot("boom".to_string())
        );
    }
}
