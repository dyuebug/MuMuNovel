use std::collections::HashMap;

use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde_json::{json, Value};

use crate::ai::service::AIService;
use crate::models::project;
use crate::services::career_service::CareerService;
use crate::services::character_service::CharacterService;
use crate::services::prompt_template_service::PromptTemplateService;

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

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use crate::models::project;

    use super::{
        build_book_import_career_system_prompt_params, build_book_import_characters_prompt_params,
        build_book_import_world_building_prompt_params, BookImportAiExecutionContext,
        BookImportAiStepKind,
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
}
