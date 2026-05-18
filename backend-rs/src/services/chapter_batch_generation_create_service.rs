use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};

use crate::models::chapter;

#[derive(Debug)]
pub enum PrepareBatchGenerationCreateRequestError {
    InvalidCount,
    ChaptersNotFound,
    Internal(String),
}

#[derive(Debug, Clone)]
pub struct BatchGenerationCreateRequest {
    pub start_chapter_number: i32,
    pub count: i32,
    pub target_word_count: Option<i32>,
}

#[derive(Debug)]
pub struct PreparedBatchGenerationCreateRequest {
    pub end_chapter_number: i32,
    pub normalized_target_word_count: i32,
    pub chapters_to_generate: Vec<chapter::Model>,
}

pub fn validate_batch_generation_count(
    count: i32,
) -> Result<(), PrepareBatchGenerationCreateRequestError> {
    if count > 0 {
        Ok(())
    } else {
        Err(PrepareBatchGenerationCreateRequestError::InvalidCount)
    }
}

pub fn normalize_batch_generation_target_word_count(target_word_count: Option<i32>) -> i32 {
    target_word_count.unwrap_or(3000).max(1)
}

pub async fn load_chapters_for_batch_generation_range(
    db: &DatabaseConnection,
    project_id: &str,
    start_chapter_number: i32,
    count: i32,
) -> Result<Vec<chapter::Model>, PrepareBatchGenerationCreateRequestError> {
    validate_batch_generation_count(count)?;

    let end_chapter_number = start_chapter_number + count - 1;
    let chapters_to_generate = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .filter(chapter::Column::ChapterNumber.gte(start_chapter_number))
        .filter(chapter::Column::ChapterNumber.lte(end_chapter_number))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(db)
        .await
        .map_err(|error| PrepareBatchGenerationCreateRequestError::Internal(error.to_string()))?;

    if chapters_to_generate.is_empty() {
        return Err(PrepareBatchGenerationCreateRequestError::ChaptersNotFound);
    }

    Ok(chapters_to_generate)
}

pub async fn prepare_batch_generation_create_request(
    db: &DatabaseConnection,
    project_id: &str,
    request: &BatchGenerationCreateRequest,
) -> Result<PreparedBatchGenerationCreateRequest, PrepareBatchGenerationCreateRequestError> {
    validate_batch_generation_count(request.count)?;

    let end_chapter_number = request.start_chapter_number + request.count - 1;
    let chapters_to_generate = load_chapters_for_batch_generation_range(
        db,
        project_id,
        request.start_chapter_number,
        request.count,
    )
    .await?;

    Ok(PreparedBatchGenerationCreateRequest {
        end_chapter_number,
        normalized_target_word_count: normalize_batch_generation_target_word_count(
            request.target_word_count,
        ),
        chapters_to_generate,
    })
}
