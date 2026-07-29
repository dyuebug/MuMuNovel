use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::models::chapter;
use crate::services::chapter_generation_execution_contract_service::{
    build_batch_request_runtime_state_owner_contract, build_prompt_overrides_from_compat_options,
    SingleChapterGenerationCompatOptions,
};
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::{
    build_chapter_generation_snapshot_owner_contract, load_chapter_generation_snapshot,
};
use crate::services::chapter_generation_runtime_service::{
    generate_and_persist_batch_chapter_content_with_candidate_route_gateway, GeneratedChapterResult,
};
use crate::services::chapter_single_generation_prepare_service::research_payload_owner::build_single_chapter_research_provider_payload;
use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationTarget;
use crate::services::generation_contract_service::GenerationContractSnapshotV1;

use super::{
    restore_batch_generation_runtime_compat_options_from_persisted_runtime_context,
    BatchGenerationPersistedRuntimeContext, BatchGenerationRuntimeSession,
};

pub(crate) fn build_batch_generation_attempt_input_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::attempt_input_prompt_provider_gateway_execution",
        "scope": "runtime_snapshot_restore_prompt_override_provider_payload_and_candidate_gateway_execute",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/attempt_input_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_driver_owner.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service/research_payload_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service.rs"
        ],
        "behavior_contract": {
            "compat_restore_entrypoints": [
                "BatchGenerationAttemptInputPlan::resolve_compat_options",
                "restore_batch_generation_runtime_compat_options_from_persisted_runtime_context"
            ],
            "prepare_entrypoints": [
                "BatchGenerationAttemptInputPlan::prepare",
                "build_prompt_overrides_from_compat_options",
                "build_single_chapter_research_provider_payload"
            ],
            "execute_entrypoints": [
                "BatchGenerationAttemptInputPlan::execute",
                "generate_and_persist_chapter_content_with_candidate_route_gateway"
            ],
            "state_contract": {
                "snapshot_restore_owner": "attempt input restores compat options from persisted runtime snapshot before prompt/provider assembly",
                "provider_payload_owner": "single-chapter research payload assembly is reused without changing batch runtime request semantics",
                "gateway_owner": "candidate route gateway execution still consumes explicit runtime session config and chapter target metadata"
            }
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_runtime_state_service::runtime_driver_owner",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "snapshot_persistence_owner_contract": build_chapter_generation_snapshot_owner_contract(),
        "request_runtime_state_owner_contract": build_batch_request_runtime_state_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "batch_generation_attempt_input_owner_is_rust_only_and_surviving_prompt_provider_gateway_surfaces_are_tracked_by_external_runtime_contracts",
            "runtime_state_keys": [
                "batch_request_runtime_state",
                "active_story_repair_payload",
                "quality_metrics_summary",
                "latest_quality_metrics"
            ],
            "gateway_fields": [
                "user_id",
                "chapter_id",
                "chapter_number",
                "target_word_count",
                "candidate_gateway_config"
            ]
        }
    })
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationAttemptInputPlan {
    pub(crate) provider_payload:
        crate::services::chapter_generation_prompt_service::PromptContextProviderPayload,
    pub(crate) prompt_overrides:
        crate::services::chapter_generation_prompt_service::ChapterGenerationPromptOverrides,
    pub(crate) generation_contract_snapshot: Option<GenerationContractSnapshotV1>,
}

impl BatchGenerationAttemptInputPlan {
    async fn load_persisted_runtime_context(
        db: &DatabaseConnection,
        task_id: &str,
    ) -> BatchGenerationPersistedRuntimeContext {
        load_chapter_generation_snapshot(db, task_id)
            .await
            .ok()
            .map(BatchGenerationPersistedRuntimeContext::from_snapshot)
            .unwrap_or_default()
    }

    pub(crate) async fn resolve_compat_options(
        db: &DatabaseConnection,
        task_id: &str,
        base_compat_options: &SingleChapterGenerationCompatOptions,
    ) -> SingleChapterGenerationCompatOptions {
        let persisted_runtime_context = Self::load_persisted_runtime_context(db, task_id).await;
        restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(
            base_compat_options,
            &persisted_runtime_context,
        )
    }

    pub(crate) async fn prepare(
        db: &DatabaseConnection,
        task_id: &str,
        session: &BatchGenerationRuntimeSession,
        chapter_model: &chapter::Model,
    ) -> Result<Self, String> {
        let persisted_runtime_context = Self::load_persisted_runtime_context(db, task_id).await;
        let resolved_compat_options =
            restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(
                &session.compat_options,
                &persisted_runtime_context,
            );
        let generation_contract_snapshot = persisted_runtime_context
            .generation_contract_snapshot()
            .cloned();
        let prompt_overrides = build_prompt_overrides_from_compat_options(&resolved_compat_options);
        let provider_payload = build_single_chapter_research_provider_payload(
            db,
            &session.user_id,
            &SingleChapterGenerationTarget {
                project_id: chapter_model.project_id.clone(),
                chapter_id: chapter_model.id.clone(),
                chapter_number: chapter_model.chapter_number,
                title: chapter_model.title.clone(),
            },
            &resolved_compat_options,
        )
        .await?;

        Ok(Self {
            provider_payload,
            prompt_overrides,
            generation_contract_snapshot,
        })
    }

    pub(crate) async fn execute(
        db: &DatabaseConnection,
        task_id: &str,
        session: &BatchGenerationRuntimeSession,
        chapter_model: &chapter::Model,
    ) -> Result<GeneratedChapterResult, String> {
        let attempt_input = Self::prepare(db, task_id, session, chapter_model).await?;
        let Self {
            provider_payload,
            prompt_overrides,
            generation_contract_snapshot,
        } = attempt_input;

        generate_and_persist_batch_chapter_content_with_candidate_route_gateway(
            db,
            &session.user_id,
            &chapter_model.id,
            session.target_word_count,
            provider_payload,
            &prompt_overrides,
            session.ai_config.clone(),
            session.candidate_gateway_config.clone(),
            generation_contract_snapshot.as_ref(),
            session.role_policy_context.clone(),
        )
        .await
    }

    #[cfg(test)]
    pub(crate) fn from_sources(
        provider_payload: crate::services::chapter_generation_prompt_service::PromptContextProviderPayload,
        prompt_overrides: crate::services::chapter_generation_prompt_service::ChapterGenerationPromptOverrides,
    ) -> Self {
        Self {
            provider_payload,
            prompt_overrides,
            generation_contract_snapshot: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_sources_with_generation_contract(
        provider_payload: crate::services::chapter_generation_prompt_service::PromptContextProviderPayload,
        prompt_overrides: crate::services::chapter_generation_prompt_service::ChapterGenerationPromptOverrides,
        generation_contract_snapshot: GenerationContractSnapshotV1,
    ) -> Self {
        Self {
            provider_payload,
            prompt_overrides,
            generation_contract_snapshot: Some(generation_contract_snapshot),
        }
    }
}
