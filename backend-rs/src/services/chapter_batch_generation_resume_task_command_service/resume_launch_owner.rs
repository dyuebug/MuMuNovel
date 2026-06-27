use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::models::batch_generation_snapshot;
use crate::services::chapter_access_service::{
    load_accessible_chapters_for_generation, LoadAccessibleChapterForGenerationError,
};
use crate::services::chapter_batch_generation_runtime_state_service::{
    dispatch_batch_generation_runtime, prepare_batch_generation_resume_restored_runtime_state,
    reset_batch_generation_task_for_resume, BatchGenerationExecutionInput,
    BatchGenerationResumeResetPersistencePlan, PreparedBatchGenerationResumeRuntimeLaunch,
    PreparedSingleChapterResumeRuntimeLaunch, RestoredResumeRuntimeStateProjection,
    ResumeBatchGenerationCommandState, ResumeExecutionSelection,
};
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use crate::services::chapter_generation_execution_contract_service::normalize_chapter_generation_target_word_count;
use crate::services::chapter_single_generation_prepare_service::{
    check_chapter_generation_prerequisites, load_single_chapter_generation_target,
    PrepareSingleChapterGenerationRequestError, SingleChapterGenerationTarget,
};
use crate::services::chapter_single_generation_runtime_state_service::{
    SingleGenerationRuntimeLaunchInput, SingleGenerationRuntimeLifecyclePlan,
};

use super::ResumeBatchGenerationDomainError;

#[derive(Debug, Clone)]
pub(crate) struct BatchGenerationResumeLaunchPersistencePlan {
    command_state: ResumeBatchGenerationCommandState,
    dispatch_plan: ResumeExecutionDispatchPlan,
    reset_persistence_plan: BatchGenerationResumeResetPersistencePlan,
    response_payload: Value,
    single_generation_gateway_config: ChapterCandidateRouteGatewayConfig,
}

pub(crate) fn build_batch_generation_resume_launch_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_resume_task_command_service::resume_launch_owner",
        "scope": "resume_execution_eligibility_validated_dispatch_plan_reset_persistence_and_response_projection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service/resume_launch_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/api/chapter_batch_generation.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "resume_launch_entrypoints": [
                "BatchGenerationResumeLaunchPersistencePlan::new",
                "BatchGenerationResumeLaunchPersistencePlan::prepare",
                "BatchGenerationResumeLaunchPersistencePlan::prepare_from_validated_execution",
                "BatchGenerationResumeLaunchPersistencePlan::persist_and_dispatch"
            ],
            "execution_selection_entrypoints": [
                "ResumeExecutionEligibilityPlan::from_command_state",
                "ResumeExecutionEligibilityPlan::validate_access_and_prerequisites",
                "ValidatedResumeExecutionPlan::from_command_state",
                "ResumeExecutionDispatchPlan::from_validated_execution",
                "ResumeExecutionDispatchPlan::dispatch"
            ],
            "runtime_restore_entrypoints": [
                "prepare_batch_generation_resume_restored_runtime_state",
                "prepare_batch_generation_resume",
                "prepare_owned_batch_generation_resume",
                "resume_owned_batch_generation_task_command"
            ],
            "response_projection_entrypoints": [
                "BatchGenerationResumeResetPersistencePlan::into_resume_response_payload"
            ],
            "dispatch_contract": {
                "single_dispatch_owner": "SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config",
                "batch_dispatch_owner": "dispatch_batch_generation_runtime",
                "reset_first_contract": "reset persistence must commit before launch dispatch",
                "gateway_contract": "single-generation gateway config is preserved across both single and batch resume dispatch paths"
            },
            "domain_error_surface": [
                "InvalidStatus",
                "ManualReviewBlocked",
                "NoResumableChaptersFound",
                "NoChaptersLeftToResume",
                "SingleChapterUnavailable",
                "ChaptersUnavailable",
                "PrerequisitesBlocked",
                "Internal"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_resume_task_command_service",
            "chapter_batch_generation::resume_batch_generation",
            "chapter_batch_generation_runtime_state_service",
            "chapter_single_generation_runtime_state_service",
            "chapter-batch-generation-active-gateway-smoke-rust"
        ],
        "validation_boundary": [
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "runtime_knob": "python_candidate_executor_fallback",
            "source_map_policy": "batch_generation_resume_launch_owner_is_rust_only_and_surviving_resume_dispatch_response_surfaces_are_tracked_by_external_runtime_contracts",
            "runtime_state_keys": [
                "batch_request_runtime_state",
                "active_story_repair_payload",
                "quality_metrics_history",
                "quality_metrics_summary",
                "latest_quality_metrics",
                "quality_history_context",
                "resumed_from_batch_id",
                "candidate_gateway"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_resume_route_smoke"
        }
    })
}

pub(super) fn map_single_chapter_resume_validation_error(
    error: PrepareSingleChapterGenerationRequestError,
) -> ResumeBatchGenerationDomainError {
    match error {
        PrepareSingleChapterGenerationRequestError::Chapter(
            LoadAccessibleChapterForGenerationError::ChapterNotFound,
        ) => ResumeBatchGenerationDomainError::SingleChapterUnavailable(
            "Chapter not found".to_string(),
        ),
        PrepareSingleChapterGenerationRequestError::Chapter(
            LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
        ) => ResumeBatchGenerationDomainError::SingleChapterUnavailable(
            "Chapter not found or access denied".to_string(),
        ),
        PrepareSingleChapterGenerationRequestError::Chapter(
            LoadAccessibleChapterForGenerationError::Internal(detail),
        )
        | PrepareSingleChapterGenerationRequestError::Config(detail)
        | PrepareSingleChapterGenerationRequestError::Internal(detail) => {
            ResumeBatchGenerationDomainError::Internal(detail)
        }
        PrepareSingleChapterGenerationRequestError::PrerequisitesBlocked(detail) => {
            ResumeBatchGenerationDomainError::PrerequisitesBlocked(detail)
        }
        error => {
            let _detail = error
                .request_validation_detail_message()
                .expect("request validation errors should share canonical detail messages");
            ResumeBatchGenerationDomainError::Internal(
                "Unexpected single chapter resume validation request error".to_string(),
            )
        }
    }
}

pub(super) fn map_prepare_resume_runtime_state_error(
    error: crate::services::chapter_batch_generation_runtime_state_service::PrepareBatchGenerationResumeRuntimeStateError,
) -> ResumeBatchGenerationDomainError {
    match error {
        crate::services::chapter_batch_generation_runtime_state_service::PrepareBatchGenerationResumeRuntimeStateError::InvalidStatus => {
            ResumeBatchGenerationDomainError::InvalidStatus
        }
        crate::services::chapter_batch_generation_runtime_state_service::PrepareBatchGenerationResumeRuntimeStateError::ManualReviewBlocked => {
            ResumeBatchGenerationDomainError::ManualReviewBlocked
        }
    }
}

impl BatchGenerationResumeLaunchPersistencePlan {
    pub(crate) fn new(
        command_state: ResumeBatchGenerationCommandState,
        dispatch_plan: ResumeExecutionDispatchPlan,
        reset_persistence_plan: BatchGenerationResumeResetPersistencePlan,
        single_generation_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Self {
        let response_payload = reset_persistence_plan
            .clone()
            .into_resume_response_payload(&command_state);

        Self {
            command_state,
            dispatch_plan,
            reset_persistence_plan,
            response_payload,
            single_generation_gateway_config,
        }
    }

    pub(crate) async fn prepare_from_validated_execution(
        db: &DatabaseConnection,
        command_state: ResumeBatchGenerationCommandState,
        execution_plan: ValidatedResumeExecutionPlan,
        restored_runtime_state: RestoredResumeRuntimeStateProjection,
        existing_workflow_runtime_state: Option<Value>,
        user_id: &str,
        single_generation_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Result<Self, String> {
        let (dispatch_plan, runtime_state_seed) =
            ResumeExecutionDispatchPlan::from_validated_execution(
                db,
                user_id,
                execution_plan,
                restored_runtime_state,
                normalize_chapter_generation_target_word_count(Some(
                    command_state.target_word_count,
                )),
                single_generation_gateway_config.clone(),
            )
            .await?;
        let reset_persistence_plan =
            BatchGenerationResumeResetPersistencePlan::from_resume_task_with_existing_runtime_state(
                &command_state,
                runtime_state_seed,
                existing_workflow_runtime_state,
            );

        Ok(Self::new(
            command_state,
            dispatch_plan,
            reset_persistence_plan,
            single_generation_gateway_config,
        ))
    }

    pub(crate) async fn prepare(
        db: &DatabaseConnection,
        command_state: ResumeBatchGenerationCommandState,
        user_id: &str,
        snapshot: Option<&batch_generation_snapshot::Model>,
        single_generation_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Result<Self, ResumeBatchGenerationDomainError> {
        let (restored_runtime_state, existing_workflow_runtime_state) =
            prepare_batch_generation_resume_restored_runtime_state(&command_state, snapshot)
                .map_err(map_prepare_resume_runtime_state_error)?;
        let execution =
            ValidatedResumeExecutionPlan::from_command_state(db, user_id, &command_state).await?;

        Self::prepare_from_validated_execution(
            db,
            command_state,
            execution,
            restored_runtime_state,
            existing_workflow_runtime_state,
            user_id,
            single_generation_gateway_config,
        )
        .await
        .map_err(ResumeBatchGenerationDomainError::Internal)
    }

    #[cfg(test)]
    pub(crate) fn dispatch_plan(&self) -> &ResumeExecutionDispatchPlan {
        &self.dispatch_plan
    }

    #[cfg(test)]
    pub(crate) fn response_payload(&self) -> Value {
        self.response_payload.clone()
    }

    #[cfg(test)]
    pub(crate) fn single_generation_gateway_config(&self) -> &ChapterCandidateRouteGatewayConfig {
        &self.single_generation_gateway_config
    }

    #[cfg(test)]
    pub(crate) fn from_contract_for_test(
        command_state: ResumeBatchGenerationCommandState,
        dispatch_plan: ResumeExecutionDispatchPlan,
        reset_persistence_plan: BatchGenerationResumeResetPersistencePlan,
        single_generation_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Self {
        Self::new(
            command_state,
            dispatch_plan,
            reset_persistence_plan,
            single_generation_gateway_config,
        )
    }

    #[cfg(test)]
    pub(crate) fn reset_persistence_plan(&self) -> &BatchGenerationResumeResetPersistencePlan {
        &self.reset_persistence_plan
    }

    pub(crate) async fn persist_and_dispatch(
        self,
        db: &DatabaseConnection,
    ) -> Result<Value, String> {
        let response_payload = self.response_payload.clone();
        let batch_id = self.command_state.batch_id.clone();

        reset_batch_generation_task_for_resume(db, self.reset_persistence_plan).await?;
        self.dispatch_plan
            .dispatch(db.clone(), batch_id, self.single_generation_gateway_config);
        Ok(response_payload)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ResumeExecutionDispatchPlan {
    SingleChapter {
        runtime_input: SingleGenerationRuntimeLaunchInput,
    },
    Batch {
        runtime_input: BatchGenerationExecutionInput,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ResumeExecutionEligibilityPlan {
    SingleChapter { chapter_id: String },
    Batch { chapter_ids: Vec<String> },
}

#[derive(Debug, Clone)]
pub(crate) enum ValidatedResumeExecutionPlan {
    SingleChapter {
        validated_single_chapter_target: SingleChapterGenerationTarget,
    },
    Batch {
        chapter_ids: Vec<String>,
    },
}

impl ResumeExecutionEligibilityPlan {
    pub(crate) fn from_command_state(
        command_state: &ResumeBatchGenerationCommandState,
    ) -> Result<Self, ResumeBatchGenerationDomainError> {
        let execution_selection = command_state
            .resolve_execution_selection()
            .map_err(ResumeBatchGenerationDomainError::from_execution_selection_error)?;

        Self::from_execution_selection(execution_selection)
    }

    pub(crate) fn from_execution_selection(
        execution_selection: ResumeExecutionSelection,
    ) -> Result<Self, ResumeBatchGenerationDomainError> {
        match execution_selection {
            ResumeExecutionSelection::SingleChapter { chapter_id } => {
                Ok(Self::SingleChapter { chapter_id })
            }
            ResumeExecutionSelection::Batch { chapter_ids } => {
                if chapter_ids.is_empty() {
                    return Err(ResumeBatchGenerationDomainError::NoResumableChaptersFound);
                }
                Ok(Self::Batch { chapter_ids })
            }
        }
    }

    async fn validate_access_and_prerequisites(
        self,
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Result<ValidatedResumeExecutionPlan, ResumeBatchGenerationDomainError> {
        match self {
            Self::SingleChapter { chapter_id } => {
                let validated_single_chapter_target =
                    load_single_chapter_generation_target(db, &chapter_id, user_id)
                        .await
                        .map_err(map_single_chapter_resume_validation_error)?;

                Ok(ValidatedResumeExecutionPlan::SingleChapter {
                    validated_single_chapter_target,
                })
            }
            Self::Batch { chapter_ids } => {
                let remaining_chapters =
                    load_accessible_chapters_for_generation(db, &chapter_ids, user_id)
                        .await
                        .map_err(|error| {
                            match error {
                    LoadAccessibleChapterForGenerationError::ChapterNotFound
                    | LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied => {
                        ResumeBatchGenerationDomainError::ChaptersUnavailable
                    }
                    LoadAccessibleChapterForGenerationError::Internal(detail) => {
                        ResumeBatchGenerationDomainError::Internal(detail)
                    }
                }
                        })?;
                if let Some(first_chapter) = remaining_chapters.first() {
                    let prerequisite = check_chapter_generation_prerequisites(db, first_chapter)
                        .await
                        .map_err(ResumeBatchGenerationDomainError::Internal)?;
                    if !prerequisite.can_generate {
                        return Err(ResumeBatchGenerationDomainError::PrerequisitesBlocked(
                            prerequisite.error_message,
                        ));
                    }
                }

                Ok(ValidatedResumeExecutionPlan::Batch { chapter_ids })
            }
        }
    }
}

impl ValidatedResumeExecutionPlan {
    pub(crate) async fn from_command_state(
        db: &DatabaseConnection,
        user_id: &str,
        command_state: &ResumeBatchGenerationCommandState,
    ) -> Result<Self, ResumeBatchGenerationDomainError> {
        ResumeExecutionEligibilityPlan::from_command_state(command_state)?
            .validate_access_and_prerequisites(db, user_id)
            .await
    }
}

impl ResumeExecutionDispatchPlan {
    pub(crate) async fn from_validated_execution(
        db: &DatabaseConnection,
        user_id: &str,
        execution_plan: ValidatedResumeExecutionPlan,
        restored_runtime_state: RestoredResumeRuntimeStateProjection,
        target_word_count: i32,
        single_generation_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Result<(Self, Option<Value>), String> {
        match execution_plan {
            ValidatedResumeExecutionPlan::SingleChapter {
                validated_single_chapter_target,
            } => {
                let PreparedSingleChapterResumeRuntimeLaunch {
                    runtime_input,
                    runtime_state_seed,
                } = restored_runtime_state
                    .prepare_single_chapter_runtime_launch(
                        db,
                        user_id,
                        &validated_single_chapter_target,
                        target_word_count,
                    )
                    .await?;

                Ok((Self::SingleChapter { runtime_input }, runtime_state_seed))
            }
            ValidatedResumeExecutionPlan::Batch { chapter_ids } => {
                let PreparedBatchGenerationResumeRuntimeLaunch {
                    runtime_input,
                    runtime_state_seed,
                } = restored_runtime_state
                    .prepare_batch_runtime_launch(
                        db,
                        user_id,
                        chapter_ids,
                        target_word_count,
                        single_generation_gateway_config,
                    )
                    .await?;

                Ok((Self::Batch { runtime_input }, runtime_state_seed))
            }
        }
    }

    pub(crate) fn dispatch(
        self,
        db: DatabaseConnection,
        task_id: String,
        single_generation_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) {
        match self {
            ResumeExecutionDispatchPlan::SingleChapter { runtime_input } => {
                SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config(
                    task_id,
                    runtime_input,
                    single_generation_gateway_config,
                )
                .spawn(db)
            }
            ResumeExecutionDispatchPlan::Batch { mut runtime_input } => {
                runtime_input.candidate_gateway_config = single_generation_gateway_config;
                dispatch_batch_generation_runtime(db, task_id, runtime_input)
            }
        }
    }
}
