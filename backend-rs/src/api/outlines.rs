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

use crate::ai::service::AIService;
use crate::models::{chapter, outline, project};
use crate::services::auth::Claims;
use crate::services::outline_service::OutlineService;
use crate::services::plot_expansion_service::create_plot_expansion_service;
use crate::services::settings_service::SettingsService;
use crate::services::wizard_service;
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
#[allow(dead_code)]
struct GenerateRequest {
    project_id: String,
    #[serde(default = "default_outline_count")]
    chapter_count: usize,
    narrative_perspective: Option<String>,
    #[serde(default = "default_target_words")]
    target_words: i32,
    requirements: Option<String>,
    creative_mode: Option<String>,
    story_focus: Option<String>,
    plot_stage: Option<String>,
    story_creation_brief: Option<String>,
    quality_preset: Option<String>,
    quality_notes: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    theme: Option<String>,
    genre: Option<String>,
    mode: Option<String>,
    story_direction: Option<String>,
    keep_existing: Option<bool>,
    world_context: Option<Value>,
    characters_context: Option<Vec<Value>>,
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

fn default_outline_count() -> usize {
    3
}

fn default_target_words() -> i32 {
    100000
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
    Json(body): Json<GenerateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some(project) = project::Entity::find_by_id(&body.project_id)
        .one(&db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": e.to_string()})),
            )
        })?
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "?????"})),
        ));
    };
    if project.user_id != claims.sub {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "?????????"})),
        ));
    }

    let (tx, mut rx) =
        mpsc::channel::<Result<axum::response::sse::Event, std::convert::Infallible>>(256);
    let result_capture: Arc<Mutex<Option<Value>>> = Arc::new(Mutex::new(None));
    let channel = SseChannel::with_result_capture(tx, result_capture.clone());
    let db_for_task = db.clone();
    let user_id = claims.sub.clone();
    let project_id = body.project_id.clone();
    let chapter_count = body.chapter_count;
    let narrative_perspective = body.narrative_perspective.clone();
    let target_words = body.target_words;
    let requirements = body.requirements.clone();
    let creative_mode = body.creative_mode.clone();
    let story_focus = body.story_focus.clone();
    let plot_stage = body.plot_stage.clone();
    let story_creation_brief = body.story_creation_brief.clone();
    let quality_preset = body.quality_preset.clone();
    let quality_notes = body.quality_notes.clone();
    let provider = body.provider.clone();
    let model = body.model.clone();

    let drain_handle = tokio::spawn(async move { while rx.recv().await.is_some() {} });

    wizard_service::generate_outline(
        &db_for_task,
        &channel,
        &user_id,
        &project_id,
        chapter_count,
        narrative_perspective.as_deref(),
        target_words,
        requirements.as_deref(),
        creative_mode.as_deref(),
        story_focus.as_deref(),
        plot_stage.as_deref(),
        story_creation_brief.as_deref(),
        quality_preset.as_deref(),
        quality_notes.as_deref(),
        provider.as_deref(),
        model.as_deref(),
    )
    .await;

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
    Json(body): Json<Value>,
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

    let target_chapter_count = body
        .get("target_chapter_count")
        .and_then(Value::as_i64)
        .unwrap_or_default() as usize;
    let expansion_strategy = body
        .get("expansion_strategy")
        .and_then(Value::as_str)
        .unwrap_or("balanced");
    let auto_create_chapters = body
        .get("auto_create_chapters")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let enable_scene_analysis = body
        .get("enable_scene_analysis")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let provider = body.get("provider").and_then(Value::as_str);
    let model = body.get("model").and_then(Value::as_str);
    let batch_size = body
        .get("batch_size")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0)
        .unwrap_or(5) as usize;

    let ai_config = SettingsService::build_ai_config(&db, &claims.sub, provider, model, None)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": error})),
            )
        })?;
    let ai_service = AIService::new(ai_config);
    let service = create_plot_expansion_service(&ai_service);

    service
        .expand_outline(
            &db,
            &claims.sub,
            &outline_id,
            target_chapter_count,
            expansion_strategy,
            auto_create_chapters,
            enable_scene_analysis,
            provider,
            model,
            batch_size,
        )
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
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let project_id = body
        .get("project_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let chapters_per_outline = body
        .get("chapters_per_outline")
        .and_then(Value::as_i64)
        .unwrap_or_default() as usize;
    let expansion_strategy = body
        .get("expansion_strategy")
        .and_then(Value::as_str)
        .unwrap_or("balanced");
    let auto_create_chapters = body
        .get("auto_create_chapters")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let enable_scene_analysis = body
        .get("enable_scene_analysis")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let provider = body.get("provider").and_then(Value::as_str);
    let model = body.get("model").and_then(Value::as_str);
    let outline_ids = body
        .get("outline_ids")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        });

    let ai_config = SettingsService::build_ai_config(&db, &claims.sub, provider, model, None)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": error})),
            )
        })?;
    let ai_service = AIService::new(ai_config);
    let service = create_plot_expansion_service(&ai_service);

    service
        .batch_expand_outlines(
            &db,
            &claims.sub,
            project_id,
            chapters_per_outline,
            expansion_strategy,
            auto_create_chapters,
            enable_scene_analysis,
            outline_ids.as_deref(),
            provider,
            model,
        )
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
