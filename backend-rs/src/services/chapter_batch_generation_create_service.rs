use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};

use crate::models::chapter;

#[derive(Debug)]
pub(crate) enum PrepareBatchGenerationCreateRequestError {
    InvalidCount,
    ChaptersNotFound,
    Internal(String),
}

#[derive(Debug)]
pub(crate) struct PreparedBatchGenerationCreateRequest {
    pub(crate) normalized_target_word_count: i32,
    pub(crate) chapters_to_generate: Vec<chapter::Model>,
}

fn validate_batch_generation_count(
    count: i32,
) -> Result<(), PrepareBatchGenerationCreateRequestError> {
    if count > 0 {
        Ok(())
    } else {
        Err(PrepareBatchGenerationCreateRequestError::InvalidCount)
    }
}

fn normalize_batch_generation_target_word_count(target_word_count: Option<i32>) -> i32 {
    target_word_count.unwrap_or(3000).max(1)
}

async fn load_chapters_for_batch_generation_range(
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

pub(crate) async fn prepare_batch_generation_create_request(
    db: &DatabaseConnection,
    project_id: &str,
    start_chapter_number: i32,
    count: i32,
    target_word_count: Option<i32>,
) -> Result<PreparedBatchGenerationCreateRequest, PrepareBatchGenerationCreateRequestError> {
    let chapters_to_generate = load_chapters_for_batch_generation_range(
        db,
        project_id,
        start_chapter_number,
        count,
    )
    .await?;

    Ok(PreparedBatchGenerationCreateRequest {
        normalized_target_word_count: normalize_batch_generation_target_word_count(target_word_count),
        chapters_to_generate,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_batch_generation_target_word_count, validate_batch_generation_count,
        PrepareBatchGenerationCreateRequestError,
    };

    #[test]
    fn should_normalize_batch_generation_target_word_count() {
        assert_eq!(normalize_batch_generation_target_word_count(None), 3000);
        assert_eq!(normalize_batch_generation_target_word_count(Some(-100)), 1);
        assert_eq!(normalize_batch_generation_target_word_count(Some(0)), 1);
        assert_eq!(
            normalize_batch_generation_target_word_count(Some(2500)),
            2500
        );
    }

    #[test]
    fn should_validate_batch_generation_count() {
        assert!(validate_batch_generation_count(1).is_ok());
        assert!(matches!(
            validate_batch_generation_count(0),
            Err(PrepareBatchGenerationCreateRequestError::InvalidCount)
        ));
    }
}
