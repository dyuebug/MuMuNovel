use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{chapter, project};

#[derive(Debug)]
pub(crate) enum ValidateProjectImportPayloadError {
    InvalidJson(String),
}

#[derive(Debug)]
pub(crate) enum ImportProjectWriteWorkflowError {
    PayloadTooLarge,
    InvalidJson(String),
    MissingProjectField,
    Internal(String),
}

pub(crate) fn validate_project_import_payload(
    file_data: &[u8],
) -> Result<Value, ValidateProjectImportPayloadError> {
    let data: Value = serde_json::from_slice(file_data)
        .map_err(|error| ValidateProjectImportPayloadError::InvalidJson(error.to_string()))?;

    let version = data.get("version").and_then(|v| v.as_str());
    let project = data.get("project");
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    if version.is_none() {
        errors.push("Missing version field".to_string());
    }
    if project.is_none() {
        errors.push("Missing project field".to_string());
    }

    if let Some(ver) = version {
        if !["1.0.0", "1.1.0", "rust-strangler-1"].contains(&ver) {
            warnings.push(format!("Unknown export version: {}", ver));
        }
    }

    let stats = if let Some(proj) = project {
        json!({
            "chapters": proj.get("chapters").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "characters": proj.get("characters").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "outlines": proj.get("outlines").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "relationships": proj.get("relationships").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "organizations": proj.get("organizations").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "writing_styles": proj.get("writing_styles").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "generation_history": proj.get("generation_history").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "careers": proj.get("careers").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "memories": proj.get("memories").and_then(|c| c.as_array()).map_or(0, |a| a.len()),
            "plot_analysis": proj.get("plot_analysis").and_then(|c| c.as_array()).map_or(0, |a| a.len())
        })
    } else {
        json!({})
    };

    Ok(json!({
        "valid": errors.is_empty(),
        "version": version,
        "project_name": project.and_then(|p| p.get("title")).and_then(|t| t.as_str()),
        "statistics": stats,
        "errors": errors,
        "warnings": warnings,
    }))
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
    let project_data = data
        .get("project")
        .ok_or(ImportProjectWriteWorkflowError::MissingProjectField)?;

    let now = Utc::now().naive_utc();
    let project_id = Uuid::new_v4().to_string();
    let title = json_string(project_data, "title").unwrap_or_else(|| "导入项目".to_string());
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
            status: Set(json_string(project_data, "status").unwrap_or_else(|| "draft".to_string())),
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

    let mut current_words = 0i32;
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

        chapter::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(imported_project.id.clone()),
            chapter_number: Set(json_i32(chapter_data, "chapter_number", index as i32 + 1)),
            title: Set(
                json_string(chapter_data, "title").unwrap_or_else(|| format!("第{}章", index + 1))
            ),
            content: Set(content),
            summary: Set(json_string(chapter_data, "summary")),
            word_count: Set(word_count),
            status: Set(json_string(chapter_data, "status").unwrap_or_else(|| "draft".to_string())),
            outline_id: Set(None),
            sub_index: Set(json_i32(chapter_data, "sub_index", 0)),
            expansion_plan: Set(json_string(chapter_data, "expansion_plan")),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;
    }

    let mut active_project: project::ActiveModel = imported_project.into();
    active_project.current_words = Set(current_words);
    active_project
        .update(db)
        .await
        .map_err(|error| ImportProjectWriteWorkflowError::Internal(error.to_string()))?;

    Ok(json!({
        "success": true,
        "project_id": project_id,
        "message": "项目导入成功",
        "statistics": {
            "chapters": chapters.len(),
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

fn json_i32(value: &Value, key: &str, default: i32) -> i32 {
    value
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::{
        import_project_write_workflow, validate_project_import_payload,
        ImportProjectWriteWorkflowError, ValidateProjectImportPayloadError,
    };
    use sea_orm::DatabaseConnection;
    use serde_json::json;

    #[test]
    fn validate_project_import_payload_keeps_existing_contract_shape() {
        let payload = json!({
            "version": "rust-strangler-1",
            "project": {
                "title": "导入测试",
                "chapters": [{ "title": "第一章" }],
                "characters": [{ "name": "甲" }],
                "plot_analysis": []
            }
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
    async fn import_project_write_workflow_requires_project_field_before_db_access() {
        let db = DatabaseConnection::Disconnected;
        let payload = serde_json::to_vec(&json!({
            "version": "rust-strangler-1"
        }))
        .expect("payload should serialize");

        let error = import_project_write_workflow(&db, "user-1", &payload)
            .await
            .expect_err("missing project field should fail");

        assert!(matches!(
            error,
            ImportProjectWriteWorkflowError::MissingProjectField
        ));
    }
}
