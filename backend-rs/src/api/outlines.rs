use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{chapter, outline, project};
use crate::services::auth::Claims;
use crate::services::outline_expansion_request_service::{
    build_outline_batch_expand_execution_request_from_route_request,
    build_outline_expand_execution_request_from_route_request,
    execute_outline_batch_expand_request, execute_outline_expand_request,
    OutlineBatchExpandRouteRequest, OutlineExpandRouteRequest,
};
use crate::services::outline_generation_request_service::{
    execute_outline_generate_route_request, OutlineGenerateRouteRequest,
};
use crate::services::outline_service::OutlineService;
use crate::utils::sse::SseChannel;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    title: String,
    content: Option<String>,
    order_index: Option<i32>,
    structure: Option<String>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    title: Option<String>,
    content: Option<String>,
    order_index: Option<i32>,
    structure: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

#[derive(Deserialize)]
struct OutlineReorderItem {
    id: String,
    order_index: i32,
}

#[derive(Deserialize)]
struct ReorderRequest {
    orders: Vec<OutlineReorderItem>,
}

fn compatible_outline_payload(outline: outline::Model) -> Value {
    let outline_value = serde_json::to_value(&outline).unwrap_or_else(|_| json!({}));
    match outline_value {
        Value::Object(mut map) => {
            map.insert("success".to_string(), json!(true));
            map.insert("data".to_string(), json!(outline));
            Value::Object(map)
        }
        _ => json!({
            "success": true,
            "data": outline
        }),
    }
}

async fn generate_outlines(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<OutlineGenerateRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (tx, mut rx) =
        mpsc::channel::<Result<axum::response::sse::Event, std::convert::Infallible>>(256);
    let result_capture: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
    let channel = SseChannel::with_result_capture(tx, result_capture.clone());
    let db_for_task = db.clone();
    let user_id = claims.sub.clone();
    let request = body.clone();

    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    execute_outline_generate_route_request(&db_for_task, &channel, &user_id, &request).await;

    let _ = drain_handle.await;
    let result = result_capture.lock().await.clone().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": "??????"})),
        )
    })?;

    let items = result
        .get("outlines")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();

    let total = items.len();
    Ok(Json(json!({
        "success": true,
        "total": total,
        "items": items,
        "outlines": result.get("outlines").cloned().unwrap_or_else(|| json!([])),
        "chapters": result.get("chapters").cloned().unwrap_or_else(|| json!([])),
        "outline_count": result.get("outline_count").cloned().unwrap_or_else(|| json!(total)),
        "chapter_count": result.get("chapter_count").cloned().unwrap_or_else(|| json!(0)),
        "message": result.get("message").cloned().unwrap_or_else(|| json!("????")),
        "result": result,
    })))
}

async fn reorder_outlines(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<ReorderRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if body.orders.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "????????"})),
        ));
    }

    let mut updated_count = 0usize;
    for order in body.orders {
        let Some(outline_model) = OutlineService::get(&db, &order.id, &claims.sub)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": e})),
                )
            })?
        else {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"success": false, "message": "?????????"})),
            ));
        };

        let mut active: outline::ActiveModel = outline_model.into();
        active.order_index = Set(Some(order.order_index));
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active.update(&db).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": e.to_string()})),
            )
        })?;
        updated_count += 1;
    }

    Ok(Json(json!({
        "success": true,
        "message": "???????",
        "updated_outlines": updated_count,
        "updated_chapters": 0,
    })))
}

async fn expand_outline_compat(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
    Json(body): Json<OutlineExpandRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some(_outline_model) = OutlineService::get(&db, &outline_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": error})),
            )
        })?
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "大纲不存在或无权限"})),
        ));
    };

    let request =
        build_outline_expand_execution_request_from_route_request(outline_id.clone(), &body);

    execute_outline_expand_request(&db, &claims.sub, &request)
        .await
        .map(Json)
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": error})),
            )
        })
}

async fn batch_expand_outlines_compat(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<OutlineBatchExpandRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_outline_batch_expand_execution_request_from_route_request(&body);

    execute_outline_batch_expand_request(&db, &claims.sub, &request)
        .await
        .map(Json)
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": error})),
            )
        })
}

async fn create_outline(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match OutlineService::create(
        &db,
        &body.project_id,
        &claims.sub,
        &body.title,
        body.content.as_deref(),
        body.order_index,
        body.structure.as_deref(),
    )
    .await
    {
        Ok(Some(outline)) => Ok((
            StatusCode::CREATED,
            Json(compatible_outline_payload(outline)),
        )),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "项目不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn list_outlines(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(outlines)) => Ok(Json(
            json!({"success": true, "data": outlines, "items": outlines, "total": outlines.len()}),
        )),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "项目不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn get_outline(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::get(&db, &outline_id, &claims.sub).await {
        Ok(Some(outline)) => Ok(Json(compatible_outline_payload(outline))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "大纲不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn update_outline(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::update(
        &db,
        &outline_id,
        &claims.sub,
        body.title.as_deref(),
        body.content.as_deref(),
        body.order_index,
        body.structure.as_deref(),
    )
    .await
    {
        Ok(Some(outline)) => Ok(Json(compatible_outline_payload(outline))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "大纲不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn delete_outline(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::delete(&db, &outline_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "大纲已删除"}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "大纲不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn create_single_chapter(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let ol = OutlineService::get(&db, &outline_id, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "大纲不存在"}))))?;

    let proj = project::Entity::find_by_id(&ol.project_id)
        .one(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "项目不存在"}))))?;

    if proj.outline_mode != "one-to-one" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "仅一对一模式支持从大纲直接创建章节"})),
        ));
    }

    let chapter_number = ol.order_index.unwrap_or(1);
    let sub_index = 1;

    // Check for duplicate
    let existing = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&ol.project_id))
        .filter(chapter::Column::ChapterNumber.eq(chapter_number))
        .filter(chapter::Column::SubIndex.eq(sub_index))
        .one(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    if existing.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"detail": format!("第{}章已存在", chapter_number)})),
        ));
    }

    let now = Utc::now().naive_utc();
    let content_str = ol.content.unwrap_or_default();
    let ch = chapter::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(ol.project_id.clone()),
        chapter_number: Set(chapter_number),
        title: Set(ol.title.clone()),
        content: Set(Some(String::new())),
        summary: Set(Some(content_str)),
        word_count: Set(0),
        status: Set("pending".to_string()),
        outline_id: Set(None), // traditional mode: no outline link
        sub_index: Set(sub_index),
        expansion_plan: Set(None),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    };

    let inserted = ch.insert(&db).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )
    })?;

    Ok(Json(json!({
        "message": "章节创建成功",
        "chapter": inserted,
    })))
}

async fn get_outline_chapters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Verify outline belongs to user
    let ol = OutlineService::get(&db, &outline_id, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    let ol = match ol {
        Some(o) => o,
        None => {
            return Ok(Json(json!({
                "has_chapters": false,
                "outline_id": outline_id,
                "outline_title": null,
                "chapter_count": 0,
                "chapters": [],
            })));
        }
    };

    let chapters = chapter::Entity::find()
        .filter(chapter::Column::OutlineId.eq(&outline_id))
        .order_by_asc(chapter::Column::SubIndex)
        .all(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    let expansion_plans: Vec<Value> = chapters
        .iter()
        .filter_map(|c| {
            c.expansion_plan
                .as_ref()
                .and_then(|p| serde_json::from_str::<Value>(p).ok())
        })
        .collect();

    let has_chapters = !chapters.is_empty();
    Ok(Json(json!({
        "has_chapters": has_chapters,
        "outline_id": outline_id,
        "outline_title": ol.title,
        "chapter_count": chapters.len(),
        "chapters": chapters,
        "expansion_plans": if expansion_plans.is_empty() { json!(null) } else { json!(expansion_plans) },
    })))
}

#[derive(Deserialize, Serialize)]
struct ChapterPlan {
    sub_index: Option<i32>,
    title: String,
    plot_summary: Option<String>,
    key_events: Option<Vec<String>>,
    character_focus: Option<Vec<String>>,
    emotional_tone: Option<String>,
    narrative_goal: Option<String>,
    conflict_type: Option<String>,
    estimated_words: Option<i32>,
    scenes: Option<Vec<Value>>,
}

#[derive(Deserialize)]
struct CreateChaptersFromPlansRequest {
    #[serde(default, alias = "chapter_plans")]
    plans: Vec<ChapterPlan>,
}

async fn create_chapters_from_plans(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(outline_id): Path<String>,
    Json(body): Json<CreateChaptersFromPlansRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let ol = OutlineService::get(&db, &outline_id, &claims.sub)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .ok_or((StatusCode::NOT_FOUND, Json(json!({"detail": "大纲不存在"}))))?;

    // Count existing chapters before this outline to determine starting chapter number
    let existing_count = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&ol.project_id))
        .all(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?
        .len() as i32;

    // Also count chapters from earlier outlines
    let earlier_outlines = outline::Entity::find()
        .filter(outline::Column::ProjectId.eq(&ol.project_id))
        .order_by_asc(outline::Column::OrderIndex)
        .all(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

    let mut start_chapter_num = 1i32;
    for eo in &earlier_outlines {
        let eo_chapters = chapter::Entity::find()
            .filter(chapter::Column::OutlineId.eq(&eo.id))
            .all(&db)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": format!("{}", e)})),
                )
            })?;
        start_chapter_num += eo_chapters.len() as i32;
        if eo.id == outline_id {
            break;
        }
    }

    // If no earlier outlines found via outline_id, use total count
    if start_chapter_num == 1 {
        start_chapter_num = existing_count + 1;
    }

    let mut created = Vec::new();
    let now = Utc::now().naive_utc();

    for (i, plan) in body.plans.iter().enumerate() {
        let chapter_number = start_chapter_num + i as i32;
        let sub_index = plan.sub_index.unwrap_or(i as i32 + 1);

        let expansion_plan = serde_json::to_string(plan).unwrap_or_default();

        let ch = chapter::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(ol.project_id.clone()),
            chapter_number: Set(chapter_number),
            title: Set(plan.title.clone()),
            content: Set(Some(String::new())),
            summary: Set(plan.plot_summary.clone()),
            word_count: Set(0),
            status: Set("pending".to_string()),
            outline_id: Set(Some(outline_id.clone())),
            sub_index: Set(sub_index),
            expansion_plan: Set(Some(expansion_plan)),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };

        let inserted = ch.insert(&db).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })?;

        created.push(inserted);
    }

    let created_chapters: Vec<Value> = created
        .iter()
        .map(|chapter| {
            json!({
                "id": chapter.id,
                "chapter_number": chapter.chapter_number,
                "title": chapter.title,
                "summary": chapter.summary,
                "outline_id": chapter.outline_id,
                "sub_index": chapter.sub_index,
                "status": chapter.status,
            })
        })
        .collect();

    Ok(Json(json!({
        "message": "??????",
        "outline_id": outline_id,
        "outline_title": ol.title,
        "chapters_created": created.len(),
        "created_chapters": created_chapters,
        "start_chapter_number": start_chapter_num,
        "chapters": created,
    })))
}

async fn list_outlines_by_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OutlineService::list(&db, &project_id, &claims.sub).await {
        Ok(Some(outlines)) => Ok(Json(
            json!({"success": true, "data": outlines, "items": outlines, "total": outlines.len()}),
        )),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "项目不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route(
            "/outlines/project/{project_id}",
            get(list_outlines_by_project),
        )
        .route("/outlines/generate", post(generate_outlines))
        .route("/outlines/generate-stream", post(generate_outlines))
        .route("/outlines/reorder", post(reorder_outlines))
        .route("/outlines/batch-expand", post(batch_expand_outlines_compat))
        .route(
            "/outlines/batch-expand-stream",
            post(batch_expand_outlines_compat),
        )
        .route("/outlines", post(create_outline).get(list_outlines))
        .route(
            "/outlines/{outline_id}",
            get(get_outline).put(update_outline).delete(delete_outline),
        )
        .route("/outlines/{outline_id}/expand", post(expand_outline_compat))
        .route(
            "/outlines/{outline_id}/expand-stream",
            post(expand_outline_compat),
        )
        .route(
            "/outlines/{outline_id}/create-single-chapter",
            post(create_single_chapter),
        )
        .route("/outlines/{outline_id}/chapters", get(get_outline_chapters))
        .route(
            "/outlines/{outline_id}/create-chapters-from-plans",
            post(create_chapters_from_plans),
        )
}
