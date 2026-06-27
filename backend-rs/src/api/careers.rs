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

use crate::api::wizard::resolve_effective_user_id;
use crate::models::{career, character, character_career, project};
use crate::services::auth::Claims;
use crate::services::career_service::CareerService;
use crate::services::wizard_service;

type ApiError = (StatusCode, Json<Value>);

const CAREERS_LIST_CREATE_ROUTE: &str = "/careers";
const CAREERS_GENERATE_SYSTEM_ROUTE: &str = "/careers/generate-system";
const CAREERS_DETAIL_ROUTE: &str = "/careers/{career_id}";
const CAREERS_CHARACTER_LIST_ROUTE: &str = "/careers/character/{character_id}/careers";
const CAREERS_CHARACTER_MAIN_ROUTE: &str = "/careers/character/{character_id}/careers/main";
const CAREERS_CHARACTER_SUB_ROUTE: &str = "/careers/character/{character_id}/careers/sub";
const CAREERS_CHARACTER_STAGE_ROUTE: &str =
    "/careers/character/{character_id}/careers/{career_id}/stage";
const CAREERS_CHARACTER_REMOVE_ROUTE: &str =
    "/careers/character/{character_id}/careers/{career_id}";

#[cfg(test)]
fn build_careers_route_owner_contract() -> Value {
    json!({
        "owner": "careers",
        "rust_owner": "backend-rs/src/api/careers.rs",
        "routes": {
            "list": CAREERS_LIST_CREATE_ROUTE,
            "create": CAREERS_LIST_CREATE_ROUTE,
            "generate_system": CAREERS_GENERATE_SYSTEM_ROUTE,
            "detail": CAREERS_DETAIL_ROUTE,
            "update": CAREERS_DETAIL_ROUTE,
            "delete": CAREERS_DETAIL_ROUTE,
            "character_list": CAREERS_CHARACTER_LIST_ROUTE,
            "set_main": CAREERS_CHARACTER_MAIN_ROUTE,
            "add_sub": CAREERS_CHARACTER_SUB_ROUTE,
            "update_stage": CAREERS_CHARACTER_STAGE_ROUTE,
            "remove_sub": CAREERS_CHARACTER_REMOVE_ROUTE
        },
        "methods": {
            "list": ["GET"],
            "create": ["POST"],
            "generate_system": ["GET"],
            "detail": ["GET", "PUT", "DELETE"],
            "character_list": ["GET"],
            "set_main": ["POST"],
            "add_sub": ["POST"],
            "update_stage": ["PUT"],
            "remove_sub": ["DELETE"]
        },
        "service_owners": [
            "backend-rs/src/services/career_service.rs",
            "backend-rs/src/api/careers.rs",
            "backend-rs/src/models/career.rs",
            "backend-rs/src/models/character_career.rs",
            "backend-rs/src/models/character.rs"
        ],
        "readiness_probes": [
            "careers-list-auth-guard-rust",
            "careers-generate-system-auth-guard-rust",
            "careers-business-project-create-rust",
            "careers-business-character-create-rust",
            "careers-configure-mock-openai-business-rust",
            "careers-create-main-business-rust",
            "careers-list-business-rust",
            "careers-detail-business-rust",
            "careers-update-business-rust",
            "careers-set-main-business-rust",
            "careers-character-list-after-main-business-rust",
            "careers-create-sub-business-rust",
            "careers-add-sub-business-rust",
            "careers-character-list-after-sub-business-rust",
            "careers-update-sub-stage-business-rust",
            "careers-remove-sub-business-rust",
            "careers-create-delete-target-business-rust",
            "careers-delete-business-rust",
            "careers-generate-system-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-careers-business-owner",
            "business_probes": [
                "careers-business-project-create-rust",
                "careers-business-character-create-rust",
                "careers-configure-mock-openai-business-rust",
                "careers-create-main-business-rust",
                "careers-list-business-rust",
                "careers-detail-business-rust",
                "careers-update-business-rust",
                "careers-set-main-business-rust",
                "careers-character-list-after-main-business-rust",
                "careers-create-sub-business-rust",
                "careers-add-sub-business-rust",
                "careers-character-list-after-sub-business-rust",
                "careers-update-sub-stage-business-rust",
                "careers-remove-sub-business-rust",
                "careers-create-delete-target-business-rust",
                "careers-delete-business-rust",
                "careers-generate-system-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "source_map_files": [
            "backend/migrator_app/models/career.py"
        ],
        "rollback_boundary": {
            "source_map_policy": "production_python_career_model_source_map_replaced_by_migrator_and_test_support_fixtures",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_route_files_status": "careers_route_source_map_deleted_remaining_career_model_only",
            "python_fallback_removal_ready": true,
            "remaining_blockers": [],
            "freeze_reason": "phase5-careers-business-owner covers project/character setup, mock OpenAI config, main/sub career CRUD, assignment, stage update, remove, delete, and generate-system probes with zero Python fallback probes; the Python careers route shell and schema file have been physically deleted, and the remaining career persistence source map has been narrowed to the dedicated career model file.",
            "rollback_files": []
        },
        "business_smoke_status": {
            "owner_profile": "phase5-careers-business-owner",
            "readiness_probe_count": 19,
            "business_probe_count": 17,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "explicit career model source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Careers route business smoke is covered by phase5-careers-business-owner; the Python careers route shell and schema file have been physically deleted, and final completion now requires explicit career model source-map freeze/delete/repoint approval with same-round rollback policy."
    })
}

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
#[allow(dead_code)]
pub(crate) struct CareerSystemRequest {
    #[serde(alias = "projectId")]
    pub(crate) project_id: String,
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    #[serde(alias = "userId")]
    pub(crate) user_id: Option<String>,
    #[serde(alias = "enableMcp")]
    pub(crate) enable_mcp: Option<bool>,
    #[serde(alias = "enableWebResearch")]
    pub(crate) enable_web_research: Option<bool>,
    #[serde(alias = "webResearchQuery")]
    pub(crate) web_research_query: Option<String>,
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

pub(crate) fn legacy_career_system_query_to_request(
    project_id: String,
    provider: Option<String>,
    model: Option<String>,
    user_id: Option<String>,
    enable_mcp: Option<bool>,
    enable_web_research: Option<bool>,
    web_research_query: Option<String>,
) -> CareerSystemRequest {
    CareerSystemRequest {
        project_id,
        provider,
        model,
        user_id,
        enable_mcp,
        enable_web_research,
        web_research_query,
    }
}

pub(crate) async fn execute_career_system_request(
    db: &DatabaseConnection,
    channel: &crate::utils::sse::SseChannel,
    user_id: &str,
    body: CareerSystemRequest,
) {
    wizard_service::generate_career_system(
        db,
        channel,
        user_id,
        &body.project_id,
        body.provider.as_deref(),
        body.model.as_deref(),
    )
    .await;
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

    let LegacyCareerSystemQuery {
        project_id,
        main_career_count,
        sub_career_count,
        enable_mcp,
        provider,
        model,
        user_id: request_user_id,
    } = query;

    let user_id = resolve_effective_user_id(request_user_id.clone(), &claims.sub);
    let request = legacy_career_system_query_to_request(
        project_id,
        provider,
        model,
        request_user_id,
        enable_mcp,
        None,
        None,
    );
    let _main_career_count = main_career_count;
    let _sub_career_count = sub_career_count;

    tokio::spawn(async move {
        execute_career_system_request(&db, &channel, &user_id, request).await;
    });

    Sse::new(ReceiverStream::new(rx))
}

pub fn routes() -> Router {
    Router::new()
        .route(
            CAREERS_LIST_CREATE_ROUTE,
            post(create_career).get(list_careers),
        )
        .route(
            CAREERS_GENERATE_SYSTEM_ROUTE,
            get(generate_career_system_legacy),
        )
        .route(
            CAREERS_DETAIL_ROUTE,
            get(get_career).put(update_career).delete(delete_career),
        )
        .route(CAREERS_CHARACTER_LIST_ROUTE, get(get_character_careers))
        .route(CAREERS_CHARACTER_MAIN_ROUTE, post(set_main_career))
        .route(CAREERS_CHARACTER_SUB_ROUTE, post(add_sub_career))
        .route(CAREERS_CHARACTER_STAGE_ROUTE, put(update_career_stage))
        .route(CAREERS_CHARACTER_REMOVE_ROUTE, delete(remove_sub_career))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_careers_route_owner_contract, legacy_career_system_query_to_request,
        CAREERS_CHARACTER_LIST_ROUTE, CAREERS_CHARACTER_MAIN_ROUTE, CAREERS_CHARACTER_REMOVE_ROUTE,
        CAREERS_CHARACTER_STAGE_ROUTE, CAREERS_CHARACTER_SUB_ROUTE, CAREERS_DETAIL_ROUTE,
        CAREERS_GENERATE_SYSTEM_ROUTE, CAREERS_LIST_CREATE_ROUTE,
    };

    #[test]
    fn should_publish_careers_route_owner_contract() {
        let contract = build_careers_route_owner_contract();

        assert_eq!(contract["owner"], "careers");
        assert_eq!(contract["rust_owner"], "backend-rs/src/api/careers.rs");
        assert_eq!(contract["routes"]["list"], CAREERS_LIST_CREATE_ROUTE);
        assert_eq!(
            contract["routes"]["generate_system"],
            CAREERS_GENERATE_SYSTEM_ROUTE
        );
        assert_eq!(contract["routes"]["detail"], CAREERS_DETAIL_ROUTE);
        assert_eq!(
            contract["routes"]["character_list"],
            CAREERS_CHARACTER_LIST_ROUTE
        );
        assert_eq!(contract["routes"]["set_main"], CAREERS_CHARACTER_MAIN_ROUTE);
        assert_eq!(contract["routes"]["add_sub"], CAREERS_CHARACTER_SUB_ROUTE);
        assert_eq!(
            contract["routes"]["update_stage"],
            CAREERS_CHARACTER_STAGE_ROUTE
        );
        assert_eq!(
            contract["routes"]["remove_sub"],
            CAREERS_CHARACTER_REMOVE_ROUTE
        );
        let readiness_probes = contract["readiness_probes"].as_array().unwrap();
        assert_eq!(readiness_probes.len(), 19);
        assert_eq!(
            readiness_probes.last().unwrap(),
            "careers-generate-system-business-rust"
        );
        assert_eq!(contract["source_map_files"].as_array().unwrap().len(), 1);
        assert_eq!(
            contract["source_map_files"][0],
            "backend/migrator_app/models/career.py"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-careers-business-owner"
        );
        let business_probes = contract["owner_profile"]["business_probes"]
            .as_array()
            .unwrap();
        assert_eq!(business_probes.len(), 17);
        assert!(business_probes
            .iter()
            .any(|probe| probe == "careers-update-sub-stage-business-rust"));
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["python_route_files_status"],
            "careers_route_source_map_deleted_remaining_career_model_only"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "production_python_career_model_source_map_replaced_by_migrator_and_test_support_fixtures"
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            19
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            17
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit career model source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert_eq!(
            contract["migration_policy"],
            "Careers route business smoke is covered by phase5-careers-business-owner; the Python careers route shell and schema file have been physically deleted, and final completion now requires explicit career model source-map freeze/delete/repoint approval with same-round rollback policy."
        );
        assert_eq!(contract["rollback_boundary"]["rollback_files"], json!([]));
    }

    #[test]
    fn should_keep_careers_route_group_paths_stable() {
        assert_eq!(CAREERS_LIST_CREATE_ROUTE, "/careers");
        assert_eq!(CAREERS_GENERATE_SYSTEM_ROUTE, "/careers/generate-system");
        assert_eq!(CAREERS_DETAIL_ROUTE, "/careers/{career_id}");
        assert_eq!(
            CAREERS_CHARACTER_LIST_ROUTE,
            "/careers/character/{character_id}/careers"
        );
        assert_eq!(
            CAREERS_CHARACTER_MAIN_ROUTE,
            "/careers/character/{character_id}/careers/main"
        );
        assert_eq!(
            CAREERS_CHARACTER_SUB_ROUTE,
            "/careers/character/{character_id}/careers/sub"
        );
        assert_eq!(
            CAREERS_CHARACTER_STAGE_ROUTE,
            "/careers/character/{character_id}/careers/{career_id}/stage"
        );
        assert_eq!(
            CAREERS_CHARACTER_REMOVE_ROUTE,
            "/careers/character/{character_id}/careers/{career_id}"
        );
    }

    #[test]
    fn legacy_career_system_query_adapter_preserves_existing_fields() {
        let request = legacy_career_system_query_to_request(
            "project-1".to_string(),
            Some("openai".to_string()),
            Some("gpt-4o-mini".to_string()),
            Some("user-1".to_string()),
            Some(true),
            None,
            None,
        );

        assert_eq!(request.project_id, "project-1");
        assert_eq!(request.provider.as_deref(), Some("openai"));
        assert_eq!(request.model.as_deref(), Some("gpt-4o-mini"));
        assert_eq!(request.user_id.as_deref(), Some("user-1"));
        assert_eq!(request.enable_mcp, Some(true));
    }
}
