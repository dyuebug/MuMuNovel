use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptContextProviderPayload {
    pub characters_info: String,
    pub foreshadow_reminders: String,
    pub relevant_memories: String,
}

impl PromptContextProviderPayload {
    pub fn into_prompt_params(self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert("characters_info".to_string(), self.characters_info);
        params.insert(
            "foreshadow_reminders".to_string(),
            self.foreshadow_reminders,
        );
        params.insert("relevant_memories".to_string(), self.relevant_memories);
        params
    }
}

pub fn build_placeholder_prompt_context_provider_payload() -> PromptContextProviderPayload {
    PromptContextProviderPayload {
        characters_info: "[]".to_string(),
        foreshadow_reminders: "[]".to_string(),
        relevant_memories: "[]".to_string(),
    }
}

pub fn resolve_default_prompt_context_provider_payload() -> PromptContextProviderPayload {
    build_placeholder_prompt_context_provider_payload()
}

#[cfg(test)]
mod tests {
    use super::{
        build_placeholder_prompt_context_provider_payload,
        resolve_default_prompt_context_provider_payload,
    };

    #[test]
    fn should_build_placeholder_prompt_context_provider_payload() {
        let payload = build_placeholder_prompt_context_provider_payload();

        assert_eq!(payload.characters_info, "[]");
        assert_eq!(payload.foreshadow_reminders, "[]");
        assert_eq!(payload.relevant_memories, "[]");
    }

    #[test]
    fn should_convert_provider_payload_into_prompt_params() {
        let params = build_placeholder_prompt_context_provider_payload().into_prompt_params();

        assert_eq!(params["characters_info"], "[]");
        assert_eq!(params["foreshadow_reminders"], "[]");
        assert_eq!(params["relevant_memories"], "[]");
    }

    #[test]
    fn should_resolve_default_prompt_context_provider_payload() {
        let payload = resolve_default_prompt_context_provider_payload();

        assert_eq!(payload.characters_info, "[]");
        assert_eq!(payload.foreshadow_reminders, "[]");
        assert_eq!(payload.relevant_memories, "[]");
    }
}
