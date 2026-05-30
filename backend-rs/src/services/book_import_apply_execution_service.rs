use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::models::project;
use crate::services::chapter_service::ChapterService;
use crate::services::outline_service::OutlineService;
use crate::services::project_service::{CreateProjectParams, ProjectService};

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
) -> Result<project::Model, String> {
    let create_params = build_book_import_create_project_params(user_id, import_mode, suggestion);
    ProjectService::create_full(db, create_params)
        .await
        .map_err(|error| format!("项目创建失败: {}", error))
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
        )
        .await;
    }

    BookImportChapterImportSummary {
        chapter_count: chapters.len(),
        total_words,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_book_import_create_project_params, read_book_import_project_suggestion,
        BookImportProjectSuggestion,
    };

    #[test]
    fn should_read_book_import_project_suggestion_with_defaults() {
        let suggestion = read_book_import_project_suggestion(&json!({}));

        assert_eq!(suggestion.title, "拆书导入项目");
        assert_eq!(suggestion.description, "");
        assert_eq!(suggestion.theme, None);
        assert_eq!(suggestion.genre, None);
        assert_eq!(suggestion.narrative_perspective, None);
        assert_eq!(suggestion.target_words, 100000);
    }

    #[test]
    fn should_build_book_import_create_project_params_from_suggestion_owner() {
        let suggestion = BookImportProjectSuggestion {
            title: "项目标题".to_string(),
            description: "项目简介".to_string(),
            theme: Some("成长".to_string()),
            genre: Some("玄幻".to_string()),
            narrative_perspective: Some("第三人称".to_string()),
            target_words: 120000,
        };

        let params = build_book_import_create_project_params("user-1", "append", &suggestion);

        assert_eq!(params.user_id, "user-1");
        assert_eq!(params.title, "项目标题");
        assert_eq!(params.description.as_deref(), Some("项目简介"));
        assert_eq!(params.theme.as_deref(), Some("成长"));
        assert_eq!(params.genre.as_deref(), Some("玄幻"));
        assert_eq!(params.narrative_perspective.as_deref(), Some("第三人称"));
        assert_eq!(params.target_words, 120000);
        assert_eq!(params.outline_mode, "append");
    }
}
