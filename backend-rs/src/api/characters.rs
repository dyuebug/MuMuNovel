use std::collections::{HashMap, HashSet};

use axum::{
    extract::{Extension, Multipart, Path, Query},
    http::{header, StatusCode},
    response::{Json, Response},
    routing::{get, post},
    Router,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ai::service::AIService;
use crate::models::{
    career, character, character_career, organization, organization_member, project, relationship,
};
use crate::services::auth::Claims;
use crate::services::character_service::CharacterService;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;
use crate::services::wizard_service::clean_json_response;

const CHARACTERS_PROJECT_LIST_ROUTE: &str = "/characters/project/{project_id}";
const CHARACTERS_LIST_CREATE_ROUTE: &str = "/characters";
const CHARACTERS_GENERATE_ROUTE: &str = "/characters/generate";
const CHARACTERS_GENERATE_STREAM_ROUTE: &str = "/characters/generate-stream";
const CHARACTERS_DETAIL_ROUTE: &str = "/characters/{character_id}";
const CHARACTERS_VALIDATE_IMPORT_ROUTE: &str = "/characters/validate-import";
const CHARACTERS_EXPORT_ROUTE: &str = "/characters/export";
const CHARACTERS_IMPORT_ROUTE: &str = "/characters/import";

#[cfg(test)]
fn build_characters_route_owner_contract() -> Value {
    json!({
        "owner": "characters",
        "scope": "characters_crud_generation_import_export_validate_route_group",
        "python_source_map": [
            "backend/app/api/characters.py",
            "backend/app/models/character.py",
            "backend/app/schemas/character.py",
            "backend/app/services/auto_character_service.py",
            "backend/app/services/character_context_service.py",
            "backend/app/services/character_state_update_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/api/characters.rs",
            "backend-rs/src/services/character_service.rs",
            "backend-rs/src/services/prompt_template_service.rs",
            "backend-rs/src/services/settings_service.rs",
            "backend-rs/src/models/character.rs",
            "backend-rs/src/models/organization.rs",
            "backend-rs/src/models/organization_member.rs",
            "backend-rs/src/models/character_career.rs",
            "deploy/strangler-gateway-probes.json"
        ],
        "route_contract": {
            "project_list": CHARACTERS_PROJECT_LIST_ROUTE,
            "list": CHARACTERS_LIST_CREATE_ROUTE,
            "create": CHARACTERS_LIST_CREATE_ROUTE,
            "generate": CHARACTERS_GENERATE_ROUTE,
            "generate_stream": CHARACTERS_GENERATE_STREAM_ROUTE,
            "detail": CHARACTERS_DETAIL_ROUTE,
            "update": CHARACTERS_DETAIL_ROUTE,
            "delete": CHARACTERS_DETAIL_ROUTE,
            "validate_import": CHARACTERS_VALIDATE_IMPORT_ROUTE,
            "export": CHARACTERS_EXPORT_ROUTE,
            "import": CHARACTERS_IMPORT_ROUTE
        },
        "behavior_contract": {
            "route_entrypoints": [
                "list_characters_by_project",
                "list_characters",
                "create_character",
                "generate_character",
                "get_character",
                "update_character",
                "delete_character",
                "validate_characters_import",
                "export_characters",
                "import_characters"
            ],
            "service_consumers": [
                "CharacterService::create",
                "CharacterService::list",
                "CharacterService::get",
                "CharacterService::update",
                "CharacterService::delete",
                "PromptTemplateService::sync_managed_templates_for_user",
                "SettingsService::build_ai_config",
                "AIService::generate"
            ],
            "regular_readiness_scope": [
                "project_list",
                "list",
                "generate_stream",
                "export",
                "import"
            ],
            "public_asymmetric_scope": [
                "validate_import"
            ],
            "import_export_contract": {
                "export_type": "characters",
                "export_version": "rust-strangler-1",
                "import_requires_data_array": true,
                "validate_import_is_public_policy": true
            },
            "generation_contract": {
                "prompt_template_key": "SINGLE_CHARACTER_GENERATION",
                "cleans_ai_json_response": true,
                "syncs_main_and_sub_careers": true,
                "supports_organization_extra_fields": true
            }
        },
        "readiness_evidence": [
            "characters-validate-import-public-rust",
            "characters-project-list-auth-guard-rust",
            "characters-list-auth-guard-rust",
            "characters-generate-stream-auth-guard-rust",
            "characters-export-auth-guard-rust",
            "characters-import-auth-guard-rust",
            "characters-setup-project-business-rust",
            "characters-create-business-rust",
            "characters-list-business-rust",
            "characters-project-list-business-rust",
            "characters-detail-business-rust",
            "characters-update-business-rust",
            "characters-export-business-rust",
            "characters-validate-import-business-rust",
            "characters-import-business-rust",
            "characters-delete-business-rust",
            "characters-missing-detail-business-rust",
            "characters-missing-import-project-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-characters-business-owner",
            "business_probes": [
                "characters-setup-project-business-rust",
                "characters-create-business-rust",
                "characters-list-business-rust",
                "characters-project-list-business-rust",
                "characters-detail-business-rust",
                "characters-update-business-rust",
                "characters-export-business-rust",
                "characters-validate-import-business-rust",
                "characters-import-business-rust",
                "characters-delete-business-rust",
                "characters-missing-detail-business-rust",
                "characters-missing-import-project-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "validation_boundary": [
            "cargo test api::characters",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only --profile phase5-characters-business-owner",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_characters_route_model_schema_service_files_as_source_map_until_explicit_freeze_delete_round",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_route_files_status": "source_map_only_for_characters_route_group",
            "python_fallback_removal_ready": false,
            "remaining_blockers": [
                "explicit source-map freeze/delete/repoint approval"
            ],
            "retired_manifest_fallbacks": [
                "characters-project-list-auth-guard-python-fallback",
                "characters-list-auth-guard-python-fallback",
                "characters-generate-stream-auth-guard-python-fallback",
                "characters-export-auth-guard-python-fallback",
                "characters-import-auth-guard-python-fallback",
                "characters-validate-import-auth-guard-python-fallback"
            ],
            "validate_import_policy": "public Rust validation route; do not restore auth-guard Python fallback unless this policy is rolled back"
        },
        "business_smoke_status": {
            "owner_profile": "phase5-characters-business-owner",
            "readiness_probe_count": 18,
            "business_probe_count": 12,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Characters route business smoke is covered by phase5-characters-business-owner; final completion now requires explicit source-map freeze/delete/repoint approval with same-round rollback policy."
    })
}

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    name: String,
    #[serde(default)]
    is_organization: bool,
    role_type: Option<String>,
    personality: Option<String>,
    background: Option<String>,
    appearance: Option<String>,
    age: Option<String>,
    gender: Option<String>,
    relationships: Option<String>,
    organization_type: Option<String>,
    organization_purpose: Option<String>,
    traits: Option<String>,
    avatar_url: Option<String>,
    main_career_id: Option<String>,
    main_career_stage: Option<i32>,
    sub_careers: Option<String>,
    power_level: Option<i32>,
    location: Option<String>,
    motto: Option<String>,
    color: Option<String>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    name: Option<String>,
    role_type: Option<String>,
    personality: Option<String>,
    background: Option<String>,
    appearance: Option<String>,
    age: Option<String>,
    gender: Option<String>,
    status: Option<String>,
    is_organization: Option<bool>,
    relationships: Option<String>,
    organization_type: Option<String>,
    organization_purpose: Option<String>,
    traits: Option<String>,
    avatar_url: Option<String>,
    main_career_id: Option<String>,
    main_career_stage: Option<i32>,
    sub_careers: Option<String>,
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
struct CharactersExportRequest {
    character_ids: Vec<String>,
}

#[derive(Deserialize)]
struct ImportCharactersQuery {
    project_id: String,
}

#[derive(Deserialize)]
pub(crate) struct GenerateCharacterRequest {
    project_id: String,
    name: Option<String>,
    role_type: Option<String>,
    background: Option<String>,
    requirements: Option<String>,
    provider: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GenerateCharacterTaskError {
    ProjectNotFoundOrAccessDenied,
    CharacterNotFoundOrAccessDenied,
    InvalidMainCareer(String),
    SyncCharacterCareers(String),
    BadRequest(String),
    BadGateway(String),
    Internal(String),
}

impl std::fmt::Display for GenerateCharacterTaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProjectNotFoundOrAccessDenied => write!(f, "项目不存在或无权限"),
            Self::CharacterNotFoundOrAccessDenied => write!(f, "角色不存在或无权限"),
            Self::InvalidMainCareer(message) => write!(f, "{}", message),
            Self::SyncCharacterCareers(message) => write!(f, "{}", message),
            Self::BadRequest(message) => write!(f, "{}", message),
            Self::BadGateway(message) => write!(f, "{}", message),
            Self::Internal(message) => write!(f, "{}", message),
        }
    }
}

fn map_generate_character_task_error(
    error: GenerateCharacterTaskError,
) -> (StatusCode, Json<Value>) {
    match error {
        GenerateCharacterTaskError::ProjectNotFoundOrAccessDenied => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "项目不存在或无权限"})),
        ),
        GenerateCharacterTaskError::CharacterNotFoundOrAccessDenied => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "角色不存在或无权限"})),
        ),
        GenerateCharacterTaskError::InvalidMainCareer(message) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": message})),
        ),
        GenerateCharacterTaskError::SyncCharacterCareers(message) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": message})),
        ),
        GenerateCharacterTaskError::BadRequest(message) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": message})),
        ),
        GenerateCharacterTaskError::BadGateway(message) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"success": false, "message": message})),
        ),
        GenerateCharacterTaskError::Internal(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": message})),
        ),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubCareerPayload {
    career_id: String,
    #[serde(default = "default_career_stage")]
    stage: i32,
}

fn default_career_stage() -> i32 {
    1
}

fn value_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalized_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn parse_sub_career_payloads(raw: Option<&str>) -> Result<Option<Vec<SubCareerPayload>>, String> {
    let Some(raw) = raw.map(str::trim) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(Some(Vec::new()));
    }

    serde_json::from_str::<Vec<SubCareerPayload>>(raw)
        .map(Some)
        .map_err(|error| format!("sub_careers JSON格式错误: {}", error))
}

fn sub_careers_value(raw: Option<&str>) -> Option<Value> {
    raw.and_then(|text| serde_json::from_str::<Value>(text).ok())
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

fn parse_stage_value(value: Option<&Value>) -> Option<i32> {
    match value {
        Some(Value::Number(number)) => number.as_i64().and_then(|stage| i32::try_from(stage).ok()),
        Some(Value::String(text)) => text.trim().parse::<i32>().ok(),
        _ => None,
    }
}

async fn load_character_prompt_template(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<String, String> {
    let _ = PromptTemplateService::sync_managed_templates_for_user(db, user_id).await;

    if let Some(template) =
        PromptTemplateService::find_user_template(db, user_id, "SINGLE_CHARACTER_GENERATION")
            .await?
    {
        if template.is_active {
            let content = template.template_content.trim();
            if !content.is_empty() {
                return Ok(content.to_string());
            }
        }
    }

    PromptTemplateService::system_template_info("SINGLE_CHARACTER_GENERATION")
        .map(|template| template.content.clone())
        .ok_or_else(|| "缺少角色生成提示词模板".to_string())
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

async fn build_character_generation_context(
    db: &DatabaseConnection,
    project_model: &project::Model,
) -> Result<String, String> {
    let existing_characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(&project_model.id))
        .order_by_desc(character::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let mut existing_chars_info = String::new();
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
        existing_chars_info.push_str("\n已有角色：\n");
        existing_chars_info.push_str(&character_list.join("\n"));
    }
    if !organization_list.is_empty() {
        existing_chars_info.push_str("\n\n已有组织：\n");
        existing_chars_info.push_str(&organization_list.join("\n"));
    }

    let careers = career::Entity::find()
        .filter(career::Column::ProjectId.eq(&project_model.id))
        .order_by_asc(career::Column::CareerType)
        .order_by_asc(career::Column::Name)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let mut careers_info = String::new();
    if !careers.is_empty() {
        let main_careers: Vec<&career::Model> = careers
            .iter()
            .filter(|item| item.career_type == "main")
            .collect();
        let sub_careers: Vec<&career::Model> = careers
            .iter()
            .filter(|item| item.career_type == "sub")
            .collect();

        if !main_careers.is_empty() {
            careers_info.push_str(
                "\n\n可用主职业列表（请在career_info中填写职业名称，系统会自动匹配ID）：\n",
            );
            for item in main_careers {
                careers_info.push_str(&format!("- 名称: {}", item.name));
                if let Some(description) = item.description.as_deref() {
                    let description = description.trim();
                    if !description.is_empty() {
                        let short_desc: String = description.chars().take(50).collect();
                        careers_info.push_str(&format!(", 描述: {}", short_desc));
                    }
                }
                careers_info.push('\n');
            }
        }

        if !sub_careers.is_empty() {
            careers_info.push_str(
                "\n可用副职业列表（请在career_info中填写职业名称，系统会自动匹配ID）：\n",
            );
            for item in sub_careers.into_iter().take(5) {
                careers_info.push_str(&format!("- 名称: {}", item.name));
                if let Some(description) = item.description.as_deref() {
                    let description = description.trim();
                    if !description.is_empty() {
                        let short_desc: String = description.chars().take(50).collect();
                        careers_info.push_str(&format!(", 描述: {}", short_desc));
                    }
                }
                careers_info.push('\n');
            }
        }
    } else {
        careers_info.push_str("\n\n⚠️ 项目中暂无职业设定");
    }

    Ok(format!(
        "项目信息：\n- 书名：{}\n- 主题：{}\n- 类型：{}\n- 时间背景：{}\n- 地理位置：{}\n- 氛围基调：{}\n- 世界规则：{}{}{}\n",
        project_model.title,
        project_model.theme.as_deref().unwrap_or("未设定"),
        project_model.genre.as_deref().unwrap_or("未设定"),
        project_model.world_time_period.as_deref().unwrap_or("未设定"),
        project_model.world_location.as_deref().unwrap_or("未设定"),
        project_model.world_atmosphere.as_deref().unwrap_or("未设定"),
        project_model.world_rules.as_deref().unwrap_or("未设定"),
        existing_chars_info,
        careers_info
    ))
}

fn build_character_generation_user_input(body: &GenerateCharacterRequest) -> String {
    format!(
        "用户要求：\n- 角色名称：{}\n- 角色定位：{}\n- 背景设定：{}\n- 其他要求：{}\n",
        body.name.as_deref().unwrap_or("请AI生成"),
        body.role_type.as_deref().unwrap_or("supporting"),
        body.background.as_deref().unwrap_or("无特殊要求"),
        body.requirements.as_deref().unwrap_or("无")
    )
}

fn resolve_career_payloads(
    project_id: &str,
    ai_payload: &Value,
    careers: &[career::Model],
) -> (Option<String>, Option<i32>, Option<String>) {
    let Some(career_info) = ai_payload.get("career_info") else {
        return (None, None, None);
    };

    let main_career_name = value_text(career_info.get("main_career_name"));
    let main_career_stage = parse_stage_value(career_info.get("main_career_stage"));

    let main_career_id = main_career_name.and_then(|career_name| {
        careers
            .iter()
            .find(|item| {
                item.project_id == project_id
                    && item.career_type == "main"
                    && item.name.trim() == career_name.trim()
            })
            .map(|item| item.id.clone())
    });

    let sub_careers = career_info
        .get("sub_careers")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let career_name = value_text(item.get("career_name"))?;
                    let career_model = careers.iter().find(|career_model| {
                        career_model.project_id == project_id
                            && career_model.career_type == "sub"
                            && career_model.name.trim() == career_name.trim()
                    })?;
                    let stage = parse_stage_value(item.get("stage"))
                        .unwrap_or(1)
                        .clamp(1, career_model.max_stage.max(1));
                    Some(SubCareerPayload {
                        career_id: career_model.id.clone(),
                        stage,
                    })
                })
                .take(2)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .and_then(|items| serde_json::to_string(&items).ok());

    (main_career_id, main_career_stage, sub_careers)
}

async fn build_relationship_summary_map(
    db: &DatabaseConnection,
    project_id: &str,
    character_ids: &[String],
) -> Result<HashMap<String, String>, String> {
    if character_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let character_id_set: HashSet<String> = character_ids.iter().cloned().collect();
    let relationships = relationship::Entity::find()
        .filter(relationship::Column::ProjectId.eq(project_id))
        .filter(
            Condition::any()
                .add(relationship::Column::CharacterFromId.is_in(character_ids.to_vec()))
                .add(relationship::Column::CharacterToId.is_in(character_ids.to_vec())),
        )
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let mut related_ids = HashSet::new();
    for item in &relationships {
        related_ids.insert(item.character_from_id.clone());
        related_ids.insert(item.character_to_id.clone());
    }

    let related_characters = character::Entity::find()
        .filter(character::Column::Id.is_in(related_ids.into_iter().collect::<Vec<_>>()))
        .all(db)
        .await
        .map_err(|error| error.to_string())?;
    let name_map: HashMap<String, String> = related_characters
        .into_iter()
        .map(|item| (item.id, item.name))
        .collect();

    let mut summaries: HashMap<String, Vec<String>> = HashMap::new();
    for item in relationships {
        if character_id_set.contains(&item.character_from_id) {
            let target_name = name_map
                .get(&item.character_to_id)
                .cloned()
                .unwrap_or_else(|| "未知".to_string());
            summaries
                .entry(item.character_from_id.clone())
                .or_default()
                .push(format!(
                    "与{}：{}",
                    target_name,
                    item.relationship_name
                        .clone()
                        .unwrap_or_else(|| "相关".to_string())
                ));
        }
        if character_id_set.contains(&item.character_to_id) {
            let target_name = name_map
                .get(&item.character_from_id)
                .cloned()
                .unwrap_or_else(|| "未知".to_string());
            summaries
                .entry(item.character_to_id.clone())
                .or_default()
                .push(format!(
                    "与{}：{}",
                    target_name,
                    item.relationship_name
                        .clone()
                        .unwrap_or_else(|| "相关".to_string())
                ));
        }
    }

    Ok(summaries
        .into_iter()
        .map(|(character_id, parts)| (character_id, parts.join("；")))
        .collect())
}

async fn build_organization_maps(
    db: &DatabaseConnection,
    character_ids: &[String],
) -> Result<
    (
        HashMap<String, organization::Model>,
        HashMap<String, String>,
    ),
    String,
> {
    if character_ids.is_empty() {
        return Ok((HashMap::new(), HashMap::new()));
    }

    let organizations = organization::Entity::find()
        .filter(organization::Column::CharacterId.is_in(character_ids.to_vec()))
        .all(db)
        .await
        .map_err(|error| error.to_string())?;
    let org_map: HashMap<String, organization::Model> = organizations
        .iter()
        .cloned()
        .map(|item| (item.character_id.clone(), item))
        .collect();

    if organizations.is_empty() {
        return Ok((org_map, HashMap::new()));
    }

    let organization_ids: Vec<String> = organizations.iter().map(|item| item.id.clone()).collect();
    let members = organization_member::Entity::find()
        .filter(organization_member::Column::OrganizationId.is_in(organization_ids))
        .order_by_desc(organization_member::Column::Rank)
        .order_by_asc(organization_member::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let member_character_ids: Vec<String> = members
        .iter()
        .map(|item| item.character_id.clone())
        .collect();
    let member_characters = character::Entity::find()
        .filter(character::Column::Id.is_in(member_character_ids))
        .all(db)
        .await
        .map_err(|error| error.to_string())?;
    let member_name_map: HashMap<String, String> = member_characters
        .into_iter()
        .map(|item| (item.id, item.name))
        .collect();

    let org_id_to_character_id: HashMap<String, String> = organizations
        .iter()
        .map(|item| (item.id.clone(), item.character_id.clone()))
        .collect();
    let mut summaries: HashMap<String, Vec<String>> = HashMap::new();
    for member in members {
        let Some(character_id) = org_id_to_character_id.get(&member.organization_id) else {
            continue;
        };
        let member_name = member_name_map
            .get(&member.character_id)
            .cloned()
            .unwrap_or_else(|| "未知".to_string());
        let position = if member.position.trim().is_empty() {
            "成员".to_string()
        } else {
            member.position.clone()
        };
        summaries
            .entry(character_id.clone())
            .or_default()
            .push(format!("{}（{}）", member_name, position));
    }

    let summary_map = summaries
        .into_iter()
        .map(|(character_id, items)| {
            (
                character_id,
                serde_json::to_string(&items).unwrap_or_else(|_| "[]".to_string()),
            )
        })
        .collect();

    Ok((org_map, summary_map))
}

fn character_to_legacy_value(
    item: &character::Model,
    relationship_summary: Option<&String>,
    organization_summary: Option<&String>,
    org_model: Option<&organization::Model>,
) -> Value {
    json!({
        "id": item.id,
        "project_id": item.project_id,
        "name": item.name,
        "age": item.age,
        "gender": item.gender,
        "is_organization": item.is_organization,
        "role_type": item.role_type,
        "personality": item.personality,
        "background": item.background,
        "appearance": item.appearance,
        "relationships": relationship_summary.cloned().unwrap_or_default(),
        "organization_type": item.organization_type,
        "organization_purpose": item.organization_purpose,
        "organization_members": if item.is_organization {
            organization_summary.cloned().unwrap_or_default()
        } else {
            String::new()
        },
        "traits": item.traits,
        "avatar_url": item.avatar_url,
        "power_level": org_model.map(|org| org.power_level),
        "location": org_model.and_then(|org| org.location.clone()),
        "motto": org_model.and_then(|org| org.motto.clone()),
        "color": org_model.and_then(|org| org.color.clone()),
        "status": item.status,
        "status_changed_chapter": item.status_changed_chapter,
        "current_state": item.current_state,
        "state_updated_chapter": item.state_updated_chapter,
        "main_career_id": item.main_career_id,
        "main_career_stage": item.main_career_stage,
        "sub_careers": sub_careers_value(item.sub_careers.as_deref()),
        "created_at": item.created_at,
        "updated_at": item.updated_at,
    })
}

async fn enrich_characters(
    db: &DatabaseConnection,
    items: Vec<character::Model>,
) -> Result<Vec<Value>, String> {
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let project_id = items[0].project_id.clone();
    let character_ids: Vec<String> = items.iter().map(|item| item.id.clone()).collect();
    let relationship_map = build_relationship_summary_map(db, &project_id, &character_ids).await?;
    let (organization_map, organization_summary_map) =
        build_organization_maps(db, &character_ids).await?;

    Ok(items
        .iter()
        .map(|item| {
            character_to_legacy_value(
                item,
                relationship_map.get(&item.id),
                organization_summary_map.get(&item.id),
                organization_map.get(&item.id),
            )
        })
        .collect())
}

async fn enrich_character(
    db: &DatabaseConnection,
    item: character::Model,
) -> Result<Value, String> {
    let mut items = enrich_characters(db, vec![item]).await?;
    Ok(items.pop().unwrap_or_else(|| json!({})))
}

async fn validate_main_career(
    db: &DatabaseConnection,
    project_id: &str,
    main_career_id: Option<&str>,
    main_career_stage: Option<i32>,
) -> Result<Option<career::Model>, (StatusCode, Json<Value>)> {
    let Some(career_id) = normalized_string(main_career_id) else {
        return Ok(None);
    };

    let Some(career_model) = career::Entity::find()
        .filter(career::Column::Id.eq(&career_id))
        .filter(career::Column::ProjectId.eq(project_id))
        .filter(career::Column::CareerType.eq("main"))
        .one(db)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": error.to_string()})),
            )
        })?
    else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "主职业不存在或类型错误"})),
        ));
    };

    if let Some(stage) = main_career_stage {
        if stage > career_model.max_stage {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(
                    json!({"success": false, "message": format!("阶段超出范围，该职业最大阶段为{}", career_model.max_stage)}),
                ),
            ));
        }
    }

    Ok(Some(career_model))
}

async fn persist_character_extra_fields(
    db: &DatabaseConnection,
    character_model: character::Model,
    relationships: Option<&str>,
    organization_type: Option<&str>,
    organization_purpose: Option<&str>,
    traits: Option<&str>,
    avatar_url: Option<&str>,
    main_career_id: Option<&str>,
    main_career_stage: Option<i32>,
    sub_careers: Option<&str>,
    is_update: bool,
) -> Result<character::Model, String> {
    let mut active: character::ActiveModel = character_model.into();

    if !is_update || relationships.is_some() {
        active.relationships = Set(normalized_string(relationships));
    }
    if !is_update || organization_type.is_some() {
        active.organization_type = Set(normalized_string(organization_type));
    }
    if !is_update || organization_purpose.is_some() {
        active.organization_purpose = Set(normalized_string(organization_purpose));
    }
    if !is_update || traits.is_some() {
        active.traits = Set(normalized_string(traits));
    }
    if !is_update || avatar_url.is_some() {
        active.avatar_url = Set(normalized_string(avatar_url));
    }
    if !is_update || main_career_id.is_some() {
        active.main_career_id = Set(normalized_string(main_career_id));
    }
    if !is_update || main_career_stage.is_some() {
        active.main_career_stage = Set(main_career_stage);
    }
    if !is_update || sub_careers.is_some() {
        let sub_career_value = match parse_sub_career_payloads(sub_careers)? {
            Some(items) => Some(serde_json::to_string(&items).map_err(|error| error.to_string())?),
            None => None,
        };
        active.sub_careers = Set(sub_career_value);
    }

    active.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
    active.update(db).await.map_err(|error| error.to_string())
}

async fn sync_character_careers(
    db: &DatabaseConnection,
    character_model: &character::Model,
    main_career_id: Option<&str>,
    main_career_stage: Option<i32>,
    sub_careers: Option<&str>,
) -> Result<(), (StatusCode, Json<Value>)> {
    if character_model.is_organization {
        character_career::Entity::delete_many()
            .filter(character_career::Column::CharacterId.eq(&character_model.id))
            .exec(db)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": error.to_string()})),
                )
            })?;
        return Ok(());
    }

    if main_career_id.is_some() || main_career_stage.is_some() {
        let existing_main = character_career::Entity::find()
            .filter(character_career::Column::CharacterId.eq(&character_model.id))
            .filter(character_career::Column::CareerType.eq("main"))
            .one(db)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": error.to_string()})),
                )
            })?;

        if let Some(main_career_id) = normalized_string(main_career_id) {
            let career_model = validate_main_career(
                db,
                &character_model.project_id,
                Some(&main_career_id),
                main_career_stage,
            )
            .await?
            .expect("validated main career should exist");
            let current_stage = main_career_stage.unwrap_or(1);

            if let Some(existing_main) = existing_main {
                let mut active: character_career::ActiveModel = existing_main.into();
                active.career_id = Set(main_career_id);
                active.current_stage = Set(current_stage);
                active.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
                active.update(db).await.map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"success": false, "message": error.to_string()})),
                    )
                })?;
            } else {
                character_career::ActiveModel {
                    id: Set(Uuid::new_v4().to_string()),
                    character_id: Set(character_model.id.clone()),
                    career_id: Set(career_model.id),
                    career_type: Set("main".to_string()),
                    current_stage: Set(current_stage),
                    stage_progress: Set(Some(0)),
                    started_at: Set(None),
                    reached_current_stage_at: Set(None),
                    notes: Set(None),
                    created_at: Set(chrono::Utc::now().naive_utc()),
                    updated_at: Set(Some(chrono::Utc::now().naive_utc())),
                }
                .insert(db)
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"success": false, "message": error.to_string()})),
                    )
                })?;
            }
        } else if let Some(existing_main) = existing_main {
            character_career::Entity::delete_by_id(existing_main.id)
                .exec(db)
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"success": false, "message": error.to_string()})),
                    )
                })?;
        }
    }

    if sub_careers.is_some() {
        let parsed_items = parse_sub_career_payloads(sub_careers).map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": error})),
            )
        })?;

        let existing_sub_careers = character_career::Entity::find()
            .filter(character_career::Column::CharacterId.eq(&character_model.id))
            .filter(character_career::Column::CareerType.eq("sub"))
            .all(db)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": error.to_string()})),
                )
            })?;
        for existing in existing_sub_careers {
            character_career::Entity::delete_by_id(existing.id)
                .exec(db)
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"success": false, "message": error.to_string()})),
                    )
                })?;
        }

        if let Some(items) = parsed_items {
            for item in items.into_iter().take(2) {
                let career_id = normalized_string(Some(&item.career_id));
                let Some(career_id) = career_id else {
                    continue;
                };

                let career_exists = career::Entity::find()
                    .filter(career::Column::Id.eq(&career_id))
                    .filter(career::Column::ProjectId.eq(&character_model.project_id))
                    .filter(career::Column::CareerType.eq("sub"))
                    .one(db)
                    .await
                    .map_err(|error| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"success": false, "message": error.to_string()})),
                        )
                    })?;
                let Some(career_model) = career_exists else {
                    continue;
                };
                let stage = item.stage.clamp(1, career_model.max_stage.max(1));

                character_career::ActiveModel {
                    id: Set(Uuid::new_v4().to_string()),
                    character_id: Set(character_model.id.clone()),
                    career_id: Set(career_model.id),
                    career_type: Set("sub".to_string()),
                    current_stage: Set(stage),
                    stage_progress: Set(Some(0)),
                    started_at: Set(None),
                    reached_current_stage_at: Set(None),
                    notes: Set(None),
                    created_at: Set(chrono::Utc::now().naive_utc()),
                    updated_at: Set(Some(chrono::Utc::now().naive_utc())),
                }
                .insert(db)
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"success": false, "message": error.to_string()})),
                    )
                })?;
            }
        }
    }

    Ok(())
}

async fn upsert_organization_details(
    db: &DatabaseConnection,
    character_model: &character::Model,
    power_level: Option<i32>,
    location: Option<&str>,
    motto: Option<&str>,
    color: Option<&str>,
    is_update: bool,
) -> Result<(), String> {
    if !character_model.is_organization {
        return Ok(());
    }

    let existing = organization::Entity::find()
        .filter(organization::Column::CharacterId.eq(&character_model.id))
        .one(db)
        .await
        .map_err(|error| error.to_string())?;

    if let Some(existing) = existing {
        let mut active: organization::ActiveModel = existing.into();
        if !is_update || power_level.is_some() {
            active.power_level = Set(power_level.unwrap_or(50));
        }
        if !is_update || location.is_some() {
            active.location = Set(normalized_string(location));
        }
        if !is_update || motto.is_some() {
            active.motto = Set(normalized_string(motto));
        }
        if !is_update || color.is_some() {
            active.color = Set(normalized_string(color));
        }
        active.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
        active.update(db).await.map_err(|error| error.to_string())?;
        return Ok(());
    }

    if is_update
        && power_level.is_none()
        && location.is_none()
        && motto.is_none()
        && color.is_none()
    {
        return Ok(());
    }

    organization::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        character_id: Set(character_model.id.clone()),
        project_id: Set(character_model.project_id.clone()),
        parent_org_id: Set(None),
        level: Set(0),
        power_level: Set(power_level.unwrap_or(50)),
        member_count: Set(0),
        location: Set(normalized_string(location)),
        motto: Set(normalized_string(motto)),
        color: Set(normalized_string(color)),
        created_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(Some(chrono::Utc::now().naive_utc())),
    }
    .insert(db)
    .await
    .map_err(|error| error.to_string())?;

    Ok(())
}

async fn create_character(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    validate_main_career(
        &db,
        &body.project_id,
        body.main_career_id.as_deref(),
        body.main_career_stage,
    )
    .await?;
    parse_sub_career_payloads(body.sub_careers.as_deref()).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": error})),
        )
    })?;

    match CharacterService::create(
        &db,
        &body.project_id,
        &claims.sub,
        &body.name,
        body.is_organization,
        body.role_type.as_deref(),
        body.personality.as_deref(),
        body.background.as_deref(),
        body.appearance.as_deref(),
        body.age.as_deref(),
        body.gender.as_deref(),
    )
    .await
    {
        Ok(Some(character)) => {
            let character = persist_character_extra_fields(
                &db,
                character,
                body.relationships.as_deref(),
                body.organization_type.as_deref(),
                body.organization_purpose.as_deref(),
                body.traits.as_deref(),
                body.avatar_url.as_deref(),
                body.main_career_id.as_deref(),
                body.main_career_stage,
                body.sub_careers.as_deref(),
                false,
            )
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": error})),
                )
            })?;

            sync_character_careers(
                &db,
                &character,
                body.main_career_id.as_deref(),
                body.main_career_stage,
                body.sub_careers.as_deref(),
            )
            .await?;
            upsert_organization_details(
                &db,
                &character,
                body.power_level,
                body.location.as_deref(),
                body.motto.as_deref(),
                body.color.as_deref(),
                false,
            )
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": error})),
                )
            })?;

            let refreshed = character::Entity::find_by_id(&character.id)
                .one(&db)
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"success": false, "message": error.to_string()})),
                    )
                })?
                .ok_or((
                    StatusCode::NOT_FOUND,
                    Json(json!({"success": false, "message": "角色不存在或无权限"})),
                ))?;
            let payload = enrich_character(&db, refreshed).await.map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": error})),
                )
            })?;
            Ok((StatusCode::CREATED, Json(payload)))
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

pub(crate) async fn generate_character_task(
    db: &DatabaseConnection,
    user_id: &str,
    body: GenerateCharacterRequest,
) -> Result<Value, GenerateCharacterTaskError> {
    let project_model = load_generate_project(db, &body.project_id, user_id)
        .await
        .map_err(GenerateCharacterTaskError::Internal)?
        .ok_or(GenerateCharacterTaskError::ProjectNotFoundOrAccessDenied)?;

    let prompt_template = load_character_prompt_template(db, user_id)
        .await
        .map_err(GenerateCharacterTaskError::Internal)?;
    let project_context = build_character_generation_context(db, &project_model)
        .await
        .map_err(GenerateCharacterTaskError::Internal)?;
    let user_input = build_character_generation_user_input(&body);

    let mut params = HashMap::new();
    params.insert("project_context".to_string(), project_context);
    params.insert("user_input".to_string(), user_input);
    let prompt = PromptTemplateService::format_prompt(&prompt_template, &params)
        .map_err(GenerateCharacterTaskError::Internal)?;

    let ai_config = SettingsService::build_ai_config(
        db,
        user_id,
        body.provider.as_deref(),
        body.model.as_deref(),
        None,
    )
    .await
    .map_err(GenerateCharacterTaskError::BadRequest)?;
    let ai_service = AIService::new(ai_config);
    let response = ai_service
        .generate_text(&prompt, None, None)
        .await
        .map_err(GenerateCharacterTaskError::BadGateway)?;

    let cleaned = clean_json_response(&response.content);
    let ai_payload = serde_json::from_str::<Value>(&cleaned).map_err(|error| {
        GenerateCharacterTaskError::BadGateway(format!("角色生成结果不是有效JSON: {}", error))
    })?;

    let name = value_text(ai_payload.get("name"))
        .or_else(|| normalized_string(body.name.as_deref()))
        .unwrap_or_else(|| "未命名角色".to_string());
    let age = value_text(ai_payload.get("age"));
    let gender = value_text(ai_payload.get("gender"));
    let appearance = value_text(ai_payload.get("appearance"));
    let personality = value_text(ai_payload.get("personality"));
    let background = value_text(ai_payload.get("background"))
        .or_else(|| normalized_string(body.background.as_deref()));
    let traits = value_text(ai_payload.get("traits"));
    let relationships = value_text(ai_payload.get("relationships_text"));
    let role_type = value_text(ai_payload.get("role_type"))
        .or_else(|| normalized_string(body.role_type.as_deref()))
        .or_else(|| Some("supporting".to_string()));

    let created = CharacterService::create(
        db,
        &project_model.id,
        user_id,
        &name,
        false,
        role_type.as_deref(),
        personality.as_deref(),
        background.as_deref(),
        appearance.as_deref(),
        age.as_deref(),
        gender.as_deref(),
    )
    .await
    .map_err(GenerateCharacterTaskError::Internal)?
    .ok_or(GenerateCharacterTaskError::ProjectNotFoundOrAccessDenied)?;

    let careers = career::Entity::find()
        .filter(career::Column::ProjectId.eq(&project_model.id))
        .all(db)
        .await
        .map_err(|error| GenerateCharacterTaskError::Internal(error.to_string()))?;
    let (main_career_id, main_career_stage, sub_careers) =
        resolve_career_payloads(&project_model.id, &ai_payload, &careers);
    validate_main_career(
        db,
        &project_model.id,
        main_career_id.as_deref(),
        main_career_stage,
    )
    .await
    .map_err(|(_, Json(payload))| {
        GenerateCharacterTaskError::InvalidMainCareer(
            payload
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("主职业信息无效")
                .to_string(),
        )
    })?;

    let character = persist_character_extra_fields(
        db,
        created,
        relationships.as_deref(),
        None,
        None,
        traits.as_deref(),
        None,
        main_career_id.as_deref(),
        main_career_stage,
        sub_careers.as_deref(),
        false,
    )
    .await
    .map_err(GenerateCharacterTaskError::Internal)?;

    sync_character_careers(
        db,
        &character,
        main_career_id.as_deref(),
        main_career_stage,
        sub_careers.as_deref(),
    )
    .await
    .map_err(|(_, Json(payload))| {
        GenerateCharacterTaskError::SyncCharacterCareers(
            payload
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("同步角色职业失败")
                .to_string(),
        )
    })?;

    let refreshed = character::Entity::find_by_id(&character.id)
        .one(db)
        .await
        .map_err(|error| GenerateCharacterTaskError::Internal(error.to_string()))?
        .ok_or(GenerateCharacterTaskError::CharacterNotFoundOrAccessDenied)?;

    enrich_character(db, refreshed)
        .await
        .map_err(GenerateCharacterTaskError::Internal)
}

async fn generate_character(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<GenerateCharacterRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = generate_character_task(&db, &claims.sub, body)
        .await
        .map_err(map_generate_character_task_error)?;
    Ok(Json(payload))
}

async fn list_characters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CharacterService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(characters)) => {
            let items = enrich_characters(&db, characters).await.map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": error})),
                )
            })?;
            let total = items.len();
            Ok(Json(json!({"items": items, "total": total})))
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

async fn get_character(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(character_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CharacterService::get(&db, &character_id, &claims.sub).await {
        Ok(Some(character)) => {
            let payload = enrich_character(&db, character).await.map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": error})),
                )
            })?;
            Ok(Json(payload))
        }
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "角色不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn update_character(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(character_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let existing_character = CharacterService::get(&db, &character_id, &claims.sub)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": error})),
            )
        })?
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "角色不存在或无权限"})),
        ))?;
    validate_main_career(
        &db,
        &existing_character.project_id,
        body.main_career_id.as_deref(),
        body.main_career_stage,
    )
    .await?;
    parse_sub_career_payloads(body.sub_careers.as_deref()).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": error})),
        )
    })?;

    match CharacterService::update(
        &db,
        &character_id,
        &claims.sub,
        body.name.as_deref(),
        body.role_type.as_deref(),
        body.personality.as_deref(),
        body.background.as_deref(),
        body.appearance.as_deref(),
        body.age.as_deref(),
        body.gender.as_deref(),
        body.status.as_deref(),
        body.is_organization,
    )
    .await
    {
        Ok(Some(character)) => {
            let character = persist_character_extra_fields(
                &db,
                character,
                body.relationships.as_deref(),
                body.organization_type.as_deref(),
                body.organization_purpose.as_deref(),
                body.traits.as_deref(),
                body.avatar_url.as_deref(),
                body.main_career_id.as_deref(),
                body.main_career_stage,
                body.sub_careers.as_deref(),
                true,
            )
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": error})),
                )
            })?;

            sync_character_careers(
                &db,
                &character,
                body.main_career_id.as_deref(),
                body.main_career_stage,
                body.sub_careers.as_deref(),
            )
            .await?;
            upsert_organization_details(
                &db,
                &character,
                body.power_level,
                body.location.as_deref(),
                body.motto.as_deref(),
                body.color.as_deref(),
                true,
            )
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": error})),
                )
            })?;

            let refreshed = character::Entity::find_by_id(&character.id)
                .one(&db)
                .await
                .map_err(|error| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"success": false, "message": error.to_string()})),
                    )
                })?
                .ok_or((
                    StatusCode::NOT_FOUND,
                    Json(json!({"success": false, "message": "角色不存在或无权限"})),
                ))?;
            let payload = enrich_character(&db, refreshed).await.map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": error})),
                )
            })?;
            Ok(Json(payload))
        }
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "角色不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn delete_character(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(character_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CharacterService::delete(&db, &character_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"success": true, "message": "角色已删除"}))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "角色不存在或无权限"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn validate_characters_import(
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut file_data: Vec<u8> = Vec::new();
    let mut file_found = false;

    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let bytes = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"detail": format!("读取文件失败: {}", e)})),
                )
            })?;
            file_data = bytes.to_vec();
            file_found = true;
            break;
        }
    }

    if !file_found {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "请上传JSON文件"})),
        ));
    }

    let data: Value = serde_json::from_slice(&file_data).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "valid": false,
                "version": null,
                "statistics": {},
                "errors": ["JSON解析失败"],
                "warnings": [],
            })),
        )
    })?;

    let version = data.get("version").and_then(|v| v.as_str());
    let export_type = data.get("export_type").and_then(|v| v.as_str());
    let items = data.get("data").and_then(|d| d.as_array());
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if version.is_none() {
        errors.push("缺少version字段".to_string());
    }
    if export_type != Some("characters") {
        errors.push(format!(
            "export_type应为'characters'，当前为{:?}",
            export_type
        ));
    }
    if items.is_none() {
        errors.push("缺少data字段或data不是数组".to_string());
    } else if let Some(arr) = items {
        if arr.is_empty() {
            warnings.push("没有需要导入的角色数据".to_string());
        }
        for (i, item) in arr.iter().enumerate() {
            if item
                .get("name")
                .and_then(|n| n.as_str())
                .map_or(true, |n| n.is_empty())
            {
                errors.push(format!("第{}项缺少name字段", i + 1));
            }
        }
    }

    let char_count = items.map_or(0, |a| a.len());
    let org_count = items.map_or(0, |a| {
        a.iter()
            .filter(|i| {
                i.get("is_organization")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .count()
    });

    Ok(Json(json!({
        "valid": errors.is_empty(),
        "version": version,
        "statistics": {
            "total": char_count,
            "characters": char_count - org_count,
            "organizations": org_count,
        },
        "errors": errors,
        "warnings": warnings,
    })))
}

async fn export_characters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CharactersExportRequest>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    if body.character_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "请至少选择一个角色/组织"})),
        ));
    }

    let mut items = Vec::new();
    for character_id in &body.character_ids {
        let character = CharacterService::get(&db, character_id, &claims.sub)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"detail": error})),
                )
            })?
            .ok_or((
                StatusCode::NOT_FOUND,
                Json(json!({"detail": format!("角色不存在: {}", character_id)})),
            ))?;
        items.push(serde_json::to_value(character).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": error.to_string()})),
            )
        })?);
    }

    let payload = json!({
        "version": "rust-strangler-1",
        "export_type": "characters",
        "data": items,
        "statistics": {
            "total": items.len(),
            "characters": items.iter().filter(|item| !item.get("is_organization").and_then(Value::as_bool).unwrap_or(false)).count(),
            "organizations": items.iter().filter(|item| item.get("is_organization").and_then(Value::as_bool).unwrap_or(false)).count(),
        },
    });
    let body = serde_json::to_vec_pretty(&payload).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": error.to_string()})),
        )
    })?;
    let filename = format!("characters_export_{}.json", items.len());

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename={}", filename),
        )
        .body(axum::body::Body::from(body))
        .unwrap())
}

async fn import_characters(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ImportCharactersQuery>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let existing = CharacterService::list(&db, &query.project_id, &claims.sub)
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
        ))?;
    let mut existing_names: std::collections::HashSet<String> =
        existing.into_iter().map(|item| item.name).collect();

    let mut file_data = Vec::new();
    let mut file_found = false;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let bytes = field.bytes().await.map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"detail": format!("读取文件失败: {}", error)})),
                )
            })?;
            file_data = bytes.to_vec();
            file_found = true;
            break;
        }
    }
    if !file_found {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "请上传JSON文件"})),
        ));
    }

    let data: Value = serde_json::from_slice(&file_data).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": format!("JSON格式错误: {}", error)})),
        )
    })?;
    let items = data.get("data").and_then(Value::as_array).ok_or((
        StatusCode::BAD_REQUEST,
        Json(json!({"detail": "缺少data字段或data不是数组"})),
    ))?;

    let mut imported_characters = Vec::new();
    let mut imported_organizations = Vec::new();
    let mut skipped = Vec::new();
    let mut errors = Vec::new();

    for item in items {
        let Some(name) = value_string(item, "name") else {
            errors.push("缺少name字段".to_string());
            continue;
        };
        if existing_names.contains(&name) {
            skipped.push(format!("名称已存在: {}", name));
            continue;
        }

        let is_organization = item
            .get("is_organization")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        match CharacterService::create(
            &db,
            &query.project_id,
            &claims.sub,
            &name,
            is_organization,
            value_string(item, "role_type").as_deref(),
            value_string(item, "personality").as_deref(),
            value_string(item, "background").as_deref(),
            value_string(item, "appearance").as_deref(),
            value_string(item, "age").as_deref(),
            value_string(item, "gender").as_deref(),
        )
        .await
        {
            Ok(Some(_)) => {
                existing_names.insert(name.clone());
                if is_organization {
                    imported_organizations.push(name);
                } else {
                    imported_characters.push(name);
                }
            }
            Ok(None) => errors.push(format!("项目不存在或无权限: {}", name)),
            Err(error) => errors.push(format!("{}: {}", name, error)),
        }
    }

    let imported = imported_characters.len() + imported_organizations.len();
    Ok(Json(json!({
        "success": errors.is_empty(),
        "message": format!("导入完成：成功{}，跳过{}，错误{}", imported, skipped.len(), errors.len()),
        "statistics": {
            "total": items.len(),
            "imported": imported,
            "skipped": skipped.len(),
            "errors": errors.len(),
        },
        "details": {
            "imported_characters": imported_characters,
            "imported_organizations": imported_organizations,
            "skipped": skipped,
            "errors": errors,
        },
        "warnings": [],
    })))
}

async fn list_characters_by_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match CharacterService::list(&db, &project_id, &claims.sub).await {
        Ok(Some(characters)) => {
            let items = enrich_characters(&db, characters).await.map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"success": false, "message": error})),
                )
            })?;
            let total = items.len();
            Ok(Json(json!({"items": items, "total": total})))
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

pub fn routes() -> Router {
    Router::new()
        .route(
            CHARACTERS_PROJECT_LIST_ROUTE,
            get(list_characters_by_project),
        )
        .route(
            CHARACTERS_LIST_CREATE_ROUTE,
            post(create_character).get(list_characters),
        )
        .route(CHARACTERS_GENERATE_ROUTE, post(generate_character))
        .route(CHARACTERS_GENERATE_STREAM_ROUTE, post(generate_character))
        .route(
            CHARACTERS_DETAIL_ROUTE,
            get(get_character)
                .put(update_character)
                .delete(delete_character),
        )
        .route(
            CHARACTERS_VALIDATE_IMPORT_ROUTE,
            post(validate_characters_import),
        )
        .route(CHARACTERS_EXPORT_ROUTE, post(export_characters))
        .route(CHARACTERS_IMPORT_ROUTE, post(import_characters))
}

#[cfg(test)]
mod tests {
    use super::{
        build_characters_route_owner_contract, CHARACTERS_DETAIL_ROUTE, CHARACTERS_EXPORT_ROUTE,
        CHARACTERS_GENERATE_ROUTE, CHARACTERS_GENERATE_STREAM_ROUTE, CHARACTERS_IMPORT_ROUTE,
        CHARACTERS_LIST_CREATE_ROUTE, CHARACTERS_PROJECT_LIST_ROUTE,
        CHARACTERS_VALIDATE_IMPORT_ROUTE,
    };
    use serde_json::json;

    #[test]
    fn should_publish_characters_route_owner_contract() {
        let contract = build_characters_route_owner_contract();

        assert_eq!(contract["owner"], "characters");
        assert_eq!(
            contract["scope"],
            "characters_crud_generation_import_export_validate_route_group"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/api/characters.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/api/characters.rs"
        );
        assert_eq!(
            contract["route_contract"]["project_list"],
            CHARACTERS_PROJECT_LIST_ROUTE
        );
        assert_eq!(
            contract["route_contract"]["generate_stream"],
            CHARACTERS_GENERATE_STREAM_ROUTE
        );
        assert_eq!(
            contract["route_contract"]["validate_import"],
            CHARACTERS_VALIDATE_IMPORT_ROUTE
        );
        assert_eq!(
            contract["behavior_contract"]["route_entrypoints"][9],
            "import_characters"
        );
        assert_eq!(
            contract["readiness_evidence"][5],
            "characters-import-auth-guard-rust"
        );
        assert_eq!(contract["readiness_evidence"].as_array().unwrap().len(), 18);
        assert_eq!(
            contract["readiness_evidence"][17],
            "characters-missing-import-project-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-characters-business-owner"
        );
        let business_probes = contract["owner_profile"]["business_probes"]
            .as_array()
            .expect("characters business probes should be present");
        assert_eq!(business_probes.len(), 12);
        assert_eq!(
            contract["owner_profile"]["business_probes"][7],
            "characters-validate-import-business-rust"
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
            json!(false)
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
            json!(18)
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            json!(12)
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .expect("characters migration policy should be present")
            .contains("phase5-characters-business-owner"));
    }

    #[test]
    fn should_keep_characters_route_group_paths_stable() {
        assert_eq!(
            CHARACTERS_PROJECT_LIST_ROUTE,
            "/characters/project/{project_id}"
        );
        assert_eq!(CHARACTERS_LIST_CREATE_ROUTE, "/characters");
        assert_eq!(CHARACTERS_GENERATE_ROUTE, "/characters/generate");
        assert_eq!(
            CHARACTERS_GENERATE_STREAM_ROUTE,
            "/characters/generate-stream"
        );
        assert_eq!(CHARACTERS_DETAIL_ROUTE, "/characters/{character_id}");
        assert_eq!(
            CHARACTERS_VALIDATE_IMPORT_ROUTE,
            "/characters/validate-import"
        );
        assert_eq!(CHARACTERS_EXPORT_ROUTE, "/characters/export");
        assert_eq!(CHARACTERS_IMPORT_ROUTE, "/characters/import");
    }
}
