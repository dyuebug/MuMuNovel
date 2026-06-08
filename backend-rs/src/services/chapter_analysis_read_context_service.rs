use sea_orm::DatabaseConnection;

use crate::models::{chapter_draft_attempt, generation_history};
use crate::services::chapter_draft_history_service::load_recent_generation_histories;
use crate::services::chapter_draft_source_service::load_candidate_draft_attempt;

pub struct ChapterAnalysisReadContext {
    pub candidate_attempt: Option<chapter_draft_attempt::Model>,
    pub histories: Vec<generation_history::Model>,
}

pub async fn load_chapter_analysis_read_context(
    db: &DatabaseConnection,
    chapter_id: &str,
) -> Result<ChapterAnalysisReadContext, String> {
    let candidate_attempt = load_candidate_draft_attempt(db, chapter_id, None)
        .await
        .map_err(|error| error.to_string())?;

    let histories = load_recent_generation_histories(db, chapter_id, 30)
        .await
        .map_err(|error| error.to_string())?;

    Ok(ChapterAnalysisReadContext {
        candidate_attempt,
        histories,
    })
}
