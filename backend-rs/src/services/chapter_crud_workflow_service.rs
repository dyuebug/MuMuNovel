use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::services::chapter_crud_payload_adapter_service::{
    compatible_chapter_list_payload, compatible_chapter_payload,
    project_path_chapter_list_payload,
};
use crate::services::chapter_service::ChapterService;

#[derive(Debug)]
pub enum CreateChapterPayloadError {
    ProjectNotFound,
    Internal(String),
}

#[derive(Debug)]
pub enum ListChaptersPayloadError {
    ProjectNotFound,
    Internal(String),
}

#[derive(Debug)]
pub enum ListChaptersByProjectPathPayloadError {
    ProjectNotFound,
    Internal(String),
}

#[derive(Debug)]
pub enum GetChapterPayloadError {
    ChapterNotFound,
    Internal(String),
}

#[derive(Debug)]
pub enum UpdateChapterPayloadError {
    ChapterNotFound,
    Internal(String),
}

#[derive(Debug)]
pub enum DeleteChapterPayloadError {
    ChapterNotFound,
    Internal(String),
}

#[derive(Debug)]
pub enum UpdateExpansionPlanPayloadError {
    ChapterNotFound,
    Internal(String),
}

pub async fn create_chapter_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    title: &str,
    chapter_number: i32,
    content: Option<&str>,
    summary: Option<&str>,
    outline_id: Option<&str>,
    sub_index: Option<i32>,
) -> Result<Value, CreateChapterPayloadError> {
    match ChapterService::create(
        db,
        project_id,
        user_id,
        title,
        chapter_number,
        content,
        summary,
        outline_id,
        sub_index,
    )
    .await
    {
        Ok(Some(chapter)) => Ok(compatible_chapter_payload(chapter)),
        Ok(None) => Err(CreateChapterPayloadError::ProjectNotFound),
        Err(error) => Err(CreateChapterPayloadError::Internal(error)),
    }
}

pub async fn list_chapters_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, ListChaptersPayloadError> {
    match ChapterService::list_by_project(db, project_id, user_id).await {
        Ok(Some(chapters)) => Ok(compatible_chapter_list_payload(&chapters)),
        Ok(None) => Err(ListChaptersPayloadError::ProjectNotFound),
        Err(error) => Err(ListChaptersPayloadError::Internal(error)),
    }
}

pub async fn list_chapters_by_project_path_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, ListChaptersByProjectPathPayloadError> {
    match ChapterService::list_by_project(db, project_id, user_id).await {
        Ok(Some(chapters)) => Ok(project_path_chapter_list_payload(&chapters)),
        Ok(None) => Err(ListChaptersByProjectPathPayloadError::ProjectNotFound),
        Err(error) => Err(ListChaptersByProjectPathPayloadError::Internal(error)),
    }
}

pub async fn get_chapter_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, GetChapterPayloadError> {
    match ChapterService::get(db, chapter_id, user_id).await {
        Ok(Some(chapter)) => Ok(compatible_chapter_payload(chapter)),
        Ok(None) => Err(GetChapterPayloadError::ChapterNotFound),
        Err(error) => Err(GetChapterPayloadError::Internal(error)),
    }
}

pub async fn update_chapter_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    title: Option<&str>,
    content: Option<&str>,
    summary: Option<&str>,
    status: Option<&str>,
    chapter_number: Option<i32>,
    expansion_plan: Option<&str>,
) -> Result<Value, UpdateChapterPayloadError> {
    match ChapterService::update(
        db,
        chapter_id,
        user_id,
        title,
        content,
        summary,
        status,
        chapter_number,
        expansion_plan,
    )
    .await
    {
        Ok(Some(chapter)) => Ok(compatible_chapter_payload(chapter)),
        Ok(None) => Err(UpdateChapterPayloadError::ChapterNotFound),
        Err(error) => Err(UpdateChapterPayloadError::Internal(error)),
    }
}

pub async fn delete_chapter_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, DeleteChapterPayloadError> {
    match ChapterService::delete(db, chapter_id, user_id).await {
        Ok(Some(())) => Ok(json!({
            "success": true,
            "message": "Chapter deleted successfully"
        })),
        Ok(None) => Err(DeleteChapterPayloadError::ChapterNotFound),
        Err(error) => Err(DeleteChapterPayloadError::Internal(error)),
    }
}

pub async fn update_expansion_plan_payload(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
    plan: &str,
) -> Result<Value, UpdateExpansionPlanPayloadError> {
    match ChapterService::update_expansion_plan(db, chapter_id, user_id, plan).await {
        Ok(Some(chapter)) => Ok(compatible_chapter_payload(chapter)),
        Ok(None) => Err(UpdateExpansionPlanPayloadError::ChapterNotFound),
        Err(error) => Err(UpdateExpansionPlanPayloadError::Internal(error)),
    }
}
