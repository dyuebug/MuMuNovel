use std::{collections::HashMap, fmt, future::Future};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use tokio_stream::StreamExt;

use crate::{
    ai::service::AIService,
    models::{character, outline, project},
    services::{
        controlled_generation_guidance_service::append_controlled_generation_guidance,
        cooperative_cancellation_service::CooperativeCancellationToken,
        generation_contract_service::GenerationIntentKind, outline_service::OutlineService,
        prompt_template_service::PromptTemplateService, settings_service::SettingsService,
        wizard_service::clean_json_response,
    },
};

const DEFAULT_ESTIMATED_WORDS: i32 = 3000;
const MAX_CHAPTERS_PER_OUTLINE_EXPANSION: usize = 20;

#[derive(Debug, Clone)]
pub(crate) struct GenerateOutlineExpansion<'a> {
    pub user_id: &'a str,
    pub outline_id: &'a str,
    pub target_chapter_count: usize,
    pub expansion_strategy: &'a str,
    pub enable_scene_analysis: bool,
    pub provider_override: Option<&'a str>,
    pub model_override: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GeneratedOutlineExpansion {
    pub project_id: String,
    pub outline_id: String,
    pub chapter_plans: Vec<Value>,
    pub provider: String,
    pub model: String,
    pub result_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutlineExpansionGenerationError {
    Cancelled,
    InvalidTarget,
    Load,
    AiConfig,
    Prompt,
    Provider,
    EmptyResponse,
    Parse,
    PlanCountMismatch,
    Observer,
}

impl OutlineExpansionGenerationError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Cancelled => "outline_expansion_cancelled",
            Self::InvalidTarget => "outline_expansion_target_invalid",
            Self::Load => "outline_expansion_context_load_failed",
            Self::AiConfig => "outline_expansion_ai_config_failed",
            Self::Prompt => "outline_expansion_prompt_failed",
            Self::Provider => "outline_expansion_provider_failed",
            Self::EmptyResponse => "outline_expansion_empty_response",
            Self::Parse => "outline_expansion_parse_failed",
            Self::PlanCountMismatch => "outline_expansion_plan_count_mismatch",
            Self::Observer => "outline_expansion_observer_failed",
        }
    }
}

impl fmt::Display for OutlineExpansionGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for OutlineExpansionGenerationError {}

pub(crate) async fn generate_outline_expansion_for_autopilot<C, CFut, R, RFut>(
    db: &DatabaseConnection,
    request: GenerateOutlineExpansion<'_>,
    additional_guidance: Option<&str>,
    cancellation_token: Option<&CooperativeCancellationToken>,
    mut on_content: C,
    mut on_reasoning: R,
) -> Result<GeneratedOutlineExpansion, OutlineExpansionGenerationError>
where
    C: FnMut(String) -> CFut,
    CFut: Future<Output = Result<(), ()>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), ()>>,
{
    ensure_not_cancelled(cancellation_token)?;
    if request.target_chapter_count == 0
        || request.target_chapter_count > MAX_CHAPTERS_PER_OUTLINE_EXPANSION
    {
        return Err(OutlineExpansionGenerationError::InvalidTarget);
    }

    let outline_model = OutlineService::get(db, request.outline_id, request.user_id)
        .await
        .map_err(|_| OutlineExpansionGenerationError::Load)?
        .ok_or(OutlineExpansionGenerationError::Load)?;
    let project_model = project::Entity::find_by_id(&outline_model.project_id)
        .one(db)
        .await
        .map_err(|_| OutlineExpansionGenerationError::Load)?
        .filter(|project| project.user_id == request.user_id)
        .ok_or(OutlineExpansionGenerationError::Load)?;
    let characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(&project_model.id))
        .order_by_asc(character::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|_| OutlineExpansionGenerationError::Load)?;
    let context_info = build_outline_context(db, &outline_model).await?;
    let characters_info = build_characters_info(&characters);
    let prompt = build_single_call_prompt(
        &project_model,
        &outline_model,
        &characters_info,
        &context_info,
        request.expansion_strategy,
        request.target_chapter_count,
        request.enable_scene_analysis,
    )?;
    let prompt = append_controlled_generation_guidance(prompt, additional_guidance);

    ensure_not_cancelled(cancellation_token)?;
    let role_aware_config = SettingsService::build_role_aware_ai_config(
        db,
        request.user_id,
        GenerationIntentKind::OutlineExpand,
        request.provider_override,
        request.model_override,
        None,
    )
    .await
    .map_err(|_| OutlineExpansionGenerationError::AiConfig)?;
    let provider = role_aware_config.ai_config.provider.clone();
    let model = role_aware_config.ai_config.model.clone();
    let ai_service = AIService::new(role_aware_config.ai_config);
    let tracked_stream = ai_service.generate_text_stream_tracked(
        prompt,
        None,
        None,
        role_aware_config.allow_model_fallback,
    );
    let mut stream = tracked_stream.stream;
    let completion = tracked_stream.completion;
    let mut accumulated = String::new();
    let mut provider_failed = false;

    loop {
        let next_chunk = if let Some(token) = cancellation_token {
            tokio::select! {
                _ = token.cancelled() => return Err(OutlineExpansionGenerationError::Cancelled),
                chunk = stream.next() => chunk,
            }
        } else {
            stream.next().await
        };
        let Some(chunk_result) = next_chunk else {
            break;
        };
        ensure_not_cancelled(cancellation_token)?;
        match chunk_result {
            Ok(chunk) => {
                if let Some(reasoning) = chunk.reasoning_content.filter(|value| !value.is_empty()) {
                    on_reasoning(reasoning)
                        .await
                        .map_err(|_| OutlineExpansionGenerationError::Observer)?;
                }
                if let Some(content) = chunk.content.filter(|value| !value.is_empty()) {
                    accumulated.push_str(&content);
                    on_content(content)
                        .await
                        .map_err(|_| OutlineExpansionGenerationError::Observer)?;
                }
                if chunk.done {
                    break;
                }
            }
            Err(_) => provider_failed = true,
        }
    }
    completion
        .await
        .map_err(|_| OutlineExpansionGenerationError::Provider)?;
    ensure_not_cancelled(cancellation_token)?;
    if accumulated.trim().is_empty() {
        return Err(if provider_failed {
            OutlineExpansionGenerationError::Provider
        } else {
            OutlineExpansionGenerationError::EmptyResponse
        });
    }

    let chapter_plans = parse_chapter_plans_strict(
        &accumulated,
        &outline_model.id,
        request.target_chapter_count,
    )?;
    let serialized =
        serde_json::to_vec(&chapter_plans).map_err(|_| OutlineExpansionGenerationError::Parse)?;
    let result_digest = format!("sha256:{:x}", Sha256::digest(serialized));

    Ok(GeneratedOutlineExpansion {
        project_id: project_model.id,
        outline_id: outline_model.id,
        chapter_plans,
        provider,
        model,
        result_digest,
    })
}

fn ensure_not_cancelled(
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<(), OutlineExpansionGenerationError> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        return Err(OutlineExpansionGenerationError::Cancelled);
    }
    Ok(())
}

fn build_single_call_prompt(
    project_model: &project::Model,
    outline_model: &outline::Model,
    characters_info: &str,
    context_info: &str,
    expansion_strategy: &str,
    target_chapter_count: usize,
    enable_scene_analysis: bool,
) -> Result<String, OutlineExpansionGenerationError> {
    let mut params = build_prompt_fields(
        project_model,
        outline_model,
        characters_info,
        context_info,
        expansion_strategy,
        target_chapter_count,
    );
    params.insert(
        "enable_scene_analysis".into(),
        enable_scene_analysis.to_string(),
    );
    let template = PromptTemplateService::system_template_info("OUTLINE_EXPAND_SINGLE")
        .ok_or(OutlineExpansionGenerationError::Prompt)?;
    PromptTemplateService::format_prompt(&template.content, &params)
        .map_err(|_| OutlineExpansionGenerationError::Prompt)
}

fn build_prompt_fields(
    project_model: &project::Model,
    outline_model: &outline::Model,
    characters_info: &str,
    context_info: &str,
    expansion_strategy: &str,
    target_chapter_count: usize,
) -> HashMap<String, String> {
    let mut params = HashMap::new();
    params.insert("project_title".into(), project_model.title.clone());
    params.insert(
        "project_genre".into(),
        project_model
            .genre
            .clone()
            .unwrap_or_else(|| "通用".to_string()),
    );
    params.insert(
        "project_theme".into(),
        project_model
            .theme
            .clone()
            .unwrap_or_else(|| "未设定".to_string()),
    );
    params.insert(
        "project_narrative_perspective".into(),
        project_model
            .narrative_perspective
            .clone()
            .unwrap_or_else(|| "第三人称".to_string()),
    );
    params.insert(
        "project_world_time_period".into(),
        project_model
            .world_time_period
            .clone()
            .unwrap_or_else(|| "未设定".to_string()),
    );
    params.insert(
        "project_world_location".into(),
        project_model
            .world_location
            .clone()
            .unwrap_or_else(|| "未设定".to_string()),
    );
    params.insert(
        "project_world_atmosphere".into(),
        project_model
            .world_atmosphere
            .clone()
            .unwrap_or_else(|| "未设定".to_string()),
    );
    params.insert("characters_info".into(), characters_info.to_string());
    params.insert(
        "outline_order_index".into(),
        outline_model.order_index.unwrap_or_default().to_string(),
    );
    params.insert("outline_title".into(), outline_model.title.clone());
    params.insert(
        "outline_content".into(),
        outline_model.content.clone().unwrap_or_default(),
    );
    params.insert("context_info".into(), context_info.to_string());
    params.insert(
        "strategy_instruction".into(),
        expansion_strategy.to_string(),
    );
    params.insert(
        "target_chapter_count".into(),
        target_chapter_count.to_string(),
    );
    params.insert("scene_instruction".into(), String::new());
    params.insert("scene_field".into(), String::new());
    params
}

async fn build_outline_context(
    db: &DatabaseConnection,
    outline_model: &outline::Model,
) -> Result<String, OutlineExpansionGenerationError> {
    let current_order = outline_model.order_index.unwrap_or_default();
    let prev_outline = outline::Entity::find()
        .filter(outline::Column::ProjectId.eq(&outline_model.project_id))
        .filter(outline::Column::OrderIndex.lt(current_order))
        .order_by_desc(outline::Column::OrderIndex)
        .one(db)
        .await
        .map_err(|_| OutlineExpansionGenerationError::Load)?;
    let next_outline = outline::Entity::find()
        .filter(outline::Column::ProjectId.eq(&outline_model.project_id))
        .filter(outline::Column::OrderIndex.gt(current_order))
        .order_by_asc(outline::Column::OrderIndex)
        .one(db)
        .await
        .map_err(|_| OutlineExpansionGenerationError::Load)?;

    let mut context = String::new();
    if let Some(prev) = prev_outline {
        context.push_str(&format!(
            "【前一节】{}: {}...\n\n",
            prev.title,
            truncate_text(prev.content.as_deref().unwrap_or(""), 200)
        ));
    }
    if let Some(next) = next_outline {
        context.push_str(&format!(
            "【后一节】{}: {}...\n",
            next.title,
            truncate_text(next.content.as_deref().unwrap_or(""), 200)
        ));
    }
    Ok(if context.is_empty() {
        "（无前后文）".to_string()
    } else {
        context
    })
}

fn build_characters_info(characters: &[character::Model]) -> String {
    let lines = characters
        .iter()
        .map(|character| {
            let kind = if character.is_organization {
                "组织"
            } else {
                "角色"
            };
            let role_type = character.role_type.as_deref().unwrap_or("未知");
            let personality = character
                .personality
                .as_deref()
                .map(|value| truncate_text(value, 100))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "暂无描述".to_string());
            format!(
                "- {} ({}, {}): {}",
                character.name, kind, role_type, personality
            )
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        "暂无角色".to_string()
    } else {
        lines.join("\n")
    }
}

fn parse_chapter_plans_strict(
    ai_response: &str,
    outline_id: &str,
    target_chapter_count: usize,
) -> Result<Vec<Value>, OutlineExpansionGenerationError> {
    let cleaned = clean_json_response(ai_response);
    let parsed = serde_json::from_str::<Value>(&cleaned)
        .map_err(|_| OutlineExpansionGenerationError::Parse)?;
    let raw_plans = match parsed {
        Value::Array(items) => items,
        Value::Object(map) => map
            .get("chapter_plans")
            .and_then(Value::as_array)
            .cloned()
            .or_else(|| map.get("plans").and_then(Value::as_array).cloned())
            .ok_or(OutlineExpansionGenerationError::Parse)?,
        _ => return Err(OutlineExpansionGenerationError::Parse),
    };
    if raw_plans.len() != target_chapter_count {
        return Err(OutlineExpansionGenerationError::PlanCountMismatch);
    }
    raw_plans
        .into_iter()
        .enumerate()
        .map(|(index, value)| match value {
            Value::Object(map) => normalize_plan_value(map, outline_id, index + 1),
            _ => Err(OutlineExpansionGenerationError::Parse),
        })
        .collect()
}

fn normalize_plan_value(
    mut plan: Map<String, Value>,
    outline_id: &str,
    index: usize,
) -> Result<Value, OutlineExpansionGenerationError> {
    let title = plan
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(OutlineExpansionGenerationError::Parse)?
        .to_string();
    let plot_summary = plan
        .get("plot_summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(OutlineExpansionGenerationError::Parse)?
        .to_string();
    let narrative_goal = plan
        .get("narrative_goal")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let estimated_words = plan
        .get("estimated_words")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_ESTIMATED_WORDS);

    plan.insert(
        "outline_id".to_string(),
        Value::String(outline_id.to_string()),
    );
    plan.insert("sub_index".to_string(), Value::Number(index.into()));
    plan.insert("title".to_string(), Value::String(title));
    plan.insert("plot_summary".to_string(), Value::String(plot_summary));
    plan.entry("key_events".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    plan.entry("character_focus".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    plan.entry("emotional_tone".to_string())
        .or_insert_with(|| Value::String("未知".to_string()));
    plan.insert(
        "narrative_goal".to_string(),
        Value::String(narrative_goal.clone()),
    );
    plan.entry("conflict_type".to_string())
        .or_insert_with(|| Value::String("未知".to_string()));
    plan.entry("ending_type".to_string()).or_insert_with(|| {
        Value::String(if narrative_goal.is_empty() {
            format!("章节收束-{index}")
        } else {
            "悬念推进".to_string()
        })
    });
    plan.insert(
        "estimated_words".to_string(),
        Value::Number(estimated_words.into()),
    );
    plan.entry("scenes".to_string()).or_insert(Value::Null);
    Ok(Value::Object(plan))
}

fn truncate_text(text: &str, limit: usize) -> String {
    text.trim().chars().take(limit).collect()
}
