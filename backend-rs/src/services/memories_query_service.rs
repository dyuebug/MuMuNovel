use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde_json::{json, Value};

use crate::models::{foreshadow, plot_analysis, story_memory};
use crate::services::project_access_query_service::{
    ensure_owned_project_access, ProjectAccessQueryError,
};
use crate::services::story_memory_vector_index_service::search_story_memory_vector_records;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryListRequest {
    memory_type: Option<String>,
    chapter_id: Option<String>,
    limit: u64,
}

impl MemoryListRequest {
    pub fn new(memory_type: Option<String>, chapter_id: Option<String>, limit: u64) -> Self {
        Self {
            memory_type,
            chapter_id,
            limit,
        }
    }

    pub fn memory_type(&self) -> Option<&str> {
        self.memory_type.as_deref()
    }

    pub fn chapter_id(&self) -> Option<&str> {
        self.chapter_id.as_deref()
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchMemoriesRequest {
    query: String,
    limit: u64,
    min_importance: f64,
    memory_types: Vec<String>,
}

impl SearchMemoriesRequest {
    pub fn new(query: String, limit: u64, min_importance: f64, memory_types: Vec<String>) -> Self {
        Self {
            query,
            limit,
            min_importance,
            memory_types,
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    pub fn min_importance(&self) -> f64 {
        self.min_importance
    }

    pub fn memory_types(&self) -> &[String] {
        &self.memory_types
    }
}

pub type LoadProjectAccessError = ProjectAccessQueryError;

pub enum MemoriesProjectQueryContextError {
    ProjectAccess(LoadProjectAccessError),
    Internal(String),
}

pub type OwnedProjectMemoriesQueryError = MemoriesProjectQueryContextError;
pub type LoadProjectMemoriesPayloadError = OwnedProjectMemoriesQueryError;

pub enum LoadProjectChapterAnalysisPayloadError {
    Context(MemoriesProjectQueryContextError),
    AnalysisNotFound,
}

pub type SearchProjectMemoriesPayloadError = OwnedProjectMemoriesQueryError;
pub type LoadUnresolvedForeshadowsPayloadError = OwnedProjectMemoriesQueryError;
pub type LoadMemoryStatsPayloadError = OwnedProjectMemoriesQueryError;

pub async fn load_owned_project_memories_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    request: MemoryListRequest,
) -> Result<Value, LoadProjectMemoriesPayloadError> {
    ensure_owned_project_access(db, project_id, user_id)
        .await
        .map_err(OwnedProjectMemoriesQueryError::ProjectAccess)?;

    let mut stmt =
        story_memory::Entity::find().filter(story_memory::Column::ProjectId.eq(project_id));
    if let Some(memory_type) = request.memory_type() {
        stmt = stmt.filter(story_memory::Column::MemoryType.eq(memory_type));
    }
    if let Some(chapter_id) = request.chapter_id() {
        stmt = stmt.filter(story_memory::Column::ChapterId.eq(chapter_id));
    }

    let memories = stmt
        .order_by_desc(story_memory::Column::ImportanceScore)
        .order_by_desc(story_memory::Column::CreatedAt)
        .limit(request.limit())
        .all(db)
        .await
        .map_err(|error| OwnedProjectMemoriesQueryError::Internal(error.to_string()))?;

    Ok(json!({
        "success": true,
        "memories": memories,
        "total": memories.len(),
    }))
}

pub async fn load_owned_project_chapter_analysis_payload(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, LoadProjectChapterAnalysisPayloadError> {
    ensure_owned_project_access(db, project_id, user_id)
        .await
        .map_err(MemoriesProjectQueryContextError::ProjectAccess)
        .map_err(LoadProjectChapterAnalysisPayloadError::Context)?;

    let analysis = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ProjectId.eq(project_id))
        .filter(plot_analysis::Column::ChapterId.eq(chapter_id))
        .one(db)
        .await
        .map_err(|error| {
            LoadProjectChapterAnalysisPayloadError::Context(
                MemoriesProjectQueryContextError::Internal(error.to_string()),
            )
        })?
        .ok_or(LoadProjectChapterAnalysisPayloadError::AnalysisNotFound)?;

    Ok(json!({
        "success": true,
        "analysis": analysis,
    }))
}

pub async fn search_owned_project_memories_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    request: SearchMemoriesRequest,
) -> Result<Value, SearchProjectMemoriesPayloadError> {
    ensure_owned_project_access(db, project_id, user_id)
        .await
        .map_err(OwnedProjectMemoriesQueryError::ProjectAccess)?;

    if !request.query().is_empty() {
        let vector_hits = search_story_memory_vector_records(
            db,
            user_id,
            project_id,
            request.query(),
            request.memory_types(),
            request.min_importance(),
            request.limit() as usize,
        )
        .await
        .map_err(OwnedProjectMemoriesQueryError::Internal)?;

        if !vector_hits.is_empty() {
            let hit_ids = vector_hits
                .iter()
                .map(|item| item.memory_id.clone())
                .collect::<Vec<_>>();
            let memories = story_memory::Entity::find()
                .filter(story_memory::Column::ProjectId.eq(project_id))
                .filter(story_memory::Column::Id.is_in(hit_ids.clone()))
                .all(db)
                .await
                .map_err(|error| OwnedProjectMemoriesQueryError::Internal(error.to_string()))?;
            let mut memory_by_id = memories
                .into_iter()
                .map(|item| (item.id.clone(), item))
                .collect::<std::collections::HashMap<_, _>>();
            let ordered = hit_ids
                .iter()
                .filter_map(|memory_id| memory_by_id.remove(memory_id))
                .collect::<Vec<_>>();

            return Ok(json!({
                "success": true,
                "query": request.query(),
                "memories": ordered,
                "total": ordered.len(),
                "search_mode": "vector",
            }));
        }
    }

    let mut stmt = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(project_id))
        .filter(story_memory::Column::ImportanceScore.gte(request.min_importance()));
    if !request.query().is_empty() {
        let pattern = format!("%{}%", request.query());
        stmt = stmt.filter(
            story_memory::Column::Title
                .like(&pattern)
                .or(story_memory::Column::Content.like(&pattern)),
        );
    }
    if !request.memory_types().is_empty() {
        stmt = stmt.filter(story_memory::Column::MemoryType.is_in(request.memory_types().to_vec()));
    }

    let memories = stmt
        .order_by_desc(story_memory::Column::ImportanceScore)
        .order_by_desc(story_memory::Column::CreatedAt)
        .limit(request.limit())
        .all(db)
        .await
        .map_err(|error| OwnedProjectMemoriesQueryError::Internal(error.to_string()))?;

    Ok(json!({
        "success": true,
        "query": request.query(),
        "memories": memories,
        "total": memories.len(),
        "search_mode": "sql_fallback",
    }))
}

pub async fn load_owned_unresolved_foreshadows_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    current_chapter: Option<i32>,
) -> Result<Value, LoadUnresolvedForeshadowsPayloadError> {
    ensure_owned_project_access(db, project_id, user_id)
        .await
        .map_err(OwnedProjectMemoriesQueryError::ProjectAccess)?;

    let mut stmt = foreshadow::Entity::find()
        .filter(foreshadow::Column::ProjectId.eq(project_id))
        .filter(foreshadow::Column::Status.ne("resolved"))
        .filter(foreshadow::Column::Status.ne("abandoned"));
    if let Some(current_chapter) = current_chapter {
        stmt = stmt.filter(
            foreshadow::Column::PlantChapterNumber
                .is_null()
                .or(foreshadow::Column::PlantChapterNumber.lte(current_chapter)),
        );
    }

    let foreshadows = stmt
        .order_by_desc(foreshadow::Column::Importance)
        .order_by_desc(foreshadow::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|error| OwnedProjectMemoriesQueryError::Internal(error.to_string()))?;

    Ok(json!({
        "success": true,
        "foreshadows": foreshadows,
        "total": foreshadows.len(),
    }))
}

pub async fn load_owned_memory_stats_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, LoadMemoryStatsPayloadError> {
    ensure_owned_project_access(db, project_id, user_id)
        .await
        .map_err(OwnedProjectMemoriesQueryError::ProjectAccess)?;

    let total = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(project_id))
        .count(db)
        .await
        .map_err(|error| OwnedProjectMemoriesQueryError::Internal(error.to_string()))?;
    let foreshadow_count = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(project_id))
        .filter(story_memory::Column::IsForeshadow.eq(1))
        .count(db)
        .await
        .map_err(|error| OwnedProjectMemoriesQueryError::Internal(error.to_string()))?;
    let plot_analysis_count = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ProjectId.eq(project_id))
        .count(db)
        .await
        .map_err(|error| OwnedProjectMemoriesQueryError::Internal(error.to_string()))?;

    Ok(json!({
        "success": true,
        "stats": {
            "total_memories": total,
            "foreshadows": foreshadow_count,
            "plot_analyses": plot_analysis_count,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        LoadMemoryStatsPayloadError, LoadProjectAccessError,
        LoadProjectChapterAnalysisPayloadError, LoadProjectMemoriesPayloadError,
        LoadUnresolvedForeshadowsPayloadError, MemoriesProjectQueryContextError,
        OwnedProjectMemoriesQueryError, SearchProjectMemoriesPayloadError,
    };

    #[test]
    fn shared_owned_project_memories_query_error_aliases_keep_same_outer_owner() {
        let list_error: LoadProjectMemoriesPayloadError =
            OwnedProjectMemoriesQueryError::ProjectAccess(
                LoadProjectAccessError::NotFoundOrAccessDenied,
            );
        let search_error: SearchProjectMemoriesPayloadError =
            OwnedProjectMemoriesQueryError::Internal("search failed".to_string());
        let foreshadow_error: LoadUnresolvedForeshadowsPayloadError =
            OwnedProjectMemoriesQueryError::Internal("foreshadow failed".to_string());
        let stats_error: LoadMemoryStatsPayloadError =
            OwnedProjectMemoriesQueryError::ProjectAccess(LoadProjectAccessError::Internal(
                "db exploded".to_string(),
            ));

        assert!(matches!(
            list_error,
            OwnedProjectMemoriesQueryError::ProjectAccess(
                LoadProjectAccessError::NotFoundOrAccessDenied
            )
        ));
        assert!(matches!(
            search_error,
            OwnedProjectMemoriesQueryError::Internal(detail) if detail == "search failed"
        ));
        assert!(matches!(
            foreshadow_error,
            OwnedProjectMemoriesQueryError::Internal(detail) if detail == "foreshadow failed"
        ));
        assert!(matches!(
            stats_error,
            OwnedProjectMemoriesQueryError::ProjectAccess(
                LoadProjectAccessError::Internal(detail)
            ) if detail == "db exploded"
        ));
    }

    #[test]
    fn chapter_analysis_query_error_keeps_extra_analysis_not_found_branch() {
        let error = LoadProjectChapterAnalysisPayloadError::AnalysisNotFound;

        assert!(matches!(
            error,
            LoadProjectChapterAnalysisPayloadError::AnalysisNotFound
        ));
    }

    #[test]
    fn chapter_analysis_query_error_wraps_shared_context_owner() {
        let project_access = LoadProjectChapterAnalysisPayloadError::Context(
            MemoriesProjectQueryContextError::ProjectAccess(
                LoadProjectAccessError::NotFoundOrAccessDenied,
            ),
        );
        let internal = LoadProjectChapterAnalysisPayloadError::Context(
            MemoriesProjectQueryContextError::Internal("db exploded".to_string()),
        );

        assert!(matches!(
            project_access,
            LoadProjectChapterAnalysisPayloadError::Context(
                MemoriesProjectQueryContextError::ProjectAccess(
                    LoadProjectAccessError::NotFoundOrAccessDenied
                )
            )
        ));
        assert!(matches!(
            internal,
            LoadProjectChapterAnalysisPayloadError::Context(
                MemoriesProjectQueryContextError::Internal(detail)
            ) if detail == "db exploded"
        ));
    }
}
