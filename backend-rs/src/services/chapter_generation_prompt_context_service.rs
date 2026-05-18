use std::collections::HashMap;

use crate::models::chapter;
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;

fn continuation_point(previous_chapter: Option<&chapter::Model>) -> String {
    previous_chapter
        .and_then(|item| item.content.clone())
        .unwrap_or_default()
        .chars()
        .take(300)
        .collect()
}

pub fn build_prompt_context_params_with_provider_payload(
    previous_chapter: Option<&chapter::Model>,
    provider_payload: PromptContextProviderPayload,
) -> HashMap<String, String> {
    let mut params = provider_payload.into_prompt_params();
    params.insert(
        "previous_chapter_summary".to_string(),
        previous_chapter
            .and_then(|item| item.summary.clone())
            .unwrap_or_default(),
    );
    params.insert(
        "previous_chapter_content".to_string(),
        previous_chapter
            .and_then(|item| item.content.clone())
            .unwrap_or_default(),
    );
    params.insert(
        "continuation_point".to_string(),
        continuation_point(previous_chapter),
    );
    params
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::build_prompt_context_params_with_provider_payload;
    use crate::models::chapter;
    use crate::services::chapter_generation_prompt_context_provider_service::{
        build_placeholder_prompt_context_provider_payload, PromptContextProviderPayload,
    };

    fn build_chapter(content: Option<&str>, summary: Option<&str>) -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            title: "第一章".to_string(),
            chapter_number: 1,
            content: content.map(str::to_string),
            summary: summary.map(str::to_string),
            expansion_plan: None,
            status: "pending".to_string(),
            word_count: 0,
            outline_id: None,
            sub_index: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    #[test]
    fn should_build_empty_prompt_context_params_without_previous_chapter() {
        let params = build_prompt_context_params_with_provider_payload(
            None,
            build_placeholder_prompt_context_provider_payload(),
        );

        assert_eq!(params["characters_info"], "[]");
        assert_eq!(params["foreshadow_reminders"], "[]");
        assert_eq!(params["relevant_memories"], "[]");
        assert_eq!(params["previous_chapter_summary"], "");
        assert_eq!(params["previous_chapter_content"], "");
        assert_eq!(params["continuation_point"], "");
    }

    #[test]
    fn should_truncate_continuation_point_to_three_hundred_chars() {
        let previous_content = "甲".repeat(320);
        let previous_chapter = build_chapter(
            Some(previous_content.as_str()),
            Some("上一章总结"),
        );

        let params = build_prompt_context_params_with_provider_payload(
            Some(&previous_chapter),
            build_placeholder_prompt_context_provider_payload(),
        );

        assert_eq!(params["previous_chapter_summary"], "上一章总结");
        assert_eq!(params["previous_chapter_content"], previous_content);
        assert_eq!(params["continuation_point"], "甲".repeat(300));
    }

    #[test]
    fn should_merge_injected_provider_payload_with_previous_chapter_context() {
        let previous_chapter = build_chapter(Some("正文"), Some("摘要"));
        let provider_payload = PromptContextProviderPayload {
            characters_info: "[角色A]".to_string(),
            foreshadow_reminders: "[伏笔A]".to_string(),
            relevant_memories: "[记忆A]".to_string(),
        };

        let params = build_prompt_context_params_with_provider_payload(
            Some(&previous_chapter),
            provider_payload,
        );

        assert_eq!(params["characters_info"], "[角色A]");
        assert_eq!(params["foreshadow_reminders"], "[伏笔A]");
        assert_eq!(params["relevant_memories"], "[记忆A]");
        assert_eq!(params["previous_chapter_summary"], "摘要");
        assert_eq!(params["previous_chapter_content"], "正文");
        assert_eq!(params["continuation_point"], "正文");
    }
}
