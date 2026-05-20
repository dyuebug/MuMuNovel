use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{sse::Event, Json, Sse},
    routing::{delete, get, post, put},
    Router,
};
use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::models::{career, character, character_career, project};
use crate::services::auth::Claims;
use crate::services::career_service::CareerService;
use crate::services::wizard_service;

type ApiError = (StatusCode, Json<Value>);

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

#[derive(Deserialize)]
struct LegacyCareerSystemQuery {
    project_id: String,
    main_career_count: Option<i32>,
    sub_career_count: Option<i32>,
    enable_mcp: Option<bool>,
    provider: Option<String>,
    model: Option<String>,
    user_id: Option<String>,
}

#[derive(Deserialize)]
struct CareerAssignmentRequest {
    career_id: String,
    current_stage: Option<i32>,
    started_at: Option<String>,
}

#[derive(Deserialize)]
struct CareerStageUpdateRequest {
    current_stage: i32,
    stage_progress: Option<i32>,
    reached_current_stage_at: Option<String>,
    notes: Option<String>,
}

#[derive(Serialize)]
struct CharacterCareerDetail {
    id: String,
    character_id: String,
    career_id: String,
    career_name: String,
    career_type: String,
    current_stage: i32,
    stage_name: String,
    stage_description: Option<String>,
    stage_progress: i32,
    max_stage: i32,
    started_at: Option<String>,
    reached_current_stage_at: Option<String>,
    notes: Option<String>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
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

fn parse_stages(value: &str) -> Vec<Value> {
    serde_json::from_str(value).unwrap_or_default()
}

fn stage_name_for(career_model: &career::Model, current_stage: i32) -> (String, Option<String>) {
    for stage in parse_stages(&career_model.stages) {
        if stage.get("level").and_then(Value::as_i64) == Some(current_stage as i64) {
            let stage_name = stage
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("第{}阶段", current_stage));
            let stage_description = stage
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_owned);
            return (stage_name, stage_description);
        }
    }

    (format!("第{}阶段", current_stage), None)
}

fn career_to_legacy_json(career_model: &career::Model) -> Value {
    json!({
        "id": career_model.id,
        "project_id": career_model.project_id,
        "name": career_model.name,
        "type": career_model.career_type,
        "description": career_model.description,
        "category": career_model.category,
        "stages": json_or_string(&career_model.stages),
        "max_stage": career_model.max_stage,
        "requirements": career_model.requirements,
        "special_abilities": career_model.special_abilities,
        "worldview_rules": career_model.worldview_rules,
        "attribute_bonuses": career_model.attribute_bonuses.as_deref().map(json_or_string),
        "source": career_model.source,
        "created_at": career_model.created_at,
        "updated_at": career_model.updated_at,
    })
}

fn to_character_career_detail(
    relation: &character_career::Model,
    career_model: &career::Model,
) -> CharacterCareerDetail {
    let (stage_name, stage_description) = stage_name_for(career_model, relation.current_stage);
    CharacterCareerDetail {
        id: relation.id.clone(),
        character_id: relation.character_id.clone(),
        career_id: relation.career_id.clone(),
        career_name: career_model.name.clone(),
        career_type: relation.career_type.clone(),
        current_stage: relation.current_stage,
        stage_name,
        stage_description,
        stage_progress: relation.stage_progress.unwrap_or(0),
        max_stage: career_model.max_stage,
        started_at: relation.started_at.clone(),
        reached_current_stage_at: relation.reached_current_stage_at.clone(),
        notes: relation.notes.clone(),
        created_at: relation.created_at,
        updated_at: relation.updated_at.unwrap_or(relation.created_at),
    }
}

fn internal_error<E: ToString>(error: E) -> ApiError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"success": false, "message": error.to_string()})),
    )
}

fn not_found(message: &str) -> ApiError {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"success": false, "message": message})),
    )
}

fn bad_request(message: impl Into<String>) -> ApiError {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"success": false, "message": message.into()})),
    )
}

async fn verify_project_access(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<bool, ApiError> {
    let exists = project::Entity::find()
        .filter(project::Column::Id.eq(project_id))
        .filter(project::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(internal_error)?;
    Ok(exists.is_some())
}

async fn load_character_for_access(
    db: &DatabaseConnection,
    character_id: &str,
    user_id: &str,
) -> Result<character::Model, ApiError> {
    let character_model = character::Entity::find_by_id(character_id)
        .one(db)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("角色不存在"))?;

    let has_access = verify_project_access(db, &character_model.project_id, user_id).await?;
    if !has_access {
        return Err(not_found("角色不存在或无权限"));
    }

    Ok(character_model)
}

async fn load_career_for_project(
    db: &DatabaseConnection,
    project_id: &str,
    career_id: &str,
) -> Result<career::Model, ApiError> {
    career::Entity::find()
        .filter(career::Column::Id.eq(career_id))
        .filter(career::Column::ProjectId.eq(project_id))
        .one(db)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("职业不存在"))
}

async fn create_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let Some(career_type) = body.career_type.as_deref().or(body.career_kind.as_deref()) else {
        return Err(bad_request("career type is required"));
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
        Ok(Some(career_model)) => Ok((
            StatusCode::CREATED,
            Json(career_to_legacy_json(&career_model)),
        )),
        Ok(None) => Err(not_found("项目不存在或无权限")),
        Err(error) => Err(internal_error(error)),
    }
}

async fn list_careers(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    match CareerService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(careers)) => {
            let total = careers.len();
            let items: Vec<Value> = careers.iter().map(career_to_legacy_json).collect();
            let mut main_careers = Vec::new();
            let mut sub_careers = Vec::new();
            for career_model in &careers {
                let career_json = career_to_legacy_json(career_model);
                if career_model.career_type == "main" {
                    main_careers.push(career_json);
                } else {
                    sub_careers.push(career_json);
                }
            }
            Ok(Json(json!({
                "success": true,
                "data": {
                    "main_careers": main_careers,
                    "sub_careers": sub_careers,
                    "mainCareers": main_careers,
                    "subCareers": sub_careers,
                },
                "total": total,
                "items": items,
                "main_careers": main_careers,
                "sub_careers": sub_careers,
                "mainCareers": main_careers,
                "subCareers": sub_careers,
            })))
        }
        Ok(None) => Err(not_found("项目不存在或无权限")),
        Err(error) => Err(internal_error(error)),
    }
}

async fn get_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(career_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    match CareerService::get(&db, &career_id, &claims.sub).await {
        Ok(Some(career_model)) => Ok(Json(career_to_legacy_json(&career_model))),
        Ok(None) => Err(not_found("职业不存在或无权限")),
        Err(error) => Err(internal_error(error)),
    }
}

async fn update_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(career_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, ApiError> {
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
        Ok(Some(career_model)) => Ok(Json(career_to_legacy_json(&career_model))),
        Ok(None) => Err(not_found("职业不存在或无权限")),
        Err(error) => Err(internal_error(error)),
    }
}

async fn delete_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(career_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    match CareerService::delete(&db, &career_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "职业已删除"}))),
        Ok(None) => Err(not_found("职业不存在或无权限")),
        Err(error) => Err(internal_error(error)),
    }
}

async fn get_character_careers(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(character_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let character_model = load_character_for_access(&db, &character_id, &claims.sub).await?;

    let relations = character_career::Entity::find()
        .filter(character_career::Column::CharacterId.eq(&character_id))
        .order_by_desc(character_career::Column::CareerType)
        .all(&db)
        .await
        .map_err(internal_error)?;

    let mut main_career = None;
    let mut sub_careers = Vec::new();

    for relation in relations {
        let career_model =
            load_career_for_project(&db, &character_model.project_id, &relation.career_id).await?;
        let detail = to_character_career_detail(&relation, &career_model);
        if relation.career_type == "main" {
            main_career = Some(detail);
        } else {
            sub_careers.push(detail);
        }
    }

    Ok(Json(json!({
        "success": true,
        "data": {
            "main_career": main_career,
            "sub_careers": sub_careers,
        },
        "main_career": main_career,
        "sub_careers": sub_careers,
        "items": sub_careers,
        "total": sub_careers.len(),
    })))
}

async fn set_main_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(character_id): Path<String>,
    Json(body): Json<CareerAssignmentRequest>,
) -> Result<Json<Value>, ApiError> {
    let character_model = load_character_for_access(&db, &character_id, &claims.sub).await?;
    let career_model =
        load_career_for_project(&db, &character_model.project_id, &body.career_id).await?;

    if career_model.career_type != "main" {
        return Err(bad_request("该职业不是主职业类型，无法设置为主职业"));
    }

    let current_stage = body.current_stage.unwrap_or(1);
    if current_stage > career_model.max_stage {
        return Err(bad_request(format!(
            "阶段超出范围，该职业最大阶段为{}",
            career_model.max_stage
        )));
    }

    if let Some(existing) = character_career::Entity::find()
        .filter(character_career::Column::CharacterId.eq(&character_id))
        .filter(character_career::Column::CareerType.eq("main"))
        .one(&db)
        .await
        .map_err(internal_error)?
    {
        character_career::Entity::delete_by_id(existing.id)
            .exec(&db)
            .await
            .map_err(internal_error)?;
    }

    let now = Utc::now().naive_utc();
    character_career::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        character_id: Set(character_id),
        career_id: Set(body.career_id),
        career_type: Set("main".to_string()),
        current_stage: Set(current_stage),
        stage_progress: Set(Some(0)),
        started_at: Set(body.started_at.clone()),
        reached_current_stage_at: Set(body.started_at),
        notes: Set(None),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(&db)
    .await
    .map_err(internal_error)?;

    Ok(Json(
        json!({"message": "主职业设置成功", "career_name": career_model.name}),
    ))
}

async fn add_sub_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(character_id): Path<String>,
    Json(body): Json<CareerAssignmentRequest>,
) -> Result<Json<Value>, ApiError> {
    let character_model = load_character_for_access(&db, &character_id, &claims.sub).await?;
    let career_model =
        load_career_for_project(&db, &character_model.project_id, &body.career_id).await?;

    if career_model.career_type != "sub" {
        return Err(bad_request("该职业不是副职业类型，无法添加为副职业"));
    }

    let current_stage = body.current_stage.unwrap_or(1);
    if current_stage > career_model.max_stage {
        return Err(bad_request(format!(
            "阶段超出范围，该职业最大阶段为{}",
            career_model.max_stage
        )));
    }

    let existing = character_career::Entity::find()
        .filter(character_career::Column::CharacterId.eq(&character_id))
        .filter(character_career::Column::CareerId.eq(&body.career_id))
        .one(&db)
        .await
        .map_err(internal_error)?;
    if existing.is_some() {
        return Err(bad_request("该角色已拥有此副职业"));
    }

    let sub_count = character_career::Entity::find()
        .filter(character_career::Column::CharacterId.eq(&character_id))
        .filter(character_career::Column::CareerType.eq("sub"))
        .count(&db)
        .await
        .map_err(internal_error)?;
    if sub_count >= 5 {
        return Err(bad_request("副职业数量已达上限（最多5个）"));
    }

    let now = Utc::now().naive_utc();
    character_career::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        character_id: Set(character_id),
        career_id: Set(body.career_id),
        career_type: Set("sub".to_string()),
        current_stage: Set(current_stage),
        stage_progress: Set(Some(0)),
        started_at: Set(body.started_at.clone()),
        reached_current_stage_at: Set(body.started_at),
        notes: Set(None),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(&db)
    .await
    .map_err(internal_error)?;

    Ok(Json(
        json!({"message": "副职业添加成功", "career_name": career_model.name}),
    ))
}

async fn update_career_stage(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path((character_id, career_id)): Path<(String, String)>,
    Json(body): Json<CareerStageUpdateRequest>,
) -> Result<Json<Value>, ApiError> {
    let relation = character_career::Entity::find()
        .filter(character_career::Column::CharacterId.eq(&character_id))
        .filter(character_career::Column::CareerId.eq(&career_id))
        .one(&db)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("角色职业关联不存在"))?;

    let character_model = load_character_for_access(&db, &character_id, &claims.sub).await?;
    let career_model =
        load_career_for_project(&db, &character_model.project_id, &career_id).await?;

    if body.current_stage > career_model.max_stage {
        return Err(bad_request(format!(
            "阶段超出范围，该职业最大阶段为{}",
            career_model.max_stage
        )));
    }

    let mut active: character_career::ActiveModel = relation.into();
    active.current_stage = Set(body.current_stage);
    active.stage_progress = Set(Some(body.stage_progress.unwrap_or(0)));
    if let Some(value) = body.reached_current_stage_at {
        active.reached_current_stage_at = Set(Some(value));
    }
    if let Some(value) = body.notes {
        active.notes = Set(Some(value));
    }
    active.updated_at = Set(Some(Utc::now().naive_utc()));
    active.update(&db).await.map_err(internal_error)?;

    Ok(Json(json!({
        "message": "职业阶段更新成功",
        "career_name": career_model.name,
        "new_stage": body.current_stage,
    })))
}

async fn remove_sub_career(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path((character_id, career_id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let character_model = load_character_for_access(&db, &character_id, &claims.sub).await?;
    let career_model =
        load_career_for_project(&db, &character_model.project_id, &career_id).await?;

    let relation = character_career::Entity::find()
        .filter(character_career::Column::CharacterId.eq(&character_id))
        .filter(character_career::Column::CareerId.eq(&career_id))
        .one(&db)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("角色职业关联不存在"))?;

    if relation.career_type == "main" {
        return Err(bad_request("无法删除主职业，只能更换"));
    }

    character_career::Entity::delete_by_id(relation.id)
        .exec(&db)
        .await
        .map_err(internal_error)?;

    Ok(Json(
        json!({"message": "副职业删除成功", "career_name": career_model.name}),
    ))
}

async fn generate_career_system_legacy(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<LegacyCareerSystemQuery>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);

    let user_id = query.user_id.unwrap_or_else(|| claims.sub.clone());
    let project_id = query.project_id;
    let provider = query.provider;
    let model = query.model;
    let _main_career_count = query.main_career_count;
    let _sub_career_count = query.sub_career_count;
    let _enable_mcp = query.enable_mcp;

    tokio::spawn(async move {
        wizard_service::generate_career_system(
            &db,
            &channel,
            &user_id,
            &project_id,
            provider.as_deref(),
            model.as_deref(),
        )
        .await;
    });

    Sse::new(ReceiverStream::new(rx))
}

pub fn routes() -> Router {
    Router::new()
        .route("/careers", post(create_career).get(list_careers))
        .route(
            "/careers/generate-system",
            get(generate_career_system_legacy),
        )
        .route(
            "/careers/{career_id}",
            get(get_career).put(update_career).delete(delete_career),
        )
        .route(
            "/careers/character/{character_id}/careers",
            get(get_character_careers),
        )
        .route(
            "/careers/character/{character_id}/careers/main",
            post(set_main_career),
        )
        .route(
            "/careers/character/{character_id}/careers/sub",
            post(add_sub_career),
        )
        .route(
            "/careers/character/{character_id}/careers/{career_id}/stage",
            put(update_career_stage),
        )
        .route(
            "/careers/character/{character_id}/careers/{career_id}",
            delete(remove_sub_career),
        )
}
