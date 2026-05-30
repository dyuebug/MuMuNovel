use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::models::{chapter_draft_attempt, generation_history};

pub struct ChapterAnalysisReadContext {
    pub candidate_attempt: Option<chapter_draft_attempt::Model>,
    pub histories: Vec<generation_history::Model>,
}

pub async fn load_chapter_analysis_read_context(
    db: &DatabaseConnection,
    chapter_id: &str,
) -> Result<ChapterAnalysisReadContext, String> {
    let chapter_id = chapter_id.to_string();

    let candidate_attempt = chapter_draft_attempt::Entity::find()
        .filter(chapter_draft_attempt::Column::ChapterId.eq(Some(chapter_id.clone())))
        .order_by_desc(chapter_draft_attempt::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| error.to_string())?;

    let histories: Vec<generation_history::Model> = generation_history::Entity::find()
        .filter(generation_history::Column::ChapterId.eq(Some(chapter_id)))
        .order_by_desc(generation_history::Column::CreatedAt)
        .limit(30)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    Ok(ChapterAnalysisReadContext {
        candidate_attempt,
        histories,
    })
}
