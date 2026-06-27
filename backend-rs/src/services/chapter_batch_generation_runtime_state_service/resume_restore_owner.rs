use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::models::batch_generation_snapshot;
use crate::services::chapter_batch_generation_task_payload_base_service::{
    resolve_failed_terminal_semantics_from_sources, BatchGenerationFailedTerminalKind,
    BatchGenerationQualityStatusContext, BatchGenerationTaskKind,
};
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use crate::services::chapter_generation_execution_contract_service::{
    active_story_repair_payload_from_runtime_state, parse_batch_generation_request_runtime_state,
    BatchGenerationRequestRuntimeState, SingleChapterGenerationCompatOptions,
};
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    apply_batch_quality_runtime_context_to_payload,
    apply_generation_quality_runtime_context_to_payload,
    resolve_batch_quality_runtime_context_from_persisted_sources,
    resolve_generation_quality_runtime_context_from_persisted_sources,
    BatchGenerationQualityRuntimeContext, GenerationQualityRuntimeContext,
};
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::{
    resolve_resumed_active_story_repair_payload,
    restore_story_repair_compat_options_from_active_snapshot,
};
use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationTarget;

use super::{
    build_batch_generation_runtime_state_payload_from_current_quality,
    build_batch_generation_runtime_state_payload_preserving_quality_state,
    prepare_batch_generation_runtime_launch_input_from_request_runtime_state,
    prepare_single_chapter_runtime_launch_input_from_request_runtime_state,
    BatchGenerationExecutionInput, ResumeBatchGenerationCommandState,
};
use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;

pub(crate) fn build_batch_generation_resume_restore_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::resume_restore_runtime_projection",
        "scope": "resume_restore_runtime_state_seed_quality_context_and_launch_projection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/resume_restore_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "restore_entrypoints": [
                "prepare_batch_generation_resume_restored_runtime_state",
                "RestoredResumeRuntimeStateProjection::from_persisted_runtime_context",
                "BatchGenerationPersistedRuntimeContext::from_snapshot",
                "BatchGenerationPersistedRuntimeContext::build_restored_resume_runtime_state"
            ],
            "compat_and_quality_restore_helpers": [
                "BatchGenerationPersistedRuntimeContext::restored_resume_compat_options",
                "BatchGenerationPersistedRuntimeContext::resolved_resume_active_story_repair_payload",
                "BatchGenerationPersistedRuntimeContext::resume_quality_status_context",
                "BatchGenerationPersistedRuntimeContext::restored_quality_runtime_context"
            ],
            "launch_projection_entrypoints": [
                "RestoredResumeRuntimeStateProjection::prepare_batch_runtime_launch",
                "RestoredResumeRuntimeStateProjection::prepare_single_chapter_runtime_launch",
                "RestoredResumeRuntimeStateProjection::into_launch_parts"
            ],
            "runtime_state_seed_fields": [
                "resume_from_batch_id",
                "current_retry_count",
                "max_retries",
                "active_story_repair_payload",
                "quality_metrics_history",
                "quality_metrics_summary",
                "quality_metrics_summary_state",
                "latest_quality_metrics"
            ],
            "manual_review_block_policy": [
                "only failed or cancelled tasks can restore runtime state",
                "manual_review blocked tasks cannot resume runtime launch",
                "resume restore preserves existing workflow runtime state for reset persistence"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_resume_task_command_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "quality_runtime_owner_contract": crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::build_generation_quality_runtime_owner_contract(),
        "request_runtime_state_owner_contract": crate::services::chapter_generation_execution_contract_service::build_batch_request_runtime_state_owner_contract(),
        "story_repair_quality_context_owner_contract": crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::build_story_repair_quality_context_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "batch_generation_resume_restore_owner_is_rust_only_and_surviving_story_repair_runtime_surfaces_are_tracked_by_external_runtime_contracts",
            "runtime_state_keys": [
                "batch_request_runtime_state",
                "active_story_repair_payload",
                "quality_metrics_history",
                "quality_metrics_summary",
                "quality_metrics_summary_state",
                "latest_quality_metrics"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_resume_gateway_smoke"
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrepareBatchGenerationResumeRuntimeStateError {
    InvalidStatus,
    ManualReviewBlocked,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct BatchGenerationPersistedRuntimeContext {
    workflow_runtime_state: Option<Value>,
    request_runtime_state: BatchGenerationRequestRuntimeState,
    explicit_story_repair_payload: Option<Value>,
    quality_metrics_history: Option<Value>,
    quality_metrics_summary_state: Option<Value>,
    quality_metrics_summary: Option<Value>,
    latest_quality_metrics: Option<Value>,
}

#[derive(Debug, Clone)]
pub(crate) struct RestoredResumeRuntimeStateProjection {
    pub(crate) quality_status_context: BatchGenerationQualityStatusContext,
    pub(crate) request_runtime_state: BatchGenerationRequestRuntimeState,
    pub(crate) runtime_state_seed: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RestoredResumeRuntimeLaunchParts {
    pub(crate) request_runtime_state: BatchGenerationRequestRuntimeState,
    pub(crate) runtime_state_seed: Option<Value>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedBatchGenerationResumeRuntimeLaunch {
    pub(crate) runtime_input: BatchGenerationExecutionInput,
    pub(crate) runtime_state_seed: Option<Value>,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedSingleChapterResumeRuntimeLaunch {
    pub(crate) runtime_input: SingleGenerationRuntimeLaunchInput,
    pub(crate) runtime_state_seed: Option<Value>,
}

#[cfg(test)]
pub(crate) fn build_batch_generation_resume_runtime_checkpoint(
    task: &ResumeBatchGenerationCommandState,
    runtime_state_seed: Option<Value>,
) -> Value {
    task.resolve_reset_semantics()
        .build_resume_checkpoint_with_seed(task.total_chapters, runtime_state_seed)
}

pub(crate) fn prepare_batch_generation_resume_restored_runtime_state(
    command_state: &ResumeBatchGenerationCommandState,
    snapshot: Option<&batch_generation_snapshot::Model>,
) -> Result<
    (RestoredResumeRuntimeStateProjection, Option<Value>),
    PrepareBatchGenerationResumeRuntimeStateError,
> {
    if !matches!(command_state.status.as_str(), "failed" | "cancelled") {
        return Err(PrepareBatchGenerationResumeRuntimeStateError::InvalidStatus);
    }

    let task_kind = command_state.task_kind();
    let persisted_runtime_context =
        BatchGenerationPersistedRuntimeContext::from_snapshot(snapshot.cloned());
    let restored_runtime_state =
        RestoredResumeRuntimeStateProjection::from_persisted_runtime_context(
            task_kind,
            &command_state.batch_id,
            command_state.max_retries,
            &persisted_runtime_context,
        );
    if restored_runtime_state.is_manual_review_blocked(command_state) {
        return Err(PrepareBatchGenerationResumeRuntimeStateError::ManualReviewBlocked);
    }

    Ok((
        restored_runtime_state,
        snapshot.and_then(|item| item.workflow_runtime_state.clone()),
    ))
}

impl RestoredResumeRuntimeStateProjection {
    pub(crate) fn from_persisted_runtime_context(
        task_kind: BatchGenerationTaskKind,
        batch_id: &str,
        max_retries: i32,
        persisted_runtime_context: &BatchGenerationPersistedRuntimeContext,
    ) -> Self {
        persisted_runtime_context.build_restored_resume_runtime_state(
            task_kind,
            batch_id,
            max_retries,
        )
    }

    #[cfg(test)]
    pub(crate) fn from_sources(
        task_kind: BatchGenerationTaskKind,
        batch_id: &str,
        max_retries: i32,
        workflow_runtime_state: Option<&Value>,
        snapshot: Option<&batch_generation_snapshot::Model>,
        request_runtime_state: &BatchGenerationRequestRuntimeState,
    ) -> Self {
        let workflow_runtime_state = match workflow_runtime_state.cloned() {
            Some(Value::Object(mut state)) => {
                state
                    .entry("batch_request_runtime_state".to_string())
                    .or_insert_with(|| json!(request_runtime_state));
                Some(Value::Object(state))
            }
            _ => {
                let state = serde_json::Map::from_iter([(
                    "batch_request_runtime_state".to_string(),
                    json!(request_runtime_state),
                )]);
                Some(Value::Object(state))
            }
        };
        let persisted_runtime_context = BatchGenerationPersistedRuntimeContext::from_sources(
            workflow_runtime_state,
            snapshot.and_then(|item| item.quality_metrics_history.clone()),
            snapshot.and_then(|item| item.quality_metrics_summary.clone()),
            snapshot.and_then(|item| item.latest_quality_metrics.clone()),
        );

        Self::from_persisted_runtime_context(
            task_kind,
            batch_id,
            max_retries,
            &persisted_runtime_context,
        )
    }

    pub(crate) fn is_manual_review_blocked(
        &self,
        command_state: &ResumeBatchGenerationCommandState,
    ) -> bool {
        resolve_failed_terminal_semantics_from_sources(
            Some(&command_state.failed_chapters),
            Some(&self.quality_status_context),
            command_state.current_retry_count,
            command_state.max_retries,
        )
        .as_ref()
        .is_some_and(|semantics| semantics.kind == BatchGenerationFailedTerminalKind::ManualReview)
    }

    pub(crate) fn into_launch_parts(self) -> RestoredResumeRuntimeLaunchParts {
        RestoredResumeRuntimeLaunchParts {
            request_runtime_state: self.request_runtime_state,
            runtime_state_seed: self.runtime_state_seed,
        }
    }

    pub(crate) async fn prepare_batch_runtime_launch(
        self,
        db: &DatabaseConnection,
        user_id: &str,
        chapter_ids: Vec<String>,
        target_word_count: i32,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Result<PreparedBatchGenerationResumeRuntimeLaunch, String> {
        let RestoredResumeRuntimeLaunchParts {
            request_runtime_state,
            runtime_state_seed,
        } = self.into_launch_parts();
        let runtime_input =
            prepare_batch_generation_runtime_launch_input_from_request_runtime_state(
                db,
                user_id,
                chapter_ids,
                target_word_count,
                &request_runtime_state,
                runtime_state_seed.as_ref(),
                candidate_gateway_config,
            )
            .await?;

        Ok(PreparedBatchGenerationResumeRuntimeLaunch {
            runtime_input,
            runtime_state_seed,
        })
    }

    pub(crate) async fn prepare_single_chapter_runtime_launch(
        self,
        db: &DatabaseConnection,
        user_id: &str,
        chapter_target: &SingleChapterGenerationTarget,
        target_word_count: i32,
    ) -> Result<PreparedSingleChapterResumeRuntimeLaunch, String> {
        let RestoredResumeRuntimeLaunchParts {
            request_runtime_state,
            runtime_state_seed,
        } = self.into_launch_parts();
        let runtime_input = prepare_single_chapter_runtime_launch_input_from_request_runtime_state(
            db,
            user_id,
            chapter_target,
            &request_runtime_state,
            target_word_count,
        )
        .await
        .map_err(|error| error.detail_message())?;

        Ok(PreparedSingleChapterResumeRuntimeLaunch {
            runtime_input,
            runtime_state_seed,
        })
    }
}

impl BatchGenerationPersistedRuntimeContext {
    pub(crate) fn from_snapshot(snapshot: Option<batch_generation_snapshot::Model>) -> Self {
        let workflow_runtime_state = snapshot
            .as_ref()
            .and_then(|item| item.workflow_runtime_state.clone());
        let snapshot_quality_metrics_history = snapshot
            .as_ref()
            .and_then(|item| item.quality_metrics_history.clone());
        let snapshot_quality_metrics_summary = snapshot
            .as_ref()
            .and_then(|item| item.quality_metrics_summary.clone());
        let snapshot_latest_quality_metrics = snapshot
            .as_ref()
            .and_then(|item| item.latest_quality_metrics.clone());

        Self::from_sources(
            workflow_runtime_state,
            snapshot_quality_metrics_history,
            snapshot_quality_metrics_summary,
            snapshot_latest_quality_metrics,
        )
    }

    pub(crate) fn from_sources(
        workflow_runtime_state: Option<Value>,
        snapshot_quality_metrics_history: Option<Value>,
        snapshot_quality_metrics_summary: Option<Value>,
        snapshot_latest_quality_metrics: Option<Value>,
    ) -> Self {
        let request_runtime_state =
            parse_batch_generation_request_runtime_state(workflow_runtime_state.as_ref());
        let explicit_story_repair_payload =
            active_story_repair_payload_from_runtime_state(workflow_runtime_state.as_ref());
        let latest_quality_metrics = snapshot_latest_quality_metrics.or_else(|| {
            workflow_runtime_state
                .as_ref()
                .and_then(Value::as_object)
                .and_then(|state| state.get("latest_quality_metrics").cloned())
        });
        let quality_metrics_history = snapshot_quality_metrics_history.or_else(|| {
            workflow_runtime_state
                .as_ref()
                .and_then(Value::as_object)
                .and_then(|state| state.get("quality_metrics_history").cloned())
        });
        let quality_metrics_summary_state = workflow_runtime_state
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|state| state.get("quality_metrics_summary_state").cloned());
        let quality_metrics_summary = snapshot_quality_metrics_summary.or_else(|| {
            workflow_runtime_state
                .as_ref()
                .and_then(Value::as_object)
                .and_then(|state| state.get("quality_metrics_summary").cloned())
        });

        Self {
            workflow_runtime_state,
            request_runtime_state,
            explicit_story_repair_payload,
            quality_metrics_history,
            quality_metrics_summary_state,
            quality_metrics_summary,
            latest_quality_metrics,
        }
    }

    pub(crate) fn has_workflow_runtime_state(&self) -> bool {
        self.workflow_runtime_state.is_some()
    }

    pub(crate) fn request_runtime_state(&self) -> &BatchGenerationRequestRuntimeState {
        &self.request_runtime_state
    }

    pub(crate) fn explicit_story_repair_payload(&self) -> Option<&Value> {
        self.explicit_story_repair_payload.as_ref()
    }

    pub(crate) fn quality_metrics_history(&self) -> Option<&Value> {
        self.quality_metrics_history.as_ref()
    }

    pub(crate) fn quality_metrics_summary_state(&self) -> Option<&Value> {
        self.quality_metrics_summary_state.as_ref()
    }

    pub(crate) fn quality_metrics_summary(&self) -> Option<&Value> {
        self.quality_metrics_summary.as_ref()
    }

    pub(crate) fn latest_quality_metrics(&self) -> Option<&Value> {
        self.latest_quality_metrics.as_ref()
    }

    pub(crate) fn restored_quality_runtime_context(
        &self,
        task_kind: BatchGenerationTaskKind,
    ) -> BatchGenerationQualityRuntimeContext {
        match task_kind {
            BatchGenerationTaskKind::SingleChapter => {
                let resolved = resolve_generation_quality_runtime_context_from_persisted_sources(
                    "chapter",
                    self.latest_quality_metrics(),
                    self.quality_metrics_history(),
                    self.quality_metrics_summary_state(),
                    self.quality_metrics_summary(),
                );

                BatchGenerationQualityRuntimeContext {
                    latest_quality_metrics: resolved.latest_quality_metrics,
                    quality_metrics_history: resolved.quality_metrics_history,
                    quality_metrics_summary_state: resolved.quality_metrics_summary_state,
                    quality_metrics_summary: resolved.quality_metrics_summary,
                    quality_history_context: resolved.quality_history_context,
                }
            }
            BatchGenerationTaskKind::Batch => {
                resolve_batch_quality_runtime_context_from_persisted_sources(
                    self.latest_quality_metrics(),
                    self.quality_metrics_history(),
                    self.quality_metrics_summary_state(),
                    self.quality_metrics_summary(),
                )
            }
        }
    }

    pub(crate) fn restored_resume_compat_options(
        &self,
        restored_quality_context: &BatchGenerationQualityRuntimeContext,
    ) -> SingleChapterGenerationCompatOptions {
        restore_story_repair_compat_options_from_active_snapshot(
            &self.request_runtime_state.compat_options,
            self.explicit_story_repair_payload(),
            restored_quality_context.quality_metrics_summary.as_ref(),
            restored_quality_context.latest_quality_metrics.as_ref(),
        )
    }

    pub(crate) fn resolved_resume_active_story_repair_payload(
        &self,
        request_active_story_repair_payload: Option<&Value>,
        restored_quality_context: &BatchGenerationQualityRuntimeContext,
        scope: &str,
    ) -> Option<Value> {
        resolve_resumed_active_story_repair_payload(
            self.explicit_story_repair_payload(),
            restored_quality_context.quality_metrics_summary.as_ref(),
            restored_quality_context.latest_quality_metrics.as_ref(),
            request_active_story_repair_payload,
            scope,
            "recent_history_summary",
            "Recent history summary",
        )
    }

    pub(crate) fn resume_quality_status_context(
        &self,
        restored_quality_context: &BatchGenerationQualityRuntimeContext,
    ) -> BatchGenerationQualityStatusContext {
        BatchGenerationQualityStatusContext::from_runtime_quality_context_and_active_payload(
            restored_quality_context,
            self.explicit_story_repair_payload(),
        )
    }

    pub(crate) fn build_restored_resume_runtime_state(
        &self,
        task_kind: BatchGenerationTaskKind,
        batch_id: &str,
        max_retries: i32,
    ) -> RestoredResumeRuntimeStateProjection {
        let restored_quality_context = self.restored_quality_runtime_context(task_kind);
        let restored_compat_options =
            self.restored_resume_compat_options(&restored_quality_context);
        let runtime_scope = match task_kind {
            BatchGenerationTaskKind::SingleChapter => "chapter",
            BatchGenerationTaskKind::Batch => "batch",
        };
        let restored_request_runtime_state = BatchGenerationRequestRuntimeState::new(
            restored_compat_options,
            self.request_runtime_state.model_override.clone(),
        );
        let request_active_story_repair_payload =
            restored_request_runtime_state.active_story_repair_payload_with_scope(runtime_scope);
        let active_story_repair_payload = self.resolved_resume_active_story_repair_payload(
            request_active_story_repair_payload.as_ref(),
            &restored_quality_context,
            runtime_scope,
        );
        let quality_status_context = self.resume_quality_status_context(&restored_quality_context);
        let runtime_state_seed = build_resume_runtime_state_seed(
            task_kind,
            batch_id,
            max_retries,
            active_story_repair_payload,
            restored_quality_context,
        );

        RestoredResumeRuntimeStateProjection {
            quality_status_context,
            request_runtime_state: restored_request_runtime_state,
            runtime_state_seed,
        }
    }

    pub(crate) fn restored_batch_runtime_compat_options(
        &self,
        base_compat_options: &SingleChapterGenerationCompatOptions,
    ) -> SingleChapterGenerationCompatOptions {
        if !self.has_workflow_runtime_state() {
            return base_compat_options.clone();
        }

        let restored_quality_context =
            self.restored_quality_runtime_context(BatchGenerationTaskKind::Batch);

        restore_story_repair_compat_options_from_active_snapshot(
            base_compat_options,
            self.explicit_story_repair_payload(),
            restored_quality_context.quality_metrics_summary.as_ref(),
            restored_quality_context.latest_quality_metrics.as_ref(),
        )
    }

    pub(crate) fn build_refreshed_runtime_state_preserving_quality(
        &self,
        refreshed_quality_summary: Option<&Value>,
    ) -> Option<Value> {
        self.has_workflow_runtime_state().then(|| {
            build_batch_generation_runtime_state_payload_preserving_quality_state(
                self.request_runtime_state(),
                self.explicit_story_repair_payload(),
                self.quality_metrics_summary_state(),
                self.quality_metrics_history(),
                self.quality_metrics_summary(),
                refreshed_quality_summary,
                self.latest_quality_metrics(),
            )
        })
    }

    pub(crate) fn build_current_chapter_quality_runtime_snapshot(
        &self,
        quality_summary: &Value,
        latest_quality_metrics: Option<&Value>,
    ) -> Value {
        build_batch_generation_runtime_state_payload_from_current_quality(
            self.request_runtime_state(),
            self.explicit_story_repair_payload(),
            self.quality_metrics_summary_state(),
            self.quality_metrics_history(),
            quality_summary,
            latest_quality_metrics,
        )
    }
}

pub(crate) fn restore_batch_generation_runtime_compat_options_from_persisted_runtime_context(
    base_compat_options: &SingleChapterGenerationCompatOptions,
    persisted_runtime_context: &BatchGenerationPersistedRuntimeContext,
) -> SingleChapterGenerationCompatOptions {
    persisted_runtime_context.restored_batch_runtime_compat_options(base_compat_options)
}

fn build_resume_runtime_state_seed(
    task_kind: BatchGenerationTaskKind,
    batch_id: &str,
    max_retries: i32,
    active_story_repair_payload: Option<Value>,
    restored_quality_context: BatchGenerationQualityRuntimeContext,
) -> Option<Value> {
    let mut runtime_state = serde_json::Map::from_iter([
        (
            "resume_from_batch_id".to_string(),
            json!(batch_id.to_string()),
        ),
        ("current_retry_count".to_string(), json!(0)),
        ("max_retries".to_string(), json!(max_retries)),
    ]);

    if let Some(payload) = active_story_repair_payload {
        runtime_state.insert("active_story_repair_payload".to_string(), payload);
    }
    match task_kind {
        BatchGenerationTaskKind::SingleChapter => {
            let quality_runtime_context = GenerationQualityRuntimeContext {
                latest_quality_metrics: restored_quality_context.latest_quality_metrics.clone(),
                quality_metrics_history: restored_quality_context.quality_metrics_history.clone(),
                quality_metrics_summary_state: restored_quality_context
                    .quality_metrics_summary_state
                    .clone(),
                quality_metrics_summary: restored_quality_context.quality_metrics_summary.clone(),
                quality_history_context: restored_quality_context.quality_history_context.clone(),
            };
            apply_generation_quality_runtime_context_to_payload(
                &mut runtime_state,
                quality_runtime_context,
                None,
                None,
                None,
            );
        }
        BatchGenerationTaskKind::Batch => {
            apply_batch_quality_runtime_context_to_payload(
                &mut runtime_state,
                restored_quality_context,
                None,
            );
        }
    }

    Some(Value::Object(runtime_state))
}
