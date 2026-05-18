use std::collections::HashMap;

use crate::models::{chapter, project};
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
use crate::services::chapter_generation_prompt_context_service::{
    build_prompt_context_params_with_provider_payload,
};

pub fn build_prompt_params_with_provider_payload(
    chapter_model: &chapter::Model,
    project_model: &project::Model,
    previous_chapter: Option<&chapter::Model>,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
) -> HashMap<String, String> {
    let mut params = HashMap::new();
    params.insert("project_title".to_string(), project_model.title.clone());
    params.insert(
        "genre".to_string(),
        project_model.genre.clone().unwrap_or_default(),
    );
    params.insert(
        "chapter_number".to_string(),
        chapter_model.chapter_number.to_string(),
    );
    params.insert("chapter_title".to_string(), chapter_model.title.clone());
    params.insert(
        "target_word_count".to_string(),
        target_word_count.to_string(),
    );
    params.insert(
        "narrative_perspective".to_string(),
        project_model
            .narrative_perspective
            .clone()
            .unwrap_or_else(|| "第三人称".to_string()),
    );
    params.insert(
        "chapter_outline".to_string(),
        chapter_model
            .expansion_plan
            .clone()
            .unwrap_or_else(|| "暂无大纲".to_string()),
    );
    params.insert(
        "world_time_period".to_string(),
        project_model.world_time_period.clone().unwrap_or_default(),
    );
    params.insert(
        "world_location".to_string(),
        project_model.world_location.clone().unwrap_or_default(),
    );
    params.insert(
        "world_atmosphere".to_string(),
        project_model.world_atmosphere.clone().unwrap_or_default(),
    );
    params.insert(
        "world_rules".to_string(),
        project_model.world_rules.clone().unwrap_or_default(),
    );
    params.extend(build_prompt_context_params_with_provider_payload(
        previous_chapter,
        provider_payload,
    ));
    params
}
