use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::{foreshadow, plot_analysis, story_memory};
use crate::services::auth::Claims;
use crate::services::chapter_analysis_runtime_service::enqueue_chapter_analysis_task;
use crate::services::chapter_analysis_service::{
    CreateChapterAnalysisTaskError,
};
use crate::services::project_service::ProjectService;

#[derive(Deserialize)]
struct MemoryListQuery {
    memory_type: Option<String>,
    chapter_id: Option<String>,
    limit: Option<u64>,
}

#[derive(Deserialize)]
struct ForeshadowQuery {
    current_chapter: Option<i32>,
}

async fn ensure_project_access(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<(), (StatusCode, Json<Value>)> {
    ProjectService::get(db, project_id, user_id)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "项目不存在或无权限"})),
        ))
        .map(|_| ())
}

async fn get_project_memories(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<MemoryListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ensure_project_access(&db, &project_id, &claims.sub).await?;

    let mut stmt =
        story_memory::Entity::find().filter(story_memory::Column::ProjectId.eq(&project_id));
    if let Some(memory_type) = query.memory_type.as_deref() {
        stmt = stmt.filter(story_memory::Column::MemoryType.eq(memory_type));
    }
    if let Some(chapter_id) = query.chapter_id.as_deref() {
        stmt = stmt.filter(story_memory::Column::ChapterId.eq(chapter_id));
    }

    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let memories = stmt
        .order_by_desc(story_memory::Column::ImportanceScore)
        .order_by_desc(story_memory::Column::CreatedAt)
        .limit(limit)
        .all(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    Ok(Json(json!({
        "success": true,
        "memories": memories,
        "total": memories.len(),
    })))
}

async fn analyze_chapter_memories(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path((project_id, chapter_id)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ensure_project_access(&db, &project_id, &claims.sub).await?;

    match enqueue_chapter_analysis_task(&db, &claims.sub, &chapter_id).await {
        Ok(payload) => Ok(Json(json!({
            "success": true,
            "message": "章节分析任务已创建",
            "project_id": project_id,
            "chapter_id": chapter_id,
            "task": payload,
        }))),
        Err(error) => Err(match error {
            CreateChapterAnalysisTaskError::ChapterEmpty => (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": "章节不存在或内容为空"})),
            ),
            CreateChapterAnalysisTaskError::ProjectMissing => (
                StatusCode::NOT_FOUND,
                Json(json!({"detail": "项目不存在"})),
            ),
            CreateChapterAnalysisTaskError::Internal(detail) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": detail})),
            ),
        }),
    }
}

async fn get_chapter_analysis(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path((project_id, chapter_id)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ensure_project_access(&db, &project_id, &claims.sub).await?;

    let analysis = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ProjectId.eq(&project_id))
        .filter(plot_analysis::Column::ChapterId.eq(&chapter_id))
        .one(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "该章节还未进行分析"})),
        ))?;

    Ok(Json(json!({
        "success": true,
        "analysis": analysis,
    })))
}

async fn search_memories(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ensure_project_access(&db, &project_id, &claims.sub).await?;

    let query = body
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let limit = body
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(10)
        .clamp(1, 100);
    let min_importance = body
        .get("min_importance")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    let mut stmt = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(&project_id))
        .filter(story_memory::Column::ImportanceScore.gte(min_importance));
    if !query.is_empty() {
        let pattern = format!("%{}%", query);
        stmt = stmt.filter(
            story_memory::Column::Title
                .like(&pattern)
                .or(story_memory::Column::Content.like(&pattern)),
        );
    }
    if let Some(memory_types) = body.get("memory_types").and_then(Value::as_array) {
        let types: Vec<String> = memory_types
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        if !types.is_empty() {
            stmt = stmt.filter(story_memory::Column::MemoryType.is_in(types));
        }
    }

    let memories = stmt
        .order_by_desc(story_memory::Column::ImportanceScore)
        .order_by_desc(story_memory::Column::CreatedAt)
        .limit(limit)
        .all(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    Ok(Json(json!({
        "success": true,
        "query": query,
        "memories": memories,
        "total": memories.len(),
    })))
}

async fn get_unresolved_foreshadows(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<ForeshadowQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ensure_project_access(&db, &project_id, &claims.sub).await?;

    let mut stmt = foreshadow::Entity::find()
        .filter(foreshadow::Column::ProjectId.eq(&project_id))
        .filter(foreshadow::Column::Status.ne("resolved"))
        .filter(foreshadow::Column::Status.ne("abandoned"));
    if let Some(current_chapter) = query.current_chapter {
        stmt = stmt.filter(
            foreshadow::Column::PlantChapterNumber
                .is_null()
                .or(foreshadow::Column::PlantChapterNumber.lte(current_chapter)),
        );
    }

    let foreshadows = stmt
        .order_by_desc(foreshadow::Column::Importance)
        .order_by_desc(foreshadow::Column::CreatedAt)
        .all(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    Ok(Json(json!({
        "success": true,
        "foreshadows": foreshadows,
        "total": foreshadows.len(),
    })))
}

async fn get_memory_stats(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ensure_project_access(&db, &project_id, &claims.sub).await?;

    let total = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(&project_id))
        .count(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;
    let foreshadow_count = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(&project_id))
        .filter(story_memory::Column::IsForeshadow.eq(1))
        .count(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;
    let plot_analysis_count = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ProjectId.eq(&project_id))
        .count(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    Ok(Json(json!({
        "success": true,
        "stats": {
            "total_memories": total,
            "foreshadows": foreshadow_count,
            "plot_analyses": plot_analysis_count,
        },
    })))
}

async fn delete_chapter_memories(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path((project_id, chapter_id)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ensure_project_access(&db, &project_id, &claims.sub).await?;

    let result = story_memory::Entity::delete_many()
        .filter(story_memory::Column::ProjectId.eq(&project_id))
        .filter(story_memory::Column::ChapterId.eq(&chapter_id))
        .exec(&db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?;

    Ok(Json(json!({
        "success": true,
        "message": format!("已删除{}条记忆", result.rows_affected),
    })))
}

pub fn routes() -> Router {
    Router::new()
        .route(
            "/memories/projects/{project_id}/analyze-chapter/{chapter_id}",
            post(analyze_chapter_memories),
        )
        .route(
            "/memories/projects/{project_id}/memories",
            get(get_project_memories),
        )
        .route(
            "/memories/projects/{project_id}/analysis/{chapter_id}",
            get(get_chapter_analysis),
        )
        .route(
            "/memories/projects/{project_id}/search",
            post(search_memories),
        )
        .route(
            "/memories/projects/{project_id}/foreshadows",
            get(get_unresolved_foreshadows),
        )
        .route(
            "/memories/projects/{project_id}/stats",
            get(get_memory_stats),
        )
        .route(
            "/memories/projects/{project_id}/chapters/{chapter_id}/memories",
            delete(delete_chapter_memories),
        )
}
