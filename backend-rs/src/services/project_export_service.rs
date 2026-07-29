use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    models::{chapter, project},
    services::project_service::ProjectService,
};

pub(crate) const PROJECT_EXPORT_DESCRIPTOR_SCHEMA_VERSION: &str = "project-export-artifact/v1";
pub(crate) const PROJECT_EXPORT_FORMAT_TXT: &str = "txt";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProjectExportArtifactDescriptorV1 {
    pub schema_version: String,
    pub project_id: String,
    pub format: String,
    pub filename: String,
    pub content_type: String,
    pub content_digest: String,
    pub chapter_count: u32,
    pub total_word_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectExportArtifact {
    pub descriptor: ProjectExportArtifactDescriptorV1,
    pub content: String,
}

impl ProjectExportArtifact {
    pub(crate) fn descriptor_json(&self) -> Result<String, ProjectExportServiceError> {
        serde_json::to_string(&self.descriptor)
            .map_err(|error| ProjectExportServiceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProjectExportServiceError {
    NotFoundOrAccessDenied,
    ProjectHasNoChapters,
    UnsupportedFormat(String),
    Database(String),
    Serialization(String),
}

impl ProjectExportServiceError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::NotFoundOrAccessDenied => "not_found_or_access_denied",
            Self::ProjectHasNoChapters => "project_has_no_chapters",
            Self::UnsupportedFormat(_) => "unsupported_export_format",
            Self::Database(_) => "database_error",
            Self::Serialization(_) => "serialization_error",
        }
    }
}

pub(crate) async fn project_export_descriptor_matches_current_artifact(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    format: &str,
    descriptor: &ProjectExportArtifactDescriptorV1,
) -> Result<bool, ProjectExportServiceError> {
    let normalized_format = format.trim().to_ascii_lowercase();
    if descriptor.schema_version != PROJECT_EXPORT_DESCRIPTOR_SCHEMA_VERSION
        || descriptor.project_id != project_id
        || descriptor.format != normalized_format
    {
        return Ok(false);
    }

    let artifact =
        build_project_export_artifact(db, project_id, user_id, &normalized_format).await?;
    Ok(artifact.descriptor == *descriptor)
}

pub(crate) async fn build_project_export_artifact(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    format: &str,
) -> Result<ProjectExportArtifact, ProjectExportServiceError> {
    let format = format.trim().to_ascii_lowercase();
    if format != PROJECT_EXPORT_FORMAT_TXT {
        return Err(ProjectExportServiceError::UnsupportedFormat(format));
    }

    let project = ProjectService::get(db, project_id, user_id)
        .await
        .map_err(ProjectExportServiceError::Database)?
        .ok_or(ProjectExportServiceError::NotFoundOrAccessDenied)?;
    let chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(db)
        .await
        .map_err(|error| ProjectExportServiceError::Database(error.to_string()))?;
    if chapters.is_empty() {
        return Err(ProjectExportServiceError::ProjectHasNoChapters);
    }

    Ok(build_project_export_artifact_from_models(
        &project, &chapters, &format,
    ))
}

fn build_project_export_artifact_from_models(
    project: &project::Model,
    chapters: &[chapter::Model],
    format: &str,
) -> ProjectExportArtifact {
    let content = build_project_export_txt_content(project, chapters);
    let content_digest = project_export_content_digest(&content);
    let total_word_count = chapters
        .iter()
        .map(|chapter| i64::from(chapter.word_count.max(0)))
        .sum();
    ProjectExportArtifact {
        descriptor: ProjectExportArtifactDescriptorV1 {
            schema_version: PROJECT_EXPORT_DESCRIPTOR_SCHEMA_VERSION.to_string(),
            project_id: project.id.clone(),
            format: format.to_string(),
            filename: build_safe_project_export_txt_filename(&project.title),
            content_type: "text/plain; charset=utf-8".to_string(),
            content_digest,
            chapter_count: u32::try_from(chapters.len()).unwrap_or(u32::MAX),
            total_word_count,
        },
        content,
    }
}

pub(crate) fn project_export_content_digest(content: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(content.as_bytes())))
}

pub(crate) fn build_project_export_txt_content(
    project: &project::Model,
    chapters: &[chapter::Model],
) -> String {
    let mut text = String::new();
    text.push_str(&format!("项目：{}\n", project.title));
    if let Some(ref desc) = project.description {
        if !desc.is_empty() {
            text.push_str(&format!("简介：{}\n", desc));
        }
    }
    if let Some(ref theme) = project.theme {
        if !theme.is_empty() {
            text.push_str(&format!("主题：{}\n", theme));
        }
    }
    if let Some(ref genre) = project.genre {
        if !genre.is_empty() {
            text.push_str(&format!("类型：{}\n", genre));
        }
    }
    text.push_str("\n\n");

    for chapter in chapters {
        text.push_str(&format!(
            "第 {} 章：{}\n\n",
            chapter.chapter_number, chapter.title
        ));
        if let Some(ref content) = chapter.content {
            text.push_str(content);
        }
        text.push_str("\n\n---\n\n");
    }

    text
}

pub(crate) fn build_safe_project_export_txt_filename(title: &str) -> String {
    let safe_title: String = title
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    format!("{}.txt", safe_title)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn project_model() -> project::Model {
        let now = Utc::now().naive_utc();
        project::Model {
            id: "project-export".to_string(),
            user_id: "user-export".to_string(),
            title: "测试 项目/Title".to_string(),
            description: Some("项目简介".to_string()),
            theme: Some("主题测试".to_string()),
            genre: Some("奇幻".to_string()),
            target_words: 10_000,
            current_words: 4,
            status: "writing".to_string(),
            wizard_status: "completed".to_string(),
            wizard_step: 6,
            outline_mode: "standard".to_string(),
            world_time_period: None,
            world_location: None,
            world_atmosphere: None,
            world_rules: None,
            chapter_count: Some(1),
            narrative_perspective: None,
            character_count: 0,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: now,
            updated_at: Some(now),
        }
    }

    fn chapter_model() -> chapter::Model {
        let now = Utc::now().naive_utc();
        chapter::Model {
            id: "chapter-export".to_string(),
            project_id: "project-export".to_string(),
            chapter_number: 1,
            title: "第一章".to_string(),
            content: Some("这里是正文".to_string()),
            summary: None,
            word_count: 4,
            status: "completed".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: now,
            updated_at: Some(now),
        }
    }

    #[test]
    fn txt_artifact_keeps_existing_download_contract_without_embedding_content_in_descriptor() {
        let artifact = build_project_export_artifact_from_models(
            &project_model(),
            &[chapter_model()],
            PROJECT_EXPORT_FORMAT_TXT,
        );

        assert!(artifact.content.contains("项目：测试 项目/Title"));
        assert!(artifact.content.contains("第 1 章：第一章"));
        assert!(artifact.content.contains("这里是正文"));
        assert_eq!(artifact.descriptor.filename, "______Title.txt");
        assert_eq!(
            artifact.descriptor.content_type,
            "text/plain; charset=utf-8"
        );
        assert_eq!(artifact.descriptor.chapter_count, 1);
        assert_eq!(artifact.descriptor.total_word_count, 4);
        assert_eq!(
            artifact.descriptor.content_digest,
            project_export_content_digest(&artifact.content)
        );
        let descriptor = artifact.descriptor_json().expect("serialize descriptor");
        assert!(!descriptor.contains("这里是正文"));
    }

    #[test]
    fn digest_changes_when_exported_content_changes() {
        let first = build_project_export_artifact_from_models(
            &project_model(),
            &[chapter_model()],
            PROJECT_EXPORT_FORMAT_TXT,
        );
        let mut changed_chapter = chapter_model();
        changed_chapter.content = Some("正文发生变化".to_string());
        let changed = build_project_export_artifact_from_models(
            &project_model(),
            &[changed_chapter],
            PROJECT_EXPORT_FORMAT_TXT,
        );

        assert_ne!(
            first.descriptor.content_digest,
            changed.descriptor.content_digest
        );
    }
}
