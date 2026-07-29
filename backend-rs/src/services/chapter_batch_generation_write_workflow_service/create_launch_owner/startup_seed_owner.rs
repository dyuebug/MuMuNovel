use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::{json, Value};

use crate::models::project_default_style;
#[cfg(test)]
use crate::services::chapter_batch_generation_runtime_state_service::runtime_launch_owner::restore_batch_generation_runtime_compat_options_from_runtime_state_seed;
use crate::services::chapter_batch_generation_runtime_state_service::{
    build_batch_generation_startup_snapshot_and_runtime_launch_input_from_runtime_state_seed,
    BatchGenerationExecutionInput, BatchGenerationQueuedSnapshotPlan,
};
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
#[cfg(test)]
use crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions;
use crate::services::chapter_generation_execution_contract_service::{
    batch_generation_request_runtime_state_payload, BatchGenerationRequestRuntimeState,
    PreparedGenerationExecutionConfig,
};
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    apply_batch_quality_runtime_context_to_payload,
    resolve_batch_quality_runtime_context_for_startup_seed,
};
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::{
    load_recent_batch_story_repair_quality_summary,
    resolve_active_story_repair_payload_with_quality_fallback,
};
use crate::services::generation_contract_service::{
    merge_generation_contract_runtime_snapshot, GenerationContractError,
    GenerationContractSnapshotV1,
};

pub(crate) fn build_batch_generation_create_startup_seed_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_write_workflow_service::create_launch_startup_seed_owner",
        "scope": "create_runtime_startup_seed_style_resolution_and_runtime_state_payload_projection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service/create_launch_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service/create_launch_owner/startup_seed_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs"
        ],
        "behavior_contract": {
            "style_resolution_entrypoints": [
                "resolve_batch_generation_create_effective_style_id",
                "select_batch_generation_create_effective_style_id"
            ],
            "startup_seed_entrypoints": [
                "BatchGenerationCreateStartupRuntimeState::prepare",
                "BatchGenerationCreateStartupRuntimeState::from_recent_history_summary",
                "BatchGenerationCreateStartupRuntimeState::into_runtime_seed",
                "BatchGenerationCreateRuntimeSeed::prepare",
                "BatchGenerationCreateRuntimeSeed::into_workflow_launch_parts"
            ],
            "runtime_state_payload_entrypoints": [
                "build_batch_generation_runtime_state_payload_from_parts",
                "build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload"
            ],
            "runtime_state_keys": [
                "batch_request_runtime_state",
                "quality_metrics_summary",
                "quality_metrics_summary_state",
                "quality_metrics_history",
                "latest_quality_metrics",
                "quality_history_context",
                "active_story_repair_payload",
                "candidate_gateway"
            ]
        },
        "validation_boundary": [
            "cargo test chapter_batch_generation_write_workflow_service",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "rollback_boundary": {
            "runtime_knob": "python_candidate_executor_fallback",
            "source_map_policy": "batch_generation_create_startup_seed_owner_is_rust_only_and_surviving_startup_seed_surfaces_are_tracked_by_external_runtime_contracts",
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_batch_create_route_smoke"
        }
    })
}

pub(crate) async fn resolve_batch_generation_create_effective_style_id(
    db: &DatabaseConnection,
    project_id: &str,
    requested_style_id: Option<i32>,
) -> Result<Option<i32>, String> {
    let default_style_id = match requested_style_id {
        Some(_) => None,
        None => load_batch_generation_project_default_style_id(db, project_id).await?,
    };

    Ok(select_batch_generation_create_effective_style_id(
        requested_style_id,
        default_style_id,
    ))
}

pub(crate) fn select_batch_generation_create_effective_style_id(
    requested_style_id: Option<i32>,
    default_style_id: Option<i32>,
) -> Option<i32> {
    requested_style_id.or(default_style_id)
}

async fn load_batch_generation_project_default_style_id(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<Option<i32>, String> {
    project_default_style::Entity::find()
        .filter(project_default_style::Column::ProjectId.eq(project_id))
        .one(db)
        .await
        .map(|default_style| default_style.map(|model| model.style_id))
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationCreateStartupSeedSource {
    RequestOnly,
    RecentHistorySummary,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationCreateStartupRuntimeState {
    request_runtime_state: BatchGenerationRequestRuntimeState,
    runtime_state_payload: Value,
    seed_source: BatchGenerationCreateStartupSeedSource,
}

impl BatchGenerationCreateStartupRuntimeState {
    pub(crate) async fn prepare(
        db: &DatabaseConnection,
        project_id: &str,
        start_chapter_number: i32,
        request_runtime_state: BatchGenerationRequestRuntimeState,
    ) -> Result<Self, String> {
        let recent_history_summary =
            load_recent_batch_story_repair_quality_summary(db, project_id, start_chapter_number)
                .await?;

        Ok(Self::from_recent_history_summary(
            request_runtime_state,
            recent_history_summary,
        ))
    }

    pub(crate) fn from_recent_history_summary(
        request_runtime_state: BatchGenerationRequestRuntimeState,
        recent_history_summary: Option<Value>,
    ) -> Self {
        let (runtime_state_payload, seed_source) = match recent_history_summary {
            Some(recent_history_summary) => (
                build_batch_generation_runtime_state_payload_from_parts(
                    &request_runtime_state,
                    Some(&recent_history_summary),
                ),
                BatchGenerationCreateStartupSeedSource::RecentHistorySummary,
            ),
            None => (
                build_batch_generation_runtime_state_payload_from_parts(
                    &request_runtime_state,
                    None,
                ),
                BatchGenerationCreateStartupSeedSource::RequestOnly,
            ),
        };

        Self {
            request_runtime_state,
            runtime_state_payload,
            seed_source,
        }
    }

    fn into_parts(self) -> (BatchGenerationRequestRuntimeState, Value) {
        (self.request_runtime_state, self.runtime_state_payload)
    }

    pub(crate) fn into_runtime_seed(self) -> BatchGenerationCreateRuntimeSeed {
        let (_, runtime_state_payload) = self.into_parts();

        BatchGenerationCreateRuntimeSeed {
            runtime_state_payload,
        }
    }

    #[cfg(test)]
    pub(crate) fn runtime_state_payload(&self) -> &Value {
        &self.runtime_state_payload
    }

    #[cfg(test)]
    pub(crate) fn request_runtime_state(&self) -> &BatchGenerationRequestRuntimeState {
        &self.request_runtime_state
    }

    #[cfg(test)]
    pub(crate) fn seed_source(&self) -> BatchGenerationCreateStartupSeedSource {
        self.seed_source
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationCreateRuntimeSeed {
    runtime_state_payload: Value,
}

impl BatchGenerationCreateRuntimeSeed {
    pub(crate) async fn prepare(
        db: &DatabaseConnection,
        project_id: &str,
        start_chapter_number: i32,
        request_runtime_state: BatchGenerationRequestRuntimeState,
    ) -> Result<Self, String> {
        BatchGenerationCreateStartupRuntimeState::prepare(
            db,
            project_id,
            start_chapter_number,
            request_runtime_state,
        )
        .await
        .map(BatchGenerationCreateStartupRuntimeState::into_runtime_seed)
    }

    #[cfg(test)]
    pub(crate) fn from_runtime_state_payload(runtime_state_payload: Value) -> Self {
        Self {
            runtime_state_payload,
        }
    }

    pub(crate) fn merge_generation_contract_snapshot(
        &mut self,
        snapshot: &GenerationContractSnapshotV1,
    ) -> Result<(), GenerationContractError> {
        merge_generation_contract_runtime_snapshot(&mut self.runtime_state_payload, snapshot)
    }

    #[cfg(test)]
    pub(crate) fn into_parts(self) -> (Value, SingleChapterGenerationCompatOptions) {
        let request_runtime_state =
            crate::services::chapter_generation_execution_contract_service::parse_batch_generation_request_runtime_state(Some(&self.runtime_state_payload));
        let resolved_compat_options =
            restore_batch_generation_runtime_compat_options_from_runtime_state_seed(
                &request_runtime_state.compat_options,
                Some(&self.runtime_state_payload),
            );

        (self.runtime_state_payload, resolved_compat_options)
    }

    pub(crate) fn into_workflow_launch_parts(
        self,
        user_id: String,
        chapter_ids: Vec<String>,
        total_chapters: i32,
        normalized_target_word_count: i32,
        execution_config: PreparedGenerationExecutionConfig,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> (
        BatchGenerationQueuedSnapshotPlan,
        BatchGenerationExecutionInput,
    ) {
        let runtime_state_payload = self.runtime_state_payload;

        build_batch_generation_startup_snapshot_and_runtime_launch_input_from_runtime_state_seed(
            user_id,
            chapter_ids,
            total_chapters,
            normalized_target_word_count,
            runtime_state_payload,
            execution_config,
            candidate_gateway_config,
        )
    }

    #[cfg(test)]
    pub(crate) fn startup_snapshot_plan(
        &self,
        total_chapters: i32,
    ) -> BatchGenerationQueuedSnapshotPlan {
        BatchGenerationQueuedSnapshotPlan::from_runtime_state_seed(
            total_chapters,
            Some(self.runtime_state_payload.clone()),
        )
    }
}

pub(crate) fn build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    explicit_story_repair_payload: Option<&Value>,
    quality_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Value {
    let mut payload = batch_generation_request_runtime_state_payload(request_runtime_state)
        .as_object()
        .cloned()
        .unwrap_or_default();
    let resolved_quality_context = resolve_batch_quality_runtime_context_for_startup_seed(
        quality_summary,
        latest_quality_metrics,
    );
    let resolved_quality_summary = resolved_quality_context
        .quality_metrics_summary
        .clone()
        .unwrap_or(Value::Null);
    let active_story_repair_payload = resolve_active_story_repair_payload_with_quality_fallback(
        explicit_story_repair_payload,
        Some(&resolved_quality_summary),
        resolved_quality_context.latest_quality_metrics.as_ref(),
        "batch",
        "recent_history_summary",
        "Recent history summary",
    );

    if let Some(active_story_repair_payload) = active_story_repair_payload {
        payload.insert(
            "active_story_repair_payload".to_string(),
            active_story_repair_payload,
        );
    }
    apply_batch_quality_runtime_context_to_payload(
        &mut payload,
        resolved_quality_context,
        Some(resolved_quality_summary.clone()),
    );

    Value::Object(payload)
}

pub(crate) fn build_batch_generation_runtime_state_payload_from_parts(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    quality_summary: Option<&Value>,
) -> Value {
    build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload(
        request_runtime_state,
        request_runtime_state
            .active_story_repair_payload_with_scope("batch")
            .as_ref(),
        quality_summary,
        None,
    )
}
