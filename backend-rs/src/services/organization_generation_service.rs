use std::{collections::HashMap, fmt};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use serde_json::Value;

use crate::{
    ai::service::AIService,
    models::character,
    services::{
        controlled_generation_guidance_service::append_controlled_generation_guidance,
        cooperative_cancellation_service::CooperativeCancellationToken,
        project_service::ProjectService, prompt_template_service::PromptTemplateService,
        settings_service::SettingsService, wizard_service::clean_json_response,
    },
};

const ORGANIZATION_TEMPLATE_KEY: &str = "SINGLE_ORGANIZATION_GENERATION";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerateOrganizationPlanForProject<'a> {
    pub user_id: &'a str,
    pub project_id: &'a str,
    pub name: Option<&'a str>,
    pub organization_type: Option<&'a str>,
    pub background: Option<&'a str>,
    pub requirements: Option<&'a str>,
    pub provider_override: Option<&'a str>,
    pub model_override: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedOrganizationPlan {
    pub name: String,
    pub organization_type: String,
    pub personality: Option<String>,
    pub background: Option<String>,
    pub appearance: Option<String>,
    pub organization_purpose: Option<String>,
    pub traits: Vec<String>,
    pub power_level: i32,
    pub location: Option<String>,
    pub motto: Option<String>,
    pub color: Option<String>,
    pub initial_members: Vec<GeneratedOrganizationInitialMember>,
    pub relationships: Vec<GeneratedOrganizationRelationship>,
    pub provider: String,
    pub model: String,
    pub content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedOrganizationInitialMember {
    pub character_name: String,
    pub position: Option<String>,
    pub rank: Option<i32>,
    pub loyalty: Option<i32>,
    pub joined_at: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedOrganizationRelationship {
    pub target_organization_name: String,
    pub relationship_type: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OrganizationGenerationError {
    Cancelled,
    ProjectNotFoundOrAccessDenied,
    ProjectRead,
    ContextRead,
    TemplateRead,
    TemplateMissing,
    PromptFormat,
    AiConfig,
    Provider,
    EmptyResponse,
    InvalidResponse,
}

impl OrganizationGenerationError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Cancelled => "organization_generation_cancelled",
            Self::ProjectNotFoundOrAccessDenied => "project_not_found_or_access_denied",
            Self::ProjectRead => "project_read_failed",
            Self::ContextRead => "organization_generation_context_read_failed",
            Self::TemplateRead => "organization_generation_template_read_failed",
            Self::TemplateMissing => "organization_generation_template_missing",
            Self::PromptFormat => "organization_generation_prompt_format_failed",
            Self::AiConfig => "ai_config_failed",
            Self::Provider => "organization_generation_provider_failed",
            Self::EmptyResponse => "organization_generation_empty_response",
            Self::InvalidResponse => "organization_generation_invalid_response",
        }
    }

    pub(crate) const fn user_message(self) -> &'static str {
        match self {
            Self::Cancelled => "组织生成已取消",
            Self::ProjectNotFoundOrAccessDenied => "项目不存在或无权访问",
            Self::ProjectRead => "加载项目失败",
            Self::ContextRead => "加载组织生成上下文失败",
            Self::TemplateRead | Self::TemplateMissing => "组织生成提示词模板不可用",
            Self::PromptFormat => "组织生成提示词格式化失败",
            Self::AiConfig => "AI配置失败",
            Self::Provider => "组织模型调用失败",
            Self::EmptyResponse => "组织模型返回为空",
            Self::InvalidResponse => "组织模型返回了无效数据",
        }
    }
}

impl fmt::Display for OrganizationGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for OrganizationGenerationError {}

/// Produces an allowlisted organization domain plan without mutating project business data.
///
/// The durable orchestrator owns the later fenced persistence transaction. This function never
/// stores a prompt, reasoning, raw provider response, organization row, character row, or
/// generation-history row.
pub(crate) async fn generate_organization_plan_for_project(
    db: &DatabaseConnection,
    request: GenerateOrganizationPlanForProject<'_>,
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<GeneratedOrganizationPlan, OrganizationGenerationError> {
    generate_organization_plan_for_project_with_guidance(db, request, None, cancellation_token)
        .await
}

pub(crate) async fn generate_organization_plan_for_project_with_guidance(
    db: &DatabaseConnection,
    request: GenerateOrganizationPlanForProject<'_>,
    additional_guidance: Option<&str>,
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<GeneratedOrganizationPlan, OrganizationGenerationError> {
    ensure_not_cancelled(cancellation_token)?;
    let project = ProjectService::get(db, request.project_id, request.user_id)
        .await
        .map_err(|_| OrganizationGenerationError::ProjectRead)?
        .ok_or(OrganizationGenerationError::ProjectNotFoundOrAccessDenied)?;

    ensure_not_cancelled(cancellation_token)?;
    let prompt_template = load_organization_prompt_template(db, request.user_id).await?;
    let project_context = build_organization_generation_context(db, &project).await?;
    let user_input = build_organization_generation_user_input(&request);
    let mut parameters = HashMap::new();
    parameters.insert("project_context".to_string(), project_context);
    parameters.insert("user_input".to_string(), user_input);
    let prompt = PromptTemplateService::format_prompt(&prompt_template, &parameters)
        .map_err(|_| OrganizationGenerationError::PromptFormat)?;
    let prompt = append_controlled_generation_guidance(prompt, additional_guidance);

    ensure_not_cancelled(cancellation_token)?;
    let ai_config = SettingsService::build_ai_config(
        db,
        request.user_id,
        request.provider_override,
        request.model_override,
        None,
    )
    .await
    .map_err(|_| OrganizationGenerationError::AiConfig)?;
    let provider = ai_config.provider.clone();
    let model = ai_config.model.clone();
    let ai_service = AIService::new(ai_config);
    let response = ai_service
        .generate_text(&prompt, None, None)
        .await
        .map_err(|_| OrganizationGenerationError::Provider)?;

    ensure_not_cancelled(cancellation_token)?;
    parse_generated_organization_plan(
        &response.content,
        &provider,
        &model,
        request.name,
        request.organization_type,
        request.background,
    )
}

async fn load_organization_prompt_template(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<String, OrganizationGenerationError> {
    if let Some(template) =
        PromptTemplateService::find_user_template(db, user_id, ORGANIZATION_TEMPLATE_KEY)
            .await
            .map_err(|_| OrganizationGenerationError::TemplateRead)?
    {
        if template.is_active {
            let content = template.template_content.trim();
            if !content.is_empty() {
                return Ok(content.to_string());
            }
        }
    }

    PromptTemplateService::system_template_info(ORGANIZATION_TEMPLATE_KEY)
        .map(|template| template.content.clone())
        .ok_or(OrganizationGenerationError::TemplateMissing)
}

async fn build_organization_generation_context(
    db: &DatabaseConnection,
    project: &crate::models::project::Model,
) -> Result<String, OrganizationGenerationError> {
    let existing_characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(&project.id))
        .order_by_desc(character::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|_| OrganizationGenerationError::ContextRead)?;

    let mut character_list = Vec::new();
    let mut organization_list = Vec::new();
    for item in existing_characters.iter().take(10) {
        if item.is_organization {
            organization_list.push(format!(
                "- {} [{}]",
                item.name,
                item.organization_type.as_deref().unwrap_or("组织")
            ));
        } else {
            character_list.push(format!(
                "- {}（{}）",
                item.name,
                item.role_type.as_deref().unwrap_or("未知")
            ));
        }
    }

    let mut existing_info = String::new();
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
        project.title,
        project.theme.as_deref().unwrap_or("未设定"),
        project.genre.as_deref().unwrap_or("未设定"),
        project.world_time_period.as_deref().unwrap_or("未设定"),
        project.world_location.as_deref().unwrap_or("未设定"),
        project.world_atmosphere.as_deref().unwrap_or("未设定"),
        project.world_rules.as_deref().unwrap_or("未设定"),
        existing_info
    ))
}

fn build_organization_generation_user_input(
    request: &GenerateOrganizationPlanForProject<'_>,
) -> String {
    format!(
        "用户要求：\n- 组织名称：{}\n- 组织类型：{}\n- 背景设定：{}\n- 其他要求：{}",
        normalized_string(request.name)
            .as_deref()
            .unwrap_or("请AI生成"),
        normalized_string(request.organization_type)
            .as_deref()
            .unwrap_or("请AI根据世界观决定"),
        normalized_string(request.background)
            .as_deref()
            .unwrap_or("无特殊要求"),
        normalized_string(request.requirements)
            .as_deref()
            .unwrap_or("无"),
    )
}

fn ensure_not_cancelled(
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<(), OrganizationGenerationError> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        Err(OrganizationGenerationError::Cancelled)
    } else {
        Ok(())
    }
}

fn parse_generated_organization_plan(
    raw_content: &str,
    provider: &str,
    model: &str,
    requested_name: Option<&str>,
    requested_organization_type: Option<&str>,
    requested_background: Option<&str>,
) -> Result<GeneratedOrganizationPlan, OrganizationGenerationError> {
    let cleaned = clean_json_response(raw_content);
    if cleaned.trim().is_empty() {
        return Err(OrganizationGenerationError::EmptyResponse);
    }
    let data = serde_json::from_str::<Value>(&cleaned)
        .map_err(|_| OrganizationGenerationError::InvalidResponse)?;
    if !data.is_object() {
        return Err(OrganizationGenerationError::InvalidResponse);
    }

    let name = optional_non_empty_string(&data, "name")
        .or_else(|| normalized_string(requested_name))
        .ok_or(OrganizationGenerationError::InvalidResponse)?;
    let organization_type = optional_non_empty_string(&data, "organization_type")
        .or_else(|| normalized_string(requested_organization_type))
        .unwrap_or_else(|| "组织".to_string());

    Ok(GeneratedOrganizationPlan {
        name,
        organization_type,
        personality: optional_non_empty_string(&data, "personality"),
        background: optional_non_empty_string(&data, "background")
            .or_else(|| normalized_string(requested_background)),
        appearance: optional_non_empty_string(&data, "appearance"),
        organization_purpose: optional_non_empty_string(&data, "organization_purpose"),
        traits: string_array(&data, "traits"),
        power_level: bounded_i32(&data, "power_level", 50, 0, 100),
        location: optional_non_empty_string(&data, "location"),
        motto: optional_non_empty_string(&data, "motto"),
        color: optional_non_empty_string(&data, "color"),
        initial_members: initial_members(&data),
        relationships: relationships(&data),
        provider: provider.to_string(),
        model: model.to_string(),
        content_digest: format!("{:x}", md5::compute(cleaned.as_bytes())),
    })
}

fn normalized_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn optional_non_empty_string(data: &Value, key: &str) -> Option<String> {
    data.get(key)
        .and_then(Value::as_str)
        .and_then(|value| normalized_string(Some(value)))
}

fn string_array(data: &Value, key: &str) -> Vec<String> {
    data.get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|value| normalized_string(Some(value)))
                .collect()
        })
        .unwrap_or_default()
}

fn bounded_i32(data: &Value, key: &str, default: i32, min: i32, max: i32) -> i32 {
    data.get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .map(|value| value.clamp(min, max))
        .unwrap_or(default)
}

fn initial_members(data: &Value) -> Vec<GeneratedOrganizationInitialMember> {
    data.get("initial_members")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let character_name = optional_non_empty_string(item, "character_name")?;
                    Some(GeneratedOrganizationInitialMember {
                        character_name,
                        position: optional_non_empty_string(item, "position"),
                        rank: optional_bounded_i32(item, "rank", 0, 10),
                        loyalty: optional_bounded_i32(item, "loyalty", 0, 100),
                        joined_at: optional_non_empty_string(item, "joined_at"),
                        status: optional_non_empty_string(item, "status"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn relationships(data: &Value) -> Vec<GeneratedOrganizationRelationship> {
    data.get("organization_relationships")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let target_organization_name =
                        optional_non_empty_string(item, "target_organization_name")?;
                    Some(GeneratedOrganizationRelationship {
                        target_organization_name,
                        relationship_type: optional_non_empty_string(item, "relationship_type"),
                        description: optional_non_empty_string(item, "description"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn optional_bounded_i32(data: &Value, key: &str, min: i32, max: i32) -> Option<i32> {
    data.get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .map(|value| value.clamp(min, max))
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_not_cancelled, parse_generated_organization_plan, OrganizationGenerationError,
    };
    use crate::services::cooperative_cancellation_service::{
        CooperativeCancellationRegistry, CooperativeCancellationScope,
    };

    #[test]
    fn parses_fenced_json_into_allowlisted_organization_plan() {
        let result = parse_generated_organization_plan(
            "```json\n{\"name\":\"玄灯司\",\"organization_type\":\"监察机构\",\"traits\":[\"隐秘\",\"严苛\"],\"power_level\":120,\"initial_members\":[{\"character_name\":\"沈砚\",\"rank\":12,\"loyalty\":-2}],\"organization_relationships\":[{\"target_organization_name\":\"北境军\",\"relationship_type\":\"竞争\"}]}\n```",
            "openai",
            "model-1",
            None,
            None,
            None,
        )
        .expect("valid organization plan");

        assert_eq!(result.name, "玄灯司");
        assert_eq!(result.organization_type, "监察机构");
        assert_eq!(result.traits, ["隐秘", "严苛"]);
        assert_eq!(result.power_level, 100);
        assert_eq!(result.initial_members[0].rank, Some(10));
        assert_eq!(result.initial_members[0].loyalty, Some(0));
        assert_eq!(result.relationships[0].target_organization_name, "北境军");
        assert!(!result.content_digest.is_empty());
    }

    #[test]
    fn missing_generated_name_can_use_explicit_request_name() {
        let result = parse_generated_organization_plan(
            r#"{"organization_type":"商会"}"#,
            "openai",
            "model-1",
            Some(" 星河商会 "),
            None,
            Some("商路垄断"),
        )
        .expect("explicit name is a safe fallback");

        assert_eq!(result.name, "星河商会");
        assert_eq!(result.background.as_deref(), Some("商路垄断"));
    }

    #[test]
    fn invalid_response_has_a_stable_error_code() {
        let error =
            parse_generated_organization_plan("not-json", "openai", "model-1", None, None, None)
                .expect_err("invalid response");

        assert_eq!(error, OrganizationGenerationError::InvalidResponse);
        assert_eq!(error.code(), "organization_generation_invalid_response");
    }

    #[test]
    fn cancelled_token_prevents_generation_before_side_effects() {
        let registry = CooperativeCancellationRegistry::default();
        let registration = registry.register(CooperativeCancellationScope::BackgroundTask, "test");
        let token = registration.token();
        assert!(token.cancel());

        assert_eq!(
            ensure_not_cancelled(Some(&token)),
            Err(OrganizationGenerationError::Cancelled)
        );
    }
}
