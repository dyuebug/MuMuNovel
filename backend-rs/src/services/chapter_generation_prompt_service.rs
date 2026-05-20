use crate::models::{chapter, project};
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
use crate::services::chapter_generation_prompt_params_service::build_prompt_params_with_provider_payload;
use crate::services::prompt_template_service::PromptTemplateService;

pub fn chapter_template_key(outline_mode: &str, has_previous: bool) -> &'static str {
    match (outline_mode, has_previous) {
        ("one-to-many", false) => "CHAPTER_GENERATION_ONE_TO_MANY",
        ("one-to-many", true) => "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
        ("one-to-one", false) | (_, false) => "CHAPTER_GENERATION_ONE_TO_ONE",
        _ => "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
    }
}

pub fn build_prompt_with_provider_payload(
    chapter_model: &chapter::Model,
    project_model: &project::Model,
    previous_chapter: Option<&chapter::Model>,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
) -> Result<String, String> {
    let template_key =
        chapter_template_key(&project_model.outline_mode, previous_chapter.is_some());
    let template = PromptTemplateService::system_template_info(template_key)
        .ok_or_else(|| format!("找不到章节模板: {}", template_key))?;
    let params = build_prompt_params_with_provider_payload(
        chapter_model,
        project_model,
        previous_chapter,
        target_word_count,
        provider_payload,
    );

    PromptTemplateService::format_prompt(&template.content, &params)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{build_prompt_with_provider_payload, chapter_template_key};
    use crate::models::{chapter, project};
    use crate::services::chapter_generation_prompt_context_provider_service::{
        build_placeholder_prompt_context_provider_payload, PromptContextProviderPayload,
    };

    fn build_project(outline_mode: &str) -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "项目标题".to_string(),
            genre: Some("奇幻".to_string()),
            description: None,
            theme: None,
            target_words: 120000,
            current_words: 0,
            status: "active".to_string(),
            wizard_status: "completed".to_string(),
            wizard_step: 0,
            outline_mode: outline_mode.to_string(),
            narrative_perspective: None,
            world_time_period: Some("近未来".to_string()),
            world_location: Some("浮空城".to_string()),
            world_atmosphere: Some("压抑".to_string()),
            world_rules: Some("魔力守恒".to_string()),
            chapter_count: Some(3),
            character_count: 0,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    fn build_chapter(
        chapter_number: i32,
        title: &str,
        expansion_plan: Option<&str>,
        content: Option<&str>,
        summary: Option<&str>,
    ) -> chapter::Model {
        chapter::Model {
            id: format!("chapter-{chapter_number}"),
            project_id: "project-1".to_string(),
            title: title.to_string(),
            chapter_number,
            content: content.map(str::to_string),
            summary: summary.map(str::to_string),
            expansion_plan: expansion_plan.map(str::to_string),
            status: "pending".to_string(),
            word_count: 0,
            outline_id: None,
            sub_index: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    #[test]
    fn should_select_template_keys_for_outline_mode_and_previous_chapter_state() {
        assert_eq!(
            chapter_template_key("one-to-many", false),
            "CHAPTER_GENERATION_ONE_TO_MANY"
        );
        assert_eq!(
            chapter_template_key("one-to-many", true),
            "CHAPTER_GENERATION_ONE_TO_MANY_NEXT"
        );
        assert_eq!(
            chapter_template_key("one-to-one", false),
            "CHAPTER_GENERATION_ONE_TO_ONE"
        );
        assert_eq!(
            chapter_template_key("custom-mode", true),
            "CHAPTER_GENERATION_ONE_TO_ONE_NEXT"
        );
    }

    #[test]
    fn should_inject_defaults_when_optional_prompt_fields_are_missing() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(3, "第三章", None, None, None);

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            None,
            3200,
            build_placeholder_prompt_context_provider_payload(),
        )
        .expect("prompt should build");

        assert!(prompt.contains("项目标题"));
        assert!(prompt.contains("第三章"));
        assert!(prompt.contains("3200"));
        assert!(prompt.contains("第三人称"));
        assert!(prompt.contains("暂无大纲"));
    }

    #[test]
    fn should_include_previous_chapter_context_and_continuation_excerpt() {
        let project_model = build_project("one-to-many");
        let chapter_model = build_chapter(4, "第四章", Some("推进主线"), None, None);
        let previous_content = "甲".repeat(320);
        let previous_summary = "上一章总结";
        let previous_chapter = build_chapter(
            3,
            "第三章",
            Some("旧大纲"),
            Some(previous_content.as_str()),
            Some(previous_summary),
        );

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            Some(&previous_chapter),
            3600,
            build_placeholder_prompt_context_provider_payload(),
        )
        .expect("prompt should build");

        assert!(prompt.contains(previous_summary));
        assert!(prompt.contains(&"甲".repeat(300)));
    }

    #[test]
    fn should_build_prompt_with_injected_provider_payload() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(2, "第二章", Some("推进冲突"), None, None);
        let provider_payload = PromptContextProviderPayload {
            characters_info: "[角色甲]".to_string(),
            foreshadow_reminders: "[伏笔甲]".to_string(),
            relevant_memories: "[记忆甲]".to_string(),
        };

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            None,
            2800,
            provider_payload,
        )
        .expect("prompt should build");

        assert!(prompt.contains("[角色甲]"));
        assert!(prompt.contains("[伏笔甲]"));
        assert!(prompt.contains("[记忆甲]"));
    }
}
