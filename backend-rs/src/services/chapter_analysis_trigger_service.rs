use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::models::chapter;

use super::chapter_access_service::{load_accessible_chapter, LoadAccessibleChapterError};
use super::chapter_analysis_runtime_service::{
    create_chapter_analysis_task, execute_chapter_analysis_background,
};
use super::chapter_analysis_service::CreateChapterAnalysisTaskError;

#[derive(Debug)]
pub enum PrepareChapterAnalysisTriggerError {
    ChapterNotFoundOrAccessDenied,
    ChapterEmpty,
    ProjectMissing,
    Internal(String),
}

pub struct PreparedChapterAnalysisTrigger {
    pub task_id: String,
    pub chapter_id: String,
    pub chapter: chapter::Model,
    pub payload: Value,
}

pub async fn prepare_chapter_analysis_trigger(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<PreparedChapterAnalysisTrigger, PrepareChapterAnalysisTriggerError> {
    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(|error| match error {
            LoadAccessibleChapterError::NotFoundOrAccessDenied => {
                PrepareChapterAnalysisTriggerError::ChapterNotFoundOrAccessDenied
            }
            LoadAccessibleChapterError::Internal(detail) => {
                PrepareChapterAnalysisTriggerError::Internal(detail)
            }
        })?;

    build_prepared_chapter_analysis_trigger(db, chapter_id, user_id, chapter).await
}

pub fn dispatch_prepared_chapter_analysis_trigger(
    db: DatabaseConnection,
    user_id: String,
    prepared: &PreparedChapterAnalysisTrigger,
) {
    let chapter_id = prepared.chapter_id.clone();
    let task_id = prepared.task_id.clone();
    tokio::spawn(async move {
        execute_chapter_analysis_background(db, user_id, chapter_id, task_id).await;
    });
}

async fn build_prepared_chapter_analysis_trigger(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    chapter: chapter::Model,
) -> Result<PreparedChapterAnalysisTrigger, PrepareChapterAnalysisTriggerError> {
    let task = create_chapter_analysis_task(db, user_id, &chapter)
        .await
        .map_err(|error| match error {
            CreateChapterAnalysisTaskError::ChapterEmpty => {
                PrepareChapterAnalysisTriggerError::ChapterEmpty
            }
            CreateChapterAnalysisTaskError::ProjectMissing => {
                PrepareChapterAnalysisTriggerError::ProjectMissing
            }
            CreateChapterAnalysisTaskError::Internal(detail) => {
                PrepareChapterAnalysisTriggerError::Internal(detail)
            }
        })?;

    let task_id = task.id;
    Ok(PreparedChapterAnalysisTrigger {
        task_id: task_id.clone(),
        chapter_id: chapter_id.to_string(),
        chapter,
        payload: json!({
            "task_id": task_id,
            "chapter_id": chapter_id,
            "status": "pending",
            "message": "章节分析任务已创建",
        }),
    })
}
