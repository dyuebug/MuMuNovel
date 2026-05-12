use std::collections::HashMap;

use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::{sse::Event, Json, Sse},
    routing::{get, post},
    Router,
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::ai::service::AIService;
use crate::models::{character, generation_history, organization, organization_member, project};
use crate::services::auth::Claims;
use crate::services::organization_service::OrganizationService;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;
use crate::services::wizard_service::clean_json_response;

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    character_id: String,
    parent_org_id: Option<String>,
    level: Option<i32>,
    power_level: Option<i32>,
    location: Option<String>,
    motto: Option<String>,
    color: Option<String>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    parent_org_id: Option<String>,
    level: Option<i32>,
    power_level: Option<i32>,
    location: Option<String>,
    motto: Option<String>,
    color: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

#[derive(Deserialize)]
struct MemberCreateRequest {
    character_id: String,
    position: String,
    rank: Option<i32>,
    status: Option<String>,
    joined_at: Option<String>,
    left_at: Option<String>,
    loyalty: Option<i32>,
    contribution: Option<i32>,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct MemberUpdateRequest {
    position: Option<String>,
    rank: Option<i32>,
    status: Option<String>,
    joined_at: Option<String>,
    left_at: Option<String>,
    loyalty: Option<i32>,
    contribution: Option<i32>,
    notes: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct GenerateOrganizationRequest {
    project_id: String,
    name: Option<String>,
    organization_type: Option<String>,
    background: Option<String>,
    requirements: Option<String>,
    provider: Option<String>,
    model: Option<String>,
}

fn normalized_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn value_text(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => normalized_string(Some(text)),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Array(items)) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| match item {
                    Value::String(text) => normalized_string(Some(text)),
                    Value::Number(number) => Some(number.to_string()),
                    _ => None,
                })
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("、"))
            }
        }
        _ => None,
    }
}

async fn load_organization_prompt_template(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<String, String> {
    let _ = PromptTemplateService::sync_managed_templates_for_user(db, user_id).await;

    if let Some(template) = PromptTemplateService::find_user_template(
        db,
        user_id,
        "SINGLE_ORGANIZATION_GENERATION",
    )
    .await?
    {
        if template.is_active {
            let content = template.template_content.trim();
            if !content.is_empty() {
                return Ok(content.to_string());
            }
        }
    }

    PromptTemplateService::system_template_info("SINGLE_ORGANIZATION_GENERATION")
        .map(|template| template.content.clone())
        .ok_or_else(|| "缺少组织生成提示词模板".to_string())
}

async fn load_generate_project(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Option<project::Model>, String> {
    project::Entity::find()
        .filter(project::Column::Id.eq(project_id))
        .filter(project::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(|error| error.to_string())
}

async fn build_organization_generation_context(
    db: &DatabaseConnection,
    project_model: &project::Model,
) -> Result<String, String> {
    let existing_characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(&project_model.id))
        .order_by_desc(character::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let mut existing_info = String::new();
    let mut character_list = Vec::new();
    let mut organization_list = Vec::new();

    for item in existing_characters.iter().take(10) {
        if item.is_organization {
            organization_list.push(format!(
                "- {} [{}]",
                item.name,
                item.organization_type
                    .clone()
                    .unwrap_or_else(|| "组织".to_string())
            ));
        } else {
            character_list.push(format!(
                "- {}（{}）",
                item.name,
                item.role_type.clone().unwrap_or_else(|| "未知".to_string())
            ));
        }
    }

    if !character_list.is_empty() {
        existing_info.push_str("\n已有角色：\n");
        existing_info.push_str(&character_list.join("\n"));
    }
    if !organization_list.is_empty() {
        existing_info.push_str("\n\n已有组织：\n");
        existing_info.push_str(&organization_list.join("\n"));
    }

    Ok(format!(
        "项目信息：\n- 书名：{}\n- 主题：{}\n- 类型：{}\n- 时间背景：{}\n- 地理位置：{}\n- 氛围基调：{}\n- 世界规则：{}\n{}",
        project_model.title,
        project_model.theme.as_deref().unwrap_or("未设定"),
        project_model.genre.as_deref().unwrap_or("未设定"),
        project_model.world_time_period.as_deref().unwrap_or("未设定"),
        project_model.world_location.as_deref().unwrap_or("未设定"),
        project_model.world_atmosphere.as_deref().unwrap_or("未设定"),
        project_model.world_rules.as_deref().unwrap_or("未设定"),
        existing_info
    ))
}

fn build_organization_generation_user_input(body: &GenerateOrganizationRequest) -> String {
    format!(
        "用户要求：\n- 组织名称：{}\n- 组织类型：{}\n- 背景设定：{}\n- 其他要求：{}",
        body.name.as_deref().unwrap_or("请AI生成"),
        body.organization_type
            .as_deref()
            .unwrap_or("请AI根据世界观决定"),
        body.background.as_deref().unwrap_or("无特殊要求"),
        body.requirements.as_deref().unwrap_or("无"),
    )
}

pub(crate) async fn generate_organization_task(
    db: &DatabaseConnection,
    user_id: &str,
    body: GenerateOrganizationRequest,
) -> Result<Value, String> {
    let project_model = load_generate_project(db, &body.project_id, user_id)
        .await?
        .ok_or_else(|| "项目不存在或无权限".to_string())?;

    let prompt_template = load_organization_prompt_template(db, user_id).await?;
    let project_context = build_organization_generation_context(db, &project_model).await?;
    let user_input = build_organization_generation_user_input(&body);

    let mut params = HashMap::new();
    params.insert("project_context".to_string(), project_context);
    params.insert("user_input".to_string(), user_input);
    let prompt = PromptTemplateService::format_prompt(&prompt_template, &params)?;

    let ai_config = SettingsService::build_ai_config(
        db,
        user_id,
        body.provider.as_deref(),
        body.model.as_deref(),
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    let used_model = ai_config.model.clone();
    let ai_service = AIService::new(ai_config);
    let response = ai_service
        .generate_text(&prompt, None, None)
        .await
        .map_err(|error| error.to_string())?;

    let cleaned = clean_json_response(&response.content);
    let ai_payload = serde_json::from_str::<Value>(&cleaned)
        .map_err(|error| format!("组织生成结果不是有效 JSON: {}", error))?;

    let name = value_text(ai_payload.get("name"))
        .or_else(|| normalized_string(body.name.as_deref()))
        .unwrap_or_else(|| "未命名组织".to_string());
    let organization_type = value_text(ai_payload.get("organization_type"))
        .or_else(|| normalized_string(body.organization_type.as_deref()))
        .or_else(|| Some("组织".to_string()));
    let personality = value_text(ai_payload.get("personality"));
    let background = value_text(ai_payload.get("background"))
        .or_else(|| normalized_string(body.background.as_deref()));
    let appearance = value_text(ai_payload.get("appearance"));
    let organization_purpose = value_text(ai_payload.get("organization_purpose"));
    let traits = ai_payload
        .get("traits")
        .cloned()
        .unwrap_or_else(|| json!([]))
        .to_string();
    let organization_members = ai_payload
        .get("organization_members")
        .cloned()
        .unwrap_or_else(|| json!([]))
        .to_string();
    let power_level = ai_payload
        .get("power_level")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(50);
    let location = value_text(ai_payload.get("location"));
    let motto = value_text(ai_payload.get("motto"));
    let color = value_text(ai_payload.get("color"));

    let now = Utc::now().naive_utc();
    let character_id = Uuid::new_v4().to_string();
    let organization_id = Uuid::new_v4().to_string();

    character::ActiveModel {
        id: Set(character_id.clone()),
        project_id: Set(project_model.id.clone()),
        name: Set(name.clone()),
        age: Set(None),
        gender: Set(None),
        is_organization: Set(true),
        role_type: Set(Some("supporting".to_string())),
        personality: Set(personality),
        background: Set(background),
        appearance: Set(appearance),
        relationships: Set(None),
        organization_type: Set(organization_type.clone()),
        organization_purpose: Set(organization_purpose),
        organization_members: Set(Some(organization_members)),
        status: Set("active".to_string()),
        status_changed_chapter: Set(None),
        current_state: Set(None),
        state_updated_chapter: Set(None),
        main_career_id: Set(None),
        main_career_stage: Set(None),
        sub_careers: Set(None),
        avatar_url: Set(None),
        traits: Set(Some(traits)),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .map_err(|error| error.to_string())?;

    organization::ActiveModel {
        id: Set(organization_id),
        character_id: Set(character_id.clone()),
        project_id: Set(project_model.id.clone()),
        parent_org_id: Set(None),
        level: Set(0),
        power_level: Set(power_level),
        member_count: Set(0),
        location: Set(location),
        motto: Set(motto),
        color: Set(color),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .map_err(|error| error.to_string())?;

    generation_history::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(project_model.id),
        chapter_id: Set(None),
        prompt: Set(Some(prompt)),
        generated_content: Set(Some(response.content)),
        model: Set(Some(used_model)),
        tokens_used: Set(None),
        generation_time: Set(None),
        created_at: Set(Some(now)),
    }
    .insert(db)
    .await
    .map_err(|error| error.to_string())?;

    Ok(json!({
        "character": {
            "id": character_id,
            "name": name,
            "organization_type": organization_type,
            "is_organization": true
        }
    }))
}

async fn generate_org_stream_legacy(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<GenerateOrganizationRequest>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let channel = crate::utils::sse::SseChannel::new(tx);
    let user_id = claims.sub;

    tokio::spawn(async move {
        channel.progress("开始生成组织...", 0, "processing").await;
        channel.progress("生成组织中...", 35, "processing").await;
        match generate_organization_task(&db, &user_id, body).await {
            Ok(result) => {
                channel.progress("保存组织数据...", 90, "processing").await;
                channel.result(&result).await;
                channel.progress("组织生成完成!", 100, "success").await;
                channel.done().await;
            }
            Err(error) => {
                channel.error(&error, 500).await;
                channel.done().await;
            }
        }
    });

    Sse::new(ReceiverStream::new(rx))
}

async fn verify_project_access(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<bool, String> {
    project::Entity::find()
        .filter(project::Column::Id.eq(project_id))
        .filter(project::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map(|project| project.is_some())
        .map_err(|e| e.to_string())
}

fn forbidden_or_missing(message: &str) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"success": false, "message": message})),
    )
}

fn server_error(error: String) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"success": false, "message": error})),
    )
}

fn org_detail_json(org: &organization::Model, char_model: Option<&character::Model>) -> Value {
    let name = char_model
        .map(|model| model.name.clone())
        .unwrap_or_else(|| format!("未关联组织角色 ({})", org.id));
    let organization_type = char_model
        .and_then(|model| model.organization_type.clone())
        .unwrap_or_else(|| "未设置".to_string());
    let purpose = char_model
        .and_then(|model| model.organization_purpose.clone())
        .unwrap_or_default();

    json!({
        "id": org.id,
        "character_id": org.character_id,
        "name": name,
        "type": organization_type,
        "organization_type": organization_type,
        "purpose": purpose,
        "organization_purpose": purpose,
        "member_count": org.member_count,
        "power_level": org.power_level,
        "location": org.location,
        "motto": org.motto,
        "color": org.color,
    })
}

fn member_detail_json(
    member: &organization_member::Model,
    char_model: Option<&character::Model>,
) -> Value {
    let character_name = char_model
        .map(|model| model.name.clone())
        .unwrap_or_else(|| format!("未关联角色 ({})", member.character_id));

    json!({
        "id": member.id,
        "character_id": member.character_id,
        "character_name": character_name,
        "position": member.position,
        "rank": member.rank,
        "loyalty": member.loyalty,
        "contribution": member.contribution,
        "status": member.status,
        "joined_at": member.joined_at,
        "left_at": member.left_at,
        "notes": member.notes,
        "character": char_model.map(|model| json!({
            "id": model.id,
            "name": model.name,
            "organization_type": model.organization_type,
            "organization_purpose": model.organization_purpose,
            "is_organization": model.is_organization,
        })),
    })
}

async fn ensure_project_organization_rows(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<(), (StatusCode, Json<Value>)> {
    let existing_orgs = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    let existing_character_ids: std::collections::HashSet<String> = existing_orgs
        .iter()
        .map(|org| org.character_id.clone())
        .collect();

    let organization_characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .filter(character::Column::IsOrganization.eq(true))
        .all(db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    for char_model in organization_characters {
        if existing_character_ids.contains(&char_model.id) {
            continue;
        }

        organization::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            character_id: Set(char_model.id.clone()),
            project_id: Set(project_id.to_string()),
            parent_org_id: Set(None),
            level: Set(0),
            power_level: Set(50),
            member_count: Set(0),
            location: Set(None),
            motto: Set(None),
            color: Set(None),
            created_at: Set(Utc::now().naive_utc()),
            updated_at: Set(Some(Utc::now().naive_utc())),
        }
        .insert(db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    }

    Ok(())
}

async fn create_org(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match OrganizationService::create(
        &db,
        &body.project_id,
        &body.character_id,
        &claims.sub,
        body.parent_org_id.as_deref(),
        body.level,
        body.power_level,
        body.location.as_deref(),
        body.motto.as_deref(),
        body.color.as_deref(),
    )
    .await
    {
        Ok(Some(org)) => Ok((StatusCode::CREATED, Json(json!(org)))),
        Ok(None) => Err(forbidden_or_missing("项目不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn list_orgs(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ensure_project_organization_rows(&db, &query.project_id).await?;
    match OrganizationService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(orgs)) => Ok(Json(json!(orgs))),
        Ok(None) => Err(forbidden_or_missing("项目不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn list_project_orgs(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !verify_project_access(&db, &project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("项目不存在或无权限"));
    }

    ensure_project_organization_rows(&db, &project_id).await?;

    let orgs = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(&project_id))
        .order_by_asc(organization::Column::Level)
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    let character_ids: Vec<String> = orgs.iter().map(|org| org.character_id.clone()).collect();
    let characters = character::Entity::find()
        .filter(character::Column::Id.is_in(character_ids))
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    let character_map: HashMap<String, character::Model> = characters
        .into_iter()
        .map(|char_model| (char_model.id.clone(), char_model))
        .collect();

    let payload: Vec<Value> = orgs
        .iter()
        .map(|org| org_detail_json(org, character_map.get(&org.character_id)))
        .collect();
    Ok(Json(json!(payload)))
}

async fn get_org(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OrganizationService::get(&db, &org_id, &claims.sub).await {
        Ok(Some(org)) => Ok(Json(json!(org))),
        Ok(None) => Err(forbidden_or_missing("组织不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn update_org(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OrganizationService::update(
        &db,
        &org_id,
        &claims.sub,
        body.parent_org_id.as_deref(),
        body.level,
        body.power_level,
        body.location.as_deref(),
        body.motto.as_deref(),
        body.color.as_deref(),
    )
    .await
    {
        Ok(Some(org)) => Ok(Json(json!(org))),
        Ok(None) => Err(forbidden_or_missing("组织不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn delete_org(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match OrganizationService::delete(&db, &org_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"message": "组织删除成功", "id": org_id}))),
        Ok(None) => Err(forbidden_or_missing("组织不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn get_member_org(
    db: &DatabaseConnection,
    member_id: &str,
) -> Result<Option<(organization_member::Model, organization::Model)>, String> {
    let Some(member) = organization_member::Entity::find_by_id(member_id)
        .one(db)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let org = organization::Entity::find_by_id(&member.organization_id)
        .one(db)
        .await
        .map_err(|e| e.to_string())?;
    Ok(org.map(|org| (member, org)))
}

async fn list_members(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some(org) = organization::Entity::find_by_id(&org_id)
        .one(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?
    else {
        return Err(forbidden_or_missing("组织不存在"));
    };
    if !verify_project_access(&db, &org.project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("组织不存在或无权限"));
    }

    let members = organization_member::Entity::find()
        .filter(organization_member::Column::OrganizationId.eq(&org_id))
        .order_by_desc(organization_member::Column::Rank)
        .order_by_asc(organization_member::Column::CreatedAt)
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    let character_ids: Vec<String> = members
        .iter()
        .map(|member| member.character_id.clone())
        .collect();
    let characters = character::Entity::find()
        .filter(character::Column::Id.is_in(character_ids))
        .all(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    let character_map: HashMap<String, character::Model> = characters
        .into_iter()
        .map(|char_model| (char_model.id.clone(), char_model))
        .collect();
    let payload: Vec<Value> = members
        .iter()
        .map(|member| member_detail_json(member, character_map.get(&member.character_id)))
        .collect();
    Ok(Json(json!(payload)))
}

async fn add_member(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(org_id): Path<String>,
    Json(body): Json<MemberCreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let Some(org) = organization::Entity::find_by_id(&org_id)
        .one(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?
    else {
        return Err(forbidden_or_missing("组织不存在"));
    };
    if !verify_project_access(&db, &org.project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("组织不存在或无权限"));
    }

    let Some(char_model) = character::Entity::find_by_id(&body.character_id)
        .one(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "角色不存在"})),
        ));
    };
    if char_model.is_organization {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "不能将组织添加为成员"})),
        ));
    }

    let duplicate = organization_member::Entity::find()
        .filter(organization_member::Column::OrganizationId.eq(&org_id))
        .filter(organization_member::Column::CharacterId.eq(&body.character_id))
        .one(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    if duplicate.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "该角色已在组织中"})),
        ));
    }

    let now = Utc::now().naive_utc();
    let active = organization_member::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        organization_id: Set(org_id.clone()),
        character_id: Set(body.character_id),
        position: Set(body.position),
        rank: Set(body.rank.unwrap_or(0)),
        status: Set(body.status.unwrap_or_else(|| "active".to_string())),
        joined_at: Set(body.joined_at),
        left_at: Set(body.left_at),
        loyalty: Set(body.loyalty.unwrap_or(50)),
        contribution: Set(body.contribution.unwrap_or(0)),
        source: Set("manual".to_string()),
        notes: Set(body.notes),
        created_at: Set(now),
        updated_at: Set(Some(now)),
    };
    let member = active
        .insert(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    let next_member_count = org.member_count + 1;
    let mut org_active: organization::ActiveModel = org.into();
    org_active.member_count = Set(next_member_count);
    org_active.updated_at = Set(Some(now));
    org_active
        .update(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(json!(member))))
}

async fn update_member(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(member_id): Path<String>,
    Json(body): Json<MemberUpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some((member, org)) = get_member_org(&db, &member_id)
        .await
        .map_err(server_error)?
    else {
        return Err(forbidden_or_missing("成员记录不存在"));
    };
    if !verify_project_access(&db, &org.project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("成员记录不存在或无权限"));
    }

    let mut active: organization_member::ActiveModel = member.into();
    if let Some(value) = body.position {
        active.position = Set(value);
    }
    if let Some(value) = body.rank {
        active.rank = Set(value);
    }
    if let Some(value) = body.status {
        active.status = Set(value);
    }
    if let Some(value) = body.joined_at {
        active.joined_at = Set(Some(value));
    }
    if let Some(value) = body.left_at {
        active.left_at = Set(Some(value));
    }
    if let Some(value) = body.loyalty {
        active.loyalty = Set(value);
    }
    if let Some(value) = body.contribution {
        active.contribution = Set(value);
    }
    if let Some(value) = body.notes {
        active.notes = Set(Some(value));
    }
    active.updated_at = Set(Some(Utc::now().naive_utc()));
    let updated = active
        .update(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    Ok(Json(json!(updated)))
}

async fn delete_member(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(member_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let Some((member, org)) = get_member_org(&db, &member_id)
        .await
        .map_err(server_error)?
    else {
        return Err(forbidden_or_missing("成员记录不存在"));
    };
    if !verify_project_access(&db, &org.project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("成员记录不存在或无权限"));
    }

    organization_member::Entity::delete_by_id(&member_id)
        .exec(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;
    let next_member_count = (org.member_count - 1).max(0);
    let mut org_active: organization::ActiveModel = org.into();
    org_active.member_count = Set(next_member_count);
    org_active.updated_at = Set(Some(Utc::now().naive_utc()));
    org_active
        .update(&db)
        .await
        .map_err(|e| server_error(e.to_string()))?;

    Ok(Json(json!({"message": "成员移除成功", "id": member.id})))
}

pub fn routes() -> Router {
    Router::new()
        .route("/organizations", post(create_org).get(list_orgs))
        .route("/organizations/generate-stream", post(generate_org_stream_legacy))
        .route(
            "/organizations/project/{project_id}",
            get(list_project_orgs),
        )
        .route(
            "/organizations/members/{member_id}",
            get(|| async { StatusCode::METHOD_NOT_ALLOWED })
                .put(update_member)
                .delete(delete_member),
        )
        .route(
            "/organizations/{org_id}/members",
            get(list_members).post(add_member),
        )
        .route(
            "/organizations/{org_id}",
            get(get_org).put(update_org).delete(delete_org),
        )
}
