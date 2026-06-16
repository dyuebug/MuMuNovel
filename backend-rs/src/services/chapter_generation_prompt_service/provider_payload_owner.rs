use std::collections::HashMap;

use serde_json::Value;

pub(crate) const PROMPT_CONTEXT_PROVIDER_FIELD_KEYS: [&str; 11] = [
    "characters_info",
    "chapter_careers",
    "recent_chapters_context",
    "previous_chapter_summary",
    "foreshadow_reminders",
    "relevant_memories",
    "research_query",
    "research_assets",
    "external_assets",
    "reference_assets",
    "mcp_references",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptContextProviderPayload {
    pub(crate) characters_info: String,
    pub(crate) chapter_careers: String,
    pub(crate) recent_chapters_context: String,
    pub(crate) previous_chapter_summary: String,
    pub(crate) foreshadow_reminders: String,
    pub(crate) relevant_memories: String,
    pub(crate) research_query: String,
    pub(crate) research_assets: String,
    pub(crate) external_assets: String,
    pub(crate) reference_assets: String,
    pub(crate) mcp_references: String,
}

impl PromptContextProviderPayload {
    pub(crate) fn into_prompt_params(self) -> HashMap<String, String> {
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

pub(crate) fn build_placeholder_prompt_context_provider_payload() -> PromptContextProviderPayload {
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

pub(crate) fn build_prompt_context_provider_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_generation_prompt_service",
        "scope": "provider_payload_owner",
        "python_source_map": [
            "backend/app/api/chapters.py",
            "backend/app/services/batch_generation_prompt_service.py",
            "backend/app/services/chapter_generation/runtime/service.py"
        ],
        "rust_target_map": [
            "backend-rs/src/services/chapter_generation_prompt_service.rs",
            "backend-rs/src/services/chapter_generation_prompt_service/provider_payload_owner.rs",
            "backend-rs/src/services/chapter_generation_execution_contract_service.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service/research_payload_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/context_compaction_owner.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "provider_payload_fields": PROMPT_CONTEXT_PROVIDER_FIELD_KEYS,
            "prompt_param_keys": PROMPT_CONTEXT_PROVIDER_FIELD_KEYS,
            "placeholder_array_defaults": [
                "characters_info",
                "chapter_careers",
                "foreshadow_reminders",
                "relevant_memories",
                "research_assets",
                "external_assets",
                "reference_assets"
            ],
            "placeholder_empty_text_defaults": [
                "recent_chapters_context",
                "previous_chapter_summary",
                "research_query",
                "mcp_references"
            ],
            "asset_prompt_visibility": [
                "external_assets",
                "reference_assets",
                "mcp_references"
            ],
            "prompt_param_bridge": "PromptContextProviderPayload::into_prompt_params",
            "prompt_render_consumer": "build_prompt_params_with_provider_payload",
            "quality_profile_asset_consumer": "build_quality_profile_payload",
            "mcp_references_preserved": true
        },
        "active_consumers": [
            "chapter_generation_prompt_service",
            "chapter_generation_execution_contract_service::execution_config",
            "chapter_single_generation_prepare_service::research_payload_owner",
            "chapter_generation_runtime_service::context_compaction_owner",
            "chapter_single_generation_prepare_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_single_generation_active_gateway_smoke_service",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "validation_boundary": [
            "cargo test chapter_generation_prompt_service",
            "cargo test api::health",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_legacy_python_prompt_builders_as_source_map_until_explicit_freeze_delete_round",
            "runtime_knob": "ChapterCandidateRouteGatewayConfig",
            "compatibility_note": "Prompt provider payload keys and placeholder defaults remain stable for single, batch, and regeneration prompt consumers"
        }
    })
}
