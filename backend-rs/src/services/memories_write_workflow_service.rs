use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::{json, Value};

use crate::models::story_memory;
use crate::services::chapter_analysis_runtime_service::analyze_chapter_now;
use crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError;
use crate::services::memories_query_service::LoadProjectAccessError;
use crate::services::project_access_query_service::ensure_owned_project_access;
use crate::services::story_memory_vector_index_service::delete_story_memory_vector_records_by_chapter;

#[derive(Debug)]
pub(crate) enum MemoriesProjectWriteContextError {
    ProjectAccess(LoadProjectAccessError),
    Internal(String),
}

#[derive(Debug)]
pub(crate) enum AnalyzeChapterMemoriesWriteWorkflowError {
    Context(MemoriesProjectWriteContextError),
    CreateTask(CreateChapterAnalysisTaskError),
}

pub(crate) type DeleteChapterMemoriesWriteWorkflowError = MemoriesProjectWriteContextError;

pub(crate) async fn analyze_chapter_memories_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, AnalyzeChapterMemoriesWriteWorkflowError> {
    ensure_owned_project_access(db, project_id, user_id)
        .await
        .map_err(MemoriesProjectWriteContextError::ProjectAccess)
        .map_err(AnalyzeChapterMemoriesWriteWorkflowError::Context)?;

    analyze_chapter_now(db, user_id, chapter_id)
        .await
        .map_err(AnalyzeChapterMemoriesWriteWorkflowError::CreateTask)
}

pub(crate) async fn delete_chapter_memories_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, DeleteChapterMemoriesWriteWorkflowError> {
    ensure_owned_project_access(db, project_id, user_id)
        .await
        .map_err(MemoriesProjectWriteContextError::ProjectAccess)?;

    let result = story_memory::Entity::delete_many()
        .filter(story_memory::Column::ProjectId.eq(project_id))
        .filter(story_memory::Column::ChapterId.eq(chapter_id))
        .exec(db)
        .await
        .map_err(|error| MemoriesProjectWriteContextError::Internal(error.to_string()))?;
    delete_story_memory_vector_records_by_chapter(project_id, chapter_id)
        .await
        .map_err(MemoriesProjectWriteContextError::Internal)?;

    Ok(json!({
        "success": true,
        "message": format!("已删除{}条记忆", result.rows_affected),
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        AnalyzeChapterMemoriesWriteWorkflowError, DeleteChapterMemoriesWriteWorkflowError,
        MemoriesProjectWriteContextError,
    };
    use crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError;
    use crate::services::memories_query_service::LoadProjectAccessError;

    #[test]
    fn should_keep_analyze_chapter_memories_write_workflow_error_shape() {
        let project_access = AnalyzeChapterMemoriesWriteWorkflowError::Context(
            MemoriesProjectWriteContextError::ProjectAccess(
                LoadProjectAccessError::NotFoundOrAccessDenied,
            ),
        );
        let create_task = AnalyzeChapterMemoriesWriteWorkflowError::CreateTask(
            CreateChapterAnalysisTaskError::ChapterEmpty,
        );

        assert!(matches!(
            project_access,
            AnalyzeChapterMemoriesWriteWorkflowError::Context(
                MemoriesProjectWriteContextError::ProjectAccess(
                    LoadProjectAccessError::NotFoundOrAccessDenied
                )
            )
        ));
        assert!(matches!(
            create_task,
            AnalyzeChapterMemoriesWriteWorkflowError::CreateTask(
                CreateChapterAnalysisTaskError::ChapterEmpty
            )
        ));
    }

    #[test]
    fn should_keep_delete_chapter_memories_write_workflow_error_shape() {
        let project_access = DeleteChapterMemoriesWriteWorkflowError::ProjectAccess(
            LoadProjectAccessError::NotFoundOrAccessDenied,
        );
        let internal = DeleteChapterMemoriesWriteWorkflowError::Internal("db exploded".to_string());

        assert!(matches!(
            project_access,
            DeleteChapterMemoriesWriteWorkflowError::ProjectAccess(
                LoadProjectAccessError::NotFoundOrAccessDenied
            )
        ));
        assert!(matches!(
            internal,
            DeleteChapterMemoriesWriteWorkflowError::Internal(detail) if detail == "db exploded"
        ));
    }

    #[test]
    fn should_keep_memories_project_write_context_error_shape() {
        let project_access = MemoriesProjectWriteContextError::ProjectAccess(
            LoadProjectAccessError::NotFoundOrAccessDenied,
        );
        let internal = MemoriesProjectWriteContextError::Internal("db exploded".to_string());

        assert!(matches!(
            project_access,
            MemoriesProjectWriteContextError::ProjectAccess(
                LoadProjectAccessError::NotFoundOrAccessDenied
            )
        ));
        assert!(matches!(
            internal,
            MemoriesProjectWriteContextError::Internal(detail) if detail == "db exploded"
        ));
    }
}
