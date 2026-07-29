use std::collections::HashMap;
use std::sync::Arc;

use crate::ai::service::AIService;
use crate::models::project;
use chrono::Utc;
use encoding_rs::*;
use sea_orm::DatabaseConnection;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::career_service::CareerService;
use super::chapter_service::ChapterService;
use super::character_service::CharacterService;
use super::novel_workflow_service::{resolve_internal_writing_transition, NovelWorkflowError};
use super::outline_service::OutlineService;
use super::project_service::{CreateProjectParams, ProjectService};
use super::prompt_template_service::PromptTemplateService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookImportProjectSuggestion {
    pub title: String,
    pub description: String,
    pub theme: Option<String>,
    pub genre: Option<String>,
    pub narrative_perspective: Option<String>,
    pub target_words: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookImportChapterImportSummary {
    pub chapter_count: usize,
    pub total_words: usize,
}

pub fn read_book_import_project_suggestion(
    project_suggestion: &Value,
) -> BookImportProjectSuggestion {
    BookImportProjectSuggestion {
        title: project_suggestion["title"]
            .as_str()
            .unwrap_or("拆书导入项目")
            .to_string(),
        description: project_suggestion["description"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        theme: project_suggestion["theme"].as_str().map(str::to_string),
        genre: project_suggestion["genre"].as_str().map(str::to_string),
        narrative_perspective: project_suggestion["narrative_perspective"]
            .as_str()
            .map(str::to_string),
        target_words: project_suggestion["target_words"]
            .as_i64()
            .unwrap_or(100000) as i32,
    }
}

pub fn build_book_import_create_project_params(
    user_id: &str,
    import_mode: &str,
    suggestion: &BookImportProjectSuggestion,
) -> CreateProjectParams {
    CreateProjectParams {
        user_id: user_id.to_string(),
        title: suggestion.title.clone(),
        description: Some(suggestion.description.clone()),
        theme: suggestion.theme.clone(),
        genre: suggestion.genre.clone(),
        target_words: suggestion.target_words,
        outline_mode: import_mode.to_string(),
        narrative_perspective: suggestion.narrative_perspective.clone(),
        ..Default::default()
    }
}

pub async fn create_book_import_project(
    db: &DatabaseConnection,
    user_id: &str,
    import_mode: &str,
    suggestion: &BookImportProjectSuggestion,
) -> Result<crate::models::project::Model, String> {
    let create_params = build_book_import_create_project_params(user_id, import_mode, suggestion);
    ProjectService::create_full(db, create_params)
        .await
        .map_err(|error| format!("项目创建失败: {}", error))
}

async fn commit_book_import_project_workflow(
    db: &DatabaseConnection,
    project: &project::Model,
) -> Result<project::Model, NovelWorkflowError> {
    let target_phase = resolve_internal_writing_transition(&project.status)?;
    let mut active: project::ActiveModel = project.clone().into();
    active.wizard_step = Set(4);
    active.wizard_status = Set("completed".to_string());
    active.status = Set(target_phase.as_str().to_string());
    active.updated_at = Set(Some(Utc::now().naive_utc()));
    active
        .update(db)
        .await
        .map_err(|error| NovelWorkflowError::Internal(error.to_string()))
}

pub async fn import_book_import_outlines(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    outlines: &[Value],
) -> usize {
    for (i, outline_item) in outlines.iter().enumerate() {
        let default_title = format!("第{}节", i + 1);
        let title = outline_item["title"].as_str().unwrap_or(&default_title);
        let _ = OutlineService::create(
            db,
            project_id,
            user_id,
            title,
            None,
            Some((i + 1) as i32),
            None,
        )
        .await;
    }

    outlines.len()
}

pub async fn import_book_import_chapters(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    chapters: &[Value],
) -> BookImportChapterImportSummary {
    let mut total_words = 0usize;

    for (i, chapter_item) in chapters.iter().enumerate() {
        let default_title = format!("第{}章", i + 1);
        let title = chapter_item["title"].as_str().unwrap_or(&default_title);
        let content = chapter_item["content"].as_str().unwrap_or("");
        total_words += content.chars().count();
        let _ = ChapterService::create(
            db,
            project_id,
            user_id,
            title,
            (i + 1) as i32,
            Some(content),
            None,
            None,
            None,
            None,
            None,
        )
        .await;
    }

    BookImportChapterImportSummary {
        chapter_count: chapters.len(),
        total_words,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookImportAiStepKind {
    WorldBuilding,
    CareerSystem,
    Characters,
}

impl BookImportAiStepKind {
    pub const ALL: [BookImportAiStepKind; 3] = [
        BookImportAiStepKind::WorldBuilding,
        BookImportAiStepKind::CareerSystem,
        BookImportAiStepKind::Characters,
    ];

    pub fn step_name(&self) -> &'static str {
        match self {
            BookImportAiStepKind::WorldBuilding => "world_building",
            BookImportAiStepKind::CareerSystem => "career_system",
            BookImportAiStepKind::Characters => "characters",
        }
    }

    pub fn step_label(&self) -> &'static str {
        match self {
            BookImportAiStepKind::WorldBuilding => "世界观生成",
            BookImportAiStepKind::CareerSystem => "职业体系生成",
            BookImportAiStepKind::Characters => "角色与组织生成",
        }
    }

    pub fn template_key(&self) -> &'static str {
        match self {
            BookImportAiStepKind::WorldBuilding => "WORLD_BUILDING",
            BookImportAiStepKind::CareerSystem => "CAREER_SYSTEM_GENERATION",
            BookImportAiStepKind::Characters => "CHARACTERS_BATCH_GENERATION",
        }
    }

    pub fn missing_template_message(&self) -> &'static str {
        match self {
            BookImportAiStepKind::WorldBuilding => "世界观模板不存在",
            BookImportAiStepKind::CareerSystem => "职业体系模板不存在",
            BookImportAiStepKind::Characters => "角色模板不存在",
        }
    }

    pub fn ai_progress_message(&self) -> &'static str {
        match self {
            BookImportAiStepKind::WorldBuilding => "🌍 AI正在生成世界观...",
            BookImportAiStepKind::CareerSystem => "💼 AI正在生成职业体系...",
            BookImportAiStepKind::Characters => "👥 AI正在生成角色...",
        }
    }

    pub fn from_step_name(step: &str) -> Option<Self> {
        match step {
            "world_building" => Some(BookImportAiStepKind::WorldBuilding),
            "career_system" => Some(BookImportAiStepKind::CareerSystem),
            "characters" => Some(BookImportAiStepKind::Characters),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BookImportAiExecutionContext<'a> {
    pub description: &'a str,
    pub theme: Option<&'a str>,
    pub genre: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookImportAiStepExecutionError {
    PromptFormat(String),
    Ai(String),
}

pub fn build_book_import_world_building_prompt_params(
    project: &project::Model,
    context: BookImportAiExecutionContext<'_>,
) -> HashMap<String, String> {
    let mut params = HashMap::new();
    params.insert("title".into(), project.title.clone());
    params.insert(
        "theme".into(),
        context.theme.unwrap_or("未设定").to_string(),
    );
    params.insert("genre".into(), context.genre.unwrap_or("通用").to_string());
    params.insert("description".into(), context.description.to_string());
    params
}

pub fn build_book_import_career_system_prompt_params(
    project: &project::Model,
    context: BookImportAiExecutionContext<'_>,
) -> HashMap<String, String> {
    let mut params = HashMap::new();
    params.insert("title".into(), project.title.clone());
    params.insert(
        "theme".into(),
        project
            .theme
            .clone()
            .or_else(|| context.theme.map(str::to_string))
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "genre".into(),
        project
            .genre
            .clone()
            .or_else(|| context.genre.map(str::to_string))
            .unwrap_or_else(|| "通用".into()),
    );
    params.insert(
        "time_period".into(),
        project
            .world_time_period
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "location".into(),
        project
            .world_location
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "atmosphere".into(),
        project
            .world_atmosphere
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "rules".into(),
        project
            .world_rules
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert("description".into(), context.description.to_string());
    params
}

pub fn build_book_import_characters_prompt_params(
    project: &project::Model,
    context: BookImportAiExecutionContext<'_>,
) -> HashMap<String, String> {
    let mut params = HashMap::new();
    params.insert("count".into(), "5".into());
    params.insert(
        "time_period".into(),
        project
            .world_time_period
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "location".into(),
        project
            .world_location
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "atmosphere".into(),
        project
            .world_atmosphere
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "rules".into(),
        project
            .world_rules
            .clone()
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "theme".into(),
        project
            .theme
            .clone()
            .or_else(|| context.theme.map(str::to_string))
            .unwrap_or_else(|| "未设定".into()),
    );
    params.insert(
        "genre".into(),
        project
            .genre
            .clone()
            .or_else(|| context.genre.map(str::to_string))
            .unwrap_or_else(|| "通用".into()),
    );
    params.insert("requirements".into(), String::new());
    params.insert("external_assets".into(), String::new());
    params.insert("reference_assets".into(), String::new());
    params
}

async fn ai_call_with_retry(
    ai_service: &AIService,
    prompt: &str,
    max_retries: u32,
) -> Result<Value, String> {
    let mut last_error = String::new();
    for attempt in 0..max_retries {
        match ai_service.generate_text(prompt, None, None).await {
            Ok(response) => {
                let cleaned =
                    crate::services::wizard_service::clean_json_response(&response.content);
                match serde_json::from_str::<Value>(&cleaned) {
                    Ok(data) => return Ok(data),
                    Err(error) => {
                        last_error = format!("JSON解析失败: {}", error);
                        if attempt + 1 < max_retries {
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    }
                }
            }
            Err(error) => {
                last_error = format!("AI调用失败: {}", error);
                if attempt + 1 < max_retries {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }
    }
    Err(last_error)
}

async fn apply_world_building_to_project(
    db: &DatabaseConnection,
    project: &project::Model,
    data: &Value,
) {
    if let Some(obj) = data.as_object() {
        let mut active: project::ActiveModel = project.clone().into();
        let mut updated = false;
        if let Some(value) = obj.get("time_period").and_then(|value| value.as_str()) {
            active.world_time_period = Set(Some(value.to_string()));
            updated = true;
        }
        if let Some(value) = obj.get("location").and_then(|value| value.as_str()) {
            active.world_location = Set(Some(value.to_string()));
            updated = true;
        }
        if let Some(value) = obj.get("atmosphere").and_then(|value| value.as_str()) {
            active.world_atmosphere = Set(Some(value.to_string()));
            updated = true;
        }
        if let Some(value) = obj.get("rules").and_then(|value| value.as_str()) {
            active.world_rules = Set(Some(value.to_string()));
            updated = true;
        }
        if updated {
            active.updated_at = Set(Some(chrono::Utc::now().naive_utc()));
            let _ = active.update(db).await;
        }
    }
}

async fn persist_career_system(
    db: &DatabaseConnection,
    project: &project::Model,
    data: &Value,
) -> i32 {
    let main_careers = data.get("main_careers").and_then(|value| value.as_array());
    let sub_careers = data.get("sub_careers").and_then(|value| value.as_array());
    let mut count = 0;

    if let Some(mains) = main_careers {
        for main in mains {
            let name = main
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or("未命名主职业");
            if CareerService::create_full(
                db,
                &project.id,
                name,
                "main",
                main.get("description").and_then(|value| value.as_str()),
                main.get("category").and_then(|value| value.as_str()),
                main.get("stages")
                    .and_then(|value| value.as_str())
                    .unwrap_or(""),
                main.get("max_stage")
                    .and_then(|value| value.as_i64())
                    .unwrap_or(1) as i32,
                main.get("requirements").and_then(|value| value.as_str()),
                main.get("special_abilities")
                    .and_then(|value| value.as_str()),
                main.get("worldview_rules").and_then(|value| value.as_str()),
                main.get("attribute_bonuses")
                    .and_then(|value| value.as_str()),
            )
            .await
            .is_ok()
            {
                count += 1;
            }
        }
    }

    if let Some(subs) = sub_careers {
        for sub in subs {
            let name = sub
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or("未命名副职业");
            if CareerService::create_full(
                db,
                &project.id,
                name,
                "sub",
                sub.get("description").and_then(|value| value.as_str()),
                sub.get("category").and_then(|value| value.as_str()),
                sub.get("stages")
                    .and_then(|value| value.as_str())
                    .unwrap_or(""),
                sub.get("max_stage")
                    .and_then(|value| value.as_i64())
                    .unwrap_or(1) as i32,
                sub.get("requirements").and_then(|value| value.as_str()),
                sub.get("special_abilities")
                    .and_then(|value| value.as_str()),
                sub.get("worldview_rules").and_then(|value| value.as_str()),
                sub.get("attribute_bonuses")
                    .and_then(|value| value.as_str()),
            )
            .await
            .is_ok()
            {
                count += 1;
            }
        }
    }

    count
}

async fn persist_characters(
    db: &DatabaseConnection,
    project: &project::Model,
    data: &Value,
) -> i32 {
    let characters: Vec<&Value> = if let Some(items) = data.as_array() {
        items.iter().collect()
    } else {
        vec![data]
    };
    let mut count = 0i32;

    for character in characters.iter().take(5) {
        let name = character
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or("未命名角色");
        let age_str: Option<String> = character.get("age").and_then(|value| {
            if let Some(number) = value.as_i64() {
                Some(number.to_string())
            } else {
                value.as_str().map(|text| text.to_string())
            }
        });
        if CharacterService::create_full(
            db,
            &project.id,
            name,
            character
                .get("is_organization")
                .and_then(|value| value.as_bool())
                .unwrap_or(false),
            character.get("role_type").and_then(|value| value.as_str()),
            character
                .get("personality")
                .and_then(|value| value.as_str()),
            character.get("background").and_then(|value| value.as_str()),
            character.get("appearance").and_then(|value| value.as_str()),
            age_str.as_deref(),
            character.get("gender").and_then(|value| value.as_str()),
            character.get("traits").and_then(|value| value.as_str()),
            character
                .get("organization_type")
                .and_then(|value| value.as_str()),
            character
                .get("organization_purpose")
                .and_then(|value| value.as_str()),
            character
                .get("relationships_text")
                .and_then(|value| value.as_str()),
        )
        .await
        .is_ok()
        {
            count += 1;
        }
    }

    count
}

pub async fn execute_book_import_ai_step(
    db: &DatabaseConnection,
    project: &project::Model,
    ai_service: &AIService,
    step: BookImportAiStepKind,
    context: BookImportAiExecutionContext<'_>,
) -> Result<Option<Value>, BookImportAiStepExecutionError> {
    let Some(template) = PromptTemplateService::system_template_info(step.template_key()) else {
        return Ok(None);
    };

    let params = match step {
        BookImportAiStepKind::WorldBuilding => {
            build_book_import_world_building_prompt_params(project, context)
        }
        BookImportAiStepKind::CareerSystem => {
            build_book_import_career_system_prompt_params(project, context)
        }
        BookImportAiStepKind::Characters => {
            build_book_import_characters_prompt_params(project, context)
        }
    };

    let prompt = PromptTemplateService::format_prompt(&template.content, &params)
        .map_err(|error| BookImportAiStepExecutionError::PromptFormat(error.to_string()))?;
    let data = ai_call_with_retry(ai_service, &prompt, 3)
        .await
        .map_err(BookImportAiStepExecutionError::Ai)?;

    let result = match step {
        BookImportAiStepKind::WorldBuilding => {
            apply_world_building_to_project(db, project, &data).await;
            data
        }
        BookImportAiStepKind::CareerSystem => {
            json!({ "count": persist_career_system(db, project, &data).await })
        }
        BookImportAiStepKind::Characters => {
            json!({ "count": persist_characters(db, project, &data).await })
        }
    };

    Ok(Some(result))
}

struct TxtParserService;

impl TxtParserService {
    fn decode_bytes(&self, content: &[u8]) -> (String, String) {
        if let Ok(text) = String::from_utf8(content.to_vec()) {
            return (text, "utf-8".to_string());
        }

        if content.len() >= 3 && &content[..3] == b"\xef\xbb\xbf" {
            if let Ok(text) = String::from_utf8(content[3..].to_vec()) {
                return (text, "utf-8-sig".to_string());
            }
        }

        let (decoded, _, had_errors) = GB18030.decode(content);
        if !had_errors || !decoded.is_empty() {
            return (decoded.into_owned(), "gb18030".to_string());
        }

        let (decoded, _, had_errors) = BIG5.decode(content);
        if !had_errors || !decoded.is_empty() {
            return (decoded.into_owned(), "big5".to_string());
        }

        let (decoded, _, _) = UTF_8.decode(content);
        (decoded.into_owned(), "utf-8(ignore)".to_string())
    }

    fn clean_text(&self, text: &str) -> String {
        let normalized = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\u{feff}', "");
        let normalized = normalized.replace('\u{3000}', "  ");

        let mut lines = String::with_capacity(normalized.len());
        for line in normalized.lines() {
            lines.push_str(line.trim_end_matches(&[' ', '\t'] as &[_]));
            lines.push('\n');
        }

        let mut compressed = String::with_capacity(lines.len());
        let mut newline_count = 0;
        for ch in lines.chars() {
            if ch == '\n' {
                newline_count += 1;
                if newline_count <= 3 {
                    compressed.push(ch);
                }
            } else {
                newline_count = 0;
                compressed.push(ch);
            }
        }

        compressed.trim().to_string()
    }

    fn split_chapters(&self, text: &str) -> Vec<Value> {
        if text.trim().is_empty() {
            return vec![];
        }

        let lines: Vec<&str> = text.lines().collect();
        let mut heading_indexes = Vec::new();

        for (idx, line) in lines.iter().enumerate() {
            let stripped = line.trim();
            if stripped.is_empty() {
                continue;
            }
            if self.is_strong_heading(stripped) || self.is_weak_heading(&lines, idx) {
                heading_indexes.push(idx);
            }
        }

        heading_indexes.sort_unstable();
        heading_indexes.dedup();

        if heading_indexes.is_empty() {
            return self.fallback_split(text);
        }

        let mut chapters = Vec::new();
        let mut chapter_no = 1;

        let first_heading = heading_indexes[0];
        if first_heading > 0 {
            let preface = lines[..first_heading].join("\n").trim().to_string();
            if preface.len() >= 200 {
                chapters.push(json!({
                    "title": "前言",
                    "content": preface,
                    "chapter_number": chapter_no,
                }));
                chapter_no += 1;
            }
        }

        for (idx, &start_idx) in heading_indexes.iter().enumerate() {
            let end_idx = if idx + 1 < heading_indexes.len() {
                heading_indexes[idx + 1]
            } else {
                lines.len()
            };

            let title = {
                let candidate = lines[start_idx].trim();
                if candidate.len() > 200 {
                    &candidate[..200]
                } else {
                    candidate
                }
            };

            let body_start = start_idx + 1;
            let body = if body_start < end_idx {
                lines[body_start..end_idx].join("\n").trim().to_string()
            } else {
                String::new()
            };

            let body = if body.is_empty() && idx + 1 < heading_indexes.len() {
                if start_idx + 1 < lines.len() {
                    lines[start_idx + 1].trim().to_string()
                } else {
                    String::new()
                }
            } else {
                body
            };

            let title = if title.is_empty() {
                format!("第{}章", chapter_no)
            } else {
                title.to_string()
            };

            chapters.push(json!({
                "title": title,
                "content": body,
                "chapter_number": chapter_no,
            }));
            chapter_no += 1;
        }

        let filtered: Vec<Value> = chapters
            .into_iter()
            .filter(|chapter| {
                let title = chapter["title"].as_str().unwrap_or("");
                let content = chapter["content"].as_str().unwrap_or("");
                !title.is_empty() || !content.is_empty()
            })
            .collect();

        if filtered.is_empty() {
            self.fallback_split(text)
        } else {
            filtered
        }
    }

    fn is_strong_heading(&self, line: &str) -> bool {
        self.match_chinese_chapter(line)
            || self.match_english_chapter(line)
            || self.match_chap_abbrev(line)
    }

    fn match_chinese_chapter(&self, line: &str) -> bool {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() || chars[0] != '第' {
            return false;
        }

        let mut has_digit = false;
        let mut idx = 1;
        while idx < chars.len() {
            let ch = chars[idx];
            if self.is_chinese_number(ch)
                || ch.is_ascii_digit()
                || ch == '零'
                || ch == '〇'
                || ch == '两'
            {
                has_digit = true;
                idx += 1;
            } else {
                break;
            }
        }

        if !has_digit || idx >= chars.len() {
            return false;
        }

        ['章', '节', '回', '卷', '集', '部', '篇'].contains(&chars[idx])
    }

    fn is_chinese_number(&self, ch: char) -> bool {
        matches!(
            ch,
            '一' | '二'
                | '三'
                | '四'
                | '五'
                | '六'
                | '七'
                | '八'
                | '九'
                | '十'
                | '百'
                | '千'
                | '万'
        )
    }

    fn match_english_chapter(&self, line: &str) -> bool {
        let lower = line.to_lowercase();
        let trimmed = lower.trim_start();
        if !trimmed.starts_with("chapter") {
            return false;
        }
        trimmed[7..]
            .trim_start()
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_digit())
    }

    fn match_chap_abbrev(&self, line: &str) -> bool {
        let lower = line.to_lowercase();
        let trimmed = lower.trim_start();
        if !trimmed.starts_with("chap.") {
            return false;
        }
        trimmed[5..]
            .trim_start()
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_digit())
    }

    fn is_weak_heading(&self, lines: &[&str], idx: usize) -> bool {
        let line = lines[idx].trim();
        if line.is_empty() || line.chars().count() > 25 {
            return false;
        }

        let punctuation = [
            '，', '。', '！', '？', '；', '：', ',', '.', '!', '?', ';', ':',
        ];
        if line.contains(&punctuation[..]) {
            return false;
        }

        let prev_blank = idx == 0 || lines[idx - 1].trim().is_empty();
        let next_blank = idx == lines.len() - 1 || lines[idx + 1].trim().is_empty();
        prev_blank && next_blank
    }

    fn fallback_split(&self, text: &str) -> Vec<Value> {
        let min_window = 3000usize;
        let max_window = 5000usize;
        let boundary: Vec<char> = "。！？!?\n".chars().collect();

        let mut chapters = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let total = chars.len();
        let mut start = 0;
        let mut chapter_no = 1;

        while start < total {
            let ideal_end = (start + max_window).min(total);
            let end = if ideal_end >= total {
                total
            } else {
                let search_from = (start + min_window).min(total);
                let segment: String = chars[search_from..ideal_end].iter().collect();
                match boundary
                    .iter()
                    .filter_map(|marker| segment.rfind(*marker))
                    .max()
                {
                    Some(offset) => search_from + offset + 1,
                    None => ideal_end,
                }
            };

            let chunk: String = chars[start..end].iter().collect();
            let chunk = chunk.trim().to_string();
            if !chunk.is_empty() {
                chapters.push(json!({
                    "title": format!("第{}章", chapter_no),
                    "content": chunk,
                    "chapter_number": chapter_no,
                }));
                chapter_no += 1;
            }
            start = end;
        }

        chapters
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct StepFailure {
    step_name: String,
    step_label: String,
    error_message: String,
    retry_count: u32,
}

#[derive(Debug, Clone)]
struct BookImportTask {
    task_id: String,
    user_id: String,
    filename: String,
    #[allow(dead_code)]
    import_mode: String,
    status: String,
    progress: i32,
    message: Option<String>,
    error: Option<String>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    preview: Option<Value>,
    cancelled: bool,
    #[allow(dead_code)]
    failed_steps: Vec<StepFailure>,
    imported_project_id: Option<String>,
}

pub struct BookImportService {
    tasks: Arc<Mutex<HashMap<String, BookImportTask>>>,
}

impl BookImportService {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn create_task(
        &self,
        user_id: &str,
        filename: &str,
        file_content: Vec<u8>,
        import_mode: &str,
    ) -> Value {
        let task_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let task = BookImportTask {
            task_id: task_id.clone(),
            user_id: user_id.to_string(),
            filename: filename.to_string(),
            import_mode: import_mode.to_string(),
            status: "pending".to_string(),
            progress: 0,
            message: Some("任务已创建".to_string()),
            error: None,
            created_at: now,
            updated_at: now,
            preview: None,
            cancelled: false,
            failed_steps: vec![],
            imported_project_id: None,
        };

        {
            let mut tasks = self.tasks.lock().await;
            tasks.insert(task_id.clone(), task);
        }

        let tasks = self.tasks.clone();
        let parser = TxtParserService;
        let tid = task_id.clone();
        tokio::spawn(async move {
            Self::run_pipeline(tasks, parser, tid, file_content).await;
        });

        json!({"task_id": task_id, "status": "pending"})
    }

    async fn run_pipeline(
        tasks: Arc<Mutex<HashMap<String, BookImportTask>>>,
        parser: TxtParserService,
        task_id: String,
        file_content: Vec<u8>,
    ) {
        let task_opt = {
            let tasks_guard = tasks.lock().await;
            tasks_guard.get(&task_id).cloned()
        };

        let Some(mut task) = task_opt else {
            return;
        };

        let update = |task: &mut BookImportTask,
                      status: &str,
                      progress: i32,
                      message: &str,
                      error: Option<&str>| {
            task.status = status.to_string();
            task.progress = progress.clamp(0, 100);
            task.message = Some(message.to_string());
            task.error = error.map(|s| s.to_string());
            task.updated_at = Utc::now();
        };

        // Helper to check cancellation
        let is_cancelled =
            |task: &BookImportTask| -> bool { task.cancelled || task.status == "cancelled" };

        // Progress: 5% — decode
        update(&mut task, "running", 5, "正在识别编码并读取文本...", None);
        if is_cancelled(&task) {
            let progress = task.progress;
            update(&mut task, "cancelled", progress, "任务已取消", None);
            Self::save_task(&tasks, &task).await;
            return;
        }

        let (text, encoding) = parser.decode_bytes(&file_content);
        let cleaned = parser.clean_text(&text);

        // 10% — clean done
        update(
            &mut task,
            "running",
            10,
            &format!("文本清洗完成（编码：{}）", encoding),
            None,
        );
        if is_cancelled(&task) {
            let progress = task.progress;
            update(&mut task, "cancelled", progress, "任务已取消", None);
            Self::save_task(&tasks, &task).await;
            return;
        }

        // 15% — split chapters
        let chapters_data = parser.split_chapters(&cleaned);
        if chapters_data.is_empty() {
            let progress = task.progress;
            update(
                &mut task,
                "failed",
                progress,
                "解析失败",
                Some("未能识别到有效章节，请检查TXT内容"),
            );
            Self::save_task(&tasks, &task).await;
            return;
        }

        update(
            &mut task,
            "running",
            15,
            &format!("已识别 {} 个章节，正在构建预览结构...", chapters_data.len()),
            None,
        );
        if is_cancelled(&task) {
            let progress = task.progress;
            update(&mut task, "cancelled", progress, "任务已取消", None);
            Self::save_task(&tasks, &task).await;
            return;
        }

        // 18% — keep last 10 chapters
        update(
            &mut task,
            "running",
            18,
            "仅保留末10章并重建预览结构...",
            None,
        );
        let preview = Self::build_preview(&task, &chapters_data);
        if is_cancelled(&task) {
            let progress = task.progress;
            update(&mut task, "cancelled", progress, "任务已取消", None);
            Self::save_task(&tasks, &task).await;
            return;
        }

        task.preview = Some(preview);
        update(
            &mut task,
            "completed",
            100,
            "解析完成，可预览并确认导入",
            None,
        );
        Self::save_task(&tasks, &task).await;
    }

    async fn save_task(tasks: &Arc<Mutex<HashMap<String, BookImportTask>>>, task: &BookImportTask) {
        let mut tasks_guard = tasks.lock().await;
        tasks_guard.insert(task.task_id.clone(), task.clone());
    }

    fn build_preview(task: &BookImportTask, chapters_data: &[Value]) -> Value {
        let filename_stem = std::path::Path::new(&task.filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("拆书导入项目");

        // Keep last 10 chapters
        let selected: Vec<&Value> = if chapters_data.len() > 10 {
            chapters_data[chapters_data.len() - 10..].iter().collect()
        } else {
            chapters_data.iter().collect()
        };
        let selected_total = selected.len();

        let mut chapters: Vec<Value> = vec![];
        let mut warnings: Vec<Value> = vec![];
        let mut title_counter: HashMap<String, u32> = HashMap::new();

        for (i, chapter) in selected.iter().enumerate() {
            let idx = i + 1;
            let fallback_title = format!("第{}章", idx);
            let raw_title = chapter["title"].as_str().unwrap_or(&fallback_title);
            let raw_title = if raw_title.len() > 200 {
                &raw_title[..200]
            } else {
                raw_title
            };
            let title = strip_chapter_prefix(raw_title);
            let content = chapter["content"].as_str().unwrap_or("").to_string();
            let summary = build_summary(&content, 120);

            chapters.push(json!({
                "title": title,
                "content": content,
                "summary": summary,
                "chapter_number": idx,
                "outline_title": title,
            }));

            *title_counter.entry(title.to_string()).or_insert(0) += 1;

            if content.chars().count() < 300 {
                warnings.push(json!({
                    "code": "chapter_too_short",
                    "message": format!("章节「{}」内容较短，建议检查切分结果", title),
                    "level": "warning",
                }));
            }
            if content.chars().count() > 12000 {
                warnings.push(json!({
                    "code": "chapter_too_long",
                    "message": format!("章节「{}」内容较长，建议确认是否应继续拆分", title),
                    "level": "info",
                }));
            }
        }

        for (title, count) in title_counter.iter() {
            if *count > 1 {
                warnings.push(json!({
                    "code": "duplicate_chapter_title",
                    "message": format!("检测到重复章节标题「{}」共 {} 次", title, count),
                    "level": "warning",
                }));
            }
        }

        if chapters_data.len() as usize > selected_total {
            warnings.push(json!({
                "code": "trimmed_to_last_ten_chapters",
                "message": format!(
                    "已按规则仅保留最后 {} 章用于导入（原始识别 {} 章）",
                    selected_total,
                    chapters_data.len()
                ),
                "level": "info",
            }));
        }

        // Build fallback project suggestion (rule-based, no AI)
        let sampled_chapters: Vec<&Value> = chapters.iter().take(3).collect();
        let sampled_text: String = sampled_chapters
            .iter()
            .filter_map(|c| {
                let content = c["content"].as_str().unwrap_or("");
                let snippet: String = content.chars().take(2000).collect();
                Some(snippet)
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let fallback_desc_source: String = sampled_chapters
            .iter()
            .filter_map(|c| {
                let summary = c["summary"].as_str().unwrap_or("");
                if !summary.is_empty() {
                    Some(summary.to_string())
                } else {
                    let content = c["content"].as_str().unwrap_or("");
                    let snippet: String = content.chars().take(600).collect();
                    if snippet.is_empty() {
                        None
                    } else {
                        Some(snippet)
                    }
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let fallback_description = build_summary(&fallback_desc_source, 500)
            .unwrap_or_else(|| "由拆书功能基于前3章自动提炼：该故事围绕核心人物与主要冲突展开，可在导入前继续修改。".to_string());

        let suggestion = json!({
            "title": if filename_stem.len() > 200 { &filename_stem[..200] } else { filename_stem },
            "description": fallback_description,
            "theme": detect_theme_from_text(&sampled_text),
            "genre": detect_genre_from_text(&sampled_text),
            "narrative_perspective": detect_narrative_perspective(&sampled_text),
            "target_words": 100000,
        });

        // Generate outlines from chapter titles (1:1 mapping, rule-based)
        let outlines: Vec<Value> = chapters
            .iter()
            .enumerate()
            .map(|(i, ch)| {
                json!({
                    "title": ch["title"],
                    "content": null,
                    "order_index": i + 1,
                    "structure": null,
                })
            })
            .collect();

        json!({
            "task_id": task.task_id,
            "project_suggestion": suggestion,
            "chapters": chapters,
            "outlines": outlines,
            "warnings": warnings,
        })
    }

    pub async fn get_task_status(&self, task_id: &str, user_id: &str) -> Result<Value, String> {
        let task = self.get_task(task_id, user_id).await?;
        Ok(json!({
            "task_id": task.task_id,
            "status": task.status,
            "progress": task.progress,
            "message": task.message,
            "error": task.error,
            "created_at": task.created_at.to_rfc3339(),
            "updated_at": task.updated_at.to_rfc3339(),
        }))
    }

    pub async fn get_preview(&self, task_id: &str, user_id: &str) -> Result<Value, String> {
        let task = self.get_task(task_id, user_id).await?;
        if task.status != "completed" {
            return Err("任务尚未完成，无法获取预览".to_string());
        }
        task.preview
            .clone()
            .ok_or_else(|| "预览数据不存在".to_string())
    }

    pub async fn cancel_task(&self, task_id: &str, user_id: &str) -> Result<Value, String> {
        let task = self.get_task(task_id, user_id).await?;
        if ["completed", "failed", "cancelled"].contains(&task.status.as_str()) {
            return Ok(
                json!({"success": true, "message": format!("任务已是终态：{}", task.status)}),
            );
        }

        let mut tasks = self.tasks.lock().await;
        if let Some(t) = tasks.get_mut(task_id) {
            t.cancelled = true;
            t.status = "cancelled".to_string();
            t.message = Some("任务已取消".to_string());
            t.updated_at = Utc::now();
        }

        Ok(json!({"success": true, "message": "取消成功"}))
    }

    async fn get_task(&self, task_id: &str, user_id: &str) -> Result<BookImportTask, String> {
        let tasks = self.tasks.lock().await;
        let task = tasks.get(task_id).ok_or("任务不存在".to_string())?;
        if task.user_id != user_id {
            return Err("无权访问该任务".to_string());
        }
        Ok(task.clone())
    }

    async fn set_imported_project_id(&self, task_id: &str, project_id: &str) {
        let mut tasks = self.tasks.lock().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.imported_project_id = Some(project_id.to_string());
        }
    }

    async fn execute_book_import_ai_generation_step(
        db: &sea_orm::DatabaseConnection,
        project: &crate::models::project::Model,
        ai_service: &crate::ai::service::AIService,
        step: BookImportAiStepKind,
        description: &str,
        theme: Option<&str>,
        genre: Option<&str>,
    ) -> Result<Option<Value>, String> {
        execute_book_import_ai_step(
            db,
            project,
            ai_service,
            step,
            BookImportAiExecutionContext {
                description,
                theme,
                genre,
            },
        )
        .await
        .map_err(|error| match error {
            BookImportAiStepExecutionError::PromptFormat(detail)
            | BookImportAiStepExecutionError::Ai(detail) => detail,
        })
    }

    pub async fn apply_import(
        &self,
        db: &sea_orm::DatabaseConnection,
        task_id: &str,
        user_id: &str,
        project_suggestion: &Value,
        chapters: &[Value],
        outlines: &[Value],
        import_mode: &str,
    ) -> Result<Value, String> {
        let task = self.get_task(task_id, user_id).await?;
        if task.status != "completed" {
            return Err("任务尚未完成解析，无法导入".to_string());
        }

        let suggestion = read_book_import_project_suggestion(project_suggestion);
        let project = create_book_import_project(db, user_id, import_mode, &suggestion).await?;
        self.set_imported_project_id(task_id, &project.id).await;
        import_book_import_outlines(db, &project.id, user_id, outlines).await;
        let _chapter_summary =
            import_book_import_chapters(db, &project.id, user_id, chapters).await;

        // --- Step 4-6: AI Wizard generation ---
        let ai_result = Self::run_wizard_generation(
            db,
            user_id,
            &project,
            &suggestion.description,
            suggestion.theme.as_deref(),
            suggestion.genre.as_deref(),
            None,
            None,
        )
        .await;

        // --- Update task ---
        {
            let mut tasks = self.tasks.lock().await;
            if let Some(t) = tasks.get_mut(task_id) {
                t.status = "imported".to_string();
                t.progress = 100;
                t.message = Some("导入完成".to_string());
                t.updated_at = Utc::now();
            }
        }

        let mut result = json!({
            "project_id": project.id,
            "project_title": project.title,
        });
        if let Some(ref ai) = ai_result {
            result["ai_generation"] = ai.clone();
        }

        Ok(result)
    }

    /// SSE streaming variant of apply_import with per-step progress events
    pub async fn apply_import_stream(
        &self,
        db: &sea_orm::DatabaseConnection,
        task_id: &str,
        user_id: &str,
        project_suggestion: &Value,
        chapters: &[Value],
        outlines: &[Value],
        import_mode: &str,
        channel: &crate::utils::sse::SseChannel,
    ) {
        use crate::ai::service::AIService;
        use crate::services::prompt_template_service::PromptTemplateService;
        use crate::services::settings_service::SettingsService;

        let task = match self.get_task(task_id, user_id).await {
            Ok(t) => t,
            Err(e) => {
                channel.error(&e, 400).await;
                channel.done().await;
                return;
            }
        };
        if task.status != "completed" {
            channel.error("任务尚未完成解析，无法导入", 400).await;
            channel.done().await;
            return;
        }

        if import_mode != "append" && import_mode != "overwrite" {
            channel
                .error("import_mode 仅支持 append 或 overwrite", 400)
                .await;
            channel.done().await;
            return;
        }

        let suggestion = read_book_import_project_suggestion(project_suggestion);
        channel
            .progress("开始导入拆书数据...", 0, "processing")
            .await;

        // --- Step 1: Create project (0-5%) ---
        channel.progress("正在创建项目...", 2, "processing").await;
        let project = match create_book_import_project(db, user_id, import_mode, &suggestion).await
        {
            Ok(p) => p,
            Err(e) => {
                channel.error(&e, 500).await;
                channel.done().await;
                return;
            }
        };
        self.set_imported_project_id(task_id, &project.id).await;
        channel.progress("项目创建完成", 5, "processing").await;

        // --- Step 2: Import outlines (5-10%) ---
        let outline_count = outlines.len();
        channel
            .progress(
                &format!("正在导入 {} 个大纲...", outline_count),
                6,
                "processing",
            )
            .await;
        import_book_import_outlines(db, &project.id, user_id, outlines).await;
        channel
            .progress(
                &format!("已导入 {} 个大纲", outline_count),
                10,
                "processing",
            )
            .await;

        // --- Step 3: Import chapters (10-20%) ---
        let chapter_count = chapters.len();
        channel
            .progress(
                &format!("正在导入 {} 个章节...", chapter_count),
                12,
                "processing",
            )
            .await;
        let chapter_summary = import_book_import_chapters(db, &project.id, user_id, chapters).await;
        let total_words = chapter_summary.total_words;
        channel
            .progress(
                &format!("已导入 {} 个章节（{}字）", chapter_count, total_words),
                20,
                "processing",
            )
            .await;

        // --- Build AI config ---
        let ai_config = match SettingsService::build_ai_config(db, user_id, None, None, None).await
        {
            Ok(cfg) => cfg,
            Err(e) => {
                channel.error(&format!("AI配置加载失败: {}", e), 500).await;
                channel.done().await;
                return;
            }
        };
        let ai_service = AIService::new(ai_config);
        let mut failed_steps: Vec<Value> = Vec::new();

        // --- Step 4: World building (20-40%) ---
        channel
            .progress("🌍 正在生成世界观...", 22, "processing")
            .await;
        channel
            .progress("🌍 正在初始化AI服务...", 25, "processing")
            .await;
        if PromptTemplateService::system_template_info("WORLD_BUILDING").is_some() {
            channel
                .progress("🌍 正在准备世界观提示词...", 28, "processing")
                .await;
            channel
                .progress("🌍 AI正在生成世界观...", 32, "processing")
                .await;
            match Self::execute_book_import_ai_generation_step(
                db,
                &project,
                &ai_service,
                BookImportAiStepKind::WorldBuilding,
                &suggestion.description,
                suggestion.theme.as_deref(),
                suggestion.genre.as_deref(),
            )
            .await
            {
                Ok(Some(_)) => {
                    channel
                        .progress("🌍 正在解析世界观数据...", 36, "processing")
                        .await;
                    channel
                        .progress("🌍 世界观写入完成", 38, "processing")
                        .await;
                    channel
                        .progress("🌍 世界观生成完成", 40, "processing")
                        .await;
                }
                Ok(None) => {
                    channel
                        .progress("🌍 世界观生成完成", 40, "processing")
                        .await;
                }
                Err(e) => {
                    failed_steps.push(json!({
                        "step": BookImportAiStepKind::WorldBuilding.step_name(),
                        "label": BookImportAiStepKind::WorldBuilding.step_label(),
                        "error": e,
                        "retry_count": 0,
                    }));
                    channel
                        .progress(
                            &format!("⚠️ 世界观生成失败：{}，将继续后续步骤", e),
                            40,
                            "warning",
                        )
                        .await;
                }
            }
        } else {
            channel
                .progress("🌍 世界观生成完成", 40, "processing")
                .await;
        }

        // --- Step 5: Career system (40-65%) ---
        channel
            .progress("💼 正在生成职业体系...", 42, "processing")
            .await;
        if PromptTemplateService::system_template_info("CAREER_SYSTEM_GENERATION").is_some() {
            channel
                .progress("💼 正在准备职业体系提示词...", 45, "processing")
                .await;
            channel
                .progress("💼 AI正在生成职业体系...", 50, "processing")
                .await;
            match Self::execute_book_import_ai_generation_step(
                db,
                &project,
                &ai_service,
                BookImportAiStepKind::CareerSystem,
                &suggestion.description,
                suggestion.theme.as_deref(),
                suggestion.genre.as_deref(),
            )
            .await
            {
                Ok(Some(data)) => {
                    let ccount = data.get("count").and_then(Value::as_i64).unwrap_or(0);
                    channel
                        .progress("💼 正在解析职业数据...", 58, "processing")
                        .await;
                    channel
                        .progress("💼 正在保存职业数据...", 62, "processing")
                        .await;
                    channel
                        .progress(
                            &format!("💼 职业体系生成完成（{}个）", ccount),
                            65,
                            "processing",
                        )
                        .await;
                }
                Ok(None) => {
                    channel
                        .progress("💼 职业体系生成完成（0个）", 65, "processing")
                        .await;
                }
                Err(e) => {
                    failed_steps.push(json!({
                        "step": BookImportAiStepKind::CareerSystem.step_name(),
                        "label": BookImportAiStepKind::CareerSystem.step_label(),
                        "error": e,
                        "retry_count": 0,
                    }));
                    channel
                        .progress(
                            &format!("⚠️ 职业体系生成失败：{}，将继续后续步骤", e),
                            65,
                            "warning",
                        )
                        .await;
                }
            }
        } else {
            channel
                .progress("💼 职业体系生成完成（0个）", 65, "processing")
                .await;
        }

        // --- Step 6: Characters (65-92%) ---
        channel
            .progress("👥 正在生成角色与组织...", 67, "processing")
            .await;
        if PromptTemplateService::system_template_info("CHARACTERS_BATCH_GENERATION").is_some() {
            channel
                .progress("👥 正在准备角色提示词...", 70, "processing")
                .await;
            channel
                .progress("👥 AI正在生成角色...", 75, "processing")
                .await;
            match Self::execute_book_import_ai_generation_step(
                db,
                &project,
                &ai_service,
                BookImportAiStepKind::Characters,
                &suggestion.description,
                suggestion.theme.as_deref(),
                suggestion.genre.as_deref(),
            )
            .await
            {
                Ok(Some(data)) => {
                    let chcount = data.get("count").and_then(Value::as_i64).unwrap_or(0);
                    channel
                        .progress("👥 正在解析角色数据...", 85, "processing")
                        .await;
                    channel
                        .progress("👥 正在保存角色...", 88, "processing")
                        .await;
                    channel
                        .progress(
                            &format!("👥 角色/组织生成完成（{}个）", chcount),
                            92,
                            "processing",
                        )
                        .await;
                }
                Ok(None) => {
                    channel
                        .progress("👥 角色/组织生成完成（0个）", 92, "processing")
                        .await;
                }
                Err(e) => {
                    failed_steps.push(json!({
                        "step": BookImportAiStepKind::Characters.step_name(),
                        "label": BookImportAiStepKind::Characters.step_label(),
                        "error": e,
                        "retry_count": 0,
                    }));
                    channel
                        .progress(&format!("⚠️ 角色/组织生成失败：{}", e), 92, "warning")
                        .await;
                }
            }
        } else {
            channel
                .progress("👥 角色/组织生成完成（0个）", 92, "processing")
                .await;
        }

        // --- Step 7: Commit (92-100%) ---
        channel
            .progress("正在保存到数据库...", 95, "processing")
            .await;

        // Update project wizard status through the novel workflow owner.
        if let Err(error) = commit_book_import_project_workflow(db, &project).await {
            let (message, status_code) = match error {
                NovelWorkflowError::Internal(message) => {
                    (format!("项目状态保存失败: {message}"), 500)
                }
                error => (format!("项目工作流状态更新失败: {error}"), 409),
            };
            channel.error(&message, status_code).await;
            channel.done().await;
            return;
        }

        // Update task + store failed_steps for retry
        {
            let mut tasks = self.tasks.lock().await;
            if let Some(t) = tasks.get_mut(task_id) {
                t.status = "imported".to_string();
                t.progress = 100;
                t.message = Some("导入完成".to_string());
                t.updated_at = Utc::now();
                t.failed_steps = failed_steps
                    .iter()
                    .map(|fs| StepFailure {
                        step_name: fs["step"].as_str().unwrap_or("").to_string(),
                        step_label: fs["label"].as_str().unwrap_or("").to_string(),
                        error_message: fs["error"].as_str().unwrap_or("").to_string(),
                        retry_count: 0,
                    })
                    .collect();
            }
        }

        channel.progress("数据保存完成", 98, "processing").await;

        if !failed_steps.is_empty() {
            let failed_count = failed_steps.len();
            channel
                .progress(
                    &format!(
                        "⚠️ 导入完成，但有 {} 个生成步骤失败，可点击重试",
                        failed_count
                    ),
                    98,
                    "warning",
                )
                .await;
            channel
                .progress(
                    &serde_json::to_string(&json!(failed_steps)).unwrap_or_default(),
                    98,
                    "step_failures",
                )
                .await;
        }

        channel
            .result(&json!({
                "success": true,
                "project_id": project.id,
                "statistics": {
                    "chapters_imported": chapter_count,
                    "total_words": total_words,
                    "outlines_imported": outline_count,
                },
                "failed_steps": failed_steps,
            }))
            .await;

        channel.progress("导入完成！", 100, "success").await;
        channel.done().await;
    }

    /// SSE streaming retry for failed AI steps
    pub async fn retry_stream(
        &self,
        db: &sea_orm::DatabaseConnection,
        task_id: &str,
        user_id: &str,
        steps_to_retry: &[String],
        channel: &crate::utils::sse::SseChannel,
    ) {
        use crate::ai::service::AIService;
        use crate::services::project_service::ProjectService;
        use crate::services::prompt_template_service::PromptTemplateService;
        use crate::services::settings_service::SettingsService;

        let task = match self.get_task(task_id, user_id).await {
            Ok(t) => t,
            Err(e) => {
                channel.error(&e, 400).await;
                channel.done().await;
                return;
            }
        };

        let project_id = match &task.imported_project_id {
            Some(id) => id.clone(),
            None => {
                channel.error("该任务尚未完成导入，无法重试", 400).await;
                channel.done().await;
                return;
            }
        };

        let failed_names: std::collections::HashSet<String> = task
            .failed_steps
            .iter()
            .map(|f| f.step_name.clone())
            .collect();
        let invalid: Vec<&String> = steps_to_retry
            .iter()
            .filter(|s| !failed_names.contains(*s))
            .collect();
        if !invalid.is_empty() {
            channel
                .error(&format!("以下步骤不在失败列表中: {:?}", invalid), 400)
                .await;
            channel.done().await;
            return;
        }

        let project = match ProjectService::get(db, &project_id, user_id).await {
            Ok(Some(p)) => p,
            Ok(None) => {
                channel.error("项目不存在或无权访问", 404).await;
                channel.done().await;
                return;
            }
            Err(e) => {
                channel.error(&format!("加载项目失败: {}", e), 500).await;
                channel.done().await;
                return;
            }
        };

        let ai_config = match SettingsService::build_ai_config(db, user_id, None, None, None).await
        {
            Ok(cfg) => cfg,
            Err(e) => {
                channel.error(&format!("AI配置加载失败: {}", e), 500).await;
                channel.done().await;
                return;
            }
        };
        let ai_service = AIService::new(ai_config);

        channel
            .progress("开始重试失败的生成步骤...", 0, "processing")
            .await;

        let total = steps_to_retry.len();
        let mut still_failed: Vec<Value> = Vec::new();
        let mut retry_results: HashMap<String, Value> = HashMap::new();

        for (idx, step) in steps_to_retry.iter().enumerate() {
            let start_pct = (5 + idx * 85 / total) as u32;
            let end_pct = (5 + (idx + 1) * 85 / total) as u32;
            let step_kind = BookImportAiStepKind::from_step_name(step);
            let label = step_kind
                .map(|kind| kind.step_label())
                .unwrap_or("未知步骤");

            channel
                .progress(&format!("🔄 正在重试{}...", label), start_pct, "processing")
                .await;

            let result: Result<Value, String> = match step_kind {
                Some(step_kind) => {
                    if PromptTemplateService::system_template_info(step_kind.template_key())
                        .is_none()
                    {
                        Err(step_kind.missing_template_message().into())
                    } else {
                        channel
                            .progress(
                                step_kind.ai_progress_message(),
                                start_pct + (end_pct - start_pct) / 2,
                                "processing",
                            )
                            .await;
                        Self::execute_book_import_ai_generation_step(
                            db,
                            &project,
                            &ai_service,
                            step_kind,
                            project.description.as_deref().unwrap_or_default(),
                            project.theme.as_deref(),
                            project.genre.as_deref(),
                        )
                        .await
                        .map(|value| value.unwrap_or(Value::Null))
                    }
                }
                None => Err(format!("未知步骤: {}", step)),
            };

            match result {
                Ok(data) => {
                    channel
                        .progress(&format!("✅ {}重试成功", label), end_pct, "processing")
                        .await;
                    retry_results.insert(step.clone(), data);
                }
                Err(e) => {
                    let retry_count = task
                        .failed_steps
                        .iter()
                        .find(|f| f.step_name == *step)
                        .map(|f| f.retry_count + 1)
                        .unwrap_or(1);
                    still_failed.push(json!({"step": step, "label": label, "error": e, "retry_count": retry_count}));
                    channel
                        .progress(&format!("⚠️ {}重试失败：{}", label, e), end_pct, "warning")
                        .await;
                }
            }
        }

        channel
            .progress("正在保存到数据库...", 93, "processing")
            .await;
        {
            let mut tasks = self.tasks.lock().await;
            if let Some(t) = tasks.get_mut(task_id) {
                t.failed_steps = still_failed
                    .iter()
                    .map(|fs| StepFailure {
                        step_name: fs["step"].as_str().unwrap_or("").to_string(),
                        step_label: fs["label"].as_str().unwrap_or("").to_string(),
                        error_message: fs["error"].as_str().unwrap_or("").to_string(),
                        retry_count: fs["retry_count"].as_u64().unwrap_or(0) as u32,
                    })
                    .collect();
                t.updated_at = Utc::now();
            }
        }
        channel.progress("数据保存完成", 96, "processing").await;

        if !still_failed.is_empty() {
            channel
                .progress(
                    &serde_json::to_string(&json!({"failed_steps": still_failed}))
                        .unwrap_or_default(),
                    98,
                    "step_failures",
                )
                .await;
        }

        channel
            .result(&json!({
                "success": true,
                "project_id": project_id,
                "retry_results": retry_results,
                "still_failed": still_failed,
            }))
            .await;

        if still_failed.is_empty() {
            channel.progress("所有步骤重试成功！", 100, "success").await;
        } else {
            channel
                .progress(
                    &format!("重试完成，仍有 {} 个步骤失败", still_failed.len()),
                    100,
                    "warning",
                )
                .await;
        }
        channel.done().await;
    }

    async fn run_wizard_generation(
        db: &sea_orm::DatabaseConnection,
        user_id: &str,
        project: &crate::models::project::Model,
        description: &str,
        theme: Option<&str>,
        genre: Option<&str>,
        provider_override: Option<&str>,
        model_override: Option<&str>,
    ) -> Option<Value> {
        use crate::ai::service::AIService;
        use crate::services::settings_service::SettingsService;

        let ai_config = match SettingsService::build_ai_config(
            db,
            user_id,
            provider_override,
            model_override,
            None,
        )
        .await
        {
            Ok(cfg) => cfg,
            Err(_) => return None,
        };
        let ai_service = AIService::new(ai_config);

        let mut results = Vec::new();

        for step in BookImportAiStepKind::ALL {
            match Self::execute_book_import_ai_generation_step(
                db,
                project,
                &ai_service,
                step,
                description,
                theme,
                genre,
            )
            .await
            {
                Ok(Some(data)) => match step {
                    BookImportAiStepKind::WorldBuilding => {
                        results.push(json!({"step": step.step_name(), "status": "ok"}));
                    }
                    BookImportAiStepKind::CareerSystem | BookImportAiStepKind::Characters => {
                        let count = data.get("count").and_then(Value::as_i64).unwrap_or(0);
                        results.push(
                            json!({"step": step.step_name(), "status": "ok", "count": count}),
                        );
                    }
                },
                Ok(None) => {}
                Err(error) => {
                    results.push(
                        json!({"step": step.step_name(), "status": "failed", "error": error}),
                    );
                }
            }
        }

        Some(json!({
            "steps": results,
            "total_steps": 3,
        }))
    }
}

fn build_summary(content: &str, max_len: usize) -> Option<String> {
    if content.is_empty() {
        return None;
    }
    let normalized: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_len {
        Some(normalized)
    } else {
        let truncated: String = normalized.chars().take(max_len).collect();
        Some(format!("{}…", truncated))
    }
}

fn strip_chapter_prefix(title: &str) -> String {
    let normalized = title.trim();
    if normalized.is_empty() {
        return normalized.to_string();
    }

    let chars: Vec<char> = normalized.chars().collect();
    if chars.is_empty() || chars[0] != '第' {
        return normalized.to_string();
    }

    // Find end of digit portion
    let mut i = 1;
    while i < chars.len() {
        let ch = chars[i];
        if ch.is_ascii_digit()
            || is_chinese_digit(ch)
            || ch == '零'
            || ch == '〇'
            || ch == '两'
            || ch == ' '
            || ch == '\u{3000}'
        {
            i += 1;
        } else {
            break;
        }
    }

    // Check for chapter unit character
    if i < chars.len() {
        let unit_chars = ['章', '节', '回', '卷', '集', '部', '篇'];
        if unit_chars.contains(&chars[i]) {
            i += 1;
            // Skip trailing separators: - — : ： 、 . ． ） ) 】 ] ]
            while i < chars.len() {
                let ch = chars[i];
                if matches!(
                    ch,
                    '-' | '—' | ':' | '：' | '、' | '.' | '．' | '）' | ')' | '】' | ']' | ' '
                ) {
                    i += 1;
                } else {
                    break;
                }
            }
            let stripped: String = chars[i..].iter().collect();
            let trimmed = stripped.trim().to_string();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
    }

    normalized.to_string()
}

fn is_chinese_digit(ch: char) -> bool {
    matches!(
        ch,
        '一' | '二' | '三' | '四' | '五' | '六' | '七' | '八' | '九' | '十' | '百' | '千' | '万'
    )
}

fn detect_theme_from_text(text: &str) -> &'static str {
    let keywords = [
        ("复仇", "复仇与救赎"),
        ("报仇", "复仇与救赎"),
        ("雪恨", "复仇与救赎"),
        ("成长", "成长与逆袭"),
        ("蜕变", "成长与逆袭"),
        ("逆袭", "成长与逆袭"),
        ("真相", "真相与抉择"),
        ("谜团", "真相与抉择"),
        ("秘密", "真相与抉择"),
        ("调查", "真相与抉择"),
        ("权谋", "权力与人性"),
        ("争权", "权力与人性"),
        ("朝堂", "权力与人性"),
        ("家族", "权力与人性"),
        ("爱情", "爱情与选择"),
        ("喜欢", "爱情与选择"),
        ("恋爱", "爱情与选择"),
        ("婚约", "爱情与选择"),
    ];
    for (keyword, theme) in &keywords {
        if text.contains(keyword) {
            return theme;
        }
    }
    "命运与选择"
}

fn detect_genre_from_text(text: &str) -> &'static str {
    let keywords = [
        ("修仙", "仙侠"),
        ("宗门", "仙侠"),
        ("灵气", "仙侠"),
        ("飞升", "仙侠"),
        ("仙门", "仙侠"),
        ("玄幻", "玄幻"),
        ("异界", "玄幻"),
        ("魔法", "玄幻"),
        ("斗气", "玄幻"),
        ("星际", "科幻"),
        ("机甲", "科幻"),
        ("赛博", "科幻"),
        ("人工智能", "科幻"),
        ("宇宙", "科幻"),
        ("悬疑", "悬疑"),
        ("凶案", "悬疑"),
        ("推理", "悬疑"),
        ("谜案", "悬疑"),
        ("诡", "悬疑"),
        ("总裁", "都市"),
        ("职场", "都市"),
        ("都市", "都市"),
        ("豪门", "都市"),
        ("恋爱", "言情"),
        ("言情", "言情"),
        ("心动", "言情"),
        ("告白", "言情"),
    ];
    for (keyword, genre) in &keywords {
        if text.contains(keyword) {
            return genre;
        }
    }
    "通用"
}

fn detect_narrative_perspective(text: &str) -> &'static str {
    let snippet: String = text.chars().take(6000).collect();
    let first_person: u32 = snippet.matches(&['我', '咱', '俺']).count() as u32;
    let third_person: u32 = snippet.matches(&['他', '她', '它']).count() as u32;

    if first_person >= 20 && first_person as f64 > third_person as f64 * 1.2 {
        "第一人称"
    } else {
        "第三人称"
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, DbBackend, EntityTrait, Schema};

    use crate::models::project;
    use crate::services::novel_workflow_service::{NovelWorkflowError, NovelWorkflowPhase};

    use super::{
        build_book_import_career_system_prompt_params, build_book_import_characters_prompt_params,
        build_book_import_world_building_prompt_params, commit_book_import_project_workflow,
        BookImportAiExecutionContext, BookImportAiStepKind, TxtParserService,
    };

    fn sample_project() -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "测试项目".to_string(),
            description: Some("项目简介".to_string()),
            theme: Some("成长".to_string()),
            genre: Some("玄幻".to_string()),
            target_words: 100000,
            current_words: 0,
            status: "draft".to_string(),
            wizard_status: "pending".to_string(),
            wizard_step: 0,
            outline_mode: "append".to_string(),
            world_time_period: Some("古代".to_string()),
            world_location: Some("王城".to_string()),
            world_atmosphere: Some("紧张".to_string()),
            world_rules: Some("强者为尊".to_string()),
            chapter_count: Some(10),
            narrative_perspective: Some("第三人称".to_string()),
            character_count: 0,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    async fn setup_book_import_project_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect book import sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(project::Entity)))
            .await
            .expect("create projects table");
        db
    }

    async fn insert_sample_project(
        db: &sea_orm::DatabaseConnection,
        status: &str,
    ) -> project::Model {
        let mut project = sample_project();
        project.status = status.to_string();
        let active: project::ActiveModel = project.into();
        active.insert(db).await.expect("insert sample project")
    }

    #[tokio::test]
    async fn should_commit_book_import_project_to_canonical_writing_phase() {
        let db = setup_book_import_project_db().await;
        let project = insert_sample_project(&db, " Draft ").await;

        let committed = commit_book_import_project_workflow(&db, &project)
            .await
            .expect("commit book import workflow");

        assert_eq!(committed.status, "writing");
        assert_eq!(committed.wizard_status, "completed");
        assert_eq!(committed.wizard_step, 4);
        let stored = project::Entity::find_by_id(&project.id)
            .one(&db)
            .await
            .expect("load committed project")
            .expect("project exists");
        assert_eq!(stored.status, "writing");
    }

    #[tokio::test]
    async fn should_reject_illegal_book_import_phase_without_mutation() {
        let db = setup_book_import_project_db().await;
        let project = insert_sample_project(&db, "completed").await;

        let error = commit_book_import_project_workflow(&db, &project)
            .await
            .expect_err("completed project cannot jump to writing");
        assert_eq!(
            error,
            NovelWorkflowError::IllegalTransition {
                from: NovelWorkflowPhase::Completed,
                to: NovelWorkflowPhase::Writing,
            }
        );
        let stored = project::Entity::find_by_id(&project.id)
            .one(&db)
            .await
            .expect("load unchanged project")
            .expect("project exists");
        assert_eq!(stored.status, "completed");
    }

    #[tokio::test]
    async fn should_surface_book_import_project_persistence_failure() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite without project table");
        let mut project = sample_project();
        project.status = "foundation".to_string();

        let error = commit_book_import_project_workflow(&db, &project)
            .await
            .expect_err("missing projects table must fail");
        assert!(matches!(error, NovelWorkflowError::Internal(_)));
    }

    #[test]
    fn should_build_world_building_prompt_params_from_context() {
        let project = sample_project();
        let params = build_book_import_world_building_prompt_params(
            &project,
            BookImportAiExecutionContext {
                description: "外部简介",
                theme: Some("复仇"),
                genre: Some("悬疑"),
            },
        );

        assert_eq!(params.get("title").map(String::as_str), Some("测试项目"));
        assert_eq!(params.get("theme").map(String::as_str), Some("复仇"));
        assert_eq!(params.get("genre").map(String::as_str), Some("悬疑"));
        assert_eq!(
            params.get("description").map(String::as_str),
            Some("外部简介")
        );
    }

    #[test]
    fn should_build_career_system_prompt_params_from_project_runtime_fields() {
        let project = sample_project();
        let params = build_book_import_career_system_prompt_params(
            &project,
            BookImportAiExecutionContext {
                description: "外部简介",
                theme: None,
                genre: None,
            },
        );

        assert_eq!(params.get("theme").map(String::as_str), Some("成长"));
        assert_eq!(params.get("genre").map(String::as_str), Some("玄幻"));
        assert_eq!(params.get("time_period").map(String::as_str), Some("古代"));
        assert_eq!(params.get("location").map(String::as_str), Some("王城"));
        assert_eq!(params.get("atmosphere").map(String::as_str), Some("紧张"));
        assert_eq!(params.get("rules").map(String::as_str), Some("强者为尊"));
        assert_eq!(
            params.get("description").map(String::as_str),
            Some("外部简介")
        );
    }

    #[test]
    fn should_build_characters_prompt_params_with_defaults_and_step_metadata() {
        let mut project = sample_project();
        project.theme = None;
        project.genre = None;

        let params = build_book_import_characters_prompt_params(
            &project,
            BookImportAiExecutionContext {
                description: "",
                theme: None,
                genre: None,
            },
        );

        assert_eq!(params.get("count").map(String::as_str), Some("5"));
        assert_eq!(params.get("theme").map(String::as_str), Some("未设定"));
        assert_eq!(params.get("genre").map(String::as_str), Some("通用"));
        assert_eq!(
            BookImportAiStepKind::Characters.missing_template_message(),
            "角色模板不存在"
        );
        assert_eq!(BookImportAiStepKind::Characters.step_name(), "characters");
        assert_eq!(
            BookImportAiStepKind::Characters.step_label(),
            "角色与组织生成"
        );
        assert_eq!(
            BookImportAiStepKind::Characters.ai_progress_message(),
            "👥 AI正在生成角色..."
        );
        assert_eq!(
            BookImportAiStepKind::from_step_name("characters"),
            Some(BookImportAiStepKind::Characters)
        );
        assert_eq!(BookImportAiStepKind::from_step_name("unknown"), None);
    }

    #[test]
    fn should_decode_utf8_sig_txt_bytes() {
        let parser = TxtParserService;
        let content = "\u{feff}第一章 开始\n这里是正文".as_bytes();

        let (text, encoding) = parser.decode_bytes(content);

        assert_eq!(encoding, "utf-8");
        assert!(text.contains("第一章 开始"));
    }

    #[test]
    fn should_clean_txt_text_and_normalize_blank_lines() {
        let parser = TxtParserService;
        let raw = "\u{feff}第一章\r\n正文第一段\u{3000}\r\n\r\n\r\n\r\n第二段  \n";

        let cleaned = parser.clean_text(raw);

        assert_eq!(cleaned, "第一章\n正文第一段\n\n\n第二段");
    }

    #[test]
    fn should_split_txt_chapters_by_detected_headings() {
        let parser = TxtParserService;
        let chapter_one_body = "甲".repeat(220);
        let raw = format!(
            "{chapter_one_body}\n第1章 初入宗门\n主角踏入山门。\n\n第2章 夜探藏经阁\n夜色里有人跟踪。"
        );

        let chapters = parser.split_chapters(&raw);

        let titles = chapters
            .iter()
            .map(|chapter| chapter["title"].as_str().unwrap_or(""))
            .collect::<Vec<_>>();
        assert_eq!(titles, vec!["前言", "第1章 初入宗门", "第2章 夜探藏经阁"]);
        assert_eq!(chapters[0]["chapter_number"].as_i64(), Some(1));
        assert_eq!(chapters[1]["content"].as_str(), Some("主角踏入山门。"));
        assert_eq!(chapters[2]["content"].as_str(), Some("夜色里有人跟踪。"));
    }

    #[test]
    fn should_fallback_split_txt_when_no_heading_found() {
        let parser = TxtParserService;
        let raw = format!("{}\n结尾。", "这是一段没有标题的正文。".repeat(220));

        let chapters = parser.split_chapters(&raw);

        assert!(!chapters.is_empty());
        assert_eq!(chapters[0]["title"].as_str(), Some("第1章"));
        assert!(chapters[0]["content"]
            .as_str()
            .is_some_and(|content| content.contains("这是一段没有标题的正文")));
    }
}
