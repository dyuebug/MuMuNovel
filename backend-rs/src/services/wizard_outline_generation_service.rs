use std::{
    collections::{HashMap, HashSet},
    fmt,
    future::Future,
};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::Serialize;
use serde_json::Value;
use tokio_stream::StreamExt;
use tracing::warn;

use crate::{
    ai::service::AIService,
    models::{character, project},
    services::{
        controlled_generation_guidance_service::append_controlled_generation_guidance,
        cooperative_cancellation_service::CooperativeCancellationToken,
        generation_contract_service::GenerationIntentKind,
        project_service::ProjectService,
        prompt_template_service::PromptTemplateService,
        settings_service::SettingsService,
        wizard_service::{
            build_outline_content, build_outline_quality_guidance_bundle,
            build_outline_runtime_system_prompt, build_project_long_term_goal,
            build_wizard_outline_requirements, clean_json_response, normalize_outline_items,
            OutlineRuntimeStage,
        },
    },
};

const MAX_OUTLINE_GENERATION_ATTEMPTS: u32 = 2;
const MAX_OUTLINE_CHAPTERS_PER_REQUEST: usize = 10;
const ESTIMATED_OUTLINE_OUTPUT_CHARS_PER_CHAPTER: usize = 1_200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerateOutlinePlanForProject<'a> {
    pub user_id: &'a str,
    pub project_id: &'a str,
    pub chapter_count: usize,
    pub narrative_perspective: Option<&'a str>,
    pub target_words: i32,
    pub requirements: Option<&'a str>,
    pub creative_mode: Option<&'a str>,
    pub story_focus: Option<&'a str>,
    pub plot_stage: Option<&'a str>,
    pub story_creation_brief: Option<&'a str>,
    pub quality_preset: Option<&'a str>,
    pub quality_notes: Option<&'a str>,
    pub compact_mode: bool,
    pub provider_override: Option<&'a str>,
    pub model_override: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedOutlinePlan {
    pub outlines: Vec<GeneratedOutlineItem>,
    pub outline_mode: String,
    pub suggested_pending_chapters: Vec<GeneratedPendingChapter>,
    pub provider: String,
    pub model: String,
    pub attempts: u32,
    pub content_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedOutlineItem {
    pub chapter_number: i32,
    pub title: String,
    pub summary: String,
    pub content: String,
    pub scenes: Vec<String>,
    pub characters: Vec<GeneratedOutlineCharacterRef>,
    pub key_points: Vec<String>,
    pub emotion: Option<String>,
    pub narrative_goal: Option<String>,
    pub conflict_line: Option<String>,
    pub decision: Option<String>,
    pub cost: Option<String>,
    pub rule_impact: Option<String>,
    pub dialogue_hook: Option<String>,
    pub character_turns: Vec<String>,
    pub suggested_target_words: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedOutlineCharacterRef {
    pub name: String,
    pub character_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GeneratedPendingChapter {
    pub chapter_number: i32,
    pub title: String,
    pub summary: String,
    pub outline_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutlineGenerationProgress {
    pub message: String,
    pub progress: u32,
    pub status: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WizardOutlineGenerationError {
    Cancelled,
    ProjectNotFoundOrAccessDenied,
    ProjectRead,
    CharacterRead,
    AiConfig,
    TemplateMissing,
    PromptFormat,
    Provider,
    ExecutionTraceClosed,
    EmptyResponse,
    InvalidResponse,
    IncompleteResponse,
    DuplicateChapterNumber(i32),
    Observer,
}

impl WizardOutlineGenerationError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "outline_generation_cancelled",
            Self::ProjectNotFoundOrAccessDenied => "project_not_found_or_access_denied",
            Self::ProjectRead => "project_read_failed",
            Self::CharacterRead => "outline_generation_character_read_failed",
            Self::AiConfig => "ai_config_failed",
            Self::TemplateMissing => "outline_create_template_missing",
            Self::PromptFormat => "outline_create_prompt_format_failed",
            Self::Provider => "outline_generation_provider_failed",
            Self::ExecutionTraceClosed => "outline_generation_execution_trace_closed",
            Self::EmptyResponse => "outline_generation_empty_response",
            Self::InvalidResponse => "outline_generation_invalid_response",
            Self::IncompleteResponse => "outline_generation_incomplete_response",
            Self::DuplicateChapterNumber(_) => "outline_generation_duplicate_chapter_number",
            Self::Observer => "outline_generation_observer_failed",
        }
    }

    pub(crate) const fn user_message(&self) -> &'static str {
        match self {
            Self::Cancelled => "大纲生成已取消",
            Self::ProjectNotFoundOrAccessDenied => "项目不存在或无权访问",
            Self::ProjectRead => "加载项目失败",
            Self::CharacterRead => "加载角色信息失败",
            Self::AiConfig => "AI配置失败",
            Self::TemplateMissing => "OUTLINE_CREATE模板未找到",
            Self::PromptFormat => "大纲提示词格式化失败",
            Self::Provider => "大纲模型调用失败",
            Self::ExecutionTraceClosed => "大纲模型执行跟踪通道已关闭",
            Self::EmptyResponse => "AI多次返回为空，请稍后重试",
            Self::InvalidResponse => "AI多次返回了无效的大纲数据，请稍后重试",
            Self::IncompleteResponse => "AI返回的大纲缺少必要章节信息",
            Self::DuplicateChapterNumber(_) => "AI返回了重复的章节号",
            Self::Observer => "大纲生成输出失败",
        }
    }
}

impl fmt::Display for WizardOutlineGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateChapterNumber(chapter_number) => {
                write!(formatter, "{}:{chapter_number}", self.code())
            }
            _ => formatter.write_str(self.code()),
        }
    }
}

impl std::error::Error for WizardOutlineGenerationError {}

/// 只生成并解析大纲计划，不写入 Outline/Chapter/Project/History，也不保存 Prompt、
/// reasoning、原始模型响应或完整 execution trace。调用方必须在独立事务中提交业务事实。
pub(crate) async fn generate_outline_plan_for_project<P, PFut, C, CFut, R, RFut>(
    db: &DatabaseConnection,
    request: GenerateOutlinePlanForProject<'_>,
    cancellation_token: Option<&CooperativeCancellationToken>,
    on_progress: P,
    on_content: C,
    on_reasoning: R,
) -> Result<GeneratedOutlinePlan, WizardOutlineGenerationError>
where
    P: FnMut(OutlineGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
    C: FnMut(String) -> CFut,
    CFut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    generate_outline_plan_for_project_with_guidance(
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

pub(crate) async fn generate_outline_plan_for_project_with_guidance<P, PFut, C, CFut, R, RFut>(
    db: &DatabaseConnection,
    request: GenerateOutlinePlanForProject<'_>,
    additional_guidance: Option<&str>,
    cancellation_token: Option<&CooperativeCancellationToken>,
    mut on_progress: P,
    mut on_content: C,
    mut on_reasoning: R,
) -> Result<GeneratedOutlinePlan, WizardOutlineGenerationError>
where
    P: FnMut(OutlineGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
    C: FnMut(String) -> CFut,
    CFut: Future<Output = Result<(), String>>,
    R: FnMut(String) -> RFut,
    RFut: Future<Output = Result<(), String>>,
{
    ensure_not_cancelled(cancellation_token)?;
    emit_progress(&mut on_progress, "加载项目信息...", 2, "processing").await?;

    let project = ProjectService::get(db, request.project_id, request.user_id)
        .await
        .map_err(|_| WizardOutlineGenerationError::ProjectRead)?
        .ok_or(WizardOutlineGenerationError::ProjectNotFoundOrAccessDenied)?;
    ensure_not_cancelled(cancellation_token)?;

    emit_progress(&mut on_progress, "加载角色信息...", 5, "processing").await?;
    let characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(request.project_id))
        .all(db)
        .await
        .map_err(|_| WizardOutlineGenerationError::CharacterRead)?;
    let characters_info = build_characters_info(&characters);
    ensure_not_cancelled(cancellation_token)?;

    let chapter_count = request
        .chapter_count
        .clamp(1, MAX_OUTLINE_CHAPTERS_PER_REQUEST);
    let role_aware_config = SettingsService::build_role_aware_ai_config(
        db,
        request.user_id,
        GenerationIntentKind::OutlineGenerate,
        request.provider_override,
        request.model_override,
        None,
    )
    .await
    .map_err(|_| WizardOutlineGenerationError::AiConfig)?;
    let allow_model_fallback = role_aware_config.allow_model_fallback;
    let ai_service = AIService::new(role_aware_config.ai_config);
    ensure_not_cancelled(cancellation_token)?;

    emit_progress(
        &mut on_progress,
        &format!("准备生成{chapter_count}个大纲节点..."),
        8,
        "processing",
    )
    .await?;
    let template = PromptTemplateService::system_template_info("OUTLINE_CREATE")
        .ok_or(WizardOutlineGenerationError::TemplateMissing)?;
    let quality_guidance_bundle =
        match build_outline_quality_guidance_bundle(db, request.project_id, chapter_count).await {
            Ok(bundle) => bundle,
            Err(error) => {
                warn!("Build outline-create quality guidance failed: {error}");
                Default::default()
            }
        };
    ensure_not_cancelled(cancellation_token)?;

    let prompt = build_outline_prompt(
        &project,
        &request,
        chapter_count,
        characters_info,
        quality_guidance_bundle.quality_repair_guidance.as_str(),
        quality_guidance_bundle.quality_trend_guidance.as_str(),
        &template.content,
    )?;
    let prompt = append_controlled_generation_guidance(prompt, additional_guidance);
    let system_prompt =
        build_outline_runtime_system_prompt(&project, chapter_count, OutlineRuntimeStage::Opening);

    let mut last_failure = WizardOutlineGenerationError::EmptyResponse;
    for attempt in 1..=MAX_OUTLINE_GENERATION_ATTEMPTS {
        ensure_not_cancelled(cancellation_token)?;
        if attempt > 1 {
            emit_progress(
                &mut on_progress,
                &format!("大纲返回无效，自动重试... ({attempt}/{MAX_OUTLINE_GENERATION_ATTEMPTS})"),
                12,
                "processing",
            )
            .await?;
        }
        emit_progress(&mut on_progress, "AI正在生成大纲...", 15, "processing").await?;

        let tracked_stream = ai_service.generate_text_stream_tracked(
            prompt.clone(),
            Some(system_prompt.clone()),
            None,
            allow_model_fallback,
        );
        let mut stream = tracked_stream.stream;
        let completion = tracked_stream.completion;
        let mut accumulated = String::new();
        let mut chunk_count = 0u64;
        let mut provider_failed = false;

        loop {
            let next_chunk = if let Some(token) = cancellation_token {
                tokio::select! {
                    _ = token.cancelled() => return Err(WizardOutlineGenerationError::Cancelled),
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
                    if let Some(reasoning) =
                        chunk.reasoning_content.filter(|value| !value.is_empty())
                    {
                        on_reasoning(reasoning)
                            .await
                            .map_err(|_| WizardOutlineGenerationError::Observer)?;
                    }
                    if let Some(content) = chunk.content.filter(|value| !value.is_empty()) {
                        accumulated.push_str(&content);
                        on_content(content)
                            .await
                            .map_err(|_| WizardOutlineGenerationError::Observer)?;
                        chunk_count += 1;
                        if chunk_count % 10 == 0 {
                            let estimated_total = chapter_count
                                .saturating_mul(ESTIMATED_OUTLINE_OUTPUT_CHARS_PER_CHAPTER)
                                .max(1);
                            let char_bonus = (accumulated.chars().count() as f64
                                / estimated_total as f64
                                * 55.0) as u32;
                            emit_progress(
                                &mut on_progress,
                                &format!("生成大纲中... ({}字符)", accumulated.chars().count()),
                                (15 + char_bonus).clamp(15, 70),
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

        ensure_not_cancelled(cancellation_token)?;
        let execution = if let Some(token) = cancellation_token {
            tokio::select! {
                _ = token.cancelled() => return Err(WizardOutlineGenerationError::Cancelled),
                result = completion => result.map_err(|_| WizardOutlineGenerationError::ExecutionTraceClosed)?,
            }
        } else {
            completion
                .await
                .map_err(|_| WizardOutlineGenerationError::ExecutionTraceClosed)?
        };
        ensure_not_cancelled(cancellation_token)?;

        if accumulated.trim().is_empty() {
            last_failure = if provider_failed {
                WizardOutlineGenerationError::Provider
            } else {
                WizardOutlineGenerationError::EmptyResponse
            };
            continue;
        }

        emit_progress(&mut on_progress, "解析大纲数据...", 80, "processing").await?;
        match parse_generated_outline_plan(
            &accumulated,
            project.outline_mode.as_str(),
            execution.actual_provider.as_str(),
            execution.actual_model.as_str(),
            attempt,
            chapter_count,
        ) {
            Ok(result) => {
                ensure_not_cancelled(cancellation_token)?;
                return Ok(result);
            }
            Err(error) => last_failure = error,
        }
    }

    Err(last_failure)
}
#[allow(clippy::too_many_arguments)]
fn build_outline_prompt(
    project: &project::Model,
    request: &GenerateOutlinePlanForProject<'_>,
    chapter_count: usize,
    characters_info: String,
    quality_repair_guidance: &str,
    quality_trend_guidance: &str,
    template_content: &str,
) -> Result<String, WizardOutlineGenerationError> {
    let mut params: HashMap<String, String> = HashMap::new();
    params.insert("title".into(), project.title.clone());
    params.insert(
        "theme".into(),
        project.theme.as_deref().unwrap_or("未设定").to_string(),
    );
    params.insert(
        "genre".into(),
        project.genre.as_deref().unwrap_or("通用").to_string(),
    );
    params.insert("chapter_count".into(), chapter_count.to_string());
    params.insert(
        "narrative_perspective".into(),
        request.narrative_perspective.unwrap_or("").to_string(),
    );
    params.insert(
        "target_words".into(),
        (request.target_words / 10).to_string(),
    );
    params.insert(
        "time_period".into(),
        project
            .world_time_period
            .as_deref()
            .unwrap_or("未设定")
            .to_string(),
    );
    params.insert(
        "location".into(),
        project
            .world_location
            .as_deref()
            .unwrap_or("未设定")
            .to_string(),
    );
    params.insert(
        "atmosphere".into(),
        project
            .world_atmosphere
            .as_deref()
            .unwrap_or("未设定")
            .to_string(),
    );
    params.insert(
        "rules".into(),
        project
            .world_rules
            .as_deref()
            .unwrap_or("未设定")
            .to_string(),
    );
    params.insert("characters_info".into(), characters_info);
    params.insert("mcp_references".into(), String::new());

    let project_long_term_goal = build_project_long_term_goal(
        project.theme.as_deref(),
        project.description.as_deref(),
        request
            .story_creation_brief
            .or(project.default_story_creation_brief.as_deref()),
        project
            .chapter_count
            .and_then(|value| usize::try_from(value).ok()),
        usize::try_from(request.target_words)
            .ok()
            .filter(|value| *value > 0),
    );
    params.insert(
        "requirements".into(),
        build_wizard_outline_requirements(
            request.requirements,
            chapter_count,
            request.creative_mode,
            request.story_focus,
            request.plot_stage,
            request.story_creation_brief,
            request.quality_preset,
            request.quality_notes,
            project_long_term_goal.as_deref(),
            usize::try_from(request.target_words)
                .ok()
                .filter(|value| *value > 0),
            Some(quality_repair_guidance),
            Some(quality_trend_guidance),
            request.compact_mode,
        ),
    );
    params.insert("external_assets".into(), String::new());
    params.insert("reference_assets".into(), String::new());
    params.insert(
        "creative_mode".into(),
        request.creative_mode.unwrap_or("").to_string(),
    );
    params.insert(
        "story_focus".into(),
        request.story_focus.unwrap_or("").to_string(),
    );
    params.insert(
        "plot_stage".into(),
        request.plot_stage.unwrap_or("").to_string(),
    );
    params.insert(
        "story_creation_brief".into(),
        request.story_creation_brief.unwrap_or("").to_string(),
    );
    params.insert(
        "quality_preset".into(),
        request.quality_preset.unwrap_or("").to_string(),
    );
    params.insert(
        "quality_notes".into(),
        request.quality_notes.unwrap_or("").to_string(),
    );

    PromptTemplateService::format_prompt(template_content, &params)
        .map_err(|_| WizardOutlineGenerationError::PromptFormat)
}

fn build_characters_info(characters: &[character::Model]) -> String {
    if characters.is_empty() {
        return "暂无角色信息".to_string();
    }

    characters
        .iter()
        .map(|character| {
            format!(
                "- {}（{}，{}）: {}",
                character.name,
                if character.is_organization {
                    "组织"
                } else {
                    "角色"
                },
                character.role_type.as_deref().unwrap_or("未知"),
                character
                    .personality
                    .as_deref()
                    .unwrap_or("暂无描述")
                    .chars()
                    .take(100)
                    .collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn emit_progress<P, PFut>(
    on_progress: &mut P,
    message: &str,
    progress: u32,
    status: &'static str,
) -> Result<(), WizardOutlineGenerationError>
where
    P: FnMut(OutlineGenerationProgress) -> PFut,
    PFut: Future<Output = Result<(), String>>,
{
    on_progress(OutlineGenerationProgress {
        message: message.to_string(),
        progress,
        status,
    })
    .await
    .map_err(|_| WizardOutlineGenerationError::Observer)
}

fn ensure_not_cancelled(
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<(), WizardOutlineGenerationError> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        Err(WizardOutlineGenerationError::Cancelled)
    } else {
        Ok(())
    }
}

fn parse_generated_outline_plan(
    raw_content: &str,
    outline_mode: &str,
    provider: &str,
    model: &str,
    attempts: u32,
    requested_chapter_count: usize,
) -> Result<GeneratedOutlinePlan, WizardOutlineGenerationError> {
    let cleaned = clean_json_response(raw_content);
    if cleaned.trim().is_empty() {
        return Err(WizardOutlineGenerationError::EmptyResponse);
    }
    let data = serde_json::from_str::<Value>(&cleaned)
        .map_err(|_| WizardOutlineGenerationError::InvalidResponse)?;
    let raw_items = normalize_outline_items(&data);
    if raw_items.is_empty() {
        return Err(WizardOutlineGenerationError::EmptyResponse);
    }

    let mut indexed_outlines = raw_items
        .into_iter()
        .enumerate()
        .map(|(index, item)| parse_outline_item(item, index))
        .collect::<Result<Vec<_>, _>>()?;
    indexed_outlines
        .sort_by_key(|(original_index, outline)| (outline.chapter_number, *original_index));

    let mut seen_chapter_numbers = HashSet::new();
    for (_, outline) in &indexed_outlines {
        if !seen_chapter_numbers.insert(outline.chapter_number) {
            return Err(WizardOutlineGenerationError::DuplicateChapterNumber(
                outline.chapter_number,
            ));
        }
    }

    let limit = requested_chapter_count
        .clamp(1, MAX_OUTLINE_CHAPTERS_PER_REQUEST)
        .min(indexed_outlines.len());
    let outlines = indexed_outlines
        .into_iter()
        .take(limit)
        .map(|(_, outline)| outline)
        .collect::<Vec<_>>();
    if outlines.is_empty() {
        return Err(WizardOutlineGenerationError::EmptyResponse);
    }

    let suggested_pending_chapters = if outline_mode == "one-to-one" {
        outlines
            .iter()
            .enumerate()
            .map(|(outline_index, outline)| GeneratedPendingChapter {
                chapter_number: outline.chapter_number,
                title: outline.title.clone(),
                summary: outline.summary.clone(),
                outline_index,
            })
            .collect()
    } else {
        Vec::new()
    };

    #[derive(Serialize)]
    struct DigestInput<'a> {
        outlines: &'a [GeneratedOutlineItem],
        outline_mode: &'a str,
        suggested_pending_chapters: &'a [GeneratedPendingChapter],
    }
    let digest_input = serde_json::to_vec(&DigestInput {
        outlines: &outlines,
        outline_mode,
        suggested_pending_chapters: &suggested_pending_chapters,
    })
    .map_err(|_| WizardOutlineGenerationError::InvalidResponse)?;

    Ok(GeneratedOutlinePlan {
        outlines,
        outline_mode: outline_mode.to_string(),
        suggested_pending_chapters,
        provider: provider.to_string(),
        model: model.to_string(),
        attempts,
        content_digest: format!("{:x}", md5::compute(digest_input)),
    })
}

fn parse_outline_item(
    item: Value,
    original_index: usize,
) -> Result<(usize, GeneratedOutlineItem), WizardOutlineGenerationError> {
    let data = item
        .as_object()
        .ok_or(WizardOutlineGenerationError::InvalidResponse)?;
    let fallback_chapter_number = i32::try_from(original_index + 1)
        .map_err(|_| WizardOutlineGenerationError::InvalidResponse)?;
    let chapter_number = positive_i32(data, &["chapter_number", "chapter", "number"])
        .unwrap_or(fallback_chapter_number);
    let title =
        non_empty_string(data, &["title"]).unwrap_or_else(|| format!("第{chapter_number}章"));
    let summary = non_empty_string(data, &["summary", "content"])
        .ok_or(WizardOutlineGenerationError::IncompleteResponse)?;
    let content = build_outline_content(&item);

    Ok((
        original_index,
        GeneratedOutlineItem {
            chapter_number,
            title,
            summary,
            content,
            scenes: string_list(data.get("scenes")),
            characters: parse_character_refs(data.get("characters")),
            key_points: string_list(data.get("key_points").or_else(|| data.get("key_events"))),
            emotion: non_empty_string(data, &["emotion"]),
            narrative_goal: non_empty_string(data, &["narrative_goal", "goal"]),
            conflict_line: non_empty_string(data, &["conflict_line", "conflict", "conflict_type"]),
            decision: non_empty_string(data, &["decision", "dilemma"]),
            cost: non_empty_string(data, &["cost", "stakes"]),
            rule_impact: non_empty_string(data, &["rule_impact", "world_rule_trigger"]),
            dialogue_hook: non_empty_string(data, &["dialogue_hook"]),
            character_turns: string_list(
                data.get("character_turns")
                    .or_else(|| data.get("character_arc"))
                    .or_else(|| data.get("twist")),
            ),
            suggested_target_words: positive_i32(data, &["suggested_target_words", "target_words"]),
        },
    ))
}

fn non_empty_string(data: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        data.get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn positive_i32(data: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<i32> {
    keys.iter().find_map(|key| {
        let number = data.get(*key)?.as_i64()?;
        i32::try_from(number).ok().filter(|number| *number > 0)
    })
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect(),
        Some(Value::String(value)) => value
            .split(['\n', '；'])
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_character_refs(value: Option<&Value>) -> Vec<GeneratedOutlineCharacterRef> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| {
            if let Some(name) = item.as_str().map(str::trim).filter(|name| !name.is_empty()) {
                return Some(GeneratedOutlineCharacterRef {
                    name: name.to_string(),
                    character_type: None,
                });
            }
            let data = item.as_object()?;
            let name = non_empty_string(data, &["name"])?;
            Some(GeneratedOutlineCharacterRef {
                name,
                character_type: non_empty_string(data, &["type", "character_type"]),
            })
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::{ensure_not_cancelled, parse_generated_outline_plan, WizardOutlineGenerationError};
    use crate::services::cooperative_cancellation_service::{
        CooperativeCancellationRegistry, CooperativeCancellationScope,
    };

    const VALID_OUTLINE: &str = r#"[
        {
            "chapter_number": 2,
            "title": "风暴前夜",
            "summary": "主角发现城外的异常星光，并在追查中与守卫发生冲突。",
            "scenes": ["城墙", "守卫室"],
            "characters": [{"name": "林舟", "type": "character"}],
            "key_points": ["发现异常", "被迫选择"],
            "emotion": "紧张",
            "goal": "查明异常来源"
        },
        {
            "chapter_number": 1,
            "title": "星门异动",
            "summary": "星门突然开启，主角必须在逃离和救人之间作出选择。",
            "dialogue_hook": "你只能带走一个人。",
            "character_turns": ["胆怯的同伴返回救人"]
        }
    ]"#;

    #[test]
    fn parses_fenced_chapter_wrapper_into_typed_plan() {
        let wrapped = format!("```json\n{{\"chapters\":{VALID_OUTLINE}}}\n```");
        let result =
            parse_generated_outline_plan(&wrapped, "one-to-one", "openai", "model-1", 2, 2)
                .expect("valid outline plan");

        assert_eq!(result.outlines.len(), 2);
        assert_eq!(result.outlines[0].chapter_number, 1);
        assert_eq!(result.outlines[1].chapter_number, 2);
        assert_eq!(result.provider, "openai");
        assert_eq!(result.model, "model-1");
        assert_eq!(result.attempts, 2);
        assert_eq!(result.suggested_pending_chapters.len(), 2);
        assert_eq!(result.suggested_pending_chapters[0].outline_index, 0);
        assert!(!result.content_digest.is_empty());
    }

    #[test]
    fn sorts_outline_items_by_chapter_number() {
        let result =
            parse_generated_outline_plan(VALID_OUTLINE, "detail", "provider", "model", 1, 2)
                .expect("valid outline plan");

        assert_eq!(
            result
                .outlines
                .iter()
                .map(|outline| outline.chapter_number)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert!(result.suggested_pending_chapters.is_empty());
    }

    #[test]
    fn duplicate_chapter_number_returns_explicit_error() {
        let duplicate = r#"[
            {"chapter_number": 1, "title": "第一章", "summary": "事件一"},
            {"chapter": 1, "title": "重复章", "summary": "事件二"}
        ]"#;
        let error =
            parse_generated_outline_plan(duplicate, "one-to-one", "provider", "model", 1, 2)
                .expect_err("duplicate chapter number must fail");

        assert_eq!(
            error,
            WizardOutlineGenerationError::DuplicateChapterNumber(1)
        );
        assert_eq!(error.code(), "outline_generation_duplicate_chapter_number");
    }

    #[test]
    fn missing_or_non_positive_chapter_number_uses_input_order() {
        let raw = r#"[
            {"chapter_number": 0, "summary": "第一章概要"},
            {"title": "第二章", "summary": "第二章概要"}
        ]"#;
        let result = parse_generated_outline_plan(raw, "detail", "provider", "model", 1, 2)
            .expect("fallback chapter numbers");

        assert_eq!(result.outlines[0].chapter_number, 1);
        assert_eq!(result.outlines[0].title, "第1章");
        assert_eq!(result.outlines[1].chapter_number, 2);
    }

    #[test]
    fn digest_is_stable_for_same_normalized_plan() {
        let first = parse_generated_outline_plan(
            VALID_OUTLINE,
            "one-to-one",
            "provider-a",
            "model-a",
            1,
            2,
        )
        .expect("first plan");
        let second = parse_generated_outline_plan(
            VALID_OUTLINE,
            "one-to-one",
            "provider-b",
            "model-b",
            2,
            2,
        )
        .expect("second plan");

        assert_eq!(first.content_digest, second.content_digest);
    }

    #[test]
    fn cancelled_token_returns_stable_error() {
        let registry = CooperativeCancellationRegistry::default();
        let registration = registry.register(
            CooperativeCancellationScope::BackgroundTask,
            "outline-generation-test",
        );
        let token = registration.token();
        token.cancel();

        let error = ensure_not_cancelled(Some(&token)).expect_err("cancelled token");
        assert_eq!(error, WizardOutlineGenerationError::Cancelled);
        assert_eq!(error.code(), "outline_generation_cancelled");
    }
}
