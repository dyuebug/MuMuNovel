use std::{
    collections::{HashMap, HashSet},
    fmt,
    future::Future,
};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use serde_json::Value;
use tokio_stream::StreamExt;

use crate::{
    ai::service::AIService,
    models::career,
    services::{
        controlled_generation_guidance_service::append_controlled_generation_guidance,
        cooperative_cancellation_service::CooperativeCancellationToken,
        project_service::ProjectService, prompt_template_service::PromptTemplateService,
        settings_service::SettingsService, wizard_service::clean_json_response,
    },
};

const CHARACTER_BATCH_SIZE: usize = 5;
const MAX_CHARACTER_GENERATION_ATTEMPTS: u32 = 3;
const ESTIMATED_CHARACTER_OUTPUT_CHARS: usize = 4_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CharacterGenerationWorldContext {
    pub time_period: Option<String>,
    pub location: Option<String>,
    pub atmosphere: Option<String>,
    pub rules: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerateCharacterGraphForProject<'a> {
    pub user_id: &'a str,
    pub project_id: &'a str,
    pub count: usize,
    pub world_context: Option<&'a CharacterGenerationWorldContext>,
    pub theme: Option<&'a str>,
    pub genre: Option<&'a str>,
    pub requirements: Option<&'a str>,
    pub provider_override: Option<&'a str>,
    pub model_override: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedCharacterGraph {
    pub characters: Vec<GeneratedCharacter>,
    pub organizations: Vec<GeneratedOrganization>,
    pub career_assignments: Vec<GeneratedCareerAssignment>,
    pub relationships: Vec<GeneratedCharacterRelationship>,
    pub organization_memberships: Vec<GeneratedOrganizationMembership>,
    pub provider: String,
    pub model: String,
    pub attempts: u32,
    pub content_digest: String,
}

impl GeneratedCharacterGraph {
    pub(crate) fn is_complete(&self) -> bool {
        !self.characters.is_empty()
            && self.characters.iter().all(|character| {
                !character.name.trim().is_empty() && !character.role_type.trim().is_empty()
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedCharacter {
    pub name: String,
    pub age: i32,
    pub gender: String,
    pub role_type: String,
    pub personality: String,
    pub background: String,
    pub appearance: String,
    pub traits: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedOrganization {
    pub name: String,
    pub role_type: String,
    pub personality: String,
    pub background: String,
    pub appearance: String,
    pub organization_type: String,
    pub organization_purpose: String,
    pub member_names: Vec<String>,
    pub power_level: i32,
    pub location: String,
    pub motto: String,
    pub color: String,
    pub traits: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedCareerAssignment {
    pub character_name: String,
    pub main_career: String,
    pub main_stage: i32,
    pub sub_careers: Vec<GeneratedSubCareerAssignment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedSubCareerAssignment {
    pub career: String,
    pub stage: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedCharacterRelationship {
    pub source_character_name: String,
    pub target_character_name: String,
    pub relationship_type: String,
    pub intimacy_level: i32,
    pub description: String,
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedOrganizationMembership {
    pub character_name: String,
    pub organization_name: String,
    pub position: String,
    pub rank: i32,
    pub loyalty: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CharacterGenerationProgress {
    pub message: String,
    pub progress: u32,
    pub status: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WizardCharacterGenerationError {
    Cancelled,
    InvalidRequestedCount,
    ProjectNotFoundOrAccessDenied,
    ProjectRead,
    CareerRead,
    AiConfig,
    TemplateMissing,
    PromptFormat,
    Provider,
    EmptyResponse,
    InvalidResponse,
    IncompleteResponse,
    Observer,
}

impl WizardCharacterGenerationError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "character_generation_cancelled",
            Self::InvalidRequestedCount => "character_generation_invalid_requested_count",
            Self::ProjectNotFoundOrAccessDenied => "project_not_found_or_access_denied",
            Self::ProjectRead => "project_read_failed",
            Self::CareerRead => "career_read_failed",
            Self::AiConfig => "ai_config_failed",
            Self::TemplateMissing => "characters_batch_template_missing",
            Self::PromptFormat => "character_generation_prompt_format_failed",
            Self::Provider => "character_generation_provider_failed",
            Self::EmptyResponse => "character_generation_empty_response",
            Self::InvalidResponse => "character_generation_invalid_response",
            Self::IncompleteResponse => "character_generation_incomplete_response",
            Self::Observer => "character_generation_observer_failed",
        }
    }

    pub(crate) const fn user_message(&self) -> &'static str {
        match self {
            Self::Cancelled => "角色生成已取消",
            Self::InvalidRequestedCount => "角色生成数量必须大于零",
            Self::ProjectNotFoundOrAccessDenied => "项目不存在或无权访问",
            Self::ProjectRead => "加载项目失败",
            Self::CareerRead => "加载职业体系失败",
            Self::AiConfig => "AI配置失败",
            Self::TemplateMissing => "CHARACTERS_BATCH_GENERATION模板未找到",
            Self::PromptFormat => "角色提示词格式化失败",
            Self::Provider => "角色模型调用失败",
            Self::EmptyResponse => "AI多次返回为空，请稍后重试",
            Self::InvalidResponse => "AI多次返回无效角色数据，请稍后重试",
            Self::IncompleteResponse => "AI多次返回不完整角色数据，请稍后重试",
            Self::Observer => "角色生成进度输出失败",
        }
    }
}

impl fmt::Display for WizardCharacterGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for WizardCharacterGenerationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CareerCatalogEntry {
    name: String,
    career_type: String,
    max_stage: i32,
}
pub(crate) async fn generate_character_graph_for_project<P, PFut, C, CFut, R, RFut>(
    db: &DatabaseConnection,
    request: GenerateCharacterGraphForProject<'_>,
    cancellation_token: Option<&CooperativeCancellationToken>,
    on_progress: P,
    on_content: C,
    on_reasoning: R,
) -> Result<GeneratedCharacterGraph, WizardCharacterGenerationError>
where
    P: FnMut(CharacterGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
    C: FnMut(String) -> CFut,
    CFut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    generate_character_graph_for_project_with_guidance(
        db,
        request,
        None,
        cancellation_token,
        on_progress,
        on_content,
        on_reasoning,
    )
    .await
}

pub(crate) async fn generate_character_graph_for_project_with_guidance<P, PFut, C, CFut, R, RFut>(
    db: &DatabaseConnection,
    request: GenerateCharacterGraphForProject<'_>,
    additional_guidance: Option<&str>,
    cancellation_token: Option<&CooperativeCancellationToken>,
    mut on_progress: P,
    mut on_content: C,
    mut on_reasoning: R,
) -> Result<GeneratedCharacterGraph, WizardCharacterGenerationError>
where
    P: FnMut(CharacterGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
    C: FnMut(String) -> CFut,
    CFut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    if request.count == 0 {
        return Err(WizardCharacterGenerationError::InvalidRequestedCount);
    }

    ensure_not_cancelled(cancellation_token)?;
    emit_progress(&mut on_progress, "加载项目信息...", 5, "processing").await?;
    let project = ProjectService::get(db, request.project_id, request.user_id)
        .await
        .map_err(|_| WizardCharacterGenerationError::ProjectRead)?
        .ok_or(WizardCharacterGenerationError::ProjectNotFoundOrAccessDenied)?;

    ensure_not_cancelled(cancellation_token)?;
    emit_progress(&mut on_progress, "加载职业体系...", 10, "processing").await?;
    let careers = career::Entity::find()
        .filter(career::Column::ProjectId.eq(request.project_id))
        .order_by_asc(career::Column::CareerType)
        .order_by_asc(career::Column::Id)
        .all(db)
        .await
        .map_err(|_| WizardCharacterGenerationError::CareerRead)?;
    let career_catalog = careers
        .into_iter()
        .map(|item| CareerCatalogEntry {
            name: item.name,
            career_type: item.career_type,
            max_stage: item.max_stage,
        })
        .collect::<Vec<_>>();

    ensure_not_cancelled(cancellation_token)?;
    let ai_config = SettingsService::build_ai_config(
        db,
        request.user_id,
        request.provider_override,
        request.model_override,
        None,
    )
    .await
    .map_err(|_| WizardCharacterGenerationError::AiConfig)?;
    let provider = ai_config.provider.clone();
    let model = ai_config.model.clone();
    let system_prompt = ai_config.system_prompt.clone();
    let ai_service = AIService::new(ai_config);

    emit_progress(&mut on_progress, "准备AI提示词...", 15, "processing").await?;
    let template = PromptTemplateService::system_template_info("CHARACTERS_BATCH_GENERATION")
        .ok_or(WizardCharacterGenerationError::TemplateMissing)?;
    let world_context = resolve_world_context(request.world_context, &project);
    let careers_context = build_careers_context(&career_catalog);
    let total_batches = request.count.div_ceil(CHARACTER_BATCH_SIZE);
    let mut all_entities = Vec::with_capacity(request.count);
    let mut accepted_batches = Vec::with_capacity(total_batches);
    let mut total_attempts = 0;

    for batch_index in 0..total_batches {
        ensure_not_cancelled(cancellation_token)?;
        let remaining = request.count.saturating_sub(all_entities.len());
        if remaining == 0 {
            break;
        }
        let batch_size = remaining.min(CHARACTER_BATCH_SIZE);
        let mut last_failure = WizardCharacterGenerationError::EmptyResponse;
        let mut batch_succeeded = false;

        for attempt in 1..=MAX_CHARACTER_GENERATION_ATTEMPTS {
            ensure_not_cancelled(cancellation_token)?;
            total_attempts += 1;
            let batch_progress = 15 + (batch_index as u32 * 60 / total_batches as u32);
            if attempt > 1 {
                emit_progress(
                    &mut on_progress,
                    &format!(
                        "⚠ 第{}批重试 ({}/{})",
                        batch_index + 1,
                        attempt - 1,
                        MAX_CHARACTER_GENERATION_ATTEMPTS
                    ),
                    batch_progress,
                    "processing",
                )
                .await?;
            }

            let requirements = build_batch_requirements(
                request.requirements,
                &careers_context,
                &all_entities,
                batch_index,
                total_batches,
                batch_size,
            );
            let mut params = HashMap::new();
            params.insert("count".to_string(), batch_size.to_string());
            params.insert("time_period".to_string(), world_context.time_period.clone());
            params.insert("location".to_string(), world_context.location.clone());
            params.insert("atmosphere".to_string(), world_context.atmosphere.clone());
            params.insert("rules".to_string(), world_context.rules.clone());
            params.insert(
                "theme".to_string(),
                request.theme.map(str::to_string).unwrap_or_else(|| {
                    project
                        .theme
                        .clone()
                        .unwrap_or_else(|| "未设定".to_string())
                }),
            );
            params.insert(
                "genre".to_string(),
                request.genre.map(str::to_string).unwrap_or_else(|| {
                    project
                        .genre
                        .clone()
                        .unwrap_or_else(|| "未设定".to_string())
                }),
            );
            params.insert("requirements".to_string(), requirements);
            let prompt = PromptTemplateService::format_prompt(&template.content, &params)
                .map_err(|_| WizardCharacterGenerationError::PromptFormat)?;
            let prompt = append_controlled_generation_guidance(prompt, additional_guidance);

            emit_progress(
                &mut on_progress,
                &format!("生成第{}/{}批角色...", batch_index + 1, total_batches),
                batch_progress,
                "processing",
            )
            .await?;
            let mut accumulated = String::new();
            let mut chunk_count = 0u64;
            let mut provider_failed = false;
            let mut stream = ai_service.generate_text_stream(prompt, system_prompt.clone(), None);
            while let Some(chunk_result) = stream.next().await {
                ensure_not_cancelled(cancellation_token)?;
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(reasoning) =
                            chunk.reasoning_content.filter(|value| !value.is_empty())
                        {
                            on_reasoning(reasoning)
                                .await
                                .map_err(|_| WizardCharacterGenerationError::Observer)?;
                        }
                        if let Some(content) = chunk.content.filter(|value| !value.is_empty()) {
                            accumulated.push_str(&content);
                            on_content(content)
                                .await
                                .map_err(|_| WizardCharacterGenerationError::Observer)?;
                            chunk_count += 1;
                            if chunk_count % 10 == 0 {
                                let extra = (accumulated.chars().count() as f64
                                    / ESTIMATED_CHARACTER_OUTPUT_CHARS as f64
                                    * 40.0) as u32;
                                emit_progress(
                                    &mut on_progress,
                                    &format!(
                                        "生成第{}/{}批角色中...",
                                        batch_index + 1,
                                        total_batches
                                    ),
                                    (batch_progress + extra).clamp(batch_progress, 80),
                                    "processing",
                                )
                                .await?;
                            }
                        }
                        if chunk.done {
                            break;
                        }
                    }
                    Err(_) => provider_failed = true,
                }
            }

            if accumulated.trim().is_empty() {
                last_failure = if provider_failed {
                    WizardCharacterGenerationError::Provider
                } else {
                    WizardCharacterGenerationError::EmptyResponse
                };
                continue;
            }

            emit_progress(&mut on_progress, "校验角色批次数据...", 82, "processing").await?;
            match parse_batch_items(&accumulated, batch_size) {
                Ok((items, cleaned)) => {
                    all_entities.extend(items);
                    accepted_batches.push(cleaned);
                    batch_succeeded = true;
                    break;
                }
                Err(error) => last_failure = error,
            }
        }

        if !batch_succeeded {
            return Err(last_failure);
        }
    }

    ensure_not_cancelled(cancellation_token)?;
    emit_progress(&mut on_progress, "整理角色关系图...", 90, "processing").await?;
    let graph = parse_generated_character_graph(
        &accepted_batches,
        &provider,
        &model,
        total_attempts,
        &career_catalog,
    )?;
    if graph.characters.len() + graph.organizations.len() != request.count {
        return Err(WizardCharacterGenerationError::IncompleteResponse);
    }
    ensure_not_cancelled(cancellation_token)?;
    emit_progress(&mut on_progress, "角色关系图生成完成", 100, "completed").await?;
    Ok(graph)
}
#[derive(Debug, Clone)]
struct ResolvedWorldContext {
    time_period: String,
    location: String,
    atmosphere: String,
    rules: String,
}

fn resolve_world_context(
    requested: Option<&CharacterGenerationWorldContext>,
    project: &crate::models::project::Model,
) -> ResolvedWorldContext {
    let requested = requested
        .cloned()
        .unwrap_or(CharacterGenerationWorldContext {
            time_period: None,
            location: None,
            atmosphere: None,
            rules: None,
        });
    ResolvedWorldContext {
        time_period: requested
            .time_period
            .or_else(|| project.world_time_period.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "未设定".to_string()),
        location: requested
            .location
            .or_else(|| project.world_location.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "未设定".to_string()),
        atmosphere: requested
            .atmosphere
            .or_else(|| project.world_atmosphere.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "未设定".to_string()),
        rules: requested
            .rules
            .or_else(|| project.world_rules.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "未设定".to_string()),
    }
}

fn build_careers_context(careers: &[CareerCatalogEntry]) -> String {
    if careers.is_empty() {
        return String::new();
    }

    let mut context = String::from("\n\n【职业体系】\n");
    for career_type in ["main", "sub"] {
        let label = if career_type == "main" {
            "主职业"
        } else {
            "副职业"
        };
        let items = careers
            .iter()
            .filter(|career| career.career_type == career_type)
            .map(|career| format!("- {}（最高阶段 {}）", career.name, career.max_stage))
            .collect::<Vec<_>>();
        if !items.is_empty() {
            context.push_str(label);
            context.push_str("：\n");
            context.push_str(&items.join("\n"));
            context.push('\n');
        }
    }
    context.push_str("每个非组织角色必须在 career_assignment 中选择上述职业：");
    context.push_str("1 个主职业、0-2 个副职业；阶段不得超过对应职业最高阶段。\n");
    context
}

fn build_batch_requirements(
    base_requirements: Option<&str>,
    careers_context: &str,
    existing_entities: &[Value],
    batch_index: usize,
    total_batches: usize,
    batch_size: usize,
) -> String {
    let mut requirements = base_requirements.unwrap_or_default().trim().to_string();
    if batch_index == 0 {
        requirements.push_str(&format!(
            "\n请精确生成{}个实体，且本批必须包含且仅包含 1 名主角（protagonist）。",
            batch_size
        ));
    } else {
        requirements.push_str(&format!(
            "\n请精确生成{}个实体；此前已经生成主角，本批不得再生成 protagonist。",
            batch_size
        ));
    }
    if batch_index + 1 == total_batches {
        requirements.push_str("\n这是最后一批；可生成能推动剧情的组织，但组织不是必选项。");
    }
    if !existing_entities.is_empty() {
        requirements.push_str("\n【已生成实体】\n");
        for entity in existing_entities {
            let name = entity
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("未知实体");
            let role_type = entity
                .get("role_type")
                .and_then(Value::as_str)
                .unwrap_or("supporting");
            requirements.push_str(&format!("- {}：{}\n", name, role_type));
        }
        requirements.push_str("只能引用上述实体或本批实体，不得引用不存在的名称。\n");
    }
    requirements.push_str(careers_context);
    requirements
}

fn parse_batch_items(
    raw_content: &str,
    expected_count: usize,
) -> Result<(Vec<Value>, String), WizardCharacterGenerationError> {
    let cleaned = clean_json_response(raw_content);
    let data = serde_json::from_str::<Value>(&cleaned)
        .map_err(|_| WizardCharacterGenerationError::InvalidResponse)?;
    let items = if let Some(items) = data.as_array() {
        items.clone()
    } else if let Some(object) = data.as_object() {
        ["characters", "items", "data", "results", "entities"]
            .into_iter()
            .find_map(|key| object.get(key).and_then(Value::as_array).cloned())
            .ok_or(WizardCharacterGenerationError::InvalidResponse)?
    } else {
        return Err(WizardCharacterGenerationError::InvalidResponse);
    };

    if items.len() != expected_count || items.iter().any(|item| !item.is_object()) {
        return Err(WizardCharacterGenerationError::IncompleteResponse);
    }
    Ok((items, cleaned))
}

fn ensure_not_cancelled(
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<(), WizardCharacterGenerationError> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        Err(WizardCharacterGenerationError::Cancelled)
    } else {
        Ok(())
    }
}

async fn emit_progress<P, PFut>(
    on_progress: &mut P,
    message: &str,
    progress: u32,
    status: &'static str,
) -> Result<(), WizardCharacterGenerationError>
where
    P: FnMut(CharacterGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
{
    on_progress(CharacterGenerationProgress {
        message: message.to_string(),
        progress,
        status,
    })
    .await
    .map_err(|_| WizardCharacterGenerationError::Observer)
}
fn parse_generated_character_graph(
    batch_contents: &[String],
    provider: &str,
    model: &str,
    attempts: u32,
    career_catalog: &[CareerCatalogEntry],
) -> Result<GeneratedCharacterGraph, WizardCharacterGenerationError> {
    let mut items = Vec::new();
    let mut cleaned_batches = Vec::with_capacity(batch_contents.len());
    for content in batch_contents {
        let (batch_items, cleaned) = parse_batch_items_without_expected_count(content)?;
        items.extend(batch_items);
        cleaned_batches.push(cleaned);
    }

    let mut characters = Vec::new();
    let mut organizations = Vec::new();
    let mut career_assignments = Vec::new();
    let mut relationships = Vec::new();
    let mut organization_memberships = Vec::new();
    let mut all_names = HashSet::new();
    let mut character_names = HashSet::new();
    let mut organization_names = HashSet::new();

    for item in &items {
        let data = item
            .as_object()
            .ok_or(WizardCharacterGenerationError::IncompleteResponse)?;
        let name = required_non_empty_string(data, "name")?;
        if !all_names.insert(name.clone()) {
            return Err(WizardCharacterGenerationError::IncompleteResponse);
        }
        let is_organization = data
            .get("is_organization")
            .and_then(Value::as_bool)
            .ok_or(WizardCharacterGenerationError::IncompleteResponse)?;
        if is_organization {
            let organization = parse_organization(data, name.clone())?;
            organization_names.insert(name);
            organizations.push(organization);
        } else {
            let character = parse_character(data, name.clone())?;
            let assignment = parse_career_assignment(data, &name, career_catalog)?;
            let parsed_relationships = parse_relationships(data, &name)?;
            let parsed_memberships = parse_organization_memberships(data, &name)?;
            character_names.insert(name);
            characters.push(character);
            if let Some(assignment) = assignment {
                career_assignments.push(assignment);
            }
            relationships.extend(parsed_relationships);
            organization_memberships.extend(parsed_memberships);
        }
    }

    validate_character_graph(
        &characters,
        &organizations,
        &relationships,
        &organization_memberships,
        &character_names,
        &organization_names,
    )?;

    let result = GeneratedCharacterGraph {
        characters,
        organizations,
        career_assignments,
        relationships,
        organization_memberships,
        provider: provider.to_string(),
        model: model.to_string(),
        attempts,
        content_digest: format!("{:x}", md5::compute(cleaned_batches.join("\n").as_bytes())),
    };
    if result.is_complete() {
        Ok(result)
    } else {
        Err(WizardCharacterGenerationError::IncompleteResponse)
    }
}

fn parse_batch_items_without_expected_count(
    raw_content: &str,
) -> Result<(Vec<Value>, String), WizardCharacterGenerationError> {
    let cleaned = clean_json_response(raw_content);
    let data = serde_json::from_str::<Value>(&cleaned)
        .map_err(|_| WizardCharacterGenerationError::InvalidResponse)?;
    let items = if let Some(items) = data.as_array() {
        items.clone()
    } else if let Some(object) = data.as_object() {
        ["characters", "items", "data", "results", "entities"]
            .into_iter()
            .find_map(|key| object.get(key).and_then(Value::as_array).cloned())
            .ok_or(WizardCharacterGenerationError::InvalidResponse)?
    } else {
        return Err(WizardCharacterGenerationError::InvalidResponse);
    };
    if items.is_empty() || items.iter().any(|item| !item.is_object()) {
        return Err(WizardCharacterGenerationError::IncompleteResponse);
    }
    Ok((items, cleaned))
}

fn parse_character(
    data: &serde_json::Map<String, Value>,
    name: String,
) -> Result<GeneratedCharacter, WizardCharacterGenerationError> {
    let role_type = parse_role_type(data)?;
    let age = required_i32_in_range(data, "age", 0, 200)?;
    Ok(GeneratedCharacter {
        name,
        age,
        gender: required_non_empty_string(data, "gender")?,
        role_type,
        personality: required_non_empty_string(data, "personality")?,
        background: required_non_empty_string(data, "background")?,
        appearance: required_non_empty_string(data, "appearance")?,
        traits: required_string_array(data, "traits")?,
    })
}

fn parse_organization(
    data: &serde_json::Map<String, Value>,
    name: String,
) -> Result<GeneratedOrganization, WizardCharacterGenerationError> {
    let role_type = parse_role_type(data)?;
    if role_type == "protagonist" {
        return Err(WizardCharacterGenerationError::IncompleteResponse);
    }
    Ok(GeneratedOrganization {
        name,
        role_type,
        personality: required_non_empty_string(data, "personality")?,
        background: required_non_empty_string(data, "background")?,
        appearance: required_non_empty_string(data, "appearance")?,
        organization_type: required_non_empty_string(data, "organization_type")?,
        organization_purpose: required_non_empty_string(data, "organization_purpose")?,
        member_names: required_string_array(data, "organization_members")?,
        power_level: required_i32_in_range(data, "power_level", 70, 95)?,
        location: required_non_empty_string(data, "location")?,
        motto: required_non_empty_string(data, "motto")?,
        color: required_non_empty_string(data, "color")?,
        traits: required_string_array(data, "traits")?,
    })
}

fn parse_role_type(
    data: &serde_json::Map<String, Value>,
) -> Result<String, WizardCharacterGenerationError> {
    let role_type = required_non_empty_string(data, "role_type")?;
    if ["protagonist", "supporting", "antagonist"].contains(&role_type.as_str()) {
        Ok(role_type)
    } else {
        Err(WizardCharacterGenerationError::IncompleteResponse)
    }
}
fn parse_career_assignment(
    data: &serde_json::Map<String, Value>,
    character_name: &str,
    career_catalog: &[CareerCatalogEntry],
) -> Result<Option<GeneratedCareerAssignment>, WizardCharacterGenerationError> {
    let Some(value) = data.get("career_assignment") else {
        return if career_catalog.is_empty() {
            Ok(None)
        } else {
            Err(WizardCharacterGenerationError::IncompleteResponse)
        };
    };
    let assignment = value
        .as_object()
        .ok_or(WizardCharacterGenerationError::IncompleteResponse)?;
    let main_career = required_non_empty_string(assignment, "main_career")?;
    let main_stage = required_i32_in_range(assignment, "main_stage", 1, i32::MAX)?;
    let sub_careers = assignment
        .get("sub_careers")
        .and_then(Value::as_array)
        .ok_or(WizardCharacterGenerationError::IncompleteResponse)?
        .iter()
        .map(|value| {
            let data = value
                .as_object()
                .ok_or(WizardCharacterGenerationError::IncompleteResponse)?;
            Ok(GeneratedSubCareerAssignment {
                career: required_non_empty_string(data, "career")?,
                stage: required_i32_in_range(data, "stage", 1, i32::MAX)?,
            })
        })
        .collect::<Result<Vec<_>, WizardCharacterGenerationError>>()?;
    if sub_careers.len() > 2 {
        return Err(WizardCharacterGenerationError::IncompleteResponse);
    }

    if !career_catalog.is_empty() {
        let by_name = career_catalog
            .iter()
            .map(|career| (career.name.as_str(), career))
            .collect::<HashMap<_, _>>();
        validate_career_stage(&by_name, &main_career, "main", main_stage)?;
        let mut used_sub_careers = HashSet::new();
        for sub_career in &sub_careers {
            if !used_sub_careers.insert(sub_career.career.as_str())
                || sub_career.career == main_career
            {
                return Err(WizardCharacterGenerationError::IncompleteResponse);
            }
            validate_career_stage(&by_name, &sub_career.career, "sub", sub_career.stage)?;
        }
    }

    Ok(Some(GeneratedCareerAssignment {
        character_name: character_name.to_string(),
        main_career,
        main_stage,
        sub_careers,
    }))
}

fn validate_career_stage(
    by_name: &HashMap<&str, &CareerCatalogEntry>,
    name: &str,
    expected_type: &str,
    stage: i32,
) -> Result<(), WizardCharacterGenerationError> {
    let career = by_name
        .get(name)
        .ok_or(WizardCharacterGenerationError::IncompleteResponse)?;
    if career.career_type != expected_type || stage > career.max_stage {
        return Err(WizardCharacterGenerationError::IncompleteResponse);
    }
    Ok(())
}

fn parse_relationships(
    data: &serde_json::Map<String, Value>,
    source_character_name: &str,
) -> Result<Vec<GeneratedCharacterRelationship>, WizardCharacterGenerationError> {
    required_array(data, "relationships_array")?
        .iter()
        .map(|value| {
            let relationship = value
                .as_object()
                .ok_or(WizardCharacterGenerationError::IncompleteResponse)?;
            Ok(GeneratedCharacterRelationship {
                source_character_name: source_character_name.to_string(),
                target_character_name: required_non_empty_string(
                    relationship,
                    "target_character_name",
                )?,
                relationship_type: required_non_empty_string(relationship, "relationship_type")?,
                intimacy_level: required_i32_in_range(relationship, "intimacy_level", -100, 100)?,
                description: required_non_empty_string(relationship, "description")?,
                started_at: optional_non_empty_string(relationship, "started_at"),
            })
        })
        .collect()
}

fn parse_organization_memberships(
    data: &serde_json::Map<String, Value>,
    character_name: &str,
) -> Result<Vec<GeneratedOrganizationMembership>, WizardCharacterGenerationError> {
    required_array(data, "organization_memberships")?
        .iter()
        .map(|value| {
            let membership = value
                .as_object()
                .ok_or(WizardCharacterGenerationError::IncompleteResponse)?;
            Ok(GeneratedOrganizationMembership {
                character_name: character_name.to_string(),
                organization_name: required_non_empty_string(membership, "organization_name")?,
                position: required_non_empty_string(membership, "position")?,
                rank: required_i32_in_range(membership, "rank", 0, 10)?,
                loyalty: required_i32_in_range(membership, "loyalty", 0, 100)?,
            })
        })
        .collect()
}

fn validate_character_graph(
    characters: &[GeneratedCharacter],
    organizations: &[GeneratedOrganization],
    relationships: &[GeneratedCharacterRelationship],
    memberships: &[GeneratedOrganizationMembership],
    character_names: &HashSet<String>,
    organization_names: &HashSet<String>,
) -> Result<(), WizardCharacterGenerationError> {
    if characters.is_empty()
        || characters
            .iter()
            .filter(|character| character.role_type == "protagonist")
            .count()
            != 1
        || characters.first().is_some_and(|character| {
            relationships
                .iter()
                .any(|item| item.source_character_name == character.name)
        })
    {
        return Err(WizardCharacterGenerationError::IncompleteResponse);
    }

    for relationship in relationships {
        if relationship.source_character_name == relationship.target_character_name
            || !character_names.contains(&relationship.target_character_name)
        {
            return Err(WizardCharacterGenerationError::IncompleteResponse);
        }
    }
    for membership in memberships {
        if !character_names.contains(&membership.character_name)
            || !organization_names.contains(&membership.organization_name)
        {
            return Err(WizardCharacterGenerationError::IncompleteResponse);
        }
    }
    for organization in organizations {
        if organization
            .member_names
            .iter()
            .any(|member| !character_names.contains(member))
        {
            return Err(WizardCharacterGenerationError::IncompleteResponse);
        }
    }
    Ok(())
}

fn required_array<'a>(
    data: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a Vec<Value>, WizardCharacterGenerationError> {
    data.get(key)
        .and_then(Value::as_array)
        .ok_or(WizardCharacterGenerationError::IncompleteResponse)
}

fn required_string_array(
    data: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Vec<String>, WizardCharacterGenerationError> {
    required_array(data, key)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or(WizardCharacterGenerationError::IncompleteResponse)
        })
        .collect()
}

fn required_i32_in_range(
    data: &serde_json::Map<String, Value>,
    key: &str,
    min: i32,
    max: i32,
) -> Result<i32, WizardCharacterGenerationError> {
    data.get(key)
        .and_then(Value::as_i64)
        .filter(|value| *value >= min as i64 && *value <= max as i64)
        .map(|value| value as i32)
        .ok_or(WizardCharacterGenerationError::IncompleteResponse)
}

fn required_non_empty_string(
    data: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, WizardCharacterGenerationError> {
    optional_non_empty_string(data, key).ok_or(WizardCharacterGenerationError::IncompleteResponse)
}

fn optional_non_empty_string(data: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    data.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
#[cfg(test)]
mod tests {
    use super::{
        ensure_not_cancelled, parse_generated_character_graph, CareerCatalogEntry,
        WizardCharacterGenerationError,
    };
    use crate::services::cooperative_cancellation_service::{
        CooperativeCancellationRegistry, CooperativeCancellationScope,
    };

    const VALID_GRAPH: &str = r#"[
        {
            "name": "林澈",
            "age": 22,
            "gender": "男",
            "is_organization": false,
            "role_type": "protagonist",
            "personality": "冷静且执着",
            "background": "来自边境小镇",
            "appearance": "身形修长",
            "traits": ["剑术", "观察"],
            "relationships_array": [],
            "organization_memberships": []
        },
        {
            "name": "巡夜司",
            "is_organization": true,
            "role_type": "supporting",
            "personality": "纪律严明",
            "background": "负责边境夜巡",
            "appearance": "黑石高塔",
            "organization_type": "执法机构",
            "organization_purpose": "维持边境秩序",
            "organization_members": ["林澈"],
            "power_level": 85,
            "location": "北境城",
            "motto": "长夜有灯",
            "color": "深蓝",
            "traits": ["夜巡"]
        }
    ]"#;

    #[test]
    fn parses_fenced_json_into_typed_character_graph() {
        let graph = parse_generated_character_graph(
            &[format!("```json\n{VALID_GRAPH}\n``")],
            "openai",
            "model-1",
            2,
            &[],
        )
        .expect("valid graph");

        assert_eq!(graph.characters.len(), 1);
        assert_eq!(graph.organizations.len(), 1);
        assert_eq!(graph.characters[0].name, "林澈");
        assert_eq!(graph.provider, "openai");
        assert_eq!(graph.model, "model-1");
        assert_eq!(graph.attempts, 2);
        assert!(!graph.content_digest.is_empty());
    }

    #[test]
    fn invalid_json_returns_stable_error_code() {
        let error =
            parse_generated_character_graph(&["not-json".to_string()], "openai", "model-1", 1, &[])
                .expect_err("invalid response");

        assert_eq!(error, WizardCharacterGenerationError::InvalidResponse);
        assert_eq!(error.code(), "character_generation_invalid_response");
    }

    #[test]
    fn dangling_relationship_is_rejected() {
        let invalid = VALID_GRAPH.replace(
            "\"relationships_array\": []",
            "\"relationships_array\": [{\"target_character_name\": \"不存在\", \"relationship_type\": \"同伴\", \"intimacy_level\": 40, \"description\": \"相识\"}]",
        );
        let error = parse_generated_character_graph(&[invalid], "openai", "model-1", 1, &[])
            .expect_err("dangling relationship");

        assert_eq!(error, WizardCharacterGenerationError::IncompleteResponse);
    }

    #[test]
    fn organization_power_level_out_of_range_is_rejected() {
        let invalid = VALID_GRAPH.replace("\"power_level\": 85", "\"power_level\": 50");
        let error = parse_generated_character_graph(&[invalid], "openai", "model-1", 1, &[])
            .expect_err("invalid organization power");

        assert_eq!(error, WizardCharacterGenerationError::IncompleteResponse);
    }

    #[test]
    fn known_career_assignment_must_match_catalog_and_stage() {
        let invalid = VALID_GRAPH.replace(
            "\"organization_memberships\": []",
            "\"organization_memberships\": [], \"career_assignment\": {\"main_career\": \"剑士\", \"main_stage\": 3, \"sub_careers\": []}",
        );
        let catalog = [CareerCatalogEntry {
            name: "剑士".to_string(),
            career_type: "main".to_string(),
            max_stage: 2,
        }];
        let error = parse_generated_character_graph(&[invalid], "openai", "model-1", 1, &catalog)
            .expect_err("stage exceeds catalog max");

        assert_eq!(error, WizardCharacterGenerationError::IncompleteResponse);
    }

    #[test]
    fn cancelled_token_returns_cancelled_error() {
        let registry = CooperativeCancellationRegistry::default();
        let registration = registry.register(
            CooperativeCancellationScope::BackgroundTask,
            "character-generation-test",
        );
        let token = registration.token();
        assert!(token.cancel());

        assert_eq!(
            ensure_not_cancelled(Some(&token)),
            Err(WizardCharacterGenerationError::Cancelled)
        );
    }
}
