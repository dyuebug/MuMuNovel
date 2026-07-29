use std::cmp::{max, min};

use serde_json::Value;

use crate::ai::service::AIService;
use crate::models::chapter;
use crate::services::chapter_generation_execution_contract_service::{
    prepare_role_aware_generation_execution_config_with_provider_payload,
    PreparedGenerationExecutionConfig, PreparedRoleModelPolicyContext,
};
use crate::services::chapter_generation_prompt_service::{
    PreviousChapterPromptContext, PromptContextProviderPayload,
};
use crate::services::chapter_generation_runtime_service::context_compaction_owner::compact_generation_context;
use crate::services::chapter_generation_runtime_service::runtime_execution_owner::load_generation_context;
use crate::services::chapter_single_generation_prepare_service::research_payload_owner::build_single_chapter_research_provider_payload;
use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationTarget;
use crate::services::generation_contract_service::{
    GenerationContractSnapshotV1, GenerationIntentKind,
};
use crate::services::settings_service::SettingsService;
use crate::services::writing_style_service::WritingStyleService;

use super::contract_prepare_owner::{
    build_full_chapter_regeneration_contract_snapshot,
    build_partial_chapter_regeneration_contract_snapshot,
};
use super::request_prepare_owner::{
    build_partial_length_requirement, calculate_partial_target_words,
    BuildRegenerationAiServiceError, FullChapterRegenerationStreamRequest,
    PartialRegenerationStreamWorkflowRequest, PreparePartialRegenerationError,
    PreparePartialRegenerationStreamError,
};

pub struct FullChapterRegenerationStreamInput {
    pub chapter: chapter::Model,
    pub user_id: String,
    pub request: FullChapterRegenerationStreamRequest,
    pub resolved_style_id: Option<i32>,
    pub chapter_id: String,
    pub chapter_word_count: usize,
    pub prompt: String,
    pub ai_service: AIService,
    pub role_policy_context: PreparedRoleModelPolicyContext,
    pub generation_contract: GenerationContractSnapshotV1,
}

pub struct PartialChapterRegenerationStreamInput {
    pub target_words: usize,
    pub original_word_count: usize,
    pub start_position: usize,
    pub end_position: usize,
    pub prompt: String,
    pub ai_service: AIService,
    pub role_policy_context: PreparedRoleModelPolicyContext,
    pub generation_contract: GenerationContractSnapshotV1,
}

pub struct PreparedPartialRegenerationInput {
    pub original_word_count: usize,
    pub target_words: usize,
    pub max_tokens: u32,
    pub selected_text: String,
    pub prompt: String,
}

fn join_regeneration_prompt_items(items: &[String], separator: &str) -> String {
    items.join(separator)
}

fn build_regeneration_external_assets_block(
    external_assets: Option<&str>,
    reference_assets: Option<&str>,
) -> String {
    let external_assets = external_assets.unwrap_or_default().trim();
    let reference_assets = reference_assets.unwrap_or_default().trim();
    if (external_assets.is_empty() || external_assets == "[]")
        && (reference_assets.is_empty() || reference_assets == "[]")
    {
        return "（未提供）".to_string();
    }

    let mut lines = Vec::new();
    if !external_assets.is_empty() && external_assets != "[]" {
        lines.push(format!("external_assets: {}", external_assets));
    }
    if !reference_assets.is_empty() && reference_assets != "[]" {
        lines.push(format!("reference_assets: {}", reference_assets));
    }

    if lines.is_empty() {
        "（未提供）".to_string()
    } else {
        lines.join("\n")
    }
}

pub fn build_regeneration_prompt(
    chapter: &chapter::Model,
    request: &FullChapterRegenerationStreamRequest,
    provider_payload: &PromptContextProviderPayload,
    web_research_note: Option<&str>,
    external_assets: Option<&str>,
    reference_assets: Option<&str>,
) -> String {
    let web_research_note = web_research_note.unwrap_or("（未启用）");
    let external_assets_block =
        build_regeneration_external_assets_block(external_assets, reference_assets);
    format!(
        "你是小说正文重写助手。请基于以下章节内容和要求输出重写后的正文，不要输出解释。\n\n章节标题：{}\n章节编号：{}\n目标字数：{}\n\n原章节内容：\n{}\n\n用户修改要求：\n{}\n\n选中建议索引：{}\n重点优化方向：{}\n创作模式：{}\n故事关注点：{}\n质量预设：{}\n\n最近章节规划：\n{}\n\n上一章已完成剧情：\n{}\n\n本章角色信息：\n{}\n\n本章职业信息：\n{}\n\n伏笔提醒：\n{}\n\n相关记忆：\n{}\n\n联网检索说明：{}\n外部参考资料：\n{}\n保留结构：{}\n保留对话：{}\n保留剧情点：{}\n保留人物特征：{}\n创作总控：{}\n质量补充偏好：{}\n剧情质量修复摘要：{}\n修复目标：{}\n保留优势：{}\n\n要求：\n- 只输出可直接替换的正文内容\n- 不要输出标题、编号、前言、后记或流程说明\n- 如果有角色/世界观信息，保持一致\n- 尽量保留原有剧情骨架",
        chapter.title,
        chapter.chapter_number,
        request.target_word_count(),
        chapter.content.clone().unwrap_or_default(),
        request.custom_instructions(),
        join_regeneration_prompt_items(request.selected_suggestion_indices(), ", "),
        join_regeneration_prompt_items(request.focus_areas(), "、"),
        request.creative_mode(),
        request.story_focus(),
        request.quality_preset(),
        if provider_payload.recent_chapters_context.trim().is_empty() {
            "（未提供）"
        } else {
            provider_payload.recent_chapters_context.as_str()
        },
        if provider_payload.previous_chapter_summary.trim().is_empty() {
            "（未提供）"
        } else {
            provider_payload.previous_chapter_summary.as_str()
        },
        if provider_payload.characters_info.trim().is_empty()
            || provider_payload.characters_info == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.characters_info.as_str()
        },
        if provider_payload.chapter_careers.trim().is_empty()
            || provider_payload.chapter_careers == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.chapter_careers.as_str()
        },
        if provider_payload.foreshadow_reminders.trim().is_empty()
            || provider_payload.foreshadow_reminders == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.foreshadow_reminders.as_str()
        },
        if provider_payload.relevant_memories.trim().is_empty()
            || provider_payload.relevant_memories == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.relevant_memories.as_str()
        },
        web_research_note,
        external_assets_block,
        request.preserve_structure(),
        join_regeneration_prompt_items(request.preserve_dialogues(), "、"),
        join_regeneration_prompt_items(request.preserve_plot_points(), "、"),
        request.preserve_character_traits(),
        request.story_creation_brief(),
        request.quality_notes(),
        request.story_repair_summary(),
        join_regeneration_prompt_items(request.story_repair_targets(), "、"),
        join_regeneration_prompt_items(request.story_preserve_strengths(), "、"),
    )
}

pub fn build_partial_regeneration_prompt(
    chapter: &chapter::Model,
    selected_text: &str,
    context_before: &str,
    context_after: &str,
    user_instructions: &str,
    length_requirement: &str,
    style_content: Option<&str>,
    provider_payload: &PromptContextProviderPayload,
    web_research_note: Option<&str>,
    external_assets: Option<&str>,
    reference_assets: Option<&str>,
) -> String {
    let style_content = style_content.unwrap_or("（未提供风格约束）");
    let web_research_note = web_research_note.unwrap_or("（未启用）");
    let external_assets_block =
        build_regeneration_external_assets_block(external_assets, reference_assets);

    format!(
        "你是小说正文局部重写助手。请基于以下内容重写选中片段，只输出可直接替换的正文内容，不要输出解释。\n\n章节标题：{}\n章节编号：{}\n原文选中片段：\n{}\n\n前文上下文：\n{}\n\n后文上下文：\n{}\n\n用户修改要求：\n{}\n\n长度要求：{}\n\n风格约束：\n{}\n\n上一章已完成剧情：\n{}\n\n本章角色信息：\n{}\n\n本章职业信息：\n{}\n\n伏笔提醒：\n{}\n\n相关记忆：\n{}\n\n联网检索说明：{}\n\n外部参考资料：\n{}\n\n要求：\n- 只输出重写后的正文\n- 不要输出标题、编号、前言、后记或流程说明\n- 保持人物、设定与上下文一致\n- 尽量贴合原文节奏与叙事视角",
        chapter.title,
        chapter.chapter_number,
        selected_text,
        if context_before.is_empty() {
            "（无前文上下文）"
        } else {
            context_before
        },
        if context_after.is_empty() {
            "（无后文上下文）"
        } else {
            context_after
        },
        if user_instructions.is_empty() {
            "（无额外要求）"
        } else {
            user_instructions
        },
        length_requirement,
        style_content,
        if provider_payload.previous_chapter_summary.trim().is_empty() {
            "（未提供）"
        } else {
            provider_payload.previous_chapter_summary.as_str()
        },
        if provider_payload.characters_info.trim().is_empty()
            || provider_payload.characters_info == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.characters_info.as_str()
        },
        if provider_payload.chapter_careers.trim().is_empty()
            || provider_payload.chapter_careers == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.chapter_careers.as_str()
        },
        if provider_payload.foreshadow_reminders.trim().is_empty()
            || provider_payload.foreshadow_reminders == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.foreshadow_reminders.as_str()
        },
        if provider_payload.relevant_memories.trim().is_empty()
            || provider_payload.relevant_memories == "[]"
        {
            "（未提供）"
        } else {
            provider_payload.relevant_memories.as_str()
        },
        web_research_note,
        external_assets_block,
    )
}

pub fn prepare_partial_regeneration_input(
    chapter: &chapter::Model,
    selected_text_override: &str,
    start_position: usize,
    end_position: usize,
    context_chars: usize,
    user_instructions: &str,
    length_mode: Option<&str>,
    target_word_count: Option<usize>,
    style_content: Option<&str>,
    provider_payload: &PromptContextProviderPayload,
    web_research_note: Option<&str>,
    external_assets: Option<&str>,
    reference_assets: Option<&str>,
) -> Result<PreparedPartialRegenerationInput, PreparePartialRegenerationError> {
    let current_content = chapter.content.clone().unwrap_or_default();
    let content_chars: Vec<char> = current_content.chars().collect();
    let content_length = content_chars.len();
    if start_position >= end_position || end_position > content_length {
        return Err(PreparePartialRegenerationError::InvalidRange);
    }

    let selected_text_from_content: String =
        content_chars[start_position..end_position].iter().collect();
    let selected_text = {
        let provided = selected_text_override.trim();
        if provided.is_empty() {
            selected_text_from_content
        } else {
            provided.to_string()
        }
    };
    if selected_text.trim().is_empty() {
        return Err(PreparePartialRegenerationError::EmptySelectedText);
    }

    let context_before_start = start_position.saturating_sub(context_chars);
    let context_before: String = content_chars[context_before_start..start_position]
        .iter()
        .collect();
    let context_after_end = end_position
        .saturating_add(context_chars)
        .min(content_length);
    let context_after: String = content_chars[end_position..context_after_end]
        .iter()
        .collect();

    let original_word_count = selected_text.chars().count();
    let length_requirement =
        build_partial_length_requirement(length_mode, target_word_count, original_word_count);
    let target_words =
        calculate_partial_target_words(length_mode, target_word_count, original_word_count);
    let max_tokens = max(500, min(target_words.saturating_mul(3), 8000)) as u32;
    let prompt = build_partial_regeneration_prompt(
        chapter,
        &selected_text,
        &context_before,
        &context_after,
        user_instructions,
        &length_requirement,
        style_content,
        provider_payload,
        web_research_note,
        external_assets,
        reference_assets,
    );

    Ok(PreparedPartialRegenerationInput {
        original_word_count,
        target_words,
        max_tokens,
        selected_text,
        prompt,
    })
}

pub async fn build_regeneration_ai_service(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    max_tokens_override: Option<u32>,
) -> Result<AIService, BuildRegenerationAiServiceError> {
    let mut ai_config = SettingsService::build_ai_config(db, user_id, None, None, None)
        .await
        .map_err(BuildRegenerationAiServiceError::InvalidConfig)?;
    if let Some(max_tokens) = max_tokens_override {
        ai_config.max_tokens = max_tokens;
    }
    Ok(AIService::new(ai_config))
}

async fn build_role_aware_regeneration_execution_config(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    intent_kind: GenerationIntentKind,
    provider_payload: PromptContextProviderPayload,
    max_tokens_override: Option<u32>,
) -> Result<PreparedGenerationExecutionConfig, BuildRegenerationAiServiceError> {
    let mut prepared = prepare_role_aware_generation_execution_config_with_provider_payload(
        db,
        user_id,
        intent_kind,
        None,
        provider_payload,
    )
    .await
    .map_err(BuildRegenerationAiServiceError::InvalidConfig)?;
    if let Some(max_tokens) = max_tokens_override {
        prepared.ai_config.max_tokens = max_tokens;
    }
    Ok(prepared)
}

pub async fn load_partial_style_content(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    style_id: Option<i32>,
) -> Result<Option<String>, String> {
    let Some(style_id) = style_id else {
        return Ok(None);
    };

    let value = WritingStyleService::get_style(db, user_id, style_id)
        .await
        .map_err(|error| error.to_string())?;

    Ok(value
        .get("prompt_content")
        .and_then(Value::as_str)
        .map(str::to_string))
}

pub async fn prepare_chapter_regeneration_stream(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter: &chapter::Model,
    request: &FullChapterRegenerationStreamRequest,
) -> Result<FullChapterRegenerationStreamInput, BuildRegenerationAiServiceError> {
    request.validate_request_bounds()?;

    let web_research_default = SettingsService::resolve_web_research_enabled(db, user_id)
        .await
        .map_err(|error| BuildRegenerationAiServiceError::InvalidConfig(error.to_string()))?;
    let compat_options = request.compat_options_with_web_research_default(web_research_default);
    let provider_payload = build_single_chapter_research_provider_payload(
        db,
        user_id,
        &SingleChapterGenerationTarget {
            project_id: chapter.project_id.clone(),
            chapter_id: chapter.id.clone(),
            chapter_number: chapter.chapter_number,
            title: chapter.title.clone(),
        },
        &compat_options,
    )
    .await
    .map_err(BuildRegenerationAiServiceError::InvalidConfig)?;
    let (provider_payload, _) = compact_generation_context(
        "one-to-many",
        request.target_word_count() as i32,
        provider_payload,
        PreviousChapterPromptContext::default(),
    );
    let web_research_note = if compat_options.web_research_enabled() {
        compat_options
            .web_research_query()
            .map(|query| format!("已请求联网检索，检索问题：{}", query))
            .or_else(|| Some("已请求联网检索，请优先吸收外部资料中的事实与细节。".to_string()))
    } else {
        None
    };
    let prompt = build_regeneration_prompt(
        chapter,
        request,
        &provider_payload,
        web_research_note.as_deref(),
        Some(&provider_payload.external_assets),
        Some(&provider_payload.reference_assets),
    );
    let generation_context = load_generation_context(db, user_id, &chapter.id)
        .await
        .map_err(|error| {
            BuildRegenerationAiServiceError::InvalidConfig(error.into_runtime_message())
        })?;
    let generation_contract = build_full_chapter_regeneration_contract_snapshot(
        &generation_context.project_model,
        generation_context.story_packet,
        request,
        web_research_default,
    )
    .map_err(|error| BuildRegenerationAiServiceError::InvalidConfig(error.to_string()))?;
    let prepared_execution = build_role_aware_regeneration_execution_config(
        db,
        user_id,
        GenerationIntentKind::ChapterRegenerate,
        provider_payload,
        None,
    )
    .await?;
    let role_policy_context = prepared_execution.role_policy_context.ok_or_else(|| {
        BuildRegenerationAiServiceError::InvalidConfig(
            "Chapter regeneration role policy context is missing".to_string(),
        )
    })?;
    let ai_service = AIService::new(prepared_execution.ai_config);

    Ok(FullChapterRegenerationStreamInput {
        chapter: chapter.clone(),
        user_id: user_id.to_string(),
        request: request.clone(),
        resolved_style_id: request.style_id(),
        chapter_id: chapter.id.clone(),
        chapter_word_count: chapter.word_count as usize,
        prompt,
        ai_service,
        role_policy_context,
        generation_contract,
    })
}

pub async fn prepare_partial_regeneration_stream(
    db: &sea_orm::DatabaseConnection,
    user_id: &str,
    chapter: &chapter::Model,
    request: &PartialRegenerationStreamWorkflowRequest,
) -> Result<PartialChapterRegenerationStreamInput, PreparePartialRegenerationStreamError> {
    request
        .validate_request_bounds()
        .map_err(PreparePartialRegenerationStreamError::Input)?;

    let style_content = load_partial_style_content(db, user_id, request.style_id())
        .await
        .map_err(PreparePartialRegenerationStreamError::Style)?;

    let web_research_default = SettingsService::resolve_web_research_enabled(db, user_id)
        .await
        .map_err(|error| {
            PreparePartialRegenerationStreamError::Config(
                BuildRegenerationAiServiceError::InvalidConfig(error.to_string()),
            )
        })?;
    let compat_options = request.compat_options_with_web_research_default(web_research_default);
    let provider_payload = build_single_chapter_research_provider_payload(
        db,
        user_id,
        &SingleChapterGenerationTarget {
            project_id: chapter.project_id.clone(),
            chapter_id: chapter.id.clone(),
            chapter_number: chapter.chapter_number,
            title: chapter.title.clone(),
        },
        &compat_options,
    )
    .await
    .map_err(|error| {
        PreparePartialRegenerationStreamError::Config(
            BuildRegenerationAiServiceError::InvalidConfig(error),
        )
    })?;
    let (provider_payload, _) = compact_generation_context(
        "one-to-one",
        request
            .target_word_count()
            .unwrap_or(chapter.word_count as usize) as i32,
        provider_payload,
        PreviousChapterPromptContext::default(),
    );

    let web_research_note = if compat_options.web_research_enabled() {
        compat_options
            .web_research_query()
            .map(|query| format!("已请求联网检索，检索问题：{}", query))
            .or_else(|| Some("已请求联网检索，请优先吸收外部资料中的事实与细节。".to_string()))
    } else {
        None
    };

    let prepared = prepare_partial_regeneration_input(
        chapter,
        request.selected_text(),
        request.start_position(),
        request.end_position(),
        request.context_chars(),
        request.user_instructions(),
        request.length_mode(),
        request.target_word_count(),
        style_content.as_deref(),
        &provider_payload,
        web_research_note.as_deref(),
        Some(&provider_payload.external_assets),
        Some(&provider_payload.reference_assets),
    )
    .map_err(PreparePartialRegenerationStreamError::Input)?;

    let generation_context = load_generation_context(db, user_id, &chapter.id)
        .await
        .map_err(|error| {
            PreparePartialRegenerationStreamError::Config(
                BuildRegenerationAiServiceError::InvalidConfig(error.into_runtime_message()),
            )
        })?;
    let generation_contract = build_partial_chapter_regeneration_contract_snapshot(
        &generation_context.project_model,
        generation_context.story_packet,
        request,
        prepared.selected_text.clone(),
        prepared.target_words,
        style_content.as_deref(),
        web_research_default,
    )
    .map_err(|error| {
        PreparePartialRegenerationStreamError::Config(
            BuildRegenerationAiServiceError::InvalidConfig(error.to_string()),
        )
    })?;
    let prepared_execution = build_role_aware_regeneration_execution_config(
        db,
        user_id,
        GenerationIntentKind::ChapterPartialRegenerate,
        provider_payload,
        Some(prepared.max_tokens),
    )
    .await
    .map_err(PreparePartialRegenerationStreamError::Config)?;
    let role_policy_context = prepared_execution.role_policy_context.ok_or_else(|| {
        PreparePartialRegenerationStreamError::Config(
            BuildRegenerationAiServiceError::InvalidConfig(
                "Partial chapter regeneration role policy context is missing".to_string(),
            ),
        )
    })?;
    let ai_service = AIService::new(prepared_execution.ai_config);

    Ok(PartialChapterRegenerationStreamInput {
        target_words: prepared.target_words,
        original_word_count: prepared.original_word_count,
        start_position: request.start_position(),
        end_position: request.end_position(),
        prompt: prepared.prompt,
        ai_service,
        role_policy_context,
        generation_contract,
    })
}
