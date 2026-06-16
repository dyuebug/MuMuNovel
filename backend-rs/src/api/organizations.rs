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
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;
use crate::services::wizard_service::clean_json_response;

const ORGANIZATIONS_LIST_CREATE_ROUTE: &str = "/organizations";
const ORGANIZATIONS_GENERATE_STREAM_ROUTE: &str = "/organizations/generate-stream";
const ORGANIZATIONS_PROJECT_LIST_ROUTE: &str = "/organizations/project/{project_id}";
const ORGANIZATIONS_MEMBER_DETAIL_ROUTE: &str = "/organizations/members/{member_id}";
const ORGANIZATIONS_MEMBERS_ROUTE: &str = "/organizations/{org_id}/members";
const ORGANIZATIONS_DETAIL_ROUTE: &str = "/organizations/{org_id}";

#[cfg(test)]
fn build_organizations_route_owner_contract() -> Value {
    json!({
        "owner": "organizations",
        "rust_owner": "backend-rs/src/api/organizations.rs",
        "routes": {
            "list": ORGANIZATIONS_LIST_CREATE_ROUTE,
            "create": ORGANIZATIONS_LIST_CREATE_ROUTE,
            "generate_stream": ORGANIZATIONS_GENERATE_STREAM_ROUTE,
            "project_list": ORGANIZATIONS_PROJECT_LIST_ROUTE,
            "member_detail": ORGANIZATIONS_MEMBER_DETAIL_ROUTE,
            "member_update": ORGANIZATIONS_MEMBER_DETAIL_ROUTE,
            "member_delete": ORGANIZATIONS_MEMBER_DETAIL_ROUTE,
            "members": ORGANIZATIONS_MEMBERS_ROUTE,
            "member_create": ORGANIZATIONS_MEMBERS_ROUTE,
            "detail": ORGANIZATIONS_DETAIL_ROUTE,
            "update": ORGANIZATIONS_DETAIL_ROUTE,
            "delete": ORGANIZATIONS_DETAIL_ROUTE
        },
        "methods": {
            "list": ["GET"],
            "create": ["POST"],
            "generate_stream": ["POST"],
            "project_list": ["GET"],
            "member_detail": ["GET", "PUT", "DELETE"],
            "members": ["GET", "POST"],
            "detail": ["GET", "PUT", "DELETE"]
        },
        "service_owners": [
            "backend-rs/src/api/organizations.rs",
            "backend-rs/src/api/background_tasks.rs",
            "backend-rs/src/models/organization.rs",
            "backend-rs/src/models/organization_member.rs",
            "backend-rs/src/models/character.rs"
        ],
        "readiness_probes": [
            "organizations-project-list-auth-guard-rust",
            "organizations-generate-stream-auth-guard-rust",
            "organizations-business-project-create-rust",
            "organizations-business-character-create-rust",
            "organizations-create-business-rust",
            "organizations-list-business-rust",
            "organizations-project-list-business-rust",
            "organizations-detail-business-rust",
            "organizations-update-business-rust",
            "organizations-business-member-character-create-rust",
            "organizations-member-add-business-rust",
            "organizations-members-list-business-rust",
            "organizations-member-update-business-rust",
            "organizations-detail-after-member-business-rust",
            "organizations-generate-configure-mock-openai-business-rust",
            "organizations-generate-stream-business-rust",
            "organizations-member-delete-business-rust",
            "organizations-delete-business-rust",
            "organizations-missing-detail-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-organizations-business-owner",
            "business_probes": [
                "organizations-business-project-create-rust",
                "organizations-business-character-create-rust",
                "organizations-create-business-rust",
                "organizations-list-business-rust",
                "organizations-project-list-business-rust",
                "organizations-detail-business-rust",
                "organizations-update-business-rust",
                "organizations-business-member-character-create-rust",
                "organizations-member-add-business-rust",
                "organizations-members-list-business-rust",
                "organizations-member-update-business-rust",
                "organizations-detail-after-member-business-rust",
                "organizations-generate-configure-mock-openai-business-rust",
                "organizations-generate-stream-business-rust",
                "organizations-member-delete-business-rust",
                "organizations-delete-business-rust",
                "organizations-missing-detail-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "source_map_files": [
            "backend/app/api/organizations.py",
            "backend/app/models/relationship.py",
            "backend/app/schemas/relationship.py",
            "backend/app/services/auto_organization_service.py"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_organizations_route_relationship_model_schema_auto_service_files_as_source_map_until_explicit_freeze_delete_round",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": false,
            "remaining_blockers": [
                "explicit source-map freeze/delete/repoint approval"
            ],
            "freeze_reason": "Rust organizations route group has dedicated phase5-organizations-business-owner probes for project/character setup, organization CRUD, project list, member add/list/update/delete, detail-after-member, generate-stream, delete, and missing-detail behavior; final Python source-map freeze/delete/repoint still requires explicit approval and rollback policy."
        },
        "business_smoke_status": {
            "owner_profile": "phase5-organizations-business-owner",
            "readiness_probe_count": 19,
            "business_probe_count": 17,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Organizations route business smoke is covered by phase5-organizations-business-owner; final completion now requires explicit source-map freeze/delete/repoint approval with same-round rollback policy."
    })
}

struct OrganizationService;

impl OrganizationService {
    async fn verify_project_access(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<bool, String> {
        let exists = project::Entity::find()
            .filter(project::Column::Id.eq(project_id))
            .filter(project::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|error| format!("{}", error))?;
        Ok(exists.is_some())
    }

    async fn create(
        db: &DatabaseConnection,
        project_id: &str,
        character_id: &str,
        user_id: &str,
        parent_org_id: Option<&str>,
        level: Option<i32>,
        power_level: Option<i32>,
        location: Option<&str>,
        motto: Option<&str>,
        color: Option<&str>,
    ) -> Result<Option<organization::Model>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        let now = Utc::now().naive_utc();
        let model = organization::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            character_id: Set(character_id.to_string()),
            project_id: Set(project_id.to_string()),
            parent_org_id: Set(parent_org_id.map(|value| value.to_string())),
            level: Set(level.unwrap_or(0)),
            power_level: Set(power_level.unwrap_or(50)),
            member_count: Set(0),
            location: Set(location.map(|value| value.to_string())),
            motto: Set(motto.map(|value| value.to_string())),
            color: Set(color.map(|value| value.to_string())),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model
            .insert(db)
            .await
            .map_err(|error| format!("{}", error))
            .map(Some)
    }

    async fn list(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<Vec<organization::Model>>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        organization::Entity::find()
            .filter(organization::Column::ProjectId.eq(project_id))
            .order_by_asc(organization::Column::CharacterId)
            .all(db)
            .await
            .map_err(|error| format!("{}", error))
            .map(Some)
    }

    async fn get(
        db: &DatabaseConnection,
        org_id: &str,
        user_id: &str,
    ) -> Result<Option<organization::Model>, String> {
        let organization = organization::Entity::find_by_id(org_id)
            .one(db)
            .await
            .map_err(|error| format!("{}", error))?;
        match organization {
            Some(ref org) => {
                if !Self::verify_project_access(db, &org.project_id, user_id).await? {
                    return Ok(None);
                }
                Ok(Some(org.clone()))
            }
            None => Ok(None),
        }
    }

    async fn update(
        db: &DatabaseConnection,
        org_id: &str,
        user_id: &str,
        parent_org_id: Option<&str>,
        level: Option<i32>,
        power_level: Option<i32>,
        location: Option<&str>,
        motto: Option<&str>,
        color: Option<&str>,
    ) -> Result<Option<organization::Model>, String> {
        let existing = Self::get(db, org_id, user_id).await?;
        let Some(model) = existing else {
            return Ok(None);
        };
        let mut active: organization::ActiveModel = model.into();
        if let Some(value) = parent_org_id {
            active.parent_org_id = Set(Some(value.to_string()));
        }
        if let Some(value) = level {
            active.level = Set(value);
        }
        if let Some(value) = power_level {
            active.power_level = Set(value);
        }
        if let Some(value) = location {
            active.location = Set(Some(value.to_string()));
        }
        if let Some(value) = motto {
            active.motto = Set(Some(value.to_string()));
        }
        if let Some(value) = color {
            active.color = Set(Some(value.to_string()));
        }
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active
            .update(db)
            .await
            .map_err(|error| format!("{}", error))
            .map(Some)
    }

    async fn delete(
        db: &DatabaseConnection,
        org_id: &str,
        user_id: &str,
    ) -> Result<Option<()>, String> {
        let existing = Self::get(db, org_id, user_id).await?;
        if existing.is_none() {
            return Ok(None);
        }
        organization::Entity::delete_by_id(org_id)
            .exec(db)
            .await
            .map_err(|error| format!("{}", error))?;
        Ok(Some(()))
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GenerateOrganizationTaskError {
    ProjectNotFoundOrAccessDenied,
    BadRequest(String),
    BadGateway(String),
    Internal(String),
}

impl std::fmt::Display for GenerateOrganizationTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectNotFoundOrAccessDenied => write!(f, "项目不存在或无权限"),
            Self::BadRequest(message) => write!(f, "{}", message),
            Self::BadGateway(message) => write!(f, "{}", message),
            Self::Internal(message) => write!(f, "{}", message),
        }
    }
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

    if let Some(template) =
        PromptTemplateService::find_user_template(db, user_id, "SINGLE_ORGANIZATION_GENERATION")
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
) -> Result<Value, GenerateOrganizationTaskError> {
    let project_model = load_generate_project(db, &body.project_id, user_id)
        .await
        .map_err(GenerateOrganizationTaskError::Internal)?
        .ok_or(GenerateOrganizationTaskError::ProjectNotFoundOrAccessDenied)?;

    let prompt_template = load_organization_prompt_template(db, user_id)
        .await
        .map_err(GenerateOrganizationTaskError::Internal)?;
    let project_context = build_organization_generation_context(db, &project_model)
        .await
        .map_err(GenerateOrganizationTaskError::Internal)?;
    let user_input = build_organization_generation_user_input(&body);

    let mut params = HashMap::new();
    params.insert("project_context".to_string(), project_context);
    params.insert("user_input".to_string(), user_input);
    let prompt = PromptTemplateService::format_prompt(&prompt_template, &params)
        .map_err(GenerateOrganizationTaskError::Internal)?;

    let ai_config = SettingsService::build_ai_config(
        db,
        user_id,
        body.provider.as_deref(),
        body.model.as_deref(),
        None,
    )
    .await
    .map_err(GenerateOrganizationTaskError::BadRequest)?;
    let used_model = ai_config.model.clone();
    let ai_service = AIService::new(ai_config);
    let response = ai_service
        .generate_text(&prompt, None, None)
        .await
        .map_err(GenerateOrganizationTaskError::BadGateway)?;

    let cleaned = clean_json_response(&response.content);
    let ai_payload = serde_json::from_str::<Value>(&cleaned).map_err(|error| {
        GenerateOrganizationTaskError::BadGateway(format!("组织生成结果不是有效 JSON: {}", error))
    })?;

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
    .map_err(|error| GenerateOrganizationTaskError::Internal(error.to_string()))?;

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
    .map_err(|error| GenerateOrganizationTaskError::Internal(error.to_string()))?;

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
    .map_err(|error| GenerateOrganizationTaskError::Internal(error.to_string()))?;

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
                channel.error(&error.to_string(), 500).await;
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
        .route(
            ORGANIZATIONS_LIST_CREATE_ROUTE,
            post(create_org).get(list_orgs),
        )
        .route(
            ORGANIZATIONS_GENERATE_STREAM_ROUTE,
            post(generate_org_stream_legacy),
        )
        .route(ORGANIZATIONS_PROJECT_LIST_ROUTE, get(list_project_orgs))
        .route(
            ORGANIZATIONS_MEMBER_DETAIL_ROUTE,
            get(|| async { StatusCode::METHOD_NOT_ALLOWED })
                .put(update_member)
                .delete(delete_member),
        )
        .route(
            ORGANIZATIONS_MEMBERS_ROUTE,
            get(list_members).post(add_member),
        )
        .route(
            ORGANIZATIONS_DETAIL_ROUTE,
            get(get_org).put(update_org).delete(delete_org),
        )
}

#[cfg(test)]
mod tests {
    use super::{
        build_organizations_route_owner_contract, GenerateOrganizationTaskError,
        ORGANIZATIONS_DETAIL_ROUTE, ORGANIZATIONS_GENERATE_STREAM_ROUTE,
        ORGANIZATIONS_LIST_CREATE_ROUTE, ORGANIZATIONS_MEMBERS_ROUTE,
        ORGANIZATIONS_MEMBER_DETAIL_ROUTE, ORGANIZATIONS_PROJECT_LIST_ROUTE,
    };

    #[test]
    fn organization_task_error_display_keeps_project_access_message() {
        assert_eq!(
            GenerateOrganizationTaskError::ProjectNotFoundOrAccessDenied.to_string(),
            "项目不存在或无权限"
        );
    }

    #[test]
    fn organization_task_error_display_keeps_bad_gateway_message() {
        assert_eq!(
            GenerateOrganizationTaskError::BadGateway(
                "组织生成结果不是有效 JSON: boom".to_string()
            )
            .to_string(),
            "组织生成结果不是有效 JSON: boom"
        );
    }

    #[test]
    fn should_publish_organizations_route_owner_contract() {
        let contract = build_organizations_route_owner_contract();

        assert_eq!(contract["owner"], "organizations");
        assert_eq!(
            contract["rust_owner"],
            "backend-rs/src/api/organizations.rs"
        );
        assert_eq!(contract["routes"]["list"], ORGANIZATIONS_LIST_CREATE_ROUTE);
        assert_eq!(
            contract["routes"]["generate_stream"],
            ORGANIZATIONS_GENERATE_STREAM_ROUTE
        );
        assert_eq!(
            contract["routes"]["project_list"],
            ORGANIZATIONS_PROJECT_LIST_ROUTE
        );
        assert_eq!(
            contract["routes"]["member_detail"],
            ORGANIZATIONS_MEMBER_DETAIL_ROUTE
        );
        assert_eq!(contract["routes"]["members"], ORGANIZATIONS_MEMBERS_ROUTE);
        assert_eq!(contract["routes"]["detail"], ORGANIZATIONS_DETAIL_ROUTE);
        assert_eq!(contract["readiness_probes"].as_array().unwrap().len(), 19);
        assert_eq!(
            contract["readiness_probes"][18],
            "organizations-missing-detail-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-organizations-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"]
                .as_array()
                .expect("business probes should be present")
                .len(),
            17
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"][13],
            "organizations-generate-stream-business-rust"
        );
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(contract["source_map_files"].as_array().unwrap().len(), 4);
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            false
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
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
            "explicit source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .expect("organizations migration policy should be present")
            .contains("phase5-organizations-business-owner"));
    }

    #[test]
    fn should_keep_organizations_route_group_paths_stable() {
        assert_eq!(ORGANIZATIONS_LIST_CREATE_ROUTE, "/organizations");
        assert_eq!(
            ORGANIZATIONS_GENERATE_STREAM_ROUTE,
            "/organizations/generate-stream"
        );
        assert_eq!(
            ORGANIZATIONS_PROJECT_LIST_ROUTE,
            "/organizations/project/{project_id}"
        );
        assert_eq!(
            ORGANIZATIONS_MEMBER_DETAIL_ROUTE,
            "/organizations/members/{member_id}"
        );
        assert_eq!(
            ORGANIZATIONS_MEMBERS_ROUTE,
            "/organizations/{org_id}/members"
        );
        assert_eq!(ORGANIZATIONS_DETAIL_ROUTE, "/organizations/{org_id}");
    }
}
