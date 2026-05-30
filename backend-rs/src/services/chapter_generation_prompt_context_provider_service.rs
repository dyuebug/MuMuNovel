use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptContextProviderPayload {
    pub characters_info: String,
    pub chapter_careers: String,
    pub recent_chapters_context: String,
    pub previous_chapter_summary: String,
    pub foreshadow_reminders: String,
    pub relevant_memories: String,
    pub research_query: String,
    pub research_assets: String,
    pub external_assets: String,
    pub reference_assets: String,
    pub mcp_references: String,
}

impl PromptContextProviderPayload {
    pub fn into_prompt_params(self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("characters_info".to_string(), self.characters_info);
        params.insert("chapter_careers".to_string(), self.chapter_careers);
        params.insert(
            "recent_chapters_context".to_string(),
            self.recent_chapters_context,
        );
        params.insert(
            "previous_chapter_summary".to_string(),
            self.previous_chapter_summary,
        );
        params.insert(
            "foreshadow_reminders".to_string(),
            self.foreshadow_reminders,
        );
        params.insert("relevant_memories".to_string(), self.relevant_memories);
        params.insert("research_query".to_string(), self.research_query);
        params.insert("research_assets".to_string(), self.research_assets);
        params.insert("external_assets".to_string(), self.external_assets);
        params.insert("reference_assets".to_string(), self.reference_assets);
        params.insert("mcp_references".to_string(), self.mcp_references);
        params
    }
}

pub fn build_placeholder_prompt_context_provider_payload() -> PromptContextProviderPayload {
    PromptContextProviderPayload {
        characters_info: "[]".to_string(),
        chapter_careers: "[]".to_string(),
        recent_chapters_context: String::new(),
        previous_chapter_summary: String::new(),
        foreshadow_reminders: "[]".to_string(),
        relevant_memories: "[]".to_string(),
        research_query: String::new(),
        research_assets: "[]".to_string(),
        external_assets: "[]".to_string(),
        reference_assets: "[]".to_string(),
        mcp_references: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::build_placeholder_prompt_context_provider_payload;

    #[test]
    fn should_build_placeholder_prompt_context_provider_payload() {
        let payload = build_placeholder_prompt_context_provider_payload();

        assert_eq!(payload.characters_info, "[]");
        assert_eq!(payload.chapter_careers, "[]");
        assert_eq!(payload.recent_chapters_context, "");
        assert_eq!(payload.previous_chapter_summary, "");
        assert_eq!(payload.foreshadow_reminders, "[]");
        assert_eq!(payload.relevant_memories, "[]");
        assert_eq!(payload.research_query, "");
        assert_eq!(payload.research_assets, "[]");
        assert_eq!(payload.external_assets, "[]");
        assert_eq!(payload.reference_assets, "[]");
        assert_eq!(payload.mcp_references, "");
    }

    #[test]
    fn should_convert_provider_payload_into_prompt_params() {
        let params = build_placeholder_prompt_context_provider_payload().into_prompt_params();

        assert_eq!(params["characters_info"], "[]");
        assert_eq!(params["chapter_careers"], "[]");
        assert_eq!(params["recent_chapters_context"], "");
        assert_eq!(params["previous_chapter_summary"], "");
        assert_eq!(params["foreshadow_reminders"], "[]");
        assert_eq!(params["relevant_memories"], "[]");
        assert_eq!(params["research_query"], "");
        assert_eq!(params["research_assets"], "[]");
        assert_eq!(params["external_assets"], "[]");
        assert_eq!(params["reference_assets"], "[]");
        assert_eq!(params["mcp_references"], "");
    }
}
