use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::{json, Value};

use self::error_mapper::{
    map_analyze_chapter_memories_write_workflow_error, map_memories_project_write_context_error,
    map_owned_project_memories_query_error, map_project_chapter_analysis_payload_error,
};
use crate::models::{foreshadow, plot_analysis, story_memory};
use crate::services::auth::Claims;
use crate::services::chapter_analysis_runtime_service::analyze_chapter_now;
use crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError;
use crate::services::project_service::{ProjectAccessQueryError, ProjectService};
use crate::services::story_memory_vector_index_service::{
    delete_story_memory_vector_records_by_chapter, search_story_memory_vector_records,
};

const MEMORIES_ANALYZE_CHAPTER_ROUTE: &str =
    "/memories/projects/{project_id}/analyze-chapter/{chapter_id}";
const MEMORIES_LIST_ROUTE: &str = "/memories/projects/{project_id}/memories";
const MEMORIES_CHAPTER_ANALYSIS_ROUTE: &str =
    "/memories/projects/{project_id}/analysis/{chapter_id}";
const MEMORIES_SEARCH_ROUTE: &str = "/memories/projects/{project_id}/search";
const MEMORIES_FORESHADOWS_ROUTE: &str = "/memories/projects/{project_id}/foreshadows";
const MEMORIES_STATS_ROUTE: &str = "/memories/projects/{project_id}/stats";
const MEMORIES_DELETE_CHAPTER_ROUTE: &str =
    "/memories/projects/{project_id}/chapters/{chapter_id}/memories";

#[cfg(test)]
fn build_memories_route_owner_contract() -> Value {
    json!({
        "owner": "memories",
        "rust_owner": "backend-rs/src/api/memories.rs",
        "route_prefix": "/api",
        "routes": {
            "analyze_chapter": MEMORIES_ANALYZE_CHAPTER_ROUTE,
            "list": MEMORIES_LIST_ROUTE,
            "chapter_analysis": MEMORIES_CHAPTER_ANALYSIS_ROUTE,
            "search": MEMORIES_SEARCH_ROUTE,
            "foreshadows": MEMORIES_FORESHADOWS_ROUTE,
            "stats": MEMORIES_STATS_ROUTE,
            "delete_chapter": MEMORIES_DELETE_CHAPTER_ROUTE
        },
        "service_handoffs": {
            "query_owner": "backend-rs/src/api/memories.rs",
            "write_workflow_owner": "backend-rs/src/api/memories.rs",
            "error_mapping": "private error_mapper module in backend-rs/src/api/memories.rs",
            "chapter_analysis_task_mapping": "backend-rs/src/api/chapters_error_mapper.rs"
        },
        "request_contract": {
            "list": "memory_type/chapter_id are trimmed; limit defaults to 50 and clamps to 1..=500",
            "search": "query is trimmed; limit defaults to 10 and clamps to 1..=100; min_importance defaults to 0.0; empty memory_types are removed",
            "foreshadows": "current_chapter is required and missing input maps to 400 current_chapter is required"
        },
        "readiness_probes": [
            "memories-stats-auth-guard-rust",
            "memories-list-auth-guard-rust",
            "memories-analysis-auth-guard-rust",
            "memories-foreshadows-auth-guard-rust",
            "memories-search-auth-guard-rust",
            "memories-delete-chapter-auth-guard-rust",
            "memories-fixture-import-project-business-rust",
            "memories-fixture-list-chapter-business-rust",
            "memories-list-business-rust",
            "memories-search-business-rust",
            "memories-analysis-business-rust",
            "memories-stats-business-rust",
            "memories-foreshadows-business-rust",
            "memories-delete-chapter-business-rust",
            "memories-stats-after-delete-business-rust",
            "memories-cleanup-project-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-memories-business-owner",
            "business_probes": [
                "memories-fixture-import-project-business-rust",
                "memories-fixture-list-chapter-business-rust",
                "memories-list-business-rust",
                "memories-search-business-rust",
                "memories-analysis-business-rust",
                "memories-stats-business-rust",
                "memories-foreshadows-business-rust",
                "memories-delete-chapter-business-rust",
                "memories-stats-after-delete-business-rust",
                "memories-cleanup-project-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "source_map_files": [
            "backend/migrator_app/models/memory_analysis.py"
        ],
        "rollback_boundary": {
            "source_map_policy": "production_python_memory_analysis_model_source_map_replaced_by_migrator_and_test_support_fixtures",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_fallback_removal_ready": true,
            "remaining_blockers": [],
            "freeze_reason": "Rust memories route group has dedicated phase5-memories-business-owner probes for fixture setup, list/search/analysis/stats/foreshadows, delete-chapter, stats-after-delete, and cleanup behavior; the Python route shell has been removed from app bootstrap, the broad memory_service.py surface has already been pushed down to narrower shared/runtime owner contracts, and the remaining persistence source map has now been narrowed to the dedicated memory analysis model file."
        },
        "business_smoke_status": {
            "owner_profile": "phase5-memories-business-owner",
            "business_probe_count": 10,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "explicit memory analysis model source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Memories route business smoke is covered by phase5-memories-business-owner; the Python route shell is no longer registered in app bootstrap, the broad memory_service.py surface has already moved to narrower shared/runtime owner contracts, and final completion now requires explicit memory analysis model source-map freeze/delete/repoint approval with same-round rollback policy."
    })
}

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

pub async fn load_owned_project_memories_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    request: MemoryListRequest,
) -> Result<Value, LoadProjectMemoriesPayloadError> {
    ProjectService::ensure_owned_access(db, project_id, user_id)
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
    ProjectService::ensure_owned_access(db, project_id, user_id)
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
    ProjectService::ensure_owned_access(db, project_id, user_id)
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
    ProjectService::ensure_owned_access(db, project_id, user_id)
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
    ProjectService::ensure_owned_access(db, project_id, user_id)
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

async fn analyze_chapter_memories_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, AnalyzeChapterMemoriesWriteWorkflowError> {
    ProjectService::ensure_owned_access(db, project_id, user_id)
        .await
        .map_err(MemoriesProjectWriteContextError::ProjectAccess)
        .map_err(AnalyzeChapterMemoriesWriteWorkflowError::Context)?;

    analyze_chapter_now(db, user_id, chapter_id)
        .await
        .map_err(AnalyzeChapterMemoriesWriteWorkflowError::CreateTask)
}

async fn delete_chapter_memories_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, DeleteChapterMemoriesWriteWorkflowError> {
    ProjectService::ensure_owned_access(db, project_id, user_id)
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

mod error_mapper {
    use axum::{http::StatusCode, response::Json};
    use serde_json::{json, Value};

    use crate::api::chapters_error_mapper::internal_detail_error;
    use crate::api::chapters_error_mapper::map_create_chapter_analysis_task_error;
    use crate::api::memories::{
        AnalyzeChapterMemoriesWriteWorkflowError, LoadProjectAccessError,
        LoadProjectChapterAnalysisPayloadError, MemoriesProjectQueryContextError,
        MemoriesProjectWriteContextError, OwnedProjectMemoriesQueryError,
    };

    type MemoriesRouteError = (StatusCode, Json<Value>);

    fn project_not_found_or_access_denied_error() -> MemoriesRouteError {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "项目不存在或无权限"})),
        )
    }

    fn map_project_access_error(error: LoadProjectAccessError) -> MemoriesRouteError {
        match error {
            LoadProjectAccessError::NotFoundOrAccessDenied => {
                project_not_found_or_access_denied_error()
            }
            LoadProjectAccessError::Internal(detail) => internal_detail_error(detail),
        }
    }

    fn map_memories_project_query_context_error(
        error: MemoriesProjectQueryContextError,
    ) -> MemoriesRouteError {
        match error {
            MemoriesProjectQueryContextError::ProjectAccess(error) => {
                map_project_access_error(error)
            }
            MemoriesProjectQueryContextError::Internal(detail) => internal_detail_error(detail),
        }
    }

    pub(super) fn map_memories_project_write_context_error(
        error: MemoriesProjectWriteContextError,
    ) -> MemoriesRouteError {
        match error {
            MemoriesProjectWriteContextError::ProjectAccess(error) => {
                map_project_access_error(error)
            }
            MemoriesProjectWriteContextError::Internal(detail) => internal_detail_error(detail),
        }
    }

    pub(super) fn map_project_chapter_analysis_payload_error(
        error: LoadProjectChapterAnalysisPayloadError,
    ) -> MemoriesRouteError {
        match error {
            LoadProjectChapterAnalysisPayloadError::Context(error) => {
                map_memories_project_query_context_error(error)
            }
            LoadProjectChapterAnalysisPayloadError::AnalysisNotFound => (
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "该章节还未进行分析"})),
            ),
        }
    }

    pub(super) fn map_analyze_chapter_memories_write_workflow_error(
        error: AnalyzeChapterMemoriesWriteWorkflowError,
    ) -> MemoriesRouteError {
        match error {
            AnalyzeChapterMemoriesWriteWorkflowError::Context(error) => {
                map_memories_project_write_context_error(error)
            }
            AnalyzeChapterMemoriesWriteWorkflowError::CreateTask(error) => {
                map_create_chapter_analysis_task_error(error)
            }
        }
    }

    pub(super) fn map_owned_project_memories_query_error(
        error: OwnedProjectMemoriesQueryError,
    ) -> MemoriesRouteError {
        map_memories_project_query_context_error(error)
    }

    #[cfg(test)]
    mod tests {
        use super::{
            map_analyze_chapter_memories_write_workflow_error,
            map_memories_project_write_context_error, map_owned_project_memories_query_error,
            map_project_access_error, map_project_chapter_analysis_payload_error,
        };
        use crate::api::memories::{
            AnalyzeChapterMemoriesWriteWorkflowError, DeleteChapterMemoriesWriteWorkflowError,
            LoadMemoryStatsPayloadError, LoadProjectAccessError,
            LoadProjectChapterAnalysisPayloadError, LoadProjectMemoriesPayloadError,
            LoadUnresolvedForeshadowsPayloadError, MemoriesProjectQueryContextError,
            MemoriesProjectWriteContextError, SearchProjectMemoriesPayloadError,
        };
        use crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError;
        use axum::http::StatusCode;
        use serde_json::json;

        #[test]
        fn project_access_not_found_keeps_chinese_not_found_detail() {
            let response = map_project_access_error(LoadProjectAccessError::NotFoundOrAccessDenied);

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(response.1 .0, json!({ "detail": "项目不存在或无权限" }));
        }

        #[test]
        fn project_memories_internal_error_keeps_internal_detail() {
            let response = map_owned_project_memories_query_error(
                LoadProjectMemoriesPayloadError::ProjectAccess(LoadProjectAccessError::Internal(
                    "db exploded".to_string(),
                )),
            );

            assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.1 .0, json!({ "detail": "db exploded" }));
        }

        #[test]
        fn chapter_analysis_not_found_keeps_existing_chinese_detail() {
            let response = map_project_chapter_analysis_payload_error(
                LoadProjectChapterAnalysisPayloadError::AnalysisNotFound,
            );

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(response.1 .0, json!({ "detail": "该章节还未进行分析" }));
        }

        #[test]
        fn search_project_memories_not_found_reuses_project_access_mapping() {
            let response = map_owned_project_memories_query_error(
                SearchProjectMemoriesPayloadError::ProjectAccess(
                    LoadProjectAccessError::NotFoundOrAccessDenied,
                ),
            );

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(response.1 .0, json!({ "detail": "项目不存在或无权限" }));
        }

        #[test]
        fn unresolved_foreshadows_internal_error_keeps_internal_detail() {
            let response = map_owned_project_memories_query_error(
                LoadUnresolvedForeshadowsPayloadError::ProjectAccess(
                    LoadProjectAccessError::Internal("foreshadow failed".to_string()),
                ),
            );

            assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.1 .0, json!({ "detail": "foreshadow failed" }));
        }

        #[test]
        fn memory_stats_not_found_reuses_project_access_mapping() {
            let response =
                map_owned_project_memories_query_error(LoadMemoryStatsPayloadError::ProjectAccess(
                    LoadProjectAccessError::NotFoundOrAccessDenied,
                ));

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(response.1 .0, json!({ "detail": "项目不存在或无权限" }));
        }

        #[test]
        fn analyze_chapter_memories_create_task_error_reuses_existing_mapping() {
            let response = map_analyze_chapter_memories_write_workflow_error(
                AnalyzeChapterMemoriesWriteWorkflowError::CreateTask(
                    CreateChapterAnalysisTaskError::ChapterEmpty,
                ),
            );

            assert_eq!(response.0, StatusCode::BAD_REQUEST);
            assert_eq!(response.1 .0, json!({ "detail": "章节不存在或内容为空" }));
        }

        #[test]
        fn delete_chapter_memories_internal_error_keeps_internal_detail() {
            let response = map_memories_project_write_context_error(
                DeleteChapterMemoriesWriteWorkflowError::Internal("delete failed".to_string()),
            );

            assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.1 .0, json!({ "detail": "delete failed" }));
        }

        #[test]
        fn analyze_chapter_memories_context_not_found_reuses_shared_write_context_mapping() {
            let response = map_analyze_chapter_memories_write_workflow_error(
                AnalyzeChapterMemoriesWriteWorkflowError::Context(
                    MemoriesProjectWriteContextError::ProjectAccess(
                        LoadProjectAccessError::NotFoundOrAccessDenied,
                    ),
                ),
            );

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(response.1 .0, json!({ "detail": "项目不存在或无权限" }));
        }

        #[test]
        fn analyze_chapter_memories_context_internal_reuses_shared_write_context_mapping() {
            let response = map_analyze_chapter_memories_write_workflow_error(
                AnalyzeChapterMemoriesWriteWorkflowError::Context(
                    MemoriesProjectWriteContextError::Internal("task creation failed".to_string()),
                ),
            );

            assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.1 .0, json!({ "detail": "task creation failed" }));
        }

        #[test]
        fn chapter_analysis_context_project_access_reuses_shared_context_mapping() {
            let response = map_project_chapter_analysis_payload_error(
                LoadProjectChapterAnalysisPayloadError::Context(
                    MemoriesProjectQueryContextError::ProjectAccess(
                        LoadProjectAccessError::NotFoundOrAccessDenied,
                    ),
                ),
            );

            assert_eq!(response.0, StatusCode::NOT_FOUND);
            assert_eq!(response.1 .0, json!({ "detail": "项目不存在或无权限" }));
        }

        #[test]
        fn chapter_analysis_context_internal_reuses_shared_context_mapping() {
            let response = map_project_chapter_analysis_payload_error(
                LoadProjectChapterAnalysisPayloadError::Context(
                    MemoriesProjectQueryContextError::Internal("analysis query failed".to_string()),
                ),
            );

            assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.1 .0, json!({ "detail": "analysis query failed" }));
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct MemoryListRouteQuery {
    pub memory_type: Option<String>,
    pub chapter_id: Option<String>,
    pub limit: Option<u64>,
}

fn normalize_optional_route_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn normalize_memory_list_limit(limit: Option<u64>) -> u64 {
    limit.unwrap_or(50).clamp(1, 500)
}

fn build_memory_list_request_from_route_query(
    route_query: MemoryListRouteQuery,
) -> MemoryListRequest {
    MemoryListRequest::new(
        normalize_optional_route_string(route_query.memory_type),
        normalize_optional_route_string(route_query.chapter_id),
        normalize_memory_list_limit(route_query.limit),
    )
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct SearchMemoriesRouteRequest {
    pub query: Option<String>,
    pub limit: Option<u64>,
    pub min_importance: Option<f64>,
    pub memory_types: Option<Vec<String>>,
}

fn normalize_search_limit(limit: Option<u64>) -> u64 {
    limit.unwrap_or(10).clamp(1, 100)
}

fn normalize_search_query(query: Option<String>) -> String {
    query.unwrap_or_default().trim().to_owned()
}

fn normalize_search_memory_types(memory_types: Option<Vec<String>>) -> Vec<String> {
    memory_types
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

fn build_search_memories_request_from_route_payload(
    route_request: SearchMemoriesRouteRequest,
) -> SearchMemoriesRequest {
    SearchMemoriesRequest::new(
        normalize_search_query(route_request.query),
        normalize_search_limit(route_request.limit),
        route_request.min_importance.unwrap_or(0.0),
        normalize_search_memory_types(route_request.memory_types),
    )
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ForeshadowListRouteQuery {
    pub current_chapter: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ForeshadowListRouteQueryError {
    CurrentChapterMissing,
}

fn build_foreshadow_list_current_chapter_from_route_query(
    route_query: ForeshadowListRouteQuery,
) -> Result<i32, ForeshadowListRouteQueryError> {
    route_query
        .current_chapter
        .ok_or(ForeshadowListRouteQueryError::CurrentChapterMissing)
}

fn map_foreshadow_list_route_query_error(
    error: ForeshadowListRouteQueryError,
) -> (StatusCode, Json<Value>) {
    match error {
        ForeshadowListRouteQueryError::CurrentChapterMissing => (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "current_chapter is required"})),
        ),
    }
}

async fn get_project_memories(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<MemoryListRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_memory_list_request_from_route_query(query);
    let payload = load_owned_project_memories_payload(&db, &project_id, &claims.sub, request)
        .await
        .map_err(map_owned_project_memories_query_error)?;
    Ok(Json(payload))
}

async fn analyze_chapter_memories(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path((project_id, chapter_id)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload =
        analyze_chapter_memories_write_workflow(&db, &project_id, &chapter_id, &claims.sub)
            .await
            .map_err(map_analyze_chapter_memories_write_workflow_error)?;
    Ok(Json(payload))
}

async fn get_chapter_analysis(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path((project_id, chapter_id)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload =
        load_owned_project_chapter_analysis_payload(&db, &project_id, &chapter_id, &claims.sub)
            .await
            .map_err(map_project_chapter_analysis_payload_error)?;
    Ok(Json(payload))
}

async fn search_memories(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(body): Json<SearchMemoriesRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_search_memories_request_from_route_payload(body);
    let payload = search_owned_project_memories_payload(&db, &project_id, &claims.sub, request)
        .await
        .map_err(map_owned_project_memories_query_error)?;
    Ok(Json(payload))
}

async fn get_unresolved_foreshadows(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<ForeshadowListRouteQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let current_chapter = build_foreshadow_list_current_chapter_from_route_query(query)
        .map_err(map_foreshadow_list_route_query_error)?;
    let payload = load_owned_unresolved_foreshadows_payload(
        &db,
        &project_id,
        &claims.sub,
        Some(current_chapter),
    )
    .await
    .map_err(map_owned_project_memories_query_error)?;
    Ok(Json(payload))
}

async fn get_memory_stats(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = load_owned_memory_stats_payload(&db, &project_id, &claims.sub)
        .await
        .map_err(map_owned_project_memories_query_error)?;
    Ok(Json(payload))
}

async fn delete_chapter_memories(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path((project_id, chapter_id)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload =
        delete_chapter_memories_write_workflow(&db, &project_id, &chapter_id, &claims.sub)
            .await
            .map_err(map_memories_project_write_context_error)?;
    Ok(Json(payload))
}

pub fn routes() -> Router {
    Router::new()
        .route(
            MEMORIES_ANALYZE_CHAPTER_ROUTE,
            post(analyze_chapter_memories),
        )
        .route(MEMORIES_LIST_ROUTE, get(get_project_memories))
        .route(MEMORIES_CHAPTER_ANALYSIS_ROUTE, get(get_chapter_analysis))
        .route(MEMORIES_SEARCH_ROUTE, post(search_memories))
        .route(MEMORIES_FORESHADOWS_ROUTE, get(get_unresolved_foreshadows))
        .route(MEMORIES_STATS_ROUTE, get(get_memory_stats))
        .route(
            MEMORIES_DELETE_CHAPTER_ROUTE,
            delete(delete_chapter_memories),
        )
}

#[cfg(test)]
mod tests {
    use super::{
        build_foreshadow_list_current_chapter_from_route_query,
        build_memories_route_owner_contract, build_memory_list_request_from_route_query,
        build_search_memories_request_from_route_payload, map_foreshadow_list_route_query_error,
        AnalyzeChapterMemoriesWriteWorkflowError, DeleteChapterMemoriesWriteWorkflowError,
        ForeshadowListRouteQuery, ForeshadowListRouteQueryError, LoadMemoryStatsPayloadError,
        LoadProjectAccessError, LoadProjectChapterAnalysisPayloadError,
        LoadProjectMemoriesPayloadError, LoadUnresolvedForeshadowsPayloadError,
        MemoriesProjectQueryContextError, MemoriesProjectWriteContextError, MemoryListRouteQuery,
        OwnedProjectMemoriesQueryError, SearchMemoriesRequest, SearchMemoriesRouteRequest,
        SearchProjectMemoriesPayloadError, MEMORIES_ANALYZE_CHAPTER_ROUTE,
        MEMORIES_CHAPTER_ANALYSIS_ROUTE, MEMORIES_DELETE_CHAPTER_ROUTE, MEMORIES_FORESHADOWS_ROUTE,
        MEMORIES_LIST_ROUTE, MEMORIES_SEARCH_ROUTE, MEMORIES_STATS_ROUTE,
    };
    use crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError;
    use axum::http::StatusCode;
    use serde_json::json;

    #[test]
    fn should_publish_memories_route_owner_contract() {
        let contract = build_memories_route_owner_contract();

        assert_eq!(contract["owner"], "memories");
        assert_eq!(contract["rust_owner"], "backend-rs/src/api/memories.rs");
        assert_eq!(
            contract["routes"]["analyze_chapter"],
            MEMORIES_ANALYZE_CHAPTER_ROUTE
        );
        assert_eq!(contract["routes"]["list"], MEMORIES_LIST_ROUTE);
        assert_eq!(
            contract["routes"]["chapter_analysis"],
            MEMORIES_CHAPTER_ANALYSIS_ROUTE
        );
        assert_eq!(contract["routes"]["search"], MEMORIES_SEARCH_ROUTE);
        assert_eq!(
            contract["routes"]["foreshadows"],
            MEMORIES_FORESHADOWS_ROUTE
        );
        assert_eq!(contract["routes"]["stats"], MEMORIES_STATS_ROUTE);
        assert_eq!(
            contract["routes"]["delete_chapter"],
            MEMORIES_DELETE_CHAPTER_ROUTE
        );
        assert_eq!(contract["source_map_files"].as_array().unwrap().len(), 1);
        assert_eq!(
            contract["source_map_files"][0],
            "backend/migrator_app/models/memory_analysis.py"
        );
        assert_eq!(contract["readiness_probes"].as_array().unwrap().len(), 16);
        assert_eq!(
            contract["readiness_probes"][15],
            "memories-cleanup-project-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-memories-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"]
                .as_array()
                .expect("business probes should be present")
                .len(),
            10
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"][7],
            "memories-delete-chapter-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            json!(true)
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            json!(true)
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(10)
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "production_python_memory_analysis_model_source_map_replaced_by_migrator_and_test_support_fixtures"
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit memory analysis model source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("business smoke is covered"));
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("memory_service.py surface has already moved"));
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("memory analysis model source-map freeze/delete/repoint approval"));
        assert!(!contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("Do not count python-fallback = 0 as completion"));
        assert!(!contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("logged-in business probes are accepted"));
    }

    #[test]
    fn should_keep_memories_route_group_paths_stable() {
        assert_eq!(
            MEMORIES_ANALYZE_CHAPTER_ROUTE,
            "/memories/projects/{project_id}/analyze-chapter/{chapter_id}"
        );
        assert_eq!(
            MEMORIES_LIST_ROUTE,
            "/memories/projects/{project_id}/memories"
        );
        assert_eq!(
            MEMORIES_CHAPTER_ANALYSIS_ROUTE,
            "/memories/projects/{project_id}/analysis/{chapter_id}"
        );
        assert_eq!(
            MEMORIES_SEARCH_ROUTE,
            "/memories/projects/{project_id}/search"
        );
        assert_eq!(
            MEMORIES_FORESHADOWS_ROUTE,
            "/memories/projects/{project_id}/foreshadows"
        );
        assert_eq!(
            MEMORIES_STATS_ROUTE,
            "/memories/projects/{project_id}/stats"
        );
        assert_eq!(
            MEMORIES_DELETE_CHAPTER_ROUTE,
            "/memories/projects/{project_id}/chapters/{chapter_id}/memories"
        );
    }

    #[test]
    fn should_build_memory_list_request_from_route_query() {
        let request = build_memory_list_request_from_route_query(MemoryListRouteQuery {
            memory_type: Some(" summary ".to_string()),
            chapter_id: Some(" chapter-1 ".to_string()),
            limit: Some(900),
        });

        assert_eq!(request.memory_type(), Some("summary"));
        assert_eq!(request.chapter_id(), Some("chapter-1"));
        assert_eq!(request.limit(), 500);
    }

    #[test]
    fn should_build_search_memories_request_from_route_payload() {
        let request =
            build_search_memories_request_from_route_payload(SearchMemoriesRouteRequest {
                query: Some("  conflict  ".to_string()),
                limit: Some(0),
                min_importance: Some(0.75),
                memory_types: Some(vec![
                    " clue ".to_string(),
                    "".to_string(),
                    " setup ".to_string(),
                ]),
            });

        assert_eq!(request.query(), "conflict");
        assert_eq!(request.limit(), 1);
        assert_eq!(request.min_importance(), 0.75);
        assert_eq!(
            request.memory_types(),
            &["clue".to_string(), "setup".to_string()]
        );
    }

    #[test]
    fn should_require_foreshadow_list_current_chapter_like_python_route() {
        assert_eq!(
            build_foreshadow_list_current_chapter_from_route_query(ForeshadowListRouteQuery {
                current_chapter: Some(12),
            }),
            Ok(12)
        );

        assert_eq!(
            build_foreshadow_list_current_chapter_from_route_query(ForeshadowListRouteQuery {
                current_chapter: None,
            }),
            Err(ForeshadowListRouteQueryError::CurrentChapterMissing)
        );
    }

    #[test]
    fn should_map_missing_foreshadow_current_chapter_to_bad_request() {
        let response = map_foreshadow_list_route_query_error(
            ForeshadowListRouteQueryError::CurrentChapterMissing,
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "current_chapter is required" })
        );
    }

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

    #[test]
    fn should_build_search_memories_request_owner_shape() {
        let request = SearchMemoriesRequest::new(
            "conflict".to_string(),
            10,
            0.75,
            vec!["setup".to_string(), "payoff".to_string()],
        );

        assert_eq!(request.query(), "conflict");
        assert_eq!(request.limit(), 10);
        assert_eq!(request.min_importance(), 0.75);
        assert_eq!(
            request.memory_types(),
            &["setup".to_string(), "payoff".to_string()]
        );
    }
}
