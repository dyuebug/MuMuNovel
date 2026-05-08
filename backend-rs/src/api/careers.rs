use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::services::auth::Claims;
use crate::services::career_service::CareerService;

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    name: String,
    career_type: Option<String>,
    #[serde(rename = "type")]
    career_kind: Option<String>,
    stages: Value,
    description: Option<String>,
    category: Option<String>,
    max_stage: Option<i32>,
    requirements: Option<String>,
    special_abilities: Option<String>,
    worldview_rules: Option<String>,
    attribute_bonuses: Option<Value>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    name: Option<String>,
    description: Option<String>,
    stages: Option<Value>,
    max_stage: Option<i32>,
    category: Option<String>,
    requirements: Option<String>,
    special_abilities: Option<String>,
    worldview_rules: Option<String>,
    attribute_bonuses: Option<Value>,
}

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

fn json_or_string(value: &str) -> Value {
    serde_json::from_str(value).unwrap_or_else(|_| json!(value))
}

fn stages_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

fn career_to_legacy_json(career: &crate::models::career::Model) -> Value {
    json!({
        "id": career.id,
        "project_id": career.project_id,
        "name": career.name,
        "type": career.career_type,
        "description": career.description,
        "category": career.category,
        "stages": json_or_string(&career.stages),
        "max_stage": career.max_stage,
        "requirements": career.requirements,
        "special_abilities": career.special_abilities,
        "worldview_rules": career.worldview_rules,
        "attribute_bonuses": career.attribute_bonuses.as_deref().map(json_or_string),
        "source": career.source,
        "created_at": career.created_at,
        "updated_at": career.updated_at,
    })
}

async fn create_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let Some(career_type) = body.career_type.as_deref().or(body.career_kind.as_deref()) else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "career type is required"})),
        ));
    };
    let stages = stages_to_string(&body.stages);
    let attribute_bonuses = body.attribute_bonuses.as_ref().map(Value::to_string);
    let result = if body.requirements.is_some()
        || body.special_abilities.is_some()
        || body.worldview_rules.is_some()
        || body.attribute_bonuses.is_some()
    {
        CareerService::create_full_for_user(
            &db,
            &body.project_id,
            &claims.sub,
            &body.name,
            career_type,
            body.description.as_deref(),
            body.category.as_deref(),
            &stages,
            body.max_stage.unwrap_or(10),
            body.requirements.as_deref(),
            body.special_abilities.as_deref(),
            body.worldview_rules.as_deref(),
            attribute_bonuses.as_deref(),
        )
        .await
    } else {
        CareerService::create(
            &db,
            &body.project_id,
            &claims.sub,
            &body.name,
            career_type,
            &stages,
            body.description.as_deref(),
            body.category.as_deref(),
            body.max_stage,
        )
        .await
    };
    match result {
        Ok(Some(career)) => Ok((StatusCode::CREATED, Json(career_to_legacy_json(&career)))),
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

async fn list_careers(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CareerService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(careers)) => {
            let total = careers.len();
            let mut main_careers = Vec::new();
            let mut sub_careers = Vec::new();
            for career in careers {
                let career_json = career_to_legacy_json(&career);
                if career.career_type == "main" {
                    main_careers.push(career_json);
                } else {
                    sub_careers.push(career_json);
                }
            }
            Ok(Json(json!({
                "total": total,
                "main_careers": main_careers,
                "sub_careers": sub_careers,
            })))
        }
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

async fn get_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(career_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CareerService::get(&db, &career_id, &claims.sub).await {
        Ok(Some(career)) => Ok(Json(career_to_legacy_json(&career))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "职业不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn update_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(career_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let stages = body.stages.as_ref().map(stages_to_string);
    let attribute_bonuses = body.attribute_bonuses.as_ref().map(Value::to_string);
    match CareerService::update_full_for_user(
        &db,
        &career_id,
        &claims.sub,
        body.name.as_deref(),
        body.description.as_deref(),
        stages.as_deref(),
        body.max_stage,
        body.category.as_deref(),
        body.requirements.as_deref(),
        body.special_abilities.as_deref(),
        body.worldview_rules.as_deref(),
        attribute_bonuses.as_deref(),
    )
    .await
    {
        Ok(Some(career)) => Ok(Json(career_to_legacy_json(&career))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "职业不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn delete_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(career_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CareerService::delete(&db, &career_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "职业已删除"}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "职业不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route("/careers", post(create_career).get(list_careers))
        .route(
            "/careers/{career_id}",
            get(get_career).put(update_career).delete(delete_career),
        )
}
