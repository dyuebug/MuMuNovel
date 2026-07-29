use std::collections::HashMap;

use chrono::{NaiveDateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    models::{
        analysis_task, career, chapter, character, character_career, generation_history,
        organization, organization_member, outline, plot_analysis, project, project_default_style,
        relationship, story_memory, writing_style,
    },
    services::novel_workflow_service::{canonicalize_import_phase, NovelWorkflowError},
};

const SUPPORTED_IMPORT_VERSIONS: [&str; 3] = ["1.0.0", "1.1.0", "rust-strangler-1"];

#[derive(Debug)]
pub(crate) enum ValidateProjectImportPayloadError {
    InvalidJson(String),
}

#[derive(Debug)]
pub(crate) enum ImportProjectWriteWorkflowError {
    PayloadTooLarge,
    InvalidJson(String),
    Internal(String),
}

pub(crate) fn validate_project_import_payload(
    file_data: &[u8],
) -> Result<Value, ValidateProjectImportPayloadError> {
    let data: Value = serde_json::from_slice(file_data)
        .map_err(|error| ValidateProjectImportPayloadError::InvalidJson(error.to_string()))?;

    Ok(build_project_import_validation_result(&data))
}

fn build_project_import_validation_result(data: &Value) -> Value {
    let version = data.get("version").and_then(|v| v.as_str()).unwrap_or("");
    let project = data.get("project");
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    if version.is_empty() {
        errors.push("缺少版本信息".to_string());
    } else if !SUPPORTED_IMPORT_VERSIONS.contains(&version) {
        warnings.push(format!(
            "版本不匹配: 导入文件版本为 {}, 当前支持版本为 {}",
            version,
            SUPPORTED_IMPORT_VERSIONS.join(", ")
        ));
    }

    if let Some(project) = project {
        if project
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .is_none()
        {
            errors.push("项目标题不能为空".to_string());
        }
        if let Err(error) = canonical_project_import_status(project) {
            errors.push(format!("项目状态无效: {error}"));
        }
    } else {
        errors.push("缺少项目信息".to_string());
    }

    let legacy_memories_count = top_level_array_len(data, "memories");
    let story_memories_count = top_level_array_len(data, "story_memories");
    let stats = json!({
        "chapters": top_level_array_len(data, "chapters"),
        "characters": top_level_array_len(data, "characters"),
        "outlines": top_level_array_len(data, "outlines"),
        "relationships": top_level_array_len(data, "relationships"),
        "organizations": top_level_array_len(data, "organizations"),
        "organization_members": top_level_array_len(data, "organization_members"),
        "writing_styles": top_level_array_len(data, "writing_styles"),
        "generation_history": top_level_array_len(data, "generation_history"),
        "careers": top_level_array_len(data, "careers"),
        "character_careers": top_level_array_len(data, "character_careers"),
        "memories": legacy_memories_count,
        "story_memories": story_memories_count,
        "plot_analysis": top_level_array_len(data, "plot_analysis"),
        "has_default_style": data.get("project_default_style").is_some(),
    });

    if top_level_array_len(data, "chapters") == 0 {
        warnings.push("项目没有章节数据".to_string());
    }
    if top_level_array_len(data, "characters") == 0 {
        warnings.push("项目没有角色数据".to_string());
    }

    json!({
        "valid": errors.is_empty(),
        "version": version,
        "project_name": project
            .and_then(|p| p.get("title"))
            .and_then(|t| t.as_str())
            .unwrap_or("未知项目"),
        "statistics": stats,
        "errors": errors,
        "warnings": warnings,
    })
}

fn canonical_project_import_status(project: &Value) -> Result<&'static str, NovelWorkflowError> {
    let status = match project.get("status") {
        None | Some(Value::Null) => None,
        Some(Value::String(status)) => Some(status.as_str()),
        Some(value) => {
            return Err(NovelWorkflowError::InvalidPhase {
                value: value.to_string(),
            });
        }
    };

    canonicalize_import_phase(status).map(|phase| phase.as_str())
}

fn build_project_import_validation_failure_result(validation: &Value) -> Value {
    let errors = validation
        .get("errors")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    json!({
        "success": false,
        "project_id": null,
        "message": format!("数据验证失败: {}", errors),
        "statistics": {},
        "warnings": validation.get("warnings").cloned().unwrap_or_else(|| json!([])),
    })
}

pub(crate) async fn import_project_write_workflow(
    db: &DatabaseConnection,
    user_id: &str,
    file_data: &[u8],
) -> Result<Value, ImportProjectWriteWorkflowError> {
    if file_data.len() > 50 * 1024 * 1024 {
        return Err(ImportProjectWriteWorkflowError::PayloadTooLarge);
    }

    let data: Value = serde_json::from_slice(file_data)
        .map_err(|error| ImportProjectWriteWorkflowError::InvalidJson(error.to_string()))?;
    let validation = build_project_import_validation_result(&data);
    if !validation
        .get("valid")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(build_project_import_validation_failure_result(&validation));
    }

    let project_data = data
        .get("project")
        .expect("valid import payload should contain project");

    let now = Utc::now().naive_utc();
    let project_id = Uuid::new_v4().to_string();
    let title = json_string(project_data, "title").unwrap_or_else(|| "导入项目".to_string());
    let project_status = canonical_project_import_status(project_data)
        .expect("valid import payload should contain a known workflow phase");
    let target_words = json_i32(project_data, "target_words", 100_000);
    let chapters = data
        .get("chapters")
        .or_else(|| project_data.get("chapters"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let imported_project =
        project::ActiveModel {
            id: Set(project_id.clone()),
            user_id: Set(user_id.to_string()),
            title: Set(title),
            description: Set(json_string(project_data, "description")),
            theme: Set(json_string(project_data, "theme")),
            genre: Set(json_string(project_data, "genre")),
            target_words: Set(target_words),
            current_words: Set(0),
            status: Set(project_status.to_string()),
            wizard_status: Set(json_string(project_data, "wizard_status")
                .unwrap_or_else(|| "completed".to_string())),
            wizard_step: Set(json_i32(project_data, "wizard_step", 0)),
            outline_mode: Set(json_string(project_data, "outline_mode")
                .unwrap_or_else(|| "traditional".to_string())),
            world_time_period: Set(json_string(project_data, "world_time_period")),
            world_location: Set(json_string(project_data, "world_location")),
            world_atmosphere: Set(json_string(project_data, "world_atmosphere")),
            world_rules: Set(json_string(project_data, "world_rules")),
            chapter_count: Set(Some(chapters.len() as i32)),
            narrative_perspective: Set(json_string(project_data, "narrative_perspective")),
            character_count: Set(0),
            default_creative_mode: Set(json_string(project_data, "default_creative_mode")),
            default_story_focus: Set(json_string(project_data, "default_story_focus")),
            default_plot_stage: Set(json_string(project_data, "default_plot_stage")),
            default_story_creation_brief: Set(json_string(
                project_data,
                "default_story_creation_brief",
            )),
            default_quality_preset: Set(json_string(project_data, "default_quality_preset")),
            default_quality_notes: Set(json_string(project_data, "default_quality_notes")),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;

    let character_mapping =
        import_characters_for_project(db, &project_id, data.get("characters"), now)
            .await
            .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    let outline_mapping = import_outlines_for_project(db, &project_id, data.get("outlines"), now)
        .await
        .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;

    let mut current_words = 0i32;
    let mut chapter_title_mapping = HashMap::new();
    for (index, chapter_data) in chapters.iter().enumerate() {
        let content = json_string(chapter_data, "content");
        let word_count = chapter_data
            .get("word_count")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or_else(|| {
                content
                    .as_ref()
                    .map(|value| value.chars().count() as i32)
                    .unwrap_or(0)
            });
        current_words += word_count;
        let outline_id = chapter_data
            .get("outline_title")
            .and_then(Value::as_str)
            .and_then(|title| outline_mapping.get(title))
            .cloned();
        let expansion_plan = read_import_chapter_expansion_plan(chapter_data);
        let chapter_id = Uuid::new_v4().to_string();
        let chapter_title =
            json_string(chapter_data, "title").unwrap_or_else(|| format!("第{}章", index + 1));

        if !chapter_title_mapping.contains_key(&chapter_title) {
            chapter_title_mapping.insert(chapter_title.clone(), chapter_id.clone());
        }

        chapter::ActiveModel {
            id: Set(chapter_id),
            project_id: Set(imported_project.id.clone()),
            chapter_number: Set(json_i32(chapter_data, "chapter_number", index as i32 + 1)),
            title: Set(chapter_title),
            content: Set(content),
            summary: Set(json_string(chapter_data, "summary")),
            word_count: Set(word_count),
            status: Set(json_string(chapter_data, "status").unwrap_or_else(|| "draft".to_string())),
            outline_id: Set(outline_id),
            sub_index: Set(json_i32(chapter_data, "sub_index", 0)),
            expansion_plan: Set(expansion_plan),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    }

    let imported_user_id = imported_project.user_id.clone();
    let mut active_project: project::ActiveModel = imported_project.into();
    active_project.current_words = Set(current_words);
    active_project.character_count = Set(character_mapping.len() as i32);
    active_project
        .update(db)
        .await
        .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    let writing_styles_count =
        import_writing_styles_for_project(db, &imported_user_id, data.get("writing_styles"), now)
            .await
            .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    let career_mapping = import_careers_for_project(db, &project_id, data.get("careers"), now)
        .await
        .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    let character_careers_count = import_character_careers_for_project(
        db,
        &character_mapping,
        &career_mapping,
        data.get("character_careers"),
        now,
    )
    .await
    .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    let relationships_count = import_relationships_for_project(
        db,
        &project_id,
        &character_mapping,
        data.get("relationships"),
        now,
    )
    .await
    .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    let organization_mapping = import_organizations_for_project(
        db,
        &project_id,
        &character_mapping,
        data.get("organizations"),
        now,
    )
    .await
    .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    let organization_members_count = import_organization_members_for_project(
        db,
        &character_mapping,
        &organization_mapping,
        data.get("organization_members"),
        now,
    )
    .await
    .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    let story_memories_count = import_story_memories_for_project(
        db,
        &project_id,
        &chapter_title_mapping,
        &character_mapping,
        data.get("story_memories"),
        now,
    )
    .await
    .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    let generation_history_count = import_generation_history_for_project(
        db,
        &project_id,
        &chapter_title_mapping,
        data.get("generation_history"),
        now,
    )
    .await
    .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    let plot_analysis_count = import_plot_analysis_for_project(
        db,
        &project_id,
        &imported_user_id,
        &chapter_title_mapping,
        data.get("plot_analysis"),
        now,
    )
    .await
    .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    let default_style_imported = import_project_default_style_for_project(
        db,
        &project_id,
        &imported_user_id,
        data.get("project_default_style"),
        now,
    )
    .await
    .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;

    Ok(json!({
        "success": true,
        "project_id": project_id,
        "message": "项目导入成功",
        "statistics": {
            "characters": character_mapping.len(),
            "chapters": chapters.len(),
            "outlines": outline_mapping.len(),
            "relationships": relationships_count,
            "organizations": organization_mapping.len(),
            "organization_members": organization_members_count,
            "writing_styles": writing_styles_count,
            "careers": career_mapping.len(),
            "character_careers": character_careers_count,
            "story_memories": story_memories_count,
            "generation_history": generation_history_count,
            "plot_analysis": plot_analysis_count,
            "project_default_style": if default_style_imported { 1 } else { 0 },
        },
        "warnings": [],
    }))
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn top_level_array_len(value: &Value, key: &str) -> usize {
    value.get(key).and_then(Value::as_array).map_or(0, Vec::len)
}

fn json_i32(value: &Value, key: &str, default: i32) -> i32 {
    value
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(default)
}

fn json_f64(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(Value::as_f64)
}

fn json_array(value: &Value, key: &str) -> Option<Value> {
    value.get(key).cloned().filter(|value| value.is_array())
}

fn json_object(value: &Value, key: &str) -> Option<Value> {
    value.get(key).cloned().filter(|value| value.is_object())
}

fn json_bool_as_i32(value: &Value, key: &str, default: i32) -> i32 {
    value
        .get(key)
        .and_then(Value::as_bool)
        .map(|value| if value { 1 } else { 0 })
        .unwrap_or(default)
}

fn read_import_chapter_expansion_plan(value: &Value) -> Option<String> {
    match value.get("expansion_plan") {
        Some(Value::Object(map)) => serde_json::to_string(map).ok(),
        Some(Value::String(raw)) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        _ => None,
    }
}

fn read_import_character_traits(value: &Value) -> Option<String> {
    match value.get("traits") {
        Some(Value::Array(items)) => serde_json::to_string(items).ok(),
        Some(Value::String(raw)) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportCharacterRecord {
    name: String,
    age: Option<String>,
    gender: Option<String>,
    is_organization: bool,
    role_type: Option<String>,
    personality: Option<String>,
    background: Option<String>,
    appearance: Option<String>,
    traits: Option<String>,
    organization_type: Option<String>,
    organization_purpose: Option<String>,
}

fn read_import_character_record(value: &Value) -> Option<ImportCharacterRecord> {
    let name = json_string(value, "name")?;

    Some(ImportCharacterRecord {
        name,
        age: json_string(value, "age"),
        gender: json_string(value, "gender"),
        is_organization: value
            .get("is_organization")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        role_type: json_string(value, "role_type"),
        personality: json_string(value, "personality"),
        background: json_string(value, "background"),
        appearance: json_string(value, "appearance"),
        traits: read_import_character_traits(value),
        organization_type: json_string(value, "organization_type"),
        organization_purpose: json_string(value, "organization_purpose"),
    })
}

fn read_import_character_records(value: Option<&Value>) -> Vec<ImportCharacterRecord> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(read_import_character_record)
                .collect()
        })
        .unwrap_or_default()
}

async fn import_characters_for_project(
    db: &DatabaseConnection,
    project_id: &str,
    characters_value: Option<&Value>,
    now: chrono::NaiveDateTime,
) -> Result<HashMap<String, String>, sea_orm::DbErr> {
    let records = read_import_character_records(characters_value);
    let mut mapping = HashMap::new();

    for record in records {
        let character_id = Uuid::new_v4().to_string();
        character::ActiveModel {
            id: Set(character_id.clone()),
            project_id: Set(project_id.to_string()),
            name: Set(record.name.clone()),
            age: Set(record.age),
            gender: Set(record.gender),
            is_organization: Set(record.is_organization),
            role_type: Set(record.role_type),
            personality: Set(record.personality),
            background: Set(record.background),
            appearance: Set(record.appearance),
            relationships: Set(None),
            organization_type: Set(record.organization_type),
            organization_purpose: Set(record.organization_purpose),
            organization_members: Set(None),
            status: Set("active".to_string()),
            status_changed_chapter: Set(None),
            current_state: Set(None),
            state_updated_chapter: Set(None),
            main_career_id: Set(None),
            main_career_stage: Set(None),
            sub_careers: Set(None),
            avatar_url: Set(None),
            traits: Set(record.traits),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await?;
        mapping.insert(record.name, character_id);
    }

    Ok(mapping)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportWritingStyleRecord {
    name: String,
    style_type: String,
    preset_id: Option<String>,
    description: Option<String>,
    prompt_content: String,
    order_index: i32,
}

fn read_import_writing_style_record(value: &Value) -> Option<ImportWritingStyleRecord> {
    let name = json_string(value, "name")?;
    let prompt_content = json_string(value, "prompt_content")?;

    Some(ImportWritingStyleRecord {
        name,
        style_type: json_string(value, "style_type").unwrap_or_else(|| "custom".to_string()),
        preset_id: json_string(value, "preset_id"),
        description: json_string(value, "description"),
        prompt_content,
        order_index: json_i32(value, "order_index", 0),
    })
}

fn read_import_writing_style_records(value: Option<&Value>) -> Vec<ImportWritingStyleRecord> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(read_import_writing_style_record)
                .collect()
        })
        .unwrap_or_default()
}

async fn import_writing_styles_for_project(
    db: &DatabaseConnection,
    user_id: &str,
    styles: Option<&Value>,
    now: chrono::NaiveDateTime,
) -> Result<usize, sea_orm::DbErr> {
    let records = read_import_writing_style_records(styles);
    let mut count = 0usize;

    for record in records {
        let existing = writing_style::Entity::find()
            .filter(writing_style::Column::UserId.eq(user_id))
            .filter(writing_style::Column::Name.eq(record.name.clone()))
            .one(db)
            .await?;
        if existing.is_some() {
            continue;
        }

        writing_style::ActiveModel {
            user_id: Set(Some(user_id.to_string())),
            name: Set(record.name),
            style_type: Set(record.style_type),
            preset_id: Set(record.preset_id),
            description: Set(record.description),
            prompt_content: Set(record.prompt_content),
            order_index: Set(record.order_index),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await?;
        count += 1;
    }

    Ok(count)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportCareerRecord {
    name: String,
    career_type: String,
    description: Option<String>,
    category: Option<String>,
    stages: String,
    max_stage: i32,
    requirements: Option<String>,
    special_abilities: Option<String>,
    worldview_rules: Option<String>,
    attribute_bonuses: Option<String>,
    source: String,
}

fn read_import_career_record(value: &Value) -> Option<ImportCareerRecord> {
    let name = json_string(value, "name")?;

    Some(ImportCareerRecord {
        name,
        career_type: json_string(value, "type").unwrap_or_else(|| "main".to_string()),
        description: json_string(value, "description"),
        category: json_string(value, "category"),
        stages: json_string(value, "stages").unwrap_or_else(|| "[]".to_string()),
        max_stage: json_i32(value, "max_stage", 10),
        requirements: json_string(value, "requirements"),
        special_abilities: json_string(value, "special_abilities"),
        worldview_rules: json_string(value, "worldview_rules"),
        attribute_bonuses: json_string(value, "attribute_bonuses"),
        source: json_string(value, "source").unwrap_or_else(|| "ai".to_string()),
    })
}

fn read_import_career_records(value: Option<&Value>) -> Vec<ImportCareerRecord> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(read_import_career_record).collect())
        .unwrap_or_default()
}

async fn import_careers_for_project(
    db: &DatabaseConnection,
    project_id: &str,
    careers_value: Option<&Value>,
    now: chrono::NaiveDateTime,
) -> Result<HashMap<String, String>, sea_orm::DbErr> {
    let records = read_import_career_records(careers_value);
    let mut mapping = HashMap::new();

    for record in records {
        let career_id = Uuid::new_v4().to_string();
        career::ActiveModel {
            id: Set(career_id.clone()),
            project_id: Set(project_id.to_string()),
            name: Set(record.name.clone()),
            career_type: Set(record.career_type),
            description: Set(record.description),
            category: Set(record.category),
            stages: Set(record.stages),
            max_stage: Set(record.max_stage),
            requirements: Set(record.requirements),
            special_abilities: Set(record.special_abilities),
            worldview_rules: Set(record.worldview_rules),
            attribute_bonuses: Set(record.attribute_bonuses),
            source: Set(record.source),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await?;
        mapping.insert(record.name, career_id);
    }

    Ok(mapping)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportOutlineRecord {
    title: String,
    content: Option<String>,
    structure: Option<String>,
    order_index: Option<i32>,
}

fn read_import_outline_record(value: &Value) -> Option<ImportOutlineRecord> {
    let title = json_string(value, "title")?;

    Some(ImportOutlineRecord {
        title,
        content: json_string(value, "content"),
        structure: json_string(value, "structure"),
        order_index: value
            .get("order_index")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok()),
    })
}

fn read_import_outline_records(value: Option<&Value>) -> Vec<ImportOutlineRecord> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(read_import_outline_record)
                .collect()
        })
        .unwrap_or_default()
}

async fn import_outlines_for_project(
    db: &DatabaseConnection,
    project_id: &str,
    outlines_value: Option<&Value>,
    now: chrono::NaiveDateTime,
) -> Result<HashMap<String, String>, sea_orm::DbErr> {
    let records = read_import_outline_records(outlines_value);
    let mut mapping = HashMap::new();

    for record in records {
        let outline_id = Uuid::new_v4().to_string();
        outline::ActiveModel {
            id: Set(outline_id.clone()),
            project_id: Set(project_id.to_string()),
            title: Set(record.title.clone()),
            content: Set(record.content),
            structure: Set(record.structure),
            order_index: Set(record.order_index),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await?;
        mapping.insert(record.title, outline_id);
    }

    Ok(mapping)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportCharacterCareerRecord {
    character_name: String,
    career_name: String,
    career_type: String,
    current_stage: i32,
    stage_progress: i32,
    started_at: Option<String>,
    reached_current_stage_at: Option<String>,
    notes: Option<String>,
}

fn read_import_character_career_record(value: &Value) -> Option<ImportCharacterCareerRecord> {
    let character_name = json_string(value, "character_name")?;
    let career_name = json_string(value, "career_name")?;

    Some(ImportCharacterCareerRecord {
        character_name,
        career_name,
        career_type: json_string(value, "career_type").unwrap_or_else(|| "main".to_string()),
        current_stage: json_i32(value, "current_stage", 1),
        stage_progress: json_i32(value, "stage_progress", 0),
        started_at: json_string(value, "started_at"),
        reached_current_stage_at: json_string(value, "reached_current_stage_at"),
        notes: json_string(value, "notes"),
    })
}

fn read_import_character_career_records(value: Option<&Value>) -> Vec<ImportCharacterCareerRecord> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(read_import_character_career_record)
                .collect()
        })
        .unwrap_or_default()
}

async fn import_character_careers_for_project(
    db: &DatabaseConnection,
    character_mapping: &HashMap<String, String>,
    career_mapping: &HashMap<String, String>,
    character_careers_value: Option<&Value>,
    now: chrono::NaiveDateTime,
) -> Result<usize, sea_orm::DbErr> {
    let records = read_import_character_career_records(character_careers_value);
    let mut count = 0usize;

    for record in records {
        let Some(character_id) = character_mapping.get(&record.character_name).cloned() else {
            continue;
        };
        let Some(career_id) = career_mapping.get(&record.career_name).cloned() else {
            continue;
        };

        let existing = character_career::Entity::find()
            .filter(character_career::Column::CharacterId.eq(character_id.clone()))
            .filter(character_career::Column::CareerId.eq(career_id.clone()))
            .one(db)
            .await?;
        if existing.is_some() {
            continue;
        }

        character_career::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            character_id: Set(character_id.clone()),
            career_id: Set(career_id.clone()),
            career_type: Set(record.career_type.clone()),
            current_stage: Set(record.current_stage),
            stage_progress: Set(Some(record.stage_progress)),
            started_at: Set(record.started_at),
            reached_current_stage_at: Set(record.reached_current_stage_at),
            notes: Set(record.notes),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await?;
        count += 1;

        if record.career_type == "main" {
            if let Some(character_model) =
                character::Entity::find_by_id(character_id).one(db).await?
            {
                let mut active_character: character::ActiveModel = character_model.into();
                active_character.main_career_id = Set(Some(career_id));
                active_character.main_career_stage = Set(Some(record.current_stage));
                active_character.updated_at = Set(Some(now));
                active_character.update(db).await?;
            }
        }
    }

    Ok(count)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportRelationshipRecord {
    source_name: String,
    target_name: String,
    relationship_name: Option<String>,
    intimacy_level: i32,
    status: String,
    description: Option<String>,
    started_at: Option<String>,
}

fn read_import_relationship_record(value: &Value) -> Option<ImportRelationshipRecord> {
    let source_name = json_string(value, "source_name")?;
    let target_name = json_string(value, "target_name")?;

    Some(ImportRelationshipRecord {
        source_name,
        target_name,
        relationship_name: json_string(value, "relationship_name"),
        intimacy_level: json_i32(value, "intimacy_level", 50),
        status: json_string(value, "status").unwrap_or_else(|| "active".to_string()),
        description: json_string(value, "description"),
        started_at: json_string(value, "started_at"),
    })
}

fn read_import_relationship_records(value: Option<&Value>) -> Vec<ImportRelationshipRecord> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(read_import_relationship_record)
                .collect()
        })
        .unwrap_or_default()
}

async fn import_relationships_for_project(
    db: &DatabaseConnection,
    project_id: &str,
    character_mapping: &HashMap<String, String>,
    relationships_value: Option<&Value>,
    now: chrono::NaiveDateTime,
) -> Result<usize, sea_orm::DbErr> {
    let records = read_import_relationship_records(relationships_value);
    let mut count = 0usize;

    for record in records {
        let Some(source_id) = character_mapping.get(&record.source_name).cloned() else {
            continue;
        };
        let Some(target_id) = character_mapping.get(&record.target_name).cloned() else {
            continue;
        };

        relationship::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            character_from_id: Set(source_id),
            character_to_id: Set(target_id),
            relationship_type_id: Set(None),
            relationship_name: Set(record.relationship_name),
            intimacy_level: Set(record.intimacy_level),
            status: Set(record.status),
            description: Set(record.description),
            started_at: Set(record.started_at),
            ended_at: Set(None),
            source: Set("imported".to_string()),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await?;
        count += 1;
    }

    Ok(count)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportOrganizationRecord {
    character_name: String,
    parent_org_name: Option<String>,
    power_level: i32,
    member_count: i32,
    location: Option<String>,
    motto: Option<String>,
    color: Option<String>,
}

fn read_import_organization_record(value: &Value) -> Option<ImportOrganizationRecord> {
    let character_name = json_string(value, "character_name")?;

    Some(ImportOrganizationRecord {
        character_name,
        parent_org_name: json_string(value, "parent_org_name"),
        power_level: json_i32(value, "power_level", 50),
        member_count: json_i32(value, "member_count", 0),
        location: json_string(value, "location"),
        motto: json_string(value, "motto"),
        color: json_string(value, "color"),
    })
}

fn read_import_organization_records(value: Option<&Value>) -> Vec<ImportOrganizationRecord> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(read_import_organization_record)
                .collect()
        })
        .unwrap_or_default()
}

async fn import_organizations_for_project(
    db: &DatabaseConnection,
    project_id: &str,
    character_mapping: &HashMap<String, String>,
    organizations_value: Option<&Value>,
    now: chrono::NaiveDateTime,
) -> Result<HashMap<String, String>, sea_orm::DbErr> {
    let records = read_import_organization_records(organizations_value);
    let mut org_mapping = HashMap::new();
    let mut pending_parent_links: Vec<(String, String)> = Vec::new();

    for record in records {
        let Some(character_id) = character_mapping.get(&record.character_name).cloned() else {
            continue;
        };

        let organization_id = Uuid::new_v4().to_string();
        organization::ActiveModel {
            id: Set(organization_id.clone()),
            character_id: Set(character_id),
            project_id: Set(project_id.to_string()),
            parent_org_id: Set(None),
            level: Set(0),
            power_level: Set(record.power_level),
            member_count: Set(record.member_count),
            location: Set(record.location),
            motto: Set(record.motto),
            color: Set(record.color),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await?;

        if let Some(parent_org_name) = record.parent_org_name {
            pending_parent_links.push((organization_id.clone(), parent_org_name));
        }
        org_mapping.insert(record.character_name, organization_id);
    }

    for (organization_id, parent_org_name) in pending_parent_links {
        let Some(parent_org_id) = org_mapping.get(&parent_org_name).cloned() else {
            continue;
        };

        if let Some(organization_model) = organization::Entity::find_by_id(organization_id)
            .one(db)
            .await?
        {
            let mut active_organization: organization::ActiveModel = organization_model.into();
            active_organization.parent_org_id = Set(Some(parent_org_id));
            active_organization.updated_at = Set(Some(now));
            active_organization.update(db).await?;
        }
    }

    Ok(org_mapping)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportOrganizationMemberRecord {
    organization_name: String,
    character_name: String,
    position: String,
    rank: i32,
    status: String,
    joined_at: Option<String>,
    loyalty: i32,
    contribution: i32,
    notes: Option<String>,
}

fn read_import_organization_member_record(value: &Value) -> Option<ImportOrganizationMemberRecord> {
    let organization_name = json_string(value, "organization_name")?;
    let character_name = json_string(value, "character_name")?;
    let position = json_string(value, "position")?;

    Some(ImportOrganizationMemberRecord {
        organization_name,
        character_name,
        position,
        rank: json_i32(value, "rank", 0),
        status: json_string(value, "status").unwrap_or_else(|| "active".to_string()),
        joined_at: json_string(value, "joined_at"),
        loyalty: json_i32(value, "loyalty", 50),
        contribution: json_i32(value, "contribution", 0),
        notes: json_string(value, "notes"),
    })
}

fn read_import_organization_member_records(
    value: Option<&Value>,
) -> Vec<ImportOrganizationMemberRecord> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(read_import_organization_member_record)
                .collect()
        })
        .unwrap_or_default()
}

async fn import_organization_members_for_project(
    db: &DatabaseConnection,
    character_mapping: &HashMap<String, String>,
    organization_mapping: &HashMap<String, String>,
    organization_members_value: Option<&Value>,
    now: chrono::NaiveDateTime,
) -> Result<usize, sea_orm::DbErr> {
    let records = read_import_organization_member_records(organization_members_value);
    let mut count = 0usize;

    for record in records {
        let Some(organization_id) = organization_mapping.get(&record.organization_name).cloned()
        else {
            continue;
        };
        let Some(character_id) = character_mapping.get(&record.character_name).cloned() else {
            continue;
        };

        organization_member::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            organization_id: Set(organization_id),
            character_id: Set(character_id),
            position: Set(record.position),
            rank: Set(record.rank),
            status: Set(record.status),
            joined_at: Set(record.joined_at),
            left_at: Set(None),
            loyalty: Set(record.loyalty),
            contribution: Set(record.contribution),
            source: Set("imported".to_string()),
            notes: Set(record.notes),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await?;
        count += 1;
    }

    Ok(count)
}

#[derive(Debug, Clone, PartialEq)]
struct ImportStoryMemoryRecord {
    chapter_title: Option<String>,
    memory_type: String,
    title: Option<String>,
    content: String,
    full_context: Option<String>,
    related_characters: Vec<String>,
    related_locations: Option<Value>,
    tags: Option<Value>,
    importance_score: f64,
    story_timeline: i32,
    chapter_position: i32,
    text_length: i32,
    is_foreshadow: i32,
    foreshadow_strength: Option<f64>,
}

fn read_import_story_memory_record(value: &Value) -> Option<ImportStoryMemoryRecord> {
    let memory_type = json_string(value, "memory_type")?;
    let content = json_string(value, "content")?;
    let related_characters = value
        .get("related_characters")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(ImportStoryMemoryRecord {
        chapter_title: json_string(value, "chapter_title"),
        memory_type,
        title: json_string(value, "title"),
        content,
        full_context: json_string(value, "full_context"),
        related_characters,
        related_locations: json_array(value, "related_locations"),
        tags: json_array(value, "tags"),
        importance_score: json_f64(value, "importance_score").unwrap_or(0.5),
        story_timeline: json_i32(value, "story_timeline", 0),
        chapter_position: json_i32(value, "chapter_position", 0),
        text_length: json_i32(value, "text_length", 0),
        is_foreshadow: value
            .get("is_foreshadow")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or_else(|| json_bool_as_i32(value, "is_foreshadow", 0)),
        foreshadow_strength: json_f64(value, "foreshadow_strength"),
    })
}

fn read_import_story_memory_records(value: Option<&Value>) -> Vec<ImportStoryMemoryRecord> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(read_import_story_memory_record)
                .collect()
        })
        .unwrap_or_default()
}

async fn import_story_memories_for_project(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_mapping: &HashMap<String, String>,
    character_mapping: &HashMap<String, String>,
    story_memories_value: Option<&Value>,
    now: chrono::NaiveDateTime,
) -> Result<usize, sea_orm::DbErr> {
    let records = read_import_story_memory_records(story_memories_value);
    let mut count = 0usize;

    for record in records {
        let chapter_id = record
            .chapter_title
            .as_ref()
            .and_then(|title| chapter_mapping.get(title))
            .cloned();
        let related_characters = if record.related_characters.is_empty() {
            None
        } else {
            let ids = record
                .related_characters
                .iter()
                .filter_map(|name| character_mapping.get(name).cloned())
                .collect::<Vec<_>>();
            if ids.is_empty() {
                None
            } else {
                Some(Value::Array(ids.into_iter().map(Value::String).collect()))
            }
        };

        story_memory::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            chapter_id: Set(chapter_id),
            memory_type: Set(record.memory_type),
            title: Set(record.title),
            content: Set(record.content),
            full_context: Set(record.full_context),
            related_characters: Set(related_characters),
            related_locations: Set(record.related_locations),
            tags: Set(record.tags),
            importance_score: Set(Some(record.importance_score)),
            story_timeline: Set(record.story_timeline),
            chapter_position: Set(record.chapter_position),
            text_length: Set(record.text_length),
            is_foreshadow: Set(record.is_foreshadow),
            foreshadow_resolved_at: Set(None),
            foreshadow_strength: Set(record.foreshadow_strength),
            vector_id: Set(None),
            embedding_model: Set(None),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await?;
        count += 1;
    }

    Ok(count)
}

#[derive(Debug, Clone, PartialEq)]
struct ImportGenerationHistoryRecord {
    chapter_title: Option<String>,
    prompt: Option<String>,
    generated_content: Option<String>,
    model: Option<String>,
    tokens_used: Option<i32>,
    generation_time: Option<f64>,
    created_at: Option<NaiveDateTime>,
}

fn read_import_datetime(value: &Value, key: &str) -> Option<NaiveDateTime> {
    let raw = json_string(value, key)?;
    NaiveDateTime::parse_from_str(&raw, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(&raw, "%Y-%m-%d %H:%M:%S"))
        .ok()
}

fn read_import_generation_history_record(value: &Value) -> Option<ImportGenerationHistoryRecord> {
    let prompt = json_string(value, "prompt");
    let generated_content = json_string(value, "generated_content");
    let model = json_string(value, "model");

    if prompt.is_none() && generated_content.is_none() && model.is_none() {
        return None;
    }

    Some(ImportGenerationHistoryRecord {
        chapter_title: json_string(value, "chapter_title"),
        prompt,
        generated_content,
        model,
        tokens_used: read_optional_i32(value, "tokens_used"),
        generation_time: json_f64(value, "generation_time"),
        created_at: read_import_datetime(value, "created_at"),
    })
}

fn read_import_generation_history_records(
    value: Option<&Value>,
) -> Vec<ImportGenerationHistoryRecord> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(read_import_generation_history_record)
                .collect()
        })
        .unwrap_or_default()
}

async fn import_generation_history_for_project(
    db: &DatabaseConnection,
    project_id: &str,
    chapter_mapping: &HashMap<String, String>,
    generation_history_value: Option<&Value>,
    now: NaiveDateTime,
) -> Result<usize, sea_orm::DbErr> {
    let records = read_import_generation_history_records(generation_history_value);
    let mut count = 0usize;

    for record in records {
        let chapter_id = record
            .chapter_title
            .as_ref()
            .and_then(|title| chapter_mapping.get(title))
            .cloned();

        generation_history::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            chapter_id: Set(chapter_id),
            prompt: Set(record.prompt),
            generated_content: Set(record.generated_content),
            model: Set(record.model),
            tokens_used: Set(record.tokens_used),
            generation_time: Set(record.generation_time),
            created_at: Set(Some(record.created_at.unwrap_or(now))),
        }
        .insert(db)
        .await?;
        count += 1;
    }

    Ok(count)
}

#[derive(Debug, Clone, PartialEq)]
struct ImportPlotAnalysisRecord {
    chapter_title: String,
    plot_stage: Option<String>,
    conflict_level: Option<i32>,
    conflict_types: Option<Value>,
    emotional_tone: Option<String>,
    emotional_intensity: Option<f64>,
    emotional_curve: Option<Value>,
    hooks: Option<Value>,
    hooks_count: i32,
    hooks_avg_strength: Option<f64>,
    foreshadows: Option<Value>,
    foreshadows_planted: i32,
    foreshadows_resolved: i32,
    plot_points: Option<Value>,
    plot_points_count: i32,
    character_states: Option<Value>,
    scenes: Option<Value>,
    pacing: Option<String>,
    overall_quality_score: Option<f64>,
    pacing_score: Option<f64>,
    engagement_score: Option<f64>,
    coherence_score: Option<f64>,
    analysis_report: Option<String>,
    suggestions: Option<Value>,
    word_count: Option<i32>,
    dialogue_ratio: Option<f64>,
    description_ratio: Option<f64>,
}

fn read_optional_i32(value: &Value, key: &str) -> Option<i32> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
}

fn read_import_plot_analysis_record(value: &Value) -> Option<ImportPlotAnalysisRecord> {
    let chapter_title = json_string(value, "chapter_title")?;

    Some(ImportPlotAnalysisRecord {
        chapter_title,
        plot_stage: json_string(value, "plot_stage"),
        conflict_level: read_optional_i32(value, "conflict_level"),
        conflict_types: json_array(value, "conflict_types"),
        emotional_tone: json_string(value, "emotional_tone"),
        emotional_intensity: json_f64(value, "emotional_intensity"),
        emotional_curve: json_object(value, "emotional_curve"),
        hooks: json_array(value, "hooks"),
        hooks_count: json_i32(value, "hooks_count", 0),
        hooks_avg_strength: json_f64(value, "hooks_avg_strength"),
        foreshadows: json_array(value, "foreshadows"),
        foreshadows_planted: json_i32(value, "foreshadows_planted", 0),
        foreshadows_resolved: json_i32(value, "foreshadows_resolved", 0),
        plot_points: json_array(value, "plot_points"),
        plot_points_count: json_i32(value, "plot_points_count", 0),
        character_states: json_array(value, "character_states"),
        scenes: json_array(value, "scenes"),
        pacing: json_string(value, "pacing"),
        overall_quality_score: json_f64(value, "overall_quality_score"),
        pacing_score: json_f64(value, "pacing_score"),
        engagement_score: json_f64(value, "engagement_score"),
        coherence_score: json_f64(value, "coherence_score"),
        analysis_report: json_string(value, "analysis_report"),
        suggestions: json_array(value, "suggestions"),
        word_count: read_optional_i32(value, "word_count"),
        dialogue_ratio: json_f64(value, "dialogue_ratio"),
        description_ratio: json_f64(value, "description_ratio"),
    })
}

fn read_import_plot_analysis_records(value: Option<&Value>) -> Vec<ImportPlotAnalysisRecord> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(read_import_plot_analysis_record)
                .collect()
        })
        .unwrap_or_default()
}

async fn import_plot_analysis_for_project(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    chapter_mapping: &HashMap<String, String>,
    plot_analysis_value: Option<&Value>,
    now: chrono::NaiveDateTime,
) -> Result<usize, sea_orm::DbErr> {
    let records = read_import_plot_analysis_records(plot_analysis_value);
    let mut count = 0usize;

    for record in records {
        let Some(chapter_id) = chapter_mapping.get(&record.chapter_title).cloned() else {
            continue;
        };

        let existing = plot_analysis::Entity::find()
            .filter(plot_analysis::Column::ChapterId.eq(chapter_id.clone()))
            .one(db)
            .await?;
        if existing.is_some() {
            continue;
        }

        plot_analysis::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            chapter_id: Set(chapter_id.clone()),
            source_content_digest: Set(None),
            plot_stage: Set(record.plot_stage),
            conflict_level: Set(record.conflict_level),
            conflict_types: Set(record.conflict_types),
            emotional_tone: Set(record.emotional_tone),
            emotional_intensity: Set(record.emotional_intensity),
            emotional_curve: Set(record.emotional_curve),
            hooks: Set(record.hooks),
            hooks_count: Set(record.hooks_count),
            hooks_avg_strength: Set(record.hooks_avg_strength),
            foreshadows: Set(record.foreshadows),
            foreshadows_planted: Set(record.foreshadows_planted),
            foreshadows_resolved: Set(record.foreshadows_resolved),
            plot_points: Set(record.plot_points),
            plot_points_count: Set(record.plot_points_count),
            character_states: Set(record.character_states),
            scenes: Set(record.scenes),
            pacing: Set(record.pacing),
            overall_quality_score: Set(record.overall_quality_score),
            pacing_score: Set(record.pacing_score),
            engagement_score: Set(record.engagement_score),
            coherence_score: Set(record.coherence_score),
            analysis_report: Set(record.analysis_report),
            suggestions: Set(record.suggestions),
            word_count: Set(record.word_count),
            dialogue_ratio: Set(record.dialogue_ratio),
            description_ratio: Set(record.description_ratio),
            created_at: Set(Some(now)),
        }
        .insert(db)
        .await?;

        analysis_task::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            chapter_id: Set(chapter_id),
            user_id: Set(user_id.to_string()),
            project_id: Set(project_id.to_string()),
            status: Set("completed".to_string()),
            progress: Set(100),
            error_message: Set(None),
            created_at: Set(Some(now)),
            started_at: Set(Some(now)),
            completed_at: Set(Some(now)),
        }
        .insert(db)
        .await?;

        count += 1;
    }

    Ok(count)
}

fn read_import_project_default_style_name(value: Option<&Value>) -> Option<String> {
    value.and_then(|style| json_string(style, "style_name"))
}

async fn import_project_default_style_for_project(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    default_style_value: Option<&Value>,
    now: chrono::NaiveDateTime,
) -> Result<bool, sea_orm::DbErr> {
    let Some(style_name) = read_import_project_default_style_name(default_style_value) else {
        return Ok(false);
    };

    let style = writing_style::Entity::find()
        .filter(writing_style::Column::UserId.eq(user_id))
        .filter(writing_style::Column::Name.eq(style_name.clone()))
        .one(db)
        .await?;

    let style = match style {
        Some(style) => Some(style),
        None => {
            writing_style::Entity::find()
                .filter(writing_style::Column::UserId.is_null())
                .filter(writing_style::Column::Name.eq(style_name))
                .one(db)
                .await?
        }
    };

    let Some(style) = style else {
        return Ok(false);
    };

    project_default_style::ActiveModel {
        project_id: Set(project_id.to_string()),
        style_id: Set(style.id),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_project_import_status, import_project_write_workflow, read_import_career_records,
        read_import_chapter_expansion_plan, read_import_character_career_records,
        read_import_character_records, read_import_character_traits,
        read_import_generation_history_records, read_import_organization_member_records,
        read_import_organization_records, read_import_outline_records,
        read_import_plot_analysis_records, read_import_project_default_style_name,
        read_import_relationship_records, read_import_story_memory_records,
        read_import_writing_style_records, validate_project_import_payload,
        ImportProjectWriteWorkflowError, ValidateProjectImportPayloadError,
    };
    use sea_orm::DatabaseConnection;
    use serde_json::json;

    #[test]
    fn validate_project_import_payload_keeps_existing_contract_shape() {
        let payload = json!({
            "version": "rust-strangler-1",
            "project": {
                "title": "导入测试"
            },
            "chapters": [{ "title": "第一章" }],
            "characters": [{ "name": "甲" }],
            "organization_members": [{ "position": "掌门" }],
            "character_careers": [{ "career_name": "剑修" }],
            "story_memories": [{ "title": "初遇", "content": "正文" }],
            "plot_analysis": [],
            "project_default_style": { "style_name": "默认" }
        });

        let result = validate_project_import_payload(
            serde_json::to_vec(&payload)
                .expect("payload should serialize")
                .as_slice(),
        )
        .expect("payload should validate");

        assert_eq!(result.get("valid").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            result.get("project_name").and_then(|v| v.as_str()),
            Some("导入测试")
        );
        assert_eq!(
            result
                .get("statistics")
                .and_then(|stats| stats.get("chapters"))
                .and_then(|v| v.as_u64()),
            Some(1)
        );
        assert_eq!(
            result
                .get("statistics")
                .and_then(|stats| stats.get("organization_members"))
                .and_then(|v| v.as_u64()),
            Some(1)
        );
        assert_eq!(
            result
                .get("statistics")
                .and_then(|stats| stats.get("character_careers"))
                .and_then(|v| v.as_u64()),
            Some(1)
        );
        assert_eq!(
            result
                .get("statistics")
                .and_then(|stats| stats.get("story_memories"))
                .and_then(|v| v.as_u64()),
            Some(1)
        );
        assert_eq!(
            result
                .get("statistics")
                .and_then(|stats| stats.get("memories"))
                .and_then(|v| v.as_u64()),
            Some(0)
        );
        assert_eq!(
            result
                .get("statistics")
                .and_then(|stats| stats.get("has_default_style"))
                .and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            result
                .get("errors")
                .and_then(|v| v.as_array())
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(
            result
                .get("warnings")
                .and_then(|v| v.as_array())
                .map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn project_import_phase_defaults_missing_null_or_blank_status_to_foundation() {
        let projects = [
            json!({ "title": "缺省状态" }),
            json!({ "title": "空状态", "status": null }),
            json!({ "title": "空串状态", "status": "" }),
            json!({ "title": "空白状态", "status": "   " }),
        ];

        for project in projects {
            assert_eq!(
                canonical_project_import_status(&project)
                    .expect("missing or blank import status should use the default phase"),
                "foundation"
            );

            let payload = json!({
                "version": "rust-strangler-1",
                "project": project,
            });
            let result = validate_project_import_payload(
                serde_json::to_vec(&payload)
                    .expect("payload should serialize")
                    .as_slice(),
            )
            .expect("payload should produce a validation result");

            assert_eq!(
                result.get("valid").and_then(|value| value.as_bool()),
                Some(true)
            );
        }
    }

    #[test]
    fn project_import_phase_canonicalizes_legacy_aliases() {
        for (legacy, canonical) in [
            ("planning", "foundation"),
            ("draft", "foundation"),
            ("revising", "reviewing"),
            ("active", "writing"),
        ] {
            let project = json!({
                "title": "历史状态导入",
                "status": legacy,
            });

            assert_eq!(
                canonical_project_import_status(&project)
                    .expect("legacy import phase should be accepted"),
                canonical
            );

            let payload = json!({
                "version": "rust-strangler-1",
                "project": project,
            });
            let result = validate_project_import_payload(
                serde_json::to_vec(&payload)
                    .expect("payload should serialize")
                    .as_slice(),
            )
            .expect("payload should produce a validation result");

            assert_eq!(
                result.get("valid").and_then(|value| value.as_bool()),
                Some(true)
            );
        }
    }

    #[tokio::test]
    async fn project_import_phase_rejects_unknown_status_before_database_access() {
        let payload = serde_json::to_vec(&json!({
            "version": "rust-strangler-1",
            "project": {
                "title": "未知状态导入",
                "status": "mystery"
            }
        }))
        .expect("payload should serialize");

        let validation = validate_project_import_payload(&payload)
            .expect("payload should produce a validation result");
        assert_eq!(
            validation.get("valid").and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            validation.get("errors"),
            Some(&json!(["项目状态无效: invalid workflow phase: mystery"]))
        );

        let malformed_payload = serde_json::to_vec(&json!({
            "version": "rust-strangler-1",
            "project": {
                "title": "错误类型状态导入",
                "status": 42
            }
        }))
        .expect("payload should serialize");
        let malformed_validation = validate_project_import_payload(&malformed_payload)
            .expect("malformed status should produce a validation result");
        assert_eq!(
            malformed_validation.get("errors"),
            Some(&json!(["项目状态无效: invalid workflow phase: 42"]))
        );

        let result =
            import_project_write_workflow(&DatabaseConnection::Disconnected, "user-1", &payload)
                .await
                .expect("unknown workflow phase should return validation failure before DB access");

        assert_eq!(
            result.get("success").and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            result.get("message").and_then(|value| value.as_str()),
            Some("数据验证失败: 项目状态无效: invalid workflow phase: mystery")
        );
    }

    #[test]
    fn validate_project_import_payload_keeps_legacy_memories_statistics_alias() {
        let payload = json!({
            "version": "1.1.0",
            "project": {
                "title": "旧记忆字段导入"
            },
            "memories": [{ "title": "旧记忆", "content": "兼容旧导出包" }],
            "story_memories": [{ "title": "新记忆", "content": "兼容新导出包" }]
        });

        let result = validate_project_import_payload(
            serde_json::to_vec(&payload)
                .expect("payload should serialize")
                .as_slice(),
        )
        .expect("payload should validate");

        assert_eq!(
            result
                .get("statistics")
                .and_then(|stats| stats.get("memories"))
                .and_then(|v| v.as_u64()),
            Some(1)
        );
        assert_eq!(
            result
                .get("statistics")
                .and_then(|stats| stats.get("story_memories"))
                .and_then(|v| v.as_u64()),
            Some(1)
        );
    }

    #[test]
    fn validate_project_import_payload_matches_python_validation_messages() {
        let payload = json!({
            "project": {
                "title": "   "
            }
        });

        let result = validate_project_import_payload(
            serde_json::to_vec(&payload)
                .expect("payload should serialize")
                .as_slice(),
        )
        .expect("payload should validate into validation result");

        assert_eq!(result.get("valid").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(result.get("version").and_then(|v| v.as_str()), Some(""));
        assert_eq!(
            result.get("project_name").and_then(|v| v.as_str()),
            Some("   ")
        );
        assert_eq!(
            result.get("errors"),
            Some(&json!(["缺少版本信息", "项目标题不能为空"]))
        );
        assert_eq!(
            result.get("warnings"),
            Some(&json!(["项目没有章节数据", "项目没有角色数据"]))
        );
    }

    #[test]
    fn validate_project_import_payload_warns_on_unknown_version_like_python() {
        let payload = json!({
            "version": "9.9.9",
            "project": {
                "title": "导入测试"
            },
            "chapters": [{ "title": "第一章" }],
            "characters": [{ "name": "甲" }]
        });

        let result = validate_project_import_payload(
            serde_json::to_vec(&payload)
                .expect("payload should serialize")
                .as_slice(),
        )
        .expect("payload should validate into validation result");

        assert_eq!(result.get("valid").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            result.get("warnings"),
            Some(&json!([
                "版本不匹配: 导入文件版本为 9.9.9, 当前支持版本为 1.0.0, 1.1.0, rust-strangler-1"
            ]))
        );
    }

    #[test]
    fn validate_project_import_payload_reports_missing_project_like_python() {
        let payload = json!({
            "version": "1.1.0"
        });

        let result = validate_project_import_payload(
            serde_json::to_vec(&payload)
                .expect("payload should serialize")
                .as_slice(),
        )
        .expect("payload should validate into validation result");

        assert_eq!(result.get("valid").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            result.get("project_name").and_then(|v| v.as_str()),
            Some("未知项目")
        );
        assert_eq!(result.get("errors"), Some(&json!(["缺少项目信息"])));
        assert_eq!(
            result.get("warnings"),
            Some(&json!(["项目没有章节数据", "项目没有角色数据"]))
        );
    }

    #[test]
    fn validate_project_import_payload_reports_invalid_json() {
        let error =
            validate_project_import_payload(b"{broken json").expect_err("broken json should fail");

        assert!(matches!(
            error,
            ValidateProjectImportPayloadError::InvalidJson(detail)
                if detail.contains("expected")
                    || detail.contains("EOF")
                    || detail.contains("key")
        ));
    }

    #[tokio::test]
    async fn import_project_write_workflow_rejects_large_payload_before_db_access() {
        let too_large = vec![b'x'; 50 * 1024 * 1024 + 1];
        let db = DatabaseConnection::Disconnected;

        let error = import_project_write_workflow(&db, "user-1", &too_large)
            .await
            .expect_err("large payload should fail");

        assert!(matches!(
            error,
            ImportProjectWriteWorkflowError::PayloadTooLarge
        ));
    }

    #[tokio::test]
    async fn import_project_write_workflow_returns_python_validation_failure_before_db_access() {
        let db = DatabaseConnection::Disconnected;
        let payload = serde_json::to_vec(&json!({
            "version": "rust-strangler-1",
            "project": {
                "title": ""
            }
        }))
        .expect("payload should serialize");

        let result = import_project_write_workflow(&db, "user-1", &payload)
            .await
            .expect("validation failure should return an ImportResult payload");

        assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            result.get("message").and_then(|v| v.as_str()),
            Some("数据验证失败: 项目标题不能为空")
        );
        assert_eq!(result.get("statistics"), Some(&json!({})));
        assert_eq!(
            result.get("warnings"),
            Some(&json!(["项目没有章节数据", "项目没有角色数据"]))
        );
    }

    #[tokio::test]
    async fn import_project_write_workflow_returns_missing_project_as_import_result_like_python() {
        let db = DatabaseConnection::Disconnected;
        let payload = serde_json::to_vec(&json!({
            "version": "rust-strangler-1"
        }))
        .expect("payload should serialize");

        let result = import_project_write_workflow(&db, "user-1", &payload)
            .await
            .expect("missing project should return validation result before DB access");

        assert_eq!(result.get("success").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            result.get("message").and_then(|v| v.as_str()),
            Some("数据验证失败: 缺少项目信息")
        );
        assert_eq!(result.get("statistics"), Some(&json!({})));
    }

    #[test]
    fn read_import_writing_style_records_matches_python_field_defaults() {
        let styles = json!([
            {
                "name": "  轻快风格  ",
                "prompt_content": "  节奏明快  ",
                "description": "  简短描述  ",
                "preset_id": " light ",
                "order_index": 7
            },
            {
                "name": "缺少提示词"
            },
            {
                "prompt_content": "缺少名称"
            }
        ]);

        let records = read_import_writing_style_records(Some(&styles));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "轻快风格");
        assert_eq!(records[0].style_type, "custom");
        assert_eq!(records[0].preset_id.as_deref(), Some("light"));
        assert_eq!(records[0].description.as_deref(), Some("简短描述"));
        assert_eq!(records[0].prompt_content, "节奏明快");
        assert_eq!(records[0].order_index, 7);
    }

    #[test]
    fn read_import_writing_style_records_preserves_python_style_type_and_order_defaults() {
        let styles = json!([
            {
                "name": "古典",
                "style_type": "preset",
                "prompt_content": "古典文风"
            }
        ]);

        let records = read_import_writing_style_records(Some(&styles));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].style_type, "preset");
        assert_eq!(records[0].order_index, 0);
    }

    #[test]
    fn read_import_career_records_matches_python_defaults() {
        let careers = json!([
            {
                "name": "  剑修  ",
                "description": "  主战职业  ",
                "stages": " [\"入门\",\"大成\"] ",
                "max_stage": 12,
                "worldview_rules": "  以剑证道  "
            },
            {
                "type": "sub"
            }
        ]);

        let records = read_import_career_records(Some(&careers));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "剑修");
        assert_eq!(records[0].career_type, "main");
        assert_eq!(records[0].description.as_deref(), Some("主战职业"));
        assert_eq!(records[0].stages, "[\"入门\",\"大成\"]");
        assert_eq!(records[0].max_stage, 12);
        assert_eq!(records[0].worldview_rules.as_deref(), Some("以剑证道"));
        assert_eq!(records[0].source, "ai");
    }

    #[test]
    fn read_import_career_records_preserves_explicit_type_and_stage_defaults() {
        let careers = json!([
            {
                "name": "炼丹师",
                "type": "sub"
            }
        ]);

        let records = read_import_career_records(Some(&careers));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].career_type, "sub");
        assert_eq!(records[0].stages, "[]");
        assert_eq!(records[0].max_stage, 10);
    }

    #[test]
    fn read_import_outline_records_matches_python_shape() {
        let outlines = json!([
            {
                "title": "  第一卷总纲  ",
                "content": "  内容摘要  ",
                "structure": "  三幕式  ",
                "order_index": 3
            },
            {
                "content": "缺少标题"
            }
        ]);

        let records = read_import_outline_records(Some(&outlines));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].title, "第一卷总纲");
        assert_eq!(records[0].content.as_deref(), Some("内容摘要"));
        assert_eq!(records[0].structure.as_deref(), Some("三幕式"));
        assert_eq!(records[0].order_index, Some(3));
    }

    #[test]
    fn read_import_character_records_matches_python_shape() {
        let characters = json!([
            {
                "name": "  林青  ",
                "age": "  19  ",
                "gender": "  女  ",
                "is_organization": true,
                "role_type": "  supporting  ",
                "personality": "  冷静克制  ",
                "background": "  山门弃徒  ",
                "appearance": "  青衣长剑  ",
                "traits": ["敏锐", "谨慎"],
                "organization_type": "  门派  ",
                "organization_purpose": "  复兴山门  "
            },
            {
                "traits": "缺少名称"
            }
        ]);

        let records = read_import_character_records(Some(&characters));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name, "林青");
        assert_eq!(records[0].age.as_deref(), Some("19"));
        assert_eq!(records[0].gender.as_deref(), Some("女"));
        assert!(records[0].is_organization);
        assert_eq!(records[0].role_type.as_deref(), Some("supporting"));
        assert_eq!(records[0].personality.as_deref(), Some("冷静克制"));
        assert_eq!(records[0].background.as_deref(), Some("山门弃徒"));
        assert_eq!(records[0].appearance.as_deref(), Some("青衣长剑"));
        assert_eq!(records[0].traits.as_deref(), Some("[\"敏锐\",\"谨慎\"]"));
        assert_eq!(records[0].organization_type.as_deref(), Some("门派"));
        assert_eq!(records[0].organization_purpose.as_deref(), Some("复兴山门"));
    }

    #[test]
    fn read_import_character_traits_supports_array_and_string_inputs() {
        let array_traits = json!({
            "traits": ["冷静", "沉着"]
        });
        let string_traits = json!({
            "traits": "  冷静沉着  "
        });

        let array_result =
            read_import_character_traits(&array_traits).expect("array traits should serialize");
        let string_result =
            read_import_character_traits(&string_traits).expect("string traits should trim");

        assert_eq!(array_result, "[\"冷静\",\"沉着\"]");
        assert_eq!(string_result, "冷静沉着");
    }

    #[test]
    fn read_import_project_default_style_name_matches_python_shape() {
        let style = json!({
            "style_name": "  默认文风  "
        });

        let style_name = read_import_project_default_style_name(Some(&style));

        assert_eq!(style_name.as_deref(), Some("默认文风"));
    }

    #[test]
    fn read_import_character_career_records_matches_python_defaults() {
        let character_careers = json!([
            {
                "character_name": "  林青  ",
                "career_name": "  剑修  ",
                "notes": "  主修本命剑  "
            },
            {
                "character_name": "缺少职业名"
            }
        ]);

        let records = read_import_character_career_records(Some(&character_careers));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].character_name, "林青");
        assert_eq!(records[0].career_name, "剑修");
        assert_eq!(records[0].career_type, "main");
        assert_eq!(records[0].current_stage, 1);
        assert_eq!(records[0].stage_progress, 0);
        assert_eq!(records[0].notes.as_deref(), Some("主修本命剑"));
    }

    #[test]
    fn read_import_relationship_records_matches_python_defaults() {
        let relationships = json!([
            {
                "source_name": "  林青  ",
                "target_name": "  顾远  ",
                "relationship_name": "  师徒  ",
                "description": "  亦师亦友  "
            },
            {
                "source_name": "缺少目标"
            }
        ]);

        let records = read_import_relationship_records(Some(&relationships));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source_name, "林青");
        assert_eq!(records[0].target_name, "顾远");
        assert_eq!(records[0].relationship_name.as_deref(), Some("师徒"));
        assert_eq!(records[0].intimacy_level, 50);
        assert_eq!(records[0].status, "active");
        assert_eq!(records[0].description.as_deref(), Some("亦师亦友"));
    }

    #[test]
    fn read_import_organization_records_matches_python_defaults() {
        let organizations = json!([
            {
                "character_name": "  青岚宗  ",
                "parent_org_name": "  太虚盟  ",
                "location": "  北境  "
            },
            {
                "power_level": 80
            }
        ]);

        let records = read_import_organization_records(Some(&organizations));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].character_name, "青岚宗");
        assert_eq!(records[0].parent_org_name.as_deref(), Some("太虚盟"));
        assert_eq!(records[0].power_level, 50);
        assert_eq!(records[0].member_count, 0);
        assert_eq!(records[0].location.as_deref(), Some("北境"));
    }

    #[test]
    fn read_import_organization_member_records_matches_python_defaults() {
        let members = json!([
            {
                "organization_name": "  青岚宗  ",
                "character_name": "  林青  ",
                "position": "  长老  ",
                "notes": "  负责外务  "
            },
            {
                "organization_name": "缺少角色名",
                "position": "执事"
            }
        ]);

        let records = read_import_organization_member_records(Some(&members));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].organization_name, "青岚宗");
        assert_eq!(records[0].character_name, "林青");
        assert_eq!(records[0].position, "长老");
        assert_eq!(records[0].rank, 0);
        assert_eq!(records[0].status, "active");
        assert_eq!(records[0].loyalty, 50);
        assert_eq!(records[0].contribution, 0);
        assert_eq!(records[0].notes.as_deref(), Some("负责外务"));
    }

    #[test]
    fn read_import_story_memory_records_matches_python_defaults() {
        let memories = json!([
            {
                "chapter_title": "  第一章  ",
                "memory_type": "  foreshadow  ",
                "title": "  初遇伏笔  ",
                "content": "  雨夜初见  ",
                "full_context": "  更长上下文  ",
                "related_characters": ["  林青  ", "顾远", "", "   "],
                "related_locations": ["山门", "后山"],
                "tags": ["初遇", "伏笔"],
                "is_foreshadow": true
            },
            {
                "memory_type": "缺少内容"
            }
        ]);

        let records = read_import_story_memory_records(Some(&memories));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].chapter_title.as_deref(), Some("第一章"));
        assert_eq!(records[0].memory_type, "foreshadow");
        assert_eq!(records[0].title.as_deref(), Some("初遇伏笔"));
        assert_eq!(records[0].content, "雨夜初见");
        assert_eq!(records[0].full_context.as_deref(), Some("更长上下文"));
        assert_eq!(records[0].related_characters, vec!["林青", "顾远"]);
        assert_eq!(records[0].related_locations, Some(json!(["山门", "后山"])));
        assert_eq!(records[0].tags, Some(json!(["初遇", "伏笔"])));
        assert_eq!(records[0].importance_score, 0.5);
        assert_eq!(records[0].story_timeline, 0);
        assert_eq!(records[0].chapter_position, 0);
        assert_eq!(records[0].text_length, 0);
        assert_eq!(records[0].is_foreshadow, 1);
        assert_eq!(records[0].foreshadow_strength, None);
    }

    #[test]
    fn read_import_generation_history_records_matches_export_shape() {
        let histories = json!([
            {
                "chapter_title": "  第一章  ",
                "prompt": "  修订提示  ",
                "generated_content": " {\"log_type\":\"chapter_text_reviser_v1\"} ",
                "model": "  chapter_text_reviser_v1  ",
                "tokens_used": 42,
                "generation_time": 1.25,
                "created_at": "2026-05-17T12:30:45"
            },
            {
                "chapter_title": "缺少有效内容"
            }
        ]);

        let records = read_import_generation_history_records(Some(&histories));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].chapter_title.as_deref(), Some("第一章"));
        assert_eq!(records[0].prompt.as_deref(), Some("修订提示"));
        assert_eq!(
            records[0].generated_content.as_deref(),
            Some("{\"log_type\":\"chapter_text_reviser_v1\"}")
        );
        assert_eq!(records[0].model.as_deref(), Some("chapter_text_reviser_v1"));
        assert_eq!(records[0].tokens_used, Some(42));
        assert_eq!(records[0].generation_time, Some(1.25));
        assert_eq!(
            records[0]
                .created_at
                .map(|value| value.to_string())
                .as_deref(),
            Some("2026-05-17 12:30:45")
        );
    }

    #[test]
    fn read_import_plot_analysis_records_matches_python_defaults() {
        let analyses = json!([
            {
                "chapter_title": "  第一章  ",
                "plot_stage": "  opening  ",
                "conflict_types": ["external"],
                "emotional_curve": {"start": 0.2, "end": 0.6},
                "hooks": [{"text": "悬念"}],
                "foreshadows": [{"text": "暗线"}],
                "plot_points": [{"text": "转折"}],
                "character_states": [{"name": "林青"}],
                "scenes": [{"name": "雨夜"}],
                "suggestions": ["加强冲突"]
            },
            {
                "plot_stage": "缺少章节标题"
            }
        ]);

        let records = read_import_plot_analysis_records(Some(&analyses));

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].chapter_title, "第一章");
        assert_eq!(records[0].plot_stage.as_deref(), Some("opening"));
        assert_eq!(records[0].conflict_level, None);
        assert_eq!(records[0].conflict_types, Some(json!(["external"])));
        assert_eq!(
            records[0].emotional_curve,
            Some(json!({"start": 0.2, "end": 0.6}))
        );
        assert_eq!(records[0].hooks, Some(json!([{"text": "悬念"}])));
        assert_eq!(records[0].hooks_count, 0);
        assert_eq!(records[0].foreshadows, Some(json!([{"text": "暗线"}])));
        assert_eq!(records[0].foreshadows_planted, 0);
        assert_eq!(records[0].foreshadows_resolved, 0);
        assert_eq!(records[0].plot_points, Some(json!([{"text": "转折"}])));
        assert_eq!(records[0].plot_points_count, 0);
        assert_eq!(records[0].character_states, Some(json!([{"name": "林青"}])));
        assert_eq!(records[0].scenes, Some(json!([{"name": "雨夜"}])));
        assert_eq!(records[0].suggestions, Some(json!(["加强冲突"])));
    }

    #[test]
    fn read_import_chapter_expansion_plan_supports_object_and_string_inputs() {
        let object_plan = json!({
            "expansion_plan": {
                "beats": ["a", "b"]
            }
        });
        let string_plan = json!({
            "expansion_plan": "  raw-plan  "
        });

        let object_result =
            read_import_chapter_expansion_plan(&object_plan).expect("object plan should serialize");
        let string_result =
            read_import_chapter_expansion_plan(&string_plan).expect("string plan should trim");

        assert_eq!(object_result, "{\"beats\":[\"a\",\"b\"]}");
        assert_eq!(string_result, "raw-plan");
    }
}
