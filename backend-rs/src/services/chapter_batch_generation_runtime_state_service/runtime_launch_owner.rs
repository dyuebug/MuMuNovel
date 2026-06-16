use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::ai::AIConfig;
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use crate::services::chapter_generation_execution_contract_service::{
    build_batch_request_runtime_state_owner_contract, prepare_generation_execution_config,
    BatchGenerationRequestRuntimeState, PreparedGenerationExecutionConfig,
    SingleChapterGenerationCompatOptions,
};

use super::{
    restore_batch_generation_runtime_compat_options_from_persisted_runtime_context,
    BatchGenerationPersistedRuntimeContext, BatchGenerationRuntimeLifecyclePlan,
};

pub(crate) fn build_batch_generation_runtime_launch_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::runtime_launch_session_dispatch",
        "scope": "batch_runtime_execution_input_compat_restore_launch_prepare_session_projection_and_dispatch",
        "python_source_map": [
            "backend/app/services/batch_generation_create_service.py",
            "backend/app/services/batch_generation_resume_service.py",
            "backend/app/services/batch_generation_run_service.py",
            "backend/app/services/batch_generation_orchestration_service.py",
            "backend/app/api/chapter_batch_generation_routes.py",
            "backend/app/api/chapters.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_launch_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_driver_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/resume_restore_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/startup_and_command_projection_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service/create_launch_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service/resume_launch_owner.rs"
        ],
        "behavior_contract": {
            "execution_input_entrypoints": [
                "build_batch_generation_execution_input",
                "restore_batch_generation_runtime_compat_options_from_runtime_state_seed",
                "build_batch_generation_runtime_launch_input_from_runtime_state_seed",
                "prepare_batch_generation_runtime_launch_input_from_request_runtime_state"
            ],
            "runtime_session_entrypoints": [
                "BatchGenerationRuntimeSession::from_execution_input"
            ],
            "dispatch_entrypoints": [
                "dispatch_batch_generation_runtime",
                "BatchGenerationRuntimeLifecyclePlan::start"
            ],
            "state_contract": {
                "compat_restore_owner": "runtime-state seed restores persisted compat options before batch launch input is materialized",
                "launch_prepare_owner": "batch runtime launch input still resolves AIConfig through prepare_generation_execution_config before dispatch",
                "dispatch_owner": "runtime dispatch remains async tokio spawn of the runtime lifecycle plan without changing route-visible behavior"
            }
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_runtime_state_service::runtime_driver_owner",
            "chapter_batch_generation_runtime_state_service::resume_restore_owner",
            "chapter_batch_generation_runtime_state_service::startup_and_command_projection_owner",
            "chapter_batch_generation_write_workflow_service::create_launch_owner",
            "chapter_batch_generation_resume_task_command_service::resume_launch_owner"
        ],
        "request_runtime_state_owner_contract": build_batch_request_runtime_state_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_batch_runtime_launch_dispatch_shells_as_source_map_until_explicit_freeze_delete_round",
            "runtime_state_keys": [
                "batch_request_runtime_state",
                "candidate_gateway",
                "quality_metrics_summary_state",
                "latest_quality_metrics",
                "active_story_repair_payload"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_runtime_dispatch_smoke"
        }
    })
}

#[derive(Debug, Clone)]
pub(crate) struct BatchGenerationExecutionInput {
    pub(crate) user_id: String,
    pub(crate) chapter_ids: Vec<String>,
    pub(crate) target_word_count: i32,
    pub(crate) compat_options: SingleChapterGenerationCompatOptions,
    pub(crate) ai_config: AIConfig,
    pub(crate) candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
}

pub(crate) fn build_batch_generation_execution_input(
    user_id: String,
    chapter_ids: Vec<String>,
    target_word_count: i32,
    compat_options: SingleChapterGenerationCompatOptions,
    execution_config: PreparedGenerationExecutionConfig,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
) -> BatchGenerationExecutionInput {
    BatchGenerationExecutionInput {
        user_id,
        chapter_ids,
        target_word_count,
        compat_options,
        ai_config: execution_config.ai_config,
        candidate_gateway_config,
    }
}

pub(crate) fn restore_batch_generation_runtime_compat_options_from_runtime_state_seed(
    base_compat_options: &SingleChapterGenerationCompatOptions,
    runtime_state_seed: Option<&Value>,
) -> SingleChapterGenerationCompatOptions {
    match runtime_state_seed {
        Some(runtime_state_seed) => {
            let persisted_runtime_context = BatchGenerationPersistedRuntimeContext::from_sources(
                Some(runtime_state_seed.clone()),
                None,
                None,
                None,
            );
            restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(
                base_compat_options,
                &persisted_runtime_context,
            )
        }
        None => base_compat_options.clone(),
    }
}

pub(crate) fn build_batch_generation_runtime_launch_input_from_runtime_state_seed(
    user_id: String,
    chapter_ids: Vec<String>,
    target_word_count: i32,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    runtime_state_seed: Option<&Value>,
    execution_config: PreparedGenerationExecutionConfig,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
) -> BatchGenerationExecutionInput {
    let resolved_compat_options =
        restore_batch_generation_runtime_compat_options_from_runtime_state_seed(
            &request_runtime_state.compat_options,
            runtime_state_seed,
        );

    build_batch_generation_execution_input(
        user_id,
        chapter_ids,
        target_word_count,
        resolved_compat_options,
        execution_config,
        candidate_gateway_config,
    )
}

pub(crate) async fn prepare_batch_generation_runtime_launch_input_from_request_runtime_state(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_ids: Vec<String>,
    target_word_count: i32,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    runtime_state_seed: Option<&Value>,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
) -> Result<BatchGenerationExecutionInput, String> {
    let model_override = request_runtime_state.model_override.clone();
    let execution_config =
        prepare_generation_execution_config(db, user_id, model_override.as_deref()).await?;

    Ok(
        build_batch_generation_runtime_launch_input_from_runtime_state_seed(
            user_id.to_string(),
            chapter_ids,
            target_word_count,
            request_runtime_state,
            runtime_state_seed,
            execution_config,
            candidate_gateway_config,
        ),
    )
}

pub(crate) fn dispatch_batch_generation_runtime(
    db: DatabaseConnection,
    task_id: String,
    execution_input: BatchGenerationExecutionInput,
) {
    tokio::spawn(async move {
        BatchGenerationRuntimeLifecyclePlan::start(&db, &task_id, execution_input).await;
    });
}

pub(crate) struct BatchGenerationRuntimeSession {
    pub(crate) user_id: String,
    pub(crate) target_word_count: i32,
    pub(crate) compat_options: SingleChapterGenerationCompatOptions,
    pub(crate) total_chapters: i32,
    pub(crate) ai_config: AIConfig,
    pub(crate) candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
}

impl BatchGenerationRuntimeSession {
    pub(crate) fn from_execution_input(
        execution_input: BatchGenerationExecutionInput,
    ) -> (Self, Vec<String>) {
        let BatchGenerationExecutionInput {
            user_id,
            chapter_ids,
            target_word_count,
            compat_options,
            ai_config,
            candidate_gateway_config,
        } = execution_input;

        (
            Self {
                user_id,
                target_word_count,
                compat_options,
                total_chapters: chapter_ids.len() as i32,
                ai_config,
                candidate_gateway_config,
            },
            chapter_ids,
        )
    }
}
