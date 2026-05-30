use serde_json::{json, Value};

use crate::models::{chapter, project};

pub fn build_project_export_data_payload(
    project: &project::Model,
    chapters: &[chapter::Model],
    include_generation_history: bool,
    include_writing_styles: bool,
    include_careers: bool,
    include_memories: bool,
    include_plot_analysis: bool,
) -> Value {
    json!({
        "version": "rust-strangler-1",
        "export_type": "project",
        "project": project,
        "chapters": chapters,
        "statistics": {
            "chapter_count": chapters.len()
        },
        "options": {
            "include_generation_history": include_generation_history,
            "include_writing_styles": include_writing_styles,
            "include_careers": include_careers,
            "include_memories": include_memories,
            "include_plot_analysis": include_plot_analysis
        }
    })
}

pub fn build_project_export_txt_content(
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

    for ch in chapters {
        text.push_str(&format!("第 {} 章：{}\n\n", ch.chapter_number, ch.title));
        if let Some(ref content) = ch.content {
            text.push_str(content);
        }
        text.push_str("\n\n---\n\n");
    }

    text
}

pub fn build_safe_project_export_json_filename(title: &str) -> String {
    let safe_title: String = title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == ' ' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    format!("project_{}.json", safe_title.trim().replace(' ', "_"))
}

pub fn build_safe_project_export_txt_filename(title: &str) -> String {
    let safe_title: String = title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    format!("{}.txt", safe_title)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use crate::models::{chapter, project};

    use super::{
        build_project_export_data_payload, build_project_export_txt_content,
        build_safe_project_export_json_filename, build_safe_project_export_txt_filename,
    };

    fn test_datetime() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse")
    }

    fn project_model() -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "测试 项目/Title".to_string(),
            description: Some("项目简介".to_string()),
            theme: Some("主题测试".to_string()),
            genre: Some("奇幻".to_string()),
            target_words: 100000,
            current_words: 1234,
            status: "draft".to_string(),
            wizard_status: "completed".to_string(),
            wizard_step: 0,
            outline_mode: "traditional".to_string(),
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
            created_at: test_datetime(),
            updated_at: Some(test_datetime()),
        }
    }

    fn chapter_model() -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 1,
            title: "第一章".to_string(),
            content: Some("这里是正文".to_string()),
            summary: None,
            word_count: 5,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: test_datetime(),
            updated_at: Some(test_datetime()),
        }
    }

    #[test]
    fn build_project_export_data_payload_keeps_existing_shape() {
        let project = project_model();
        let chapters = vec![chapter_model()];

        let payload =
            build_project_export_data_payload(&project, &chapters, true, false, true, false, true);

        assert_eq!(payload["version"], "rust-strangler-1");
        assert_eq!(payload["export_type"], "project");
        assert_eq!(payload["project"]["title"], "测试 项目/Title");
        assert_eq!(payload["chapters"][0]["title"], "第一章");
        assert_eq!(payload["statistics"]["chapter_count"], 1);
        assert_eq!(payload["options"]["include_generation_history"], true);
        assert_eq!(payload["options"]["include_writing_styles"], false);
        assert_eq!(payload["options"]["include_careers"], true);
        assert_eq!(payload["options"]["include_memories"], false);
        assert_eq!(payload["options"]["include_plot_analysis"], true);
    }

    #[test]
    fn build_project_export_txt_content_keeps_existing_text_format() {
        let project = project_model();
        let chapters = vec![chapter_model()];

        let text = build_project_export_txt_content(&project, &chapters);

        assert!(text.contains("项目：测试 项目/Title"));
        assert!(text.contains("简介：项目简介"));
        assert!(text.contains("主题：主题测试"));
        assert!(text.contains("类型：奇幻"));
        assert!(text.contains("第 1 章：第一章"));
        assert!(text.contains("这里是正文"));
        assert!(text.contains("\n\n---\n\n"));
    }

    #[test]
    fn build_safe_project_export_filenames_keep_existing_normalization() {
        assert_eq!(
            build_safe_project_export_json_filename("测试 项目/Title"),
            "project_______Title.json"
        );
        assert_eq!(
            build_safe_project_export_txt_filename("测试 项目/Title"),
            "______Title.txt"
        );
    }
}
