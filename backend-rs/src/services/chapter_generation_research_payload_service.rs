use std::path::PathBuf;
use std::sync::OnceLock;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ai::clients::openai::OpenAIClient;
use crate::ai::types::ChatMessage;
use crate::config::load as load_app_config;
use crate::models::{career, chapter, character, character_career, foreshadow, story_memory};
use crate::services::chapter_generation_prompt_context_provider_service::{
    build_placeholder_prompt_context_provider_payload, PromptContextProviderPayload,
};
use crate::services::settings_service::SettingsService;
use crate::services::settings_runtime_config_service::normalize_openai_compatible_base_url;
use crate::services::story_memory_vector_index_service::{
    delete_story_memory_vector_records_by_types, search_story_memory_vector_records,
    upsert_story_memory_vector_record,
};

use super::chapter_single_generation_prepare_service::{
    SingleChapterGenerationCompatOptions, SingleChapterGenerationTarget,
};

const DEFAULT_EXA_BASE_URL: &str = "https://api.exa.ai";
const DEFAULT_GROK_BASE_URL: &str = "https://api.x.ai/v1";
const DEFAULT_GROK_MODEL: &str = "grok-4.1-fast";
const DEFAULT_MAX_ASSETS: usize = 2;
const GENERATION_MEMORY_COUNT: usize = 10;
const GENERATION_MEMORY_RANK_LIMIT: usize = 15;
const GENERATION_MEMORY_PREVIEW_LENGTH: usize = 76;
const GENERATION_MEMORY_TOTAL_CHARS_BUDGET: usize = 860;
const GENERATION_MEMORY_SIMILARITY_THRESHOLD: f64 = 0.6;
const RESEARCH_MEMORY_TYPE: &str = "research_reference";
const GENERATION_MEMORY_TYPES: &[&str] = &[
    "plot_point",
    "character_event",
    "hook",
    "world_detail",
    "chapter_summary",
];
const GENERATION_MEMORY_TYPE_COVERAGE_PRIORITY: &[&str] =
    &["plot_point", "character_event", "hook", "world_detail", "chapter_summary"];
const GENERATION_CHARACTER_LIMIT: u64 = 10;
const GENERATION_FORESHADOW_LIMIT: usize = 3;
const GENERATION_CHARACTER_ARC_LIMIT: usize = 5;
const GENERATION_CHARACTER_ARC_MEMORY_LIMIT: u64 = 80;
const GENERATION_CHARACTER_ARC_MEMORIES_PER_CHARACTER: usize = 2;

fn character_role_priority(role_type: Option<&str>) -> i32 {
    match role_type.unwrap_or_default() {
        "protagonist" => 4,
        "antagonist" => 3,
        "supporting" => 2,
        _ => 1,
    }
}

fn character_status_label(status: &str) -> &str {
    match status {
        "dead" => "死亡",
        "missing" => "失踪",
        "injured" => "受伤",
        "retired" => "退场",
        "captured" => "被控制",
        _ => status,
    }
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("research http client should build")
    })
}

fn normalize_research_text(value: impl AsRef<str>, limit: usize) -> String {
    let text = value
        .as_ref()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if text.chars().count() <= limit {
        return text;
    }

    text.chars()
        .take(limit.saturating_sub(3))
        .collect::<String>()
        + "..."
}

fn compose_single_chapter_research_query(
    chapter_target: &SingleChapterGenerationTarget,
    compat_options: &SingleChapterGenerationCompatOptions,
) -> String {
    if let Some(custom_query) = compat_options.web_research_query() {
        let normalized = normalize_research_text(custom_query, 320);
        if !normalized.is_empty() {
            return normalized;
        }
    }

    let mut parts = vec![format!(
        "小说章节创作背景资料 | 第{}章《{}》",
        chapter_target.chapter_number, chapter_target.title
    )];

    let story_brief = normalize_research_text(compat_options.story_creation_brief(), 120);
    if !story_brief.is_empty() {
        parts.push(story_brief);
    }

    let story_focus = normalize_research_text(compat_options.story_focus(), 80);
    if !story_focus.is_empty() {
        parts.push(format!("故事侧重点：{}", story_focus));
    }

    let plot_stage = normalize_research_text(compat_options.plot_stage(), 80);
    if !plot_stage.is_empty() {
        parts.push(format!("剧情阶段：{}", plot_stage));
    }

    normalize_research_text(parts.join(" | "), 320)
}

fn compose_single_chapter_grok_query(
    chapter_target: &SingleChapterGenerationTarget,
    compat_options: &SingleChapterGenerationCompatOptions,
) -> String {
    if let Some(custom_query) = compat_options.web_research_query() {
        let normalized = normalize_research_text(custom_query, 260);
        if !normalized.is_empty() {
            return format!(
                "请围绕以下小说创作主题进行实时网络研究，并给出来源：{}",
                normalized
            );
        }
    }

    let mut context_parts = Vec::new();
    context_parts.push(format!(
        "章节：第{}章《{}》",
        chapter_target.chapter_number, chapter_target.title
    ));

    let story_brief = normalize_research_text(compat_options.story_creation_brief(), 120);
    if !story_brief.is_empty() {
        context_parts.push(format!("创作总控摘要：{}", story_brief));
    }

    let story_focus = normalize_research_text(compat_options.story_focus(), 80);
    if !story_focus.is_empty() {
        context_parts.push(format!("故事侧重点：{}", story_focus));
    }

    let plot_stage = normalize_research_text(compat_options.plot_stage(), 80);
    if !plot_stage.is_empty() {
        context_parts.push(format!("剧情阶段：{}", plot_stage));
    }

    if context_parts.is_empty() {
        return String::new();
    }

    normalize_research_text(
        format!(
            "请为小说章节创作做实时网络研究，优先提炼事实、职业细节、社会情绪与可借鉴表达，并保留来源。背景：{}",
            context_parts.join("；")
        ),
        320,
    )
}

fn memory_type_display_label(memory_type: &str) -> &str {
    match memory_type {
        "plot_point" => "剧情节点",
        "character_event" => "角色事件",
        "hook" => "钩子",
        "world_detail" => "世界细节",
        "chapter_summary" => "章节摘要",
        "research_reference" => "研究资料",
        _ => "未分类",
    }
}

fn role_type_display_label(role_type: Option<&str>) -> &str {
    match role_type.unwrap_or_default() {
        "protagonist" => "主角",
        "antagonist" => "反派",
        "supporting" => "配角",
        _ => "未知",
    }
}

fn extract_query_focus_lines(value: &str, limit: usize, max_length: usize) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            line.trim_start_matches("- ")
                .trim_start_matches("【")
                .trim_end_matches("】")
                .trim()
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .take(limit)
        .map(|line| preview_text(&line, max_length))
        .collect()
}

fn compose_single_chapter_generation_memory_query(
    chapter_target: &SingleChapterGenerationTarget,
    compat_options: &SingleChapterGenerationCompatOptions,
    characters_info: Option<&str>,
    chapter_careers: Option<&str>,
    character_arc_snapshot: Option<&str>,
    foreshadow_reminders: Option<&str>,
) -> String {
    let mut parts = vec![format!(
        "本章标题：第{}章《{}》",
        chapter_target.chapter_number, chapter_target.title
    )];

    let story_brief = normalize_research_text(compat_options.story_creation_brief(), 320);
    if !story_brief.is_empty() {
        parts.push(format!("本章大纲：{}", story_brief));
    }

    let story_focus = normalize_research_text(compat_options.story_focus(), 120);
    if !story_focus.is_empty() {
        parts.push(format!("故事侧重点：{}", story_focus));
    }

    let plot_stage = normalize_research_text(compat_options.plot_stage(), 120);
    if !plot_stage.is_empty() {
        parts.push(format!("剧情阶段：{}", plot_stage));
    }

    let character_hints = characters_info
        .map(|value| extract_query_focus_lines(value, 3, 80))
        .unwrap_or_default();
    if !character_hints.is_empty() {
        parts.push(format!("角色状态：{}", character_hints.join("；")));
    }

    let career_hints = chapter_careers
        .map(|value| extract_query_focus_lines(value, 2, 80))
        .unwrap_or_default();
    if !career_hints.is_empty() {
        parts.push(format!("职业线索：{}", career_hints.join("；")));
    }

    let arc_hints = character_arc_snapshot
        .map(|value| extract_query_focus_lines(value, 3, 80))
        .unwrap_or_default();
    if !arc_hints.is_empty() {
        parts.push(format!("角色弧光：{}", arc_hints.join("；")));
    }

    let foreshadow_hints = foreshadow_reminders
        .map(|value| extract_query_focus_lines(value, 3, 80))
        .unwrap_or_default();
    if !foreshadow_hints.is_empty() {
        parts.push(format!("伏笔线索：{}", foreshadow_hints.join("；")));
    }

    normalize_research_text(parts.join(" ").replace('\n', " "), 720)
}

fn normalize_memory_content_key(content: &str, max_length: usize) -> String {
    let raw_value = content.trim().to_lowercase().replace(char::is_whitespace, "");
    let normalized = raw_value
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(ch))
        .take(max_length)
        .collect::<String>();
    if normalized.is_empty() {
        raw_value.chars().take(max_length).collect()
    } else {
        normalized
    }
}

fn preview_text(value: &str, max_length: usize) -> String {
    let normalized = normalize_research_text(value, max_length);
    normalized.trim().to_string()
}

fn build_memory_prompt_line(memory: &story_memory::Model, similarity: f64) -> Option<String> {
    let content_preview = preview_text(&memory.content, GENERATION_MEMORY_PREVIEW_LENGTH);
    if content_preview.is_empty() {
        return None;
    }
    Some(format!(
        "- ({} / 相关度:{:.2}) {}",
        memory_type_display_label(&memory.memory_type),
        similarity,
        content_preview
    ))
}

fn build_character_prompt_line(character: &character::Model) -> String {
    let kind = if character.is_organization {
        "组织"
    } else {
        "角色"
    };
    let mut parts = vec![format!(
        "- {} ({}, {})",
        character.name,
        kind,
        role_type_display_label(character.role_type.as_deref())
    )];

    let personality = character
        .personality
        .as_deref()
        .map(|value| preview_text(value, 80))
        .filter(|value| !value.is_empty());
    if let Some(personality) = personality {
        parts.push(format!("性格: {}", personality));
    }

    let background = character
        .background
        .as_deref()
        .map(|value| preview_text(value, 100))
        .filter(|value| !value.is_empty());
    if let Some(background) = background {
        parts.push(format!("背景: {}", background));
    }

    let state = character
        .current_state
        .as_deref()
        .map(|value| preview_text(value, 80))
        .filter(|value| !value.is_empty());
    if let Some(state) = state {
        parts.push(format!("当前状态: {}", state));
    }

    if !character.is_organization {
        if let Some(stage) = character.main_career_stage {
            parts.push(format!("职业阶段: 第{}阶", stage));
        }
    } else if let Some(purpose) = character
        .organization_purpose
        .as_deref()
        .map(|value| preview_text(value, 80))
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("组织目标: {}", purpose));
    }

    parts.join("；")
}

fn build_career_prompt_line(career: &career::Model, current_stage: Option<i32>) -> String {
    let mut parts = vec![format!("{} ({}职业)", career.name, career.career_type)];
    if let Some(description) = career
        .description
        .as_deref()
        .map(|value| preview_text(value, 80))
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("描述: {}", description));
    }
    if let Some(category) = career.category.as_deref().filter(|value| !value.is_empty()) {
        parts.push(format!("分类: {}", category));
    }
    if let Some(stage) = current_stage {
        parts.push(format!("当前阶段: 第{}阶", stage));
    }
    parts.join("；")
}

fn story_memory_matches_character(memory: &story_memory::Model, character: &character::Model) -> bool {
    if let Some(related_characters) = memory.related_characters.as_ref() {
        if let Some(items) = related_characters.as_array() {
            if items
                .iter()
                .any(|item| item.as_str() == Some(character.name.as_str()))
            {
                return true;
            }
        }
    }

    let name = character.name.trim();
    if name.is_empty() {
        return false;
    }

    memory.title.as_deref().unwrap_or_default().contains(name) || memory.content.contains(name)
}

fn build_character_arc_snapshot(
    characters: &[character::Model],
    memories: &[story_memory::Model],
    current_chapter: i32,
) -> Option<String> {
    if characters.is_empty() {
        return None;
    }

    let mut sorted_memories = memories.to_vec();
    sorted_memories.sort_by(|left, right| {
        right
            .story_timeline
            .cmp(&left.story_timeline)
            .then_with(|| {
                right
                    .importance_score
                    .partial_cmp(&left.importance_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut ranked = characters
        .iter()
        .filter(|item| !item.is_organization)
        .filter_map(|character| {
            let matched_memories = sorted_memories
                .iter()
                .filter(|memory| story_memory_matches_character(memory, character))
                .cloned()
                .collect::<Vec<_>>();
            let has_state = character
                .current_state
                .as_deref()
                .map(|value| !preview_text(value, 60).is_empty())
                .unwrap_or(false);
            let has_non_active_status =
                !character.status.trim().is_empty() && character.status != "active";
            if !has_state && !has_non_active_status && matched_memories.is_empty() {
                return None;
            }

            Some((
                (has_state as i32)
                    + (has_non_active_status as i32)
                    + if matched_memories.is_empty() { 0 } else { 1 },
                character.state_updated_chapter.unwrap_or_default(),
                character_role_priority(character.role_type.as_deref()),
                character.clone(),
                matched_memories,
            ))
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| right.2.cmp(&left.2))
    });

    let mut lines = Vec::new();
    for (_, _, _, character, matched_memories) in ranked
        .into_iter()
        .take(GENERATION_CHARACTER_ARC_LIMIT)
    {
        let mut summary_parts = Vec::new();
        let state_preview = character
            .current_state
            .as_deref()
            .map(|value| preview_text(value, 36))
            .unwrap_or_default();
        if !state_preview.is_empty() {
            summary_parts.push(format!("当前状态「{}」", state_preview));
        }

        if !character.status.trim().is_empty() && character.status != "active" {
            let label = character_status_label(&character.status);
            if let Some(chapter_number) = character
                .status_changed_chapter
                .filter(|value| *value < current_chapter)
            {
                summary_parts.push(format!("生存状态={}(第{}章)", label, chapter_number));
            } else {
                summary_parts.push(format!("生存状态={}", label));
            }
        }

        if let Some(updated_chapter) = character.state_updated_chapter {
            summary_parts.push(format!("状态更新时间=第{}章", updated_chapter));
        }

        let memory_fragments = matched_memories
            .into_iter()
            .take(GENERATION_CHARACTER_ARC_MEMORIES_PER_CHARACTER)
            .filter_map(|memory| {
                let chapter_prefix = if memory.story_timeline > 0 {
                    format!("第{}章", memory.story_timeline)
                } else {
                    "近期".to_string()
                };
                let snippet = preview_text(
                    memory.title.as_deref().unwrap_or(memory.content.as_str()),
                    28,
                );
                if snippet.is_empty() {
                    None
                } else {
                    Some(format!("{}{}", chapter_prefix, snippet))
                }
            })
            .collect::<Vec<_>>();
        if !memory_fragments.is_empty() {
            summary_parts.push(format!("近期轨迹={}", memory_fragments.join("；")));
        }

        if !summary_parts.is_empty() {
            lines.push(format!("- {}：{}", character.name, summary_parts.join("；")));
        }
    }

    if lines.is_empty() {
        return None;
    }

    Some(format!("【角色弧光快照】\n{}", lines.join("\n")))
}

async fn build_single_chapter_characters_info_payload(
    db: &DatabaseConnection,
    chapter_target: &SingleChapterGenerationTarget,
) -> Result<Option<String>, String> {
    let characters: Vec<character::Model> = character::Entity::find()
        .filter(character::Column::ProjectId.eq(&chapter_target.project_id))
        .order_by_asc(character::Column::CreatedAt)
        .limit(GENERATION_CHARACTER_LIMIT)
        .all(db)
        .await
        .map_err(|error| format!("load characters for generation failed: {}", error))?;
    if characters.is_empty() {
        return Ok(None);
    }

    let lines = characters
        .iter()
        .map(build_character_prompt_line)
        .filter(|line: &String| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return Ok(None);
    }

    Ok(Some(lines.join("\n")))
}

async fn build_single_chapter_careers_payload(
    db: &DatabaseConnection,
    chapter_target: &SingleChapterGenerationTarget,
) -> Result<Option<String>, String> {
    let characters: Vec<character::Model> = character::Entity::find()
        .filter(character::Column::ProjectId.eq(&chapter_target.project_id))
        .all(db)
        .await
        .map_err(|error| format!("load characters for careers failed: {}", error))?;
    if characters.is_empty() {
        return Ok(None);
    }

    let mut career_ids = std::collections::HashSet::new();
    let mut main_stage_by_career = std::collections::HashMap::new();
    for character in &characters {
        if let Some(career_id) = character.main_career_id.as_ref() {
            career_ids.insert(career_id.clone());
            if let Some(stage) = character.main_career_stage {
                main_stage_by_career.entry(career_id.clone()).or_insert(stage);
            }
        }
    }

    let character_ids = characters.iter().map(|item| item.id.clone()).collect::<Vec<_>>();
    let career_relations = if character_ids.is_empty() {
        Vec::new()
    } else {
        character_career::Entity::find()
            .filter(character_career::Column::CharacterId.is_in(character_ids))
            .all(db)
            .await
            .map_err(|error| format!("load character careers failed: {}", error))?
    };
    for relation in &career_relations {
        career_ids.insert(relation.career_id.clone());
    }

    if career_ids.is_empty() {
        return Ok(None);
    }

    let careers = career::Entity::find()
        .filter(career::Column::Id.is_in(career_ids.into_iter().collect::<Vec<_>>()))
        .order_by_asc(career::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|error| format!("load careers failed: {}", error))?;
    if careers.is_empty() {
        return Ok(None);
    }

    let relation_stage_by_career = career_relations
        .into_iter()
        .map(|relation| (relation.career_id, relation.current_stage))
        .collect::<std::collections::HashMap<_, _>>();

    let lines = careers
        .iter()
        .map(|career| {
            let current_stage = relation_stage_by_career
                .get(&career.id)
                .copied()
                .or_else(|| main_stage_by_career.get(&career.id).copied());
            build_career_prompt_line(career, current_stage)
        })
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return Ok(None);
    }

    Ok(Some(lines.join("\n")))
}

async fn build_single_chapter_character_arc_snapshot_payload(
    db: &DatabaseConnection,
    chapter_target: &SingleChapterGenerationTarget,
) -> Result<Option<String>, String> {
    let characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(&chapter_target.project_id))
        .filter(character::Column::IsOrganization.eq(false))
        .all(db)
        .await
        .map_err(|error| format!("load characters for character arc failed: {}", error))?;
    if characters.is_empty() {
        return Ok(None);
    }

    let memories = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(&chapter_target.project_id))
        .filter(story_memory::Column::StoryTimeline.lt(chapter_target.chapter_number))
        .filter(story_memory::Column::MemoryType.is_in(vec![
            "character_event".to_string(),
            "plot_point".to_string(),
        ]))
        .order_by_desc(story_memory::Column::StoryTimeline)
        .order_by_desc(story_memory::Column::ImportanceScore)
        .limit(GENERATION_CHARACTER_ARC_MEMORY_LIMIT)
        .all(db)
        .await
        .map_err(|error| format!("load memories for character arc failed: {}", error))?;

    Ok(build_character_arc_snapshot(
        &characters,
        &memories,
        chapter_target.chapter_number,
    ))
}

async fn build_single_chapter_recent_context_payload(
    db: &DatabaseConnection,
    chapter_target: &SingleChapterGenerationTarget,
) -> Result<Option<String>, String> {
    let recent_chapters = crate::models::chapter::Entity::find()
        .filter(crate::models::chapter::Column::ProjectId.eq(&chapter_target.project_id))
        .filter(crate::models::chapter::Column::ChapterNumber.lt(chapter_target.chapter_number))
        .order_by_desc(crate::models::chapter::Column::ChapterNumber)
        .limit(10)
        .all(db)
        .await
        .map_err(|error| format!("load recent chapters for generation failed: {}", error))?;
    if recent_chapters.is_empty() {
        return Ok(None);
    }

    let mut recent_chapters = recent_chapters;
    recent_chapters.sort_by_key(|item| item.chapter_number);

    let mut lines = vec!["【最近章节规划】".to_string()];
    for chapter in recent_chapters {
        if let Some(expansion_plan) = chapter.expansion_plan.as_deref() {
            if let Ok(plan) = serde_json::from_str::<Value>(expansion_plan) {
                let plot_summary = plan
                    .get("plot_summary")
                    .and_then(Value::as_str)
                    .map(|value| preview_text(value, 160))
                    .unwrap_or_default();
                let key_events = plan
                    .get("key_events")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .take(3)
                            .map(|value| preview_text(value, 40))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !plot_summary.is_empty() {
                    let mut line =
                        format!("第{}章《{}》：{}", chapter.chapter_number, chapter.title, plot_summary);
                    if !key_events.is_empty() {
                        line.push_str(&format!("（关键事件：{}）", key_events.join("；")));
                    }
                    lines.push(line);
                    continue;
                }
            }
        }

        if let Some(summary) = chapter.summary.as_deref().filter(|value| !value.trim().is_empty()) {
            lines.push(format!(
                "第{}章《{}》：{}",
                chapter.chapter_number,
                chapter.title,
                preview_text(summary, 100)
            ));
        }
    }

    if lines.len() <= 1 {
        return Ok(None);
    }

    Ok(Some(lines.join("\n")))
}

async fn build_single_chapter_previous_summary_payload(
    db: &DatabaseConnection,
    chapter_target: &SingleChapterGenerationTarget,
) -> Result<Option<String>, String> {
    if chapter_target.chapter_number <= 1 {
        return Ok(None);
    }

    let previous_chapter = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(chapter_target.project_id.clone()))
        .filter(chapter::Column::ChapterNumber.eq(chapter_target.chapter_number - 1))
        .one(db)
        .await
        .map_err(|error| format!("load previous chapter failed: {}", error))?;

    let Some(previous_chapter) = previous_chapter else {
        return Ok(None);
    };

    let memory_summary = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(chapter_target.project_id.clone()))
        .filter(story_memory::Column::ChapterId.eq(Some(previous_chapter.id.clone())))
        .filter(story_memory::Column::MemoryType.eq("chapter_summary"))
        .order_by_desc(story_memory::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| format!("load previous chapter summary memory failed: {}", error))?
        .map(|memory| memory.content);

    let summary = memory_summary
        .or(previous_chapter.summary)
        .unwrap_or_default()
        .trim()
        .chars()
        .take(300)
        .collect::<String>();

    if summary.is_empty() {
        return Ok(None);
    }

    Ok(Some(summary))
}

fn build_foreshadow_summary_line(
    item: &foreshadow::Model,
    chapter_number: i32,
    overdue: bool,
) -> String {
    if overdue {
        let overdue_chapters = chapter_number - item.target_resolve_chapter_number.unwrap_or(0);
        return format!(
            "- {} [已超期{}章]\n  埋入章节：第{}章，原计划第{}章回收\n  伏笔内容：{}",
            item.title,
            overdue_chapters,
            item.plant_chapter_number.unwrap_or_default(),
            item.target_resolve_chapter_number.unwrap_or_default(),
            preview_text(&item.content, 80)
        );
    }

    if item.target_resolve_chapter_number == Some(chapter_number) {
        let mut line = format!(
            "- {}\n  埋入章节：第{}章\n  伏笔内容：{}",
            item.title,
            item.plant_chapter_number.unwrap_or_default(),
            preview_text(&item.content, 100)
        );
        if let Some(notes) = item
            .resolution_notes
            .as_deref()
            .map(|value| preview_text(value, 100))
            .filter(|value| !value.is_empty())
        {
            line.push_str(&format!("\n  回收提示：{}", notes));
        }
        return line;
    }

    let remaining = item.target_resolve_chapter_number.unwrap_or(chapter_number) - chapter_number;
    format!(
        "- {}（计划第{}章回收，还有{}章）",
        item.title,
        item.target_resolve_chapter_number.unwrap_or_default(),
        remaining
    )
}

async fn build_single_chapter_foreshadow_reminders_payload(
    db: &DatabaseConnection,
    chapter_target: &SingleChapterGenerationTarget,
) -> Result<Option<String>, String> {
    let all_items = foreshadow::Entity::find()
        .filter(foreshadow::Column::ProjectId.eq(&chapter_target.project_id))
        .filter(foreshadow::Column::Status.ne("resolved"))
        .filter(foreshadow::Column::Status.ne("abandoned"))
        .order_by_desc(foreshadow::Column::Importance)
        .order_by_desc(foreshadow::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|error| format!("load foreshadows for generation failed: {}", error))?;
    if all_items.is_empty() {
        return Ok(None);
    }

    let chapter_number = chapter_target.chapter_number;
    let must_resolve = all_items
        .iter()
        .filter(|item| item.target_resolve_chapter_number == Some(chapter_number))
        .take(GENERATION_FORESHADOW_LIMIT)
        .cloned()
        .collect::<Vec<_>>();
    let overdue = all_items
        .iter()
        .filter(|item| {
            item.target_resolve_chapter_number
                .map(|target| target < chapter_number)
                .unwrap_or(false)
        })
        .take(GENERATION_FORESHADOW_LIMIT)
        .cloned()
        .collect::<Vec<_>>();
    let upcoming = all_items
        .iter()
        .filter(|item| {
            item.target_resolve_chapter_number
                .map(|target| target > chapter_number && target <= chapter_number + 3)
                .unwrap_or(false)
        })
        .take(GENERATION_FORESHADOW_LIMIT)
        .cloned()
        .collect::<Vec<_>>();

    let mut lines = Vec::new();
    if !must_resolve.is_empty() {
        lines.push("【🎯 本章必须回收的伏笔】".to_string());
        for item in must_resolve {
            lines.push(build_foreshadow_summary_line(&item, chapter_number, false));
            lines.push(String::new());
        }
    }
    if !overdue.is_empty() {
        lines.push("【⚠️ 超期待回收伏笔】".to_string());
        for item in overdue {
            lines.push(build_foreshadow_summary_line(&item, chapter_number, true));
            lines.push(String::new());
        }
    }
    if !upcoming.is_empty() {
        lines.push("【📋 即将到期的伏笔（仅供参考）】".to_string());
        for item in upcoming {
            lines.push(build_foreshadow_summary_line(&item, chapter_number, false));
        }
        lines.push(String::new());
    }

    let content = lines.join("\n").trim().to_string();
    if content.is_empty() {
        return Ok(None);
    }
    Ok(Some(content))
}

fn select_memories_for_prompt(
    memories: Vec<(story_memory::Model, f64)>,
) -> Vec<(story_memory::Model, f64)> {
    if memories.is_empty() {
        return Vec::new();
    }

    let mut deduped = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_content_keys = std::collections::HashSet::new();
    for (memory, similarity) in memories {
        let content_key = normalize_memory_content_key(&memory.content, 96);
        if content_key.is_empty() || seen_content_keys.contains(&content_key) {
            continue;
        }
        if seen_ids.contains(&memory.id) {
            continue;
        }
        seen_ids.insert(memory.id.clone());
        seen_content_keys.insert(content_key);
        deduped.push((memory, similarity));
    }

    if deduped.is_empty() {
        return Vec::new();
    }

    let mut selected = Vec::new();
    let mut selected_keys = std::collections::HashSet::new();
    let mut current_size = "【相关记忆】".chars().count();

    for memory_type in GENERATION_MEMORY_TYPE_COVERAGE_PRIORITY {
        for memory in &deduped {
            if memory.0.memory_type != *memory_type {
                continue;
            }
            if try_select_memory_for_prompt(
                memory,
                &mut selected,
                &mut selected_keys,
                &mut current_size,
            ) {
                break;
            }
        }
        if selected.len() >= GENERATION_MEMORY_COUNT {
            break;
        }
    }

    if selected.len() < GENERATION_MEMORY_COUNT {
        for memory in &deduped {
            if try_select_memory_for_prompt(
                memory,
                &mut selected,
                &mut selected_keys,
                &mut current_size,
            ) && selected.len() >= GENERATION_MEMORY_COUNT
            {
                break;
            }
        }
    }

    if selected.is_empty() {
        selected.push(deduped[0].clone());
    }

    selected
}

fn try_select_memory_for_prompt(
    memory: &(story_memory::Model, f64),
    selected: &mut Vec<(story_memory::Model, f64)>,
    selected_keys: &mut std::collections::HashSet<String>,
    current_size: &mut usize,
) -> bool {
    let key = memory.0.id.clone();
    if selected_keys.contains(&key) {
        return false;
    }
    let Some(line) = build_memory_prompt_line(&memory.0, memory.1) else {
        return false;
    };
    let projected_size = *current_size + line.chars().count() + 1;
    if !selected.is_empty() && projected_size > GENERATION_MEMORY_TOTAL_CHARS_BUDGET {
        return false;
    }
    *current_size = projected_size;
    selected_keys.insert(key);
    selected.push(memory.clone());
    true
}

async fn build_single_chapter_relevant_memories_payload(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_target: &SingleChapterGenerationTarget,
    compat_options: &SingleChapterGenerationCompatOptions,
    characters_info: Option<&str>,
    chapter_careers: Option<&str>,
    character_arc_snapshot: Option<&str>,
    foreshadow_reminders: Option<&str>,
) -> Result<Option<String>, String> {
    let query = compose_single_chapter_generation_memory_query(
        chapter_target,
        compat_options,
        characters_info,
        chapter_careers,
        character_arc_snapshot,
        foreshadow_reminders,
    );
    if query.trim().is_empty() {
        return Ok(None);
    }

    let vector_hits = search_story_memory_vector_records(
        db,
        user_id,
        &chapter_target.project_id,
        &query,
        &GENERATION_MEMORY_TYPES
            .iter()
            .map(|item| item.to_string())
            .collect::<Vec<_>>(),
        0.0,
        GENERATION_MEMORY_RANK_LIMIT,
    )
    .await?;
    if vector_hits.is_empty() {
        return Ok(None);
    }

    let hit_ids = vector_hits
        .iter()
        .filter(|item| item.similarity >= GENERATION_MEMORY_SIMILARITY_THRESHOLD)
        .map(|item| item.memory_id.clone())
        .collect::<Vec<_>>();
    if hit_ids.is_empty() {
        return Ok(None);
    }

    let memories = story_memory::Entity::find()
        .filter(story_memory::Column::ProjectId.eq(&chapter_target.project_id))
        .filter(story_memory::Column::Id.is_in(hit_ids.clone()))
        .order_by_desc(story_memory::Column::ImportanceScore)
        .order_by_desc(story_memory::Column::CreatedAt)
        .all(db)
        .await
        .map_err(|error| format!("load relevant memories failed: {}", error))?;

    let memory_by_id = memories
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect::<std::collections::HashMap<_, _>>();
    let ordered = vector_hits
        .into_iter()
        .filter(|item| item.similarity >= GENERATION_MEMORY_SIMILARITY_THRESHOLD)
        .filter_map(|item| {
            memory_by_id
                .get(&item.memory_id)
                .cloned()
                .map(|memory| (memory, item.similarity))
        })
        .collect::<Vec<_>>();
    let selected = select_memories_for_prompt(ordered);
    if selected.is_empty() {
        return Ok(None);
    }

    let mut lines = vec!["【相关记忆】".to_string()];
    for (memory, similarity) in selected {
        if let Some(line) = build_memory_prompt_line(&memory, similarity) {
            lines.push(line);
        }
    }

    let memory_block = if lines.len() > 1 {
        Some(lines.join("\n"))
    } else {
        None
    };
    let merged = [memory_block.as_deref(), character_arc_snapshot]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .fold(Vec::<String>::new(), |mut acc, value| {
            if !acc.iter().any(|item| item == value) {
                acc.push(value.to_string());
            }
            acc
        });

    if merged.is_empty() {
        return Ok(None);
    }

    Ok(Some(merged.join("\n\n")))
}

fn web_research_exa_enabled(settings: &Value) -> bool {
    settings
        .get("web_research_exa_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn web_research_exa_api_key(settings: &Value) -> String {
    settings
        .get("web_research_exa_api_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn web_research_exa_base_url(settings: &Value) -> String {
    settings
        .get("web_research_exa_base_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_EXA_BASE_URL)
        .trim_end_matches('/')
        .to_string()
}

fn web_research_grok_enabled(settings: &Value) -> bool {
    settings
        .get("web_research_grok_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn web_research_grok_api_key(settings: &Value) -> String {
    settings
        .get("web_research_grok_api_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn web_research_grok_base_url(settings: &Value) -> String {
    let raw = settings
        .get("web_research_grok_base_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_GROK_BASE_URL);

    normalize_openai_compatible_base_url(raw)
}

fn web_research_grok_model(settings: &Value) -> String {
    settings
        .get("web_research_grok_model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_GROK_MODEL)
        .to_string()
}

#[derive(Debug, Deserialize, Serialize)]
struct ExaSearchResponse {
    #[serde(default)]
    results: Vec<ExaSearchResult>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ExaSearchResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    highlights: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GrokResearchSource {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    snippet: String,
}

#[derive(Debug, Deserialize)]
struct GrokResearchResponse {
    #[serde(default)]
    content: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    sources: Vec<GrokResearchSource>,
}

#[derive(Debug, Serialize)]
struct ResearchArchiveBundle {
    generated_at: String,
    query: String,
    chapter_id: String,
    chapter_number: i32,
    assets: Vec<Value>,
}

async fn run_exa_search(query: &str, api_key: &str, base_url: &str) -> Result<ExaSearchResponse, String> {
    let response = http_client()
        .post(format!("{}/search", base_url.trim_end_matches('/')))
        .header(AUTHORIZATION, format!("Bearer {}", api_key))
        .header(CONTENT_TYPE, "application/json")
        .json(&json!({
            "query": query,
            "numResults": 3,
            "text": true,
        }))
        .send()
        .await
        .map_err(|error| format!("exa request failed: {}", error))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("exa response read failed: {}", error))?;

    if !status.is_success() {
        return Err(format!(
            "exa request failed with status {}: {}",
            status,
            normalize_research_text(text, 400)
        ));
    }

    serde_json::from_str::<ExaSearchResponse>(&text)
        .map_err(|error| format!("exa response decode failed: {}", error))
}

async fn run_grok_search(query: &str, api_key: &str, base_url: &str, model: &str) -> Result<Value, String> {
    let client = OpenAIClient::new(api_key, base_url);
    let response = client
        .chat_completion(
            &[
                ChatMessage {
                    role: "system".to_string(),
                    content: "You are a web research assistant. Return JSON only with keys content and sources. sources must be an array of objects with title, url, snippet. If you do not have reliable source URLs, return an empty array.".to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: format!("Research this topic and keep it concise: {}", query),
                },
            ],
            model,
            0.2,
            512,
            None,
        )
        .await
        .map_err(|error| format!("grok request failed: {}", error))?;

    let trimmed = response.content.trim();
    if trimmed.is_empty() {
        return Err("grok response was empty".to_string());
    }

    let parsed = serde_json::from_str::<Value>(trimmed).or_else(|_| {
        let start = trimmed.find('{').ok_or_else(|| "missing json object start".to_string())?;
        let end = trimmed.rfind('}').ok_or_else(|| "missing json object end".to_string())?;
        serde_json::from_str::<Value>(&trimmed[start..=end]).map_err(|error| error.to_string())
    });

    parsed.map_err(|error| format!("grok response decode failed: {}", error))
}

fn build_exa_assets(results: &[ExaSearchResult]) -> Vec<Value> {
    results
        .iter()
        .take(DEFAULT_MAX_ASSETS)
        .filter_map(|item| {
            let title = normalize_research_text(
                if item.title.trim().is_empty() {
                    &item.url
                } else {
                    &item.title
                },
                120,
            );
            let source = normalize_research_text(&item.url, 300);
            let summary = normalize_research_text(
                if !item.highlights.is_empty() {
                    item.highlights.join(" ")
                } else {
                    item.text.clone()
                },
                360,
            );
            let raw_content = normalize_research_text(&item.text, 1200);

            if summary.is_empty() {
                return None;
            }

            Some(json!({
                "title": if title.is_empty() { "Exa 参考资料" } else { &title },
                "source": source,
                "summary": summary,
                "usage_hint": "用于补强真实设定、职业/地点/历史细节，吸收信息结构，不要直接照抄原文。",
                "asset_type": "exa_search_result",
                "raw_content": raw_content,
            }))
        })
        .collect()
}

fn build_grok_assets(payload: &Value) -> Vec<Value> {
    let parsed = serde_json::from_value::<GrokResearchResponse>(payload.clone()).unwrap_or_else(|_| {
        GrokResearchResponse {
            content: payload
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            summary: payload
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            sources: Vec::new(),
        }
    });

    let mut assets = Vec::new();
    let summary = normalize_research_text(
        if parsed.content.trim().is_empty() {
            &parsed.summary
        } else {
            &parsed.content
        },
        360,
    );
    let raw_content = normalize_research_text(
        if parsed.content.trim().is_empty() {
            &parsed.summary
        } else {
            &parsed.content
        },
        1200,
    );
    let primary_source = parsed
        .sources
        .first()
        .map(|item| normalize_research_text(&item.url, 300))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "grok-search".to_string());

    if !summary.is_empty() {
        assets.push(json!({
            "title": "Grok 实时综述",
            "source": primary_source,
            "summary": summary,
            "usage_hint": "用于提炼当下语感、讨论热点和社会氛围，避免把观点原样写成正文。",
            "asset_type": "grok_search_summary",
            "raw_content": raw_content,
        }));
    }

    for item in parsed.sources.iter().take(2) {
        let title = normalize_research_text(
            if item.title.trim().is_empty() {
                &item.url
            } else {
                &item.title
            },
            120,
        );
        let source = normalize_research_text(&item.url, 300);
        let snippet = normalize_research_text(
            if item.snippet.trim().is_empty() {
                &item.title
            } else {
                &item.snippet
            },
            220,
        );
        if snippet.is_empty() {
            continue;
        }

        assets.push(json!({
            "title": if title.is_empty() { "Grok 来源" } else { &title },
            "source": if source.is_empty() { "grok-search" } else { &source },
            "summary": snippet,
            "usage_hint": "作为外部讨论样本参考，用来优化用词、氛围与现实感。",
            "asset_type": "grok_search_source",
            "raw_content": snippet,
        }));
    }

    assets
}

fn resolve_archive_root() -> PathBuf {
    load_app_config()
        .ok()
        .map(|cfg| PathBuf::from(cfg.static_dir).join("..").join("data").join("web_research"))
        .unwrap_or_else(|| PathBuf::from("../backend/data/web_research"))
}

async fn write_research_archive(
    chapter_target: &SingleChapterGenerationTarget,
    query: &str,
    assets: &[Value],
) -> Result<String, String> {
    let archive_dir = resolve_archive_root().join(&chapter_target.project_id);
    tokio::fs::create_dir_all(&archive_dir)
        .await
        .map_err(|error| format!("create archive dir failed: {}", error))?;

    let archive_path = archive_dir.join(format!("{}.json", chapter_target.chapter_id));
    let bundle = ResearchArchiveBundle {
        generated_at: chrono::Utc::now().to_rfc3339(),
        query: query.to_string(),
        chapter_id: chapter_target.chapter_id.clone(),
        chapter_number: chapter_target.chapter_number,
        assets: assets.to_vec(),
    };
    let content = serde_json::to_string_pretty(&bundle)
        .map_err(|error| format!("serialize archive failed: {}", error))?;
    tokio::fs::write(&archive_path, content)
        .await
        .map_err(|error| format!("write archive failed: {}", error))?;

    Ok(archive_path.to_string_lossy().to_string())
}

async fn replace_chapter_research_memories(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_target: &SingleChapterGenerationTarget,
    query: &str,
    archive_path: &str,
    assets: &[Value],
) -> Result<Vec<String>, String> {
    delete_story_memory_vector_records_by_types(
        &chapter_target.project_id,
        Some(&chapter_target.chapter_id),
        &[RESEARCH_MEMORY_TYPE.to_string()],
    )
    .await?;

    story_memory::Entity::delete_many()
        .filter(story_memory::Column::ProjectId.eq(chapter_target.project_id.clone()))
        .filter(story_memory::Column::MemoryType.eq(RESEARCH_MEMORY_TYPE))
        .filter(story_memory::Column::ChapterId.eq(chapter_target.chapter_id.clone()))
        .exec(db)
        .await
        .map_err(|error| format!("delete previous research memories failed: {}", error))?;

    let mut saved_ids = Vec::new();
    for (index, asset) in assets.iter().enumerate() {
        let title = normalize_research_text(
            format!(
                "外部资料 {}: {}",
                index + 1,
                asset.get("title").and_then(Value::as_str).unwrap_or("未命名资料")
            ),
            180,
        );
        let summary = normalize_research_text(
            asset.get("summary").and_then(Value::as_str).unwrap_or_default(),
            500,
        );
        let memory_content = normalize_research_text(
            format!(
                "{} 来源：{} 摘要：{}",
                title,
                asset.get("source").and_then(Value::as_str).unwrap_or("未知来源"),
                summary
            ),
            600,
        );
        let memory_id = Uuid::new_v4().to_string();
        let full_context = json!({
            "query": query,
            "archive_path": archive_path,
            "asset": asset,
        })
        .to_string();
        let now = chrono::Utc::now().naive_utc();

        let saved = story_memory::ActiveModel {
            id: Set(memory_id.clone()),
            project_id: Set(chapter_target.project_id.clone()),
            chapter_id: Set(Some(chapter_target.chapter_id.clone())),
            memory_type: Set(RESEARCH_MEMORY_TYPE.to_string()),
            title: Set(Some(title)),
            content: Set(if summary.is_empty() {
                memory_content.clone()
            } else {
                summary
            }),
            full_context: Set(Some(full_context)),
            related_characters: Set(None),
            related_locations: Set(None),
            tags: Set(Some(json!(["web_research", asset.get("asset_type").and_then(Value::as_str).unwrap_or("external_asset")]))),
            importance_score: Set(Some(0.62)),
            story_timeline: Set(chapter_target.chapter_number),
            chapter_position: Set(0),
            text_length: Set(memory_content.chars().count() as i32),
            is_foreshadow: Set(0),
            foreshadow_resolved_at: Set(None),
            foreshadow_strength: Set(None),
            vector_id: Set(Some(memory_id.clone())),
            embedding_model: Set(None),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .map_err(|error| format!("insert research memory failed: {}", error))?;
        upsert_story_memory_vector_record(
            db,
            user_id,
            &saved,
            &memory_content,
            json!({
                "chapter_id": chapter_target.chapter_id,
                "chapter_number": chapter_target.chapter_number,
                "importance_score": 0.62,
                "tags": ["web_research", asset.get("asset_type").and_then(Value::as_str).unwrap_or("external_asset")],
                "title": asset.get("title").and_then(Value::as_str).unwrap_or_default(),
                "query": query,
                "archive_path": archive_path,
            }),
        )
        .await?;

        saved_ids.push(memory_id);
    }

    Ok(saved_ids)
}

pub(crate) async fn build_single_chapter_research_provider_payload(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_target: &SingleChapterGenerationTarget,
    compat_options: &SingleChapterGenerationCompatOptions,
) -> Result<PromptContextProviderPayload, String> {
    let mut payload = build_placeholder_prompt_context_provider_payload();
    if let Some(characters_info) =
        build_single_chapter_characters_info_payload(db, chapter_target).await?
    {
        payload.characters_info = characters_info;
    }
    if let Some(chapter_careers) = build_single_chapter_careers_payload(db, chapter_target).await? {
        payload.chapter_careers = chapter_careers;
    }
    if let Some(recent_chapters_context) =
        build_single_chapter_recent_context_payload(db, chapter_target).await?
    {
        payload.recent_chapters_context = recent_chapters_context;
    }
    if let Some(previous_chapter_summary) =
        build_single_chapter_previous_summary_payload(db, chapter_target).await?
    {
        payload.previous_chapter_summary = previous_chapter_summary;
    }
    let character_arc_snapshot =
        build_single_chapter_character_arc_snapshot_payload(db, chapter_target).await?;
    if let Some(foreshadow_reminders) =
        build_single_chapter_foreshadow_reminders_payload(db, chapter_target).await?
    {
        payload.foreshadow_reminders = foreshadow_reminders;
    }
    if let Some(relevant_memories) = build_single_chapter_relevant_memories_payload(
        db,
        user_id,
        chapter_target,
        compat_options,
        Some(payload.characters_info.as_str()).filter(|value| !value.trim().is_empty() && *value != "[]"),
        Some(payload.chapter_careers.as_str()).filter(|value| !value.trim().is_empty() && *value != "[]"),
        character_arc_snapshot.as_deref().filter(|value| !value.trim().is_empty()),
        Some(payload.foreshadow_reminders.as_str())
            .filter(|value| !value.trim().is_empty() && *value != "[]"),
    )
    .await?
    {
        payload.relevant_memories = relevant_memories;
    }

    if !compat_options.web_research_enabled() {
        return Ok(payload);
    }

    let settings = SettingsService::resolve_web_research_settings(db, user_id)
        .await
        .map_err(|error| error.to_string())?;
    let exa_enabled = web_research_exa_enabled(&settings);
    let grok_enabled = web_research_grok_enabled(&settings);
    if !exa_enabled && !grok_enabled {
        return Ok(payload);
    }

    let research_query = compose_single_chapter_research_query(chapter_target, compat_options);
    let grok_query = compose_single_chapter_grok_query(chapter_target, compat_options);
    if research_query.is_empty() && grok_query.is_empty() {
        return Ok(payload);
    }

    let mut assets = Vec::new();
    let mut exa_payload: Option<Value> = None;
    let mut grok_payload: Option<Value> = None;

    if exa_enabled {
        let api_key = web_research_exa_api_key(&settings);
        if !api_key.is_empty() && !research_query.is_empty() {
            let response = run_exa_search(
                &research_query,
                &api_key,
                &web_research_exa_base_url(&settings),
            )
            .await?;
            assets.extend(build_exa_assets(&response.results));
            exa_payload = serde_json::to_value(&response).ok();
        }
    }

    if grok_enabled {
        let api_key = web_research_grok_api_key(&settings);
        if !api_key.is_empty() && !grok_query.is_empty() {
            let response = run_grok_search(
                &grok_query,
                &api_key,
                &web_research_grok_base_url(&settings),
                &web_research_grok_model(&settings),
            )
            .await?;
            assets.extend(build_grok_assets(&response));
            grok_payload = Some(response);
        }
    }

    assets.truncate(DEFAULT_MAX_ASSETS);
    let effective_query = if !research_query.is_empty() {
        research_query.clone()
    } else {
        grok_query.clone()
    };
    let archive_path = write_research_archive(chapter_target, &effective_query, &assets).await?;
    let saved_memory_ids = replace_chapter_research_memories(
        db,
        user_id,
        chapter_target,
        &effective_query,
        &archive_path,
        &assets,
    )
    .await?;

    payload.research_query = effective_query.clone();
    payload.research_assets = serde_json::to_string(&assets).unwrap_or_else(|_| "[]".to_string());
    payload.external_assets = payload.research_assets.clone();
    payload.reference_assets = payload.research_assets.clone();
    payload.mcp_references = json!({
        "saved_memory_ids": saved_memory_ids,
        "query": {
            "exa": research_query,
            "grok": grok_query,
        },
        "providers": {
            "exa": exa_payload,
            "grok": grok_payload,
        }
    })
    .to_string();
    if assets.is_empty() {
        payload.external_assets = json!([{
            "kind": "web_research_query",
            "source": "single_chapter_request",
            "summary": effective_query,
            "archive_path": archive_path,
            "saved_memory_ids": saved_memory_ids,
        }])
        .to_string();
        payload.reference_assets = payload.external_assets.clone();
    }

    Ok(payload)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_career_prompt_line, build_character_arc_snapshot, build_character_prompt_line,
        build_exa_assets, build_foreshadow_summary_line, build_grok_assets,
        build_memory_prompt_line, compose_single_chapter_generation_memory_query,
        compose_single_chapter_grok_query, compose_single_chapter_research_query,
        normalize_research_text, ExaSearchResult, RESEARCH_MEMORY_TYPE,
    };
    use crate::models::{career, character, foreshadow, story_memory};
    use crate::services::chapter_single_generation_prepare_service::{
        SingleChapterGenerationCompatOptions, SingleChapterGenerationTarget,
    };

    fn sample_target() -> SingleChapterGenerationTarget {
        SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-1".to_string(),
            chapter_number: 1,
            title: "夜渡".to_string(),
        }
    }

    #[test]
    fn should_normalize_research_text_length() {
        let normalized = normalize_research_text("a b c d e", 5);
        assert!(normalized.len() <= 5);
    }

    #[test]
    fn should_preserve_custom_research_query_for_single_chapter_payload() {
        let query = compose_single_chapter_research_query(
            &sample_target(),
            &SingleChapterGenerationCompatOptions {
                web_research_enabled: true,
                web_research_query: Some("晚清漕运夜航避税路线".to_string()),
                ..SingleChapterGenerationCompatOptions::default()
            },
        );

        assert_eq!(query, "晚清漕运夜航避税路线");
    }

    #[test]
    fn should_compose_default_single_chapter_research_query_from_target_and_story_context() {
        let query = compose_single_chapter_research_query(
            &sample_target(),
            &SingleChapterGenerationCompatOptions {
                web_research_enabled: true,
                story_creation_brief: Some("主角深夜潜入漕帮码头查失踪账册".to_string()),
                story_focus: Some("reveal_mystery".to_string()),
                plot_stage: Some("development".to_string()),
                ..SingleChapterGenerationCompatOptions::default()
            },
        );

        assert!(query.contains("第1章《夜渡》"));
        assert!(query.contains("主角深夜潜入漕帮码头查失踪账册"));
    }

    #[test]
    fn should_compose_grok_query_from_single_chapter_story_context() {
        let query = compose_single_chapter_grok_query(
            &sample_target(),
            &SingleChapterGenerationCompatOptions {
                web_research_enabled: true,
                story_creation_brief: Some("主角深夜潜入漕帮码头查失踪账册".to_string()),
                story_focus: Some("reveal_mystery".to_string()),
                plot_stage: Some("development".to_string()),
                ..SingleChapterGenerationCompatOptions::default()
            },
        );

        assert!(query.contains("实时网络研究"));
        assert!(query.contains("第1章《夜渡》"));
    }

    #[test]
    fn should_compose_generation_memory_query_from_story_context() {
        let query = compose_single_chapter_generation_memory_query(
            &sample_target(),
            &SingleChapterGenerationCompatOptions {
                story_creation_brief: Some("主角夜探码头，试图找回丢失账册".to_string()),
                story_focus: Some("调查失踪账册与漕帮暗线".to_string()),
                plot_stage: Some("development".to_string()),
                ..SingleChapterGenerationCompatOptions::default()
            },
            Some("- 沈夜 (角色, 主角)；当前状态: 怀疑账册失踪和内鬼有关"),
            Some("账房 (main职业)；描述: 精于账目、税卡和帮会账册往来"),
            Some("【角色弧光快照】\n- 沈夜：当前状态「怀疑账册失踪和内鬼有关」；近期轨迹=第1章账册失踪"),
            Some("【🎯 本章必须回收的伏笔】\n- 丢失账册的暗记"),
        );

        assert!(query.contains("本章标题：第1章《夜渡》"));
        assert!(query.contains("主角夜探码头"));
        assert!(query.contains("故事侧重点"));
        assert!(query.contains("角色状态"));
        assert!(query.contains("职业线索"));
        assert!(query.contains("角色弧光"));
        assert!(query.contains("伏笔线索"));
    }

    #[test]
    fn should_build_character_arc_snapshot_from_character_state_and_recent_memories() {
        let characters = vec![character::Model {
            id: "character-1".to_string(),
            project_id: "project-1".to_string(),
            name: "沈夜".to_string(),
            age: None,
            gender: None,
            is_organization: false,
            role_type: Some("protagonist".to_string()),
            personality: None,
            background: None,
            appearance: None,
            relationships: None,
            organization_type: None,
            organization_purpose: None,
            organization_members: None,
            status: "active".to_string(),
            status_changed_chapter: None,
            current_state: Some("怀疑账册失踪和内鬼有关".to_string()),
            state_updated_chapter: Some(3),
            main_career_id: None,
            main_career_stage: Some(2),
            sub_careers: None,
            avatar_url: None,
            traits: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        }];
        let memories = vec![story_memory::Model {
            id: "memory-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-2".to_string()),
            memory_type: "character_event".to_string(),
            title: Some("沈夜发现账册被调包".to_string()),
            content: "沈夜意识到账册失踪不是意外，背后有人提前动手。".to_string(),
            full_context: None,
            related_characters: Some(json!(["沈夜"])),
            related_locations: None,
            tags: None,
            importance_score: Some(0.91),
            story_timeline: 2,
            chapter_position: 1,
            text_length: 30,
            is_foreshadow: 0,
            foreshadow_resolved_at: None,
            foreshadow_strength: None,
            vector_id: None,
            embedding_model: None,
            created_at: None,
            updated_at: None,
        }];

        let snapshot =
            build_character_arc_snapshot(&characters, &memories, 4).expect("snapshot should exist");

        assert!(snapshot.contains("【角色弧光快照】"));
        assert!(snapshot.contains("沈夜"));
        assert!(snapshot.contains("当前状态"));
        assert!(snapshot.contains("近期轨迹"));
    }

    #[test]
    fn should_build_memory_prompt_line_with_similarity_and_type_label() {
        let memory = story_memory::Model {
            id: "memory-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            memory_type: "plot_point".to_string(),
            title: Some("账册失踪".to_string()),
            content: "主角发现账册在上一章被调包，漕帮内部有人提前动手。".to_string(),
            full_context: None,
            related_characters: None,
            related_locations: None,
            tags: None,
            importance_score: Some(0.9),
            story_timeline: 1,
            chapter_position: 1,
            text_length: 30,
            is_foreshadow: 0,
            foreshadow_resolved_at: None,
            foreshadow_strength: None,
            vector_id: None,
            embedding_model: None,
            created_at: None,
            updated_at: None,
        };

        let line = build_memory_prompt_line(&memory, 0.82).expect("line should exist");

        assert!(line.contains("剧情节点 / 相关度:0.82"));
        assert!(line.contains("主角发现账册"));
    }

    #[test]
    fn should_build_character_prompt_line_from_model_fields() {
        let character = character::Model {
            id: "character-1".to_string(),
            project_id: "project-1".to_string(),
            name: "沈夜".to_string(),
            age: None,
            gender: None,
            is_organization: false,
            role_type: Some("protagonist".to_string()),
            personality: Some("谨慎、冷静，但在关键时刻敢赌命".to_string()),
            background: Some("曾在漕帮外账房做过短工，对码头暗账门路很熟".to_string()),
            appearance: None,
            relationships: None,
            organization_type: None,
            organization_purpose: None,
            organization_members: None,
            status: "active".to_string(),
            status_changed_chapter: None,
            current_state: Some("怀疑账册失踪和内鬼有关".to_string()),
            state_updated_chapter: None,
            main_career_id: None,
            main_career_stage: Some(2),
            sub_careers: None,
            avatar_url: None,
            traits: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };

        let line = build_character_prompt_line(&character);

        assert!(line.contains("沈夜 (角色, 主角)"));
        assert!(line.contains("性格:"));
        assert!(line.contains("职业阶段: 第2阶"));
    }

    #[test]
    fn should_build_must_resolve_foreshadow_summary_line() {
        let item = foreshadow::Model {
            id: "f-1".to_string(),
            project_id: "project-1".to_string(),
            title: "丢失账册的暗记".to_string(),
            content: "账册封皮内层藏着只有旧账房才知道的暗记，能指向幕后接头人。".to_string(),
            hint_text: None,
            resolution_text: None,
            source_type: "manual".to_string(),
            source_memory_id: None,
            source_analysis_id: None,
            plant_chapter_id: Some("chapter-2".to_string()),
            plant_chapter_number: Some(2),
            target_resolve_chapter_id: Some("chapter-5".to_string()),
            target_resolve_chapter_number: Some(5),
            actual_resolve_chapter_id: None,
            actual_resolve_chapter_number: None,
            status: "planted".to_string(),
            is_long_term: false,
            importance: 0.9,
            strength: 8,
            subtlety: 6,
            urgency: 8,
            related_characters: None,
            related_foreshadow_ids: None,
            tags: None,
            category: None,
            notes: None,
            resolution_notes: Some("本章至少让主角确认暗记来源".to_string()),
            auto_remind: true,
            remind_before_chapters: 1,
            include_in_context: true,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
            planted_at: None,
            resolved_at: None,
        };

        let line = build_foreshadow_summary_line(&item, 5, false);

        assert!(line.contains("丢失账册的暗记"));
        assert!(line.contains("埋入章节：第2章"));
        assert!(line.contains("回收提示：本章至少让主角确认暗记来源"));
    }

    #[test]
    fn should_build_career_prompt_line_from_model_fields() {
        let career = career::Model {
            id: "career-1".to_string(),
            project_id: "project-1".to_string(),
            name: "账房".to_string(),
            career_type: "main".to_string(),
            description: Some("精于账目、税卡和帮会账册往来".to_string()),
            category: Some("经营".to_string()),
            stages: "[]".to_string(),
            max_stage: 5,
            requirements: None,
            special_abilities: None,
            worldview_rules: None,
            attribute_bonuses: None,
            source: "manual".to_string(),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: None,
        };

        let line = build_career_prompt_line(&career, Some(2));

        assert!(line.contains("账房 (main职业)"));
        assert!(line.contains("描述:"));
        assert!(line.contains("当前阶段: 第2阶"));
    }

    #[test]
    fn should_build_exa_assets_from_search_results() {
        let assets = build_exa_assets(&[ExaSearchResult {
            title: "晚清漕运研究".to_string(),
            url: "https://example.com/research".to_string(),
            text: "晚清漕运夜航多依赖水路帮会网络与地方税卡协调。".to_string(),
            highlights: vec!["晚清漕运夜航多依赖水路帮会网络".to_string()],
        }]);

        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0]["asset_type"], "exa_search_result");
        assert!(assets[0]["summary"].as_str().unwrap_or_default().contains("晚清漕运夜航"));
    }

    #[test]
    fn should_build_grok_assets_from_structured_response() {
        let assets = build_grok_assets(&json!({
            "content": "晚清漕运夜航与地方税卡、帮会协商关系紧密。",
            "sources": [
                {
                    "title": "晚清漕运研究",
                    "url": "https://example.com/grok-source",
                    "snippet": "研究指出夜航与税卡协调密切相关。"
                }
            ]
        }));

        assert!(!assets.is_empty());
        assert_eq!(assets[0]["asset_type"], "grok_search_summary");
    }

    #[test]
    fn should_keep_research_memory_type_constant() {
        assert_eq!(RESEARCH_MEMORY_TYPE, "research_reference");
    }
}
