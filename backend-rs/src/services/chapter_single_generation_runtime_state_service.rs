mod lifecycle_owner;

use chrono::{NaiveDateTime, Utc};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_analysis_runtime_service::build_chapter_analysis_runtime_owner_contract;
use crate::services::chapter_generation_runtime_service::build_single_generation_candidate_runtime_owner_contract;
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    manual_review_label_from_quality_context_with_retry_budget,
    retryable_repair_label_from_quality_context_with_retry_budget,
};
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::{
    build_chapter_generation_snapshot_owner_contract, upsert_chapter_generation_runtime_snapshot,
};
use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
use crate::services::chapter_single_generation_result_lifecycle_service::build_single_generation_result_lifecycle_owner_contract;

#[cfg(test)]
pub(crate) use self::lifecycle_owner::append_single_generation_failed_chapter_entry;
#[cfg(test)]
pub(crate) use self::lifecycle_owner::SingleGenerationRuntimeOutcome;
pub(crate) use self::lifecycle_owner::{
    SingleGenerationRuntimeLaunchInput, SingleGenerationRuntimeLifecyclePlan,
};
#[cfg(test)]
pub(crate) use crate::services::chapter_generation_execution_contract_service::build_prompt_overrides_from_compat_options;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SingleGenerationQualityGateTerminalState {
    pub(crate) checkpoint_payload: Value,
    pub(crate) error_message: String,
    pub(crate) failed_entry: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SingleGenerationFollowUpAnalysisDecision {
    pub(crate) manual_review_label: String,
    pub(crate) quality_metrics: Option<Value>,
}

pub(crate) fn build_single_generation_runtime_checkpoint_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_runtime_state_service::runtime_checkpoint_owner",
        "scope": "runtime_checkpoint_projection_candidate_gateway_metadata_and_persisted_stage_updates",
        "python_source_map": [
            "backend/app/services/chapter_generation/stream/service.py",
            "backend/app/services/chapter_generation/stream/finalize_service.py",
            "backend/app/services/chapter_generation/stream/candidate_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_single_generation_runtime_checkpoint_for_stage",
                "attach_single_generation_candidate_gateway_checkpoint_metadata",
                "build_single_generation_runtime_terminal_checkpoint_projection"
            ],
            "persistence_entrypoints": [
                "SingleGenerationTaskStage::persist_runtime_preparation",
                "SingleGenerationTaskStage::persist_with_checkpoint_payload"
            ],
            "checkpoint_fields": [
                "phase",
                "status",
                "progress",
                "chapter_id",
                "current_chapter_number",
                "word_count",
                "candidate_gateway"
            ]
        },
        "active_consumers": [
            "chapter_single_generation_runtime_state_service",
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_single_generation_active_gateway_smoke_service"
        ],
        "snapshot_persistence_owner_contract": build_chapter_generation_snapshot_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_single_generation_runtime_state_service",
            "cargo test chapter_single_generation_runtime_restore_workflow_service",
            "cargo test api::health",
            "cargo check"
        ]
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SingleGenerationSnapshotStage {
    Pending,
    Preparing,
    Generating,
    Finalizing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModelFieldUpdate<T> {
    Keep,
    Set(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskTimestampUpdate {
    Keep,
    Clear,
    Now,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SingleGenerationTaskStage {
    Preparing,
    Completed,
    Failed,
}

impl SingleGenerationSnapshotStage {
    fn build_checkpoint(
        self,
        chapter_id: &str,
        current_chapter_number: Option<i32>,
        word_count: Option<i32>,
    ) -> Value {
        let (phase, progress, status, last_event, last_message) = match self {
            SingleGenerationSnapshotStage::Pending => (
                "pending",
                0,
                "pending",
                "queued",
                "单章生成任务已创建，等待开始...",
            ),
            SingleGenerationSnapshotStage::Preparing => (
                "generating",
                15,
                "running",
                "chapter_start",
                "正在准备章节生成...",
            ),
            SingleGenerationSnapshotStage::Generating => {
                ("generating", 65, "running", "progress", "正在生成正文...")
            }
            SingleGenerationSnapshotStage::Finalizing => (
                "finalizing",
                95,
                "running",
                "progress",
                "正在整理生成结果...",
            ),
            SingleGenerationSnapshotStage::Completed => {
                ("completed", 100, "completed", "done", "章节生成完成")
            }
            SingleGenerationSnapshotStage::Failed => {
                ("failed", 100, "failed", "error", "章节生成失败")
            }
        };
        let mut checkpoint = json!({
            "phase": phase,
            "progress": progress.clamp(0, 100),
            "status": status,
            "last_event": last_event,
            "last_message": last_message,
            "chapter_id": chapter_id,
            "current_chapter_id": chapter_id,
            "current_chapter_number": current_chapter_number,
            "updated_at": Utc::now().to_rfc3339(),
        });
        if let Some(object) = checkpoint.as_object_mut() {
            if let Some(value) = word_count {
                object.insert("word_count".to_string(), json!(value.max(0)));
            }
        }

        checkpoint
    }
}

impl SingleGenerationTaskStage {
    pub(crate) fn status(self) -> &'static str {
        match self {
            SingleGenerationTaskStage::Preparing => "running",
            SingleGenerationTaskStage::Completed => "completed",
            SingleGenerationTaskStage::Failed => "failed",
        }
    }

    pub(crate) fn started_at_update(self) -> TaskTimestampUpdate {
        match self {
            SingleGenerationTaskStage::Preparing => TaskTimestampUpdate::Now,
            SingleGenerationTaskStage::Completed | SingleGenerationTaskStage::Failed => {
                TaskTimestampUpdate::Keep
            }
        }
    }

    pub(crate) fn completed_at_update(self) -> TaskTimestampUpdate {
        match self {
            SingleGenerationTaskStage::Preparing => TaskTimestampUpdate::Clear,
            SingleGenerationTaskStage::Completed | SingleGenerationTaskStage::Failed => {
                TaskTimestampUpdate::Now
            }
        }
    }

    pub(crate) fn completed_chapters_update(self) -> ModelFieldUpdate<i32> {
        match self {
            SingleGenerationTaskStage::Preparing | SingleGenerationTaskStage::Failed => {
                ModelFieldUpdate::Keep
            }
            SingleGenerationTaskStage::Completed => ModelFieldUpdate::Set(1),
        }
    }

    pub(crate) fn current_retry_count_update(self) -> ModelFieldUpdate<i32> {
        match self {
            SingleGenerationTaskStage::Preparing => ModelFieldUpdate::Set(0),
            SingleGenerationTaskStage::Completed | SingleGenerationTaskStage::Failed => {
                ModelFieldUpdate::Keep
            }
        }
    }

    pub(crate) fn current_chapter_id_update(
        self,
        chapter_id: &str,
    ) -> ModelFieldUpdate<Option<String>> {
        match self {
            SingleGenerationTaskStage::Preparing | SingleGenerationTaskStage::Completed => {
                ModelFieldUpdate::Set(Some(chapter_id.to_string()))
            }
            SingleGenerationTaskStage::Failed => ModelFieldUpdate::Keep,
        }
    }

    pub(crate) fn current_chapter_number_update(
        self,
        chapter_number: Option<i32>,
    ) -> ModelFieldUpdate<Option<i32>> {
        match self {
            SingleGenerationTaskStage::Preparing | SingleGenerationTaskStage::Failed => {
                ModelFieldUpdate::Keep
            }
            SingleGenerationTaskStage::Completed => ModelFieldUpdate::Set(chapter_number),
        }
    }

    pub(crate) async fn persist_for_task(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        chapter_id: &str,
        chapter_number: Option<i32>,
        error_message: Option<String>,
        now: NaiveDateTime,
    ) -> Result<(), String> {
        if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?
        {
            let mut active: batch_generation_task::ActiveModel = task_model.into();
            self.apply_to_active_model(&mut active, chapter_id, chapter_number, error_message, now);
            active.update(db).await.map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    pub(crate) fn apply_to_active_model(
        self,
        active: &mut batch_generation_task::ActiveModel,
        chapter_id: &str,
        chapter_number: Option<i32>,
        error_message: Option<String>,
        now: NaiveDateTime,
    ) {
        active.status = Set(self.status().to_string());

        match self.started_at_update() {
            TaskTimestampUpdate::Keep => {}
            TaskTimestampUpdate::Clear => active.started_at = Set(None),
            TaskTimestampUpdate::Now => active.started_at = Set(Some(now)),
        }

        match self.completed_at_update() {
            TaskTimestampUpdate::Keep => {}
            TaskTimestampUpdate::Clear => active.completed_at = Set(None),
            TaskTimestampUpdate::Now => active.completed_at = Set(Some(now)),
        }

        active.error_message = Set(match self {
            SingleGenerationTaskStage::Preparing | SingleGenerationTaskStage::Completed => None,
            SingleGenerationTaskStage::Failed => error_message,
        });

        match self.completed_chapters_update() {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.completed_chapters = Set(value),
        }

        match self.current_retry_count_update() {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_retry_count = Set(value),
        }

        match self.current_chapter_id_update(chapter_id) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_chapter_id = Set(value),
        }

        match self.current_chapter_number_update(chapter_number) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_chapter_number = Set(value),
        }
    }

    pub(crate) async fn persist_runtime_preparation(
        db: &DatabaseConnection,
        task_id: &str,
        chapter_id: &str,
    ) -> Result<(), String> {
        let now = Utc::now().naive_utc();
        Self::Preparing
            .persist_for_task(db, task_id, chapter_id, None, None, now)
            .await?;

        upsert_chapter_generation_runtime_snapshot(
            db,
            task_id,
            build_single_generation_runtime_checkpoint_for_stage(
                SingleGenerationSnapshotStage::Preparing,
                chapter_id,
                None,
                None,
            ),
            Utc::now().naive_utc(),
        )
        .await?;
        upsert_chapter_generation_runtime_snapshot(
            db,
            task_id,
            build_single_generation_runtime_checkpoint_for_stage(
                SingleGenerationSnapshotStage::Generating,
                chapter_id,
                None,
                None,
            ),
            Utc::now().naive_utc(),
        )
        .await
    }

    pub(crate) async fn persist_with_checkpoint_payload(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        chapter_id: &str,
        chapter_number: Option<i32>,
        error_message: Option<String>,
        checkpoint_payload: Value,
    ) -> Result<(), String> {
        let now = Utc::now().naive_utc();
        self.persist_for_task(
            db,
            task_id,
            chapter_id,
            chapter_number,
            error_message.clone(),
            now,
        )
        .await?;

        upsert_chapter_generation_runtime_snapshot(
            db,
            task_id,
            checkpoint_payload,
            Utc::now().naive_utc(),
        )
        .await
    }
}

pub(crate) fn build_single_generation_runtime_checkpoint_for_stage(
    stage: SingleGenerationSnapshotStage,
    chapter_id: &str,
    current_chapter_number: Option<i32>,
    word_count: Option<i32>,
) -> Value {
    stage.build_checkpoint(chapter_id, current_chapter_number, word_count)
}

pub(crate) fn attach_single_generation_candidate_gateway_checkpoint_metadata(
    mut checkpoint_payload: Value,
    generated_result: &GeneratedChapterResult,
) -> Value {
    if let (Some(object), Some(candidate_gateway_metadata)) = (
        checkpoint_payload.as_object_mut(),
        generated_result.candidate_gateway_metadata.as_ref(),
    ) {
        object.insert(
            "candidate_gateway".to_string(),
            candidate_gateway_metadata.clone(),
        );
    }

    checkpoint_payload
}

pub(crate) fn build_single_generation_runtime_terminal_checkpoint_projection(
    stage: SingleGenerationSnapshotStage,
    chapter_id: &str,
    chapter_number: Option<i32>,
    word_count: Option<i32>,
    extra_payload: Option<Value>,
    generated_result: Option<&GeneratedChapterResult>,
) -> Value {
    let base_checkpoint = build_single_generation_runtime_checkpoint_for_stage(
        stage,
        chapter_id,
        chapter_number,
        word_count,
    );
    let checkpoint_payload = match extra_payload {
        Some(payload) => {
            merge_single_generation_terminal_checkpoint_payload(base_checkpoint, payload)
        }
        None => base_checkpoint,
    };

    match generated_result {
        Some(result) => attach_single_generation_candidate_gateway_checkpoint_metadata(
            checkpoint_payload,
            result,
        ),
        None => checkpoint_payload,
    }
}

fn merge_single_generation_terminal_checkpoint_payload(
    base_checkpoint: Value,
    extra_payload: Value,
) -> Value {
    match (base_checkpoint, extra_payload) {
        (Value::Object(mut base), Value::Object(extra)) => {
            for (key, value) in extra {
                base.insert(key, value);
            }
            Value::Object(base)
        }
        (_, extra) => extra,
    }
}

pub(crate) fn build_single_generation_runtime_state_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_runtime_state_service",
        "scope": "single_generation_runtime_lifecycle_candidate_gateway_checkpoint_terminal_follow_up_analysis_and_task_persistence",
        "python_source_map": [
            "backend/app/api/chapter_generation_routes.py",
            "backend/app/api/chapters.py",
            "backend/app/services/chapter_generation/stream/service.py",
            "backend/app/services/chapter_generation/stream/execution_service.py",
            "backend/app/services/chapter_generation/stream/finalize_service.py",
            "backend/app/services/chapter_generation/stream/candidate_service.py",
            "backend/app/services/manual_chapter_analysis_execution_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service.rs",
            "backend-rs/src/services/chapter_single_generation_result_lifecycle_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/snapshot_persistence_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "runtime_lifecycle_entrypoints": [
                "SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config",
                "SingleGenerationRuntimeLifecyclePlan::spawn",
                "SingleGenerationRuntimeOutcome::run"
            ],
            "runtime_generation_entrypoints": [
                "SingleGenerationRuntimeLaunchInput::execute_generation_with_gateway_config",
                "generate_and_persist_chapter_content_with_candidate_route_gateway"
            ],
            "checkpoint_entrypoints": [
                "build_single_generation_runtime_checkpoint_for_stage",
                "build_single_generation_runtime_terminal_checkpoint_projection",
                "attach_single_generation_candidate_gateway_checkpoint_metadata"
            ],
            "terminal_state_entrypoints": [
                "resolve_single_generation_quality_gate_terminal_state",
                "build_single_generation_error_terminal_state",
                "SingleGenerationTaskStage::apply_to_active_model"
            ],
            "follow_up_analysis_entrypoints": [
                "SingleGenerationRuntimeOutcome::run_follow_up_analysis",
                "resolve_single_generation_manual_review_label_from_analysis_payload",
                "analyze_generated_chapter_follow_up"
            ],
            "gateway_config": [
                "ChapterCandidateRouteGatewayConfig",
                "runtime lifecycle receives route/AppConfig supplied gateway config",
                "default_single_generation_candidate_gateway_config is test-only rollback/source-map helper"
            ],
            "task_stage_statuses": [
                "pending",
                "running",
                "completed",
                "failed"
            ],
            "terminal_phases": [
                "completed",
                "failed",
                "quality_blocked",
                "quality_retry"
            ]
        },
        "active_consumers": [
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_single_generation_stream_workflow_service",
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_single_generation_active_gateway_smoke_service",
            "chapter_generation_routes"
        ],
        "shared_candidate_runtime_owner_contract": build_single_generation_candidate_runtime_owner_contract(),
        "result_lifecycle_owner_contract": build_single_generation_result_lifecycle_owner_contract(),
        "checkpoint_owner_contract": build_single_generation_runtime_checkpoint_owner_contract(),
        "terminal_state_owner_contract": build_single_generation_terminal_state_owner_contract(),
        "analysis_runtime_owner_contract": build_chapter_analysis_runtime_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_single_generation_runtime_state_service",
            "cargo test chapter_single_generation_runtime_restore_workflow_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-single-generation-owner",
            "runtime_checkpoint_owner": "build_single_generation_runtime_checkpoint_for_stage",
            "terminal_state_owner": "resolve_single_generation_quality_gate_terminal_state",
            "follow_up_analysis_owner": "SingleGenerationRuntimeOutcome::run_follow_up_analysis",
            "manifest_probe_count": 6,
            "rust_manifest_probe_count": 6,
            "python_fallback_probe_count": 0,
            "source_map_closeout_ready": true,
            "remaining_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
            "status": "rust_runtime_state_owner_ready_for_source_map_closeout_review"
        },
        "rollback_boundary": {
            "runtime_knobs": [
                "legacy_single_generation_direct_ai",
                "python_candidate_executor_fallback"
            ],
            "source_map_policy": "keep_python_single_generation_runtime_and_analysis_shells_as_source_map_until_explicit_freeze_delete_round",
            "python_fallback_removal_ready": true,
            "rollback_files": [
                "backend/app/api/chapter_generation_routes.py",
                "backend/app/api/chapters.py",
                "backend/app/services/chapter_generation/stream/service.py",
                "backend/app/services/chapter_generation/stream/finalize_service.py",
                "backend/app/services/manual_chapter_analysis_execution_service.py"
            ]
        }
    })
}

pub(crate) fn build_single_generation_terminal_state_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_runtime_state_service::terminal_state_owner",
        "scope": "quality_gate_terminal_state_manual_review_retry_error_projection_and_follow_up_decision",
        "python_source_map": [
            "backend/app/services/chapter_generation/stream/finalize_service.py",
            "backend/app/services/manual_chapter_analysis_execution_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "resolve_single_generation_manual_review_label_from_analysis_payload",
                "resolve_single_generation_quality_gate_terminal_state",
                "build_single_generation_error_terminal_state"
            ],
            "terminal_state_fields": [
                "checkpoint_payload",
                "error_message",
                "failed_entry"
            ],
            "manual_review_contract": [
                "manual review label follows quality context and retry budget",
                "analysis payload quality_metrics may override generated result quality_metrics"
            ]
        },
        "active_consumers": [
            "chapter_single_generation_runtime_state_service",
            "chapter_single_generation_active_gateway_smoke_service",
            "chapter_generation_routes"
        ],
        "validation_boundary": [
            "cargo test chapter_single_generation_runtime_state_service",
            "cargo test api::health",
            "cargo check"
        ]
    })
}

pub(crate) fn resolve_single_generation_manual_review_label_from_analysis_payload(
    payload: &Value,
) -> Option<String> {
    let quality_metrics = payload.get("quality_metrics");
    manual_review_label_from_quality_context_with_retry_budget(
        None,
        quality_metrics,
        quality_metrics,
        0,
        0,
    )
}

pub(crate) fn resolve_single_generation_quality_gate_terminal_state(
    persisted_task: &Option<batch_generation_task::Model>,
    generated_result: &GeneratedChapterResult,
    analysis_decision: Option<&SingleGenerationFollowUpAnalysisDecision>,
) -> Option<SingleGenerationQualityGateTerminalState> {
    let current_retry_count = persisted_task
        .as_ref()
        .map(|task| task.current_retry_count)
        .unwrap_or(0);
    let max_retries = persisted_task
        .as_ref()
        .map(|task| task.max_retries)
        .unwrap_or(0);
    let quality_metrics = analysis_decision
        .and_then(|decision| decision.quality_metrics.as_ref())
        .or(generated_result.quality_metrics.as_ref());

    let manual_review_label = analysis_decision
        .map(|decision| decision.manual_review_label.clone())
        .or_else(|| {
            resolve_single_generation_manual_review_label_from_quality_context(
                generated_result,
                quality_metrics,
                current_retry_count,
                max_retries,
            )
        });
    if let Some(label) = manual_review_label {
        return Some(build_single_generation_manual_review_terminal_state(
            persisted_task,
            generated_result,
            &label,
            quality_metrics,
        ));
    }

    if generated_result_requires_retry_follow_up(generated_result) {
        let retry_label = resolve_single_generation_retry_terminal_label(
            generated_result,
            quality_metrics,
            current_retry_count,
            max_retries,
        );
        return Some(build_single_generation_retry_terminal_state(
            persisted_task,
            generated_result,
            retry_label.as_deref(),
            quality_metrics,
        ));
    }

    None
}

pub(crate) fn build_single_generation_error_terminal_state(
    persisted_task: &Option<batch_generation_task::Model>,
    chapter_id: &str,
    chapter_number: Option<i32>,
    chapter_title: Option<&str>,
    error_message: &str,
) -> SingleGenerationQualityGateTerminalState {
    let failed_entry = build_single_generation_failed_chapter_entry(
        Some(chapter_id),
        chapter_number,
        chapter_title,
        error_message,
        persisted_task
            .as_ref()
            .map(|task| task.current_retry_count)
            .unwrap_or(0),
    );

    SingleGenerationQualityGateTerminalState {
        checkpoint_payload: json!({
            "analysis_task_message": Value::Null,
            "analysis_task_progress": 100,
            "analysis_last_error": error_message,
            "phase": "failed",
        }),
        error_message: error_message.to_string(),
        failed_entry,
    }
}

fn resolve_single_generation_manual_review_label_from_quality_context(
    generated_result: &GeneratedChapterResult,
    quality_metrics: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    if matches!(
        generated_result.quality_gate_action.as_deref(),
        Some("manual_review")
    ) {
        return generated_result
            .quality_gate_message
            .clone()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                manual_review_label_from_quality_context_with_retry_budget(
                    None,
                    quality_metrics,
                    quality_metrics,
                    current_retry_count,
                    max_retries,
                )
            });
    }

    manual_review_label_from_quality_context_with_retry_budget(
        None,
        quality_metrics,
        quality_metrics,
        current_retry_count,
        max_retries,
    )
}

fn generated_result_requires_retry_follow_up(generated_result: &GeneratedChapterResult) -> bool {
    matches!(
        generated_result.quality_gate_action.as_deref(),
        Some("retry")
    ) || generated_result.provisional_draft_saved
        || (!generated_result.content_applied && generated_result.attempt_state.trim() == "retry")
}

fn resolve_single_generation_retry_terminal_label(
    generated_result: &GeneratedChapterResult,
    quality_metrics: Option<&Value>,
    current_retry_count: i32,
    max_retries: i32,
) -> Option<String> {
    generated_result
        .quality_gate_message
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            retryable_repair_label_from_quality_context_with_retry_budget(
                None,
                quality_metrics,
                quality_metrics,
                current_retry_count,
                max_retries,
            )
        })
        .or_else(|| Some("可自动修复后重试".to_string()))
}

fn build_single_generation_failed_chapter_entry(
    chapter_id: Option<&str>,
    chapter_number: Option<i32>,
    chapter_title: Option<&str>,
    error_message: &str,
    retry_count: i32,
) -> Value {
    json!({
        "chapter_id": chapter_id,
        "chapter_number": chapter_number,
        "title": chapter_title,
        "error": error_message,
        "retry_count": retry_count.max(0),
    })
}

fn apply_single_generation_quality_gate_terminal_fields(
    entry: &mut Value,
    decision: &str,
    label: &str,
    phase: &str,
    quality_metrics: Option<&Value>,
) {
    let failed_metric_labels = quality_metrics
        .and_then(|metrics| metrics.get("quality_gate"))
        .and_then(|gate| gate.get("failed_metrics"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("label").and_then(Value::as_str))
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Some(object) = entry.as_object_mut() {
        object.insert("phase".to_string(), json!(phase));
        object.insert("quality_gate_status".to_string(), json!("failed"));
        object.insert("quality_gate_decision".to_string(), json!(decision));
        object.insert("quality_gate_label".to_string(), json!(label));
        object.insert(
            "quality_gate_failed_metrics".to_string(),
            json!(failed_metric_labels),
        );
    }
}

fn build_single_generation_manual_review_terminal_state(
    persisted_task: &Option<batch_generation_task::Model>,
    generated_result: &GeneratedChapterResult,
    manual_review_label: &str,
    quality_metrics: Option<&Value>,
) -> SingleGenerationQualityGateTerminalState {
    let error_message = format!("章节触发质量门禁，需人工复核: {manual_review_label}");
    let mut failed_entry = build_single_generation_failed_chapter_entry(
        Some(&generated_result.chapter_id),
        Some(generated_result.chapter_number),
        Some(&generated_result.title),
        &error_message,
        persisted_task
            .as_ref()
            .map(|task| task.current_retry_count)
            .unwrap_or(0),
    );
    apply_single_generation_quality_gate_terminal_fields(
        &mut failed_entry,
        "manual_review",
        manual_review_label,
        "quality_blocked",
        quality_metrics,
    );

    SingleGenerationQualityGateTerminalState {
        checkpoint_payload: json!({
            "analysis_task_message": "单章生成触发质量门禁，需人工复核",
            "analysis_task_progress": 100,
            "analysis_last_error": Value::Null,
            "quality_gate_decision": "manual_review",
            "quality_gate_label": manual_review_label,
            "phase": "quality_blocked",
        }),
        error_message,
        failed_entry,
    }
}

fn build_single_generation_retry_terminal_state(
    persisted_task: &Option<batch_generation_task::Model>,
    generated_result: &GeneratedChapterResult,
    retry_label: Option<&str>,
    quality_metrics: Option<&Value>,
) -> SingleGenerationQualityGateTerminalState {
    let retry_label = retry_label
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("可自动修复后重试");
    let error_message = format!("章节触发质量修复重试: {retry_label}");
    let mut failed_entry = build_single_generation_failed_chapter_entry(
        Some(&generated_result.chapter_id),
        Some(generated_result.chapter_number),
        Some(&generated_result.title),
        &error_message,
        persisted_task
            .as_ref()
            .map(|task| task.current_retry_count)
            .unwrap_or(0),
    );
    apply_single_generation_quality_gate_terminal_fields(
        &mut failed_entry,
        "auto_repair",
        retry_label,
        "quality_retry",
        quality_metrics,
    );

    SingleGenerationQualityGateTerminalState {
        checkpoint_payload: json!({
            "analysis_task_message": "单章生成已保存修复草稿，等待后续重试",
            "analysis_task_progress": 100,
            "analysis_last_error": Value::Null,
            "quality_gate_decision": "auto_repair",
            "quality_gate_label": retry_label,
            "phase": "quality_retry",
        }),
        error_message,
        failed_entry,
    }
}

#[cfg(test)]
mod tests {
    use crate::ai::AIConfig;
    use chrono::NaiveDate;
    use sea_orm::Set;
    use serde_json::json;

    use super::{
        append_single_generation_failed_chapter_entry,
        build_chapter_analysis_runtime_owner_contract,
        build_chapter_generation_snapshot_owner_contract,
        build_prompt_overrides_from_compat_options,
        build_single_generation_candidate_runtime_owner_contract,
        build_single_generation_error_terminal_state,
        build_single_generation_runtime_checkpoint_owner_contract,
        build_single_generation_runtime_state_owner_contract,
        build_single_generation_terminal_state_owner_contract,
        merge_single_generation_terminal_checkpoint_payload,
        resolve_single_generation_manual_review_label_from_analysis_payload,
        resolve_single_generation_quality_gate_terminal_state, SingleGenerationRuntimeLaunchInput,
        SingleGenerationRuntimeLifecyclePlan, SingleGenerationRuntimeOutcome,
    };
    use crate::models::batch_generation_task;
    use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
    use crate::services::chapter_generation_execution_contract_service::{
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
    };
    use crate::services::chapter_generation_prompt_service::PromptContextProviderPayload;
    use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
    use crate::services::chapter_single_generation_runtime_state_service::{
        attach_single_generation_candidate_gateway_checkpoint_metadata,
        build_single_generation_runtime_checkpoint_for_stage,
        build_single_generation_runtime_terminal_checkpoint_projection, ModelFieldUpdate,
        SingleGenerationSnapshotStage, SingleGenerationTaskStage, TaskTimestampUpdate,
    };
    fn empty_compat_options() -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: None,
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: false,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: Vec::new(),
            story_preserve_strengths: Vec::new(),
        }
    }

    #[test]
    fn should_publish_single_generation_runtime_state_owner_contract() {
        let contract = build_single_generation_runtime_state_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_runtime_state_service"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/api/chapter_generation_routes.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_single_generation_runtime_state_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_lifecycle_entrypoints"][0],
            "SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config"
        );
        assert_eq!(
            contract["behavior_contract"]["checkpoint_entrypoints"][2],
            "attach_single_generation_candidate_gateway_checkpoint_metadata"
        );
        assert_eq!(
            contract["behavior_contract"]["terminal_state_entrypoints"][0],
            "resolve_single_generation_quality_gate_terminal_state"
        );
        assert_eq!(
            contract["behavior_contract"]["follow_up_analysis_entrypoints"][2],
            "analyze_generated_chapter_follow_up"
        );
        assert_eq!(
            contract["active_consumers"][3],
            "chapter_single_generation_active_gateway_smoke_service"
        );
        assert_eq!(
            contract["shared_candidate_runtime_owner_contract"]["owner"],
            build_single_generation_candidate_runtime_owner_contract()["owner"]
        );
        assert_eq!(
            contract["checkpoint_owner_contract"]["owner"],
            build_single_generation_runtime_checkpoint_owner_contract()["owner"]
        );
        assert_eq!(
            contract["terminal_state_owner_contract"]["owner"],
            build_single_generation_terminal_state_owner_contract()["owner"]
        );
        assert_eq!(
            contract["analysis_runtime_owner_contract"]["owner"],
            build_chapter_analysis_runtime_owner_contract()["owner"]
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["runtime_checkpoint_owner"],
            "build_single_generation_runtime_checkpoint_for_stage"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["terminal_state_owner"],
            "resolve_single_generation_quality_gate_terminal_state"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["follow_up_analysis_owner"],
            "SingleGenerationRuntimeOutcome::run_follow_up_analysis"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            json!(6)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            json!(0)
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_runtime_state_owner_ready_for_source_map_closeout_review"
        );
    }

    #[test]
    fn should_publish_single_generation_runtime_checkpoint_owner_contract() {
        let contract = build_single_generation_runtime_checkpoint_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_runtime_state_service::runtime_checkpoint_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][0],
            "build_single_generation_runtime_checkpoint_for_stage"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][2],
            "build_single_generation_runtime_terminal_checkpoint_projection"
        );
        assert_eq!(
            contract["behavior_contract"]["checkpoint_fields"][6],
            "candidate_gateway"
        );
        assert_eq!(
            contract["snapshot_persistence_owner_contract"]["owner"],
            build_chapter_generation_snapshot_owner_contract()["owner"]
        );
    }

    #[test]
    fn should_publish_single_generation_terminal_state_owner_contract() {
        let contract = build_single_generation_terminal_state_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_single_generation_runtime_state_service::terminal_state_owner"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][0],
            "resolve_single_generation_manual_review_label_from_analysis_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][2],
            "build_single_generation_error_terminal_state"
        );
        assert_eq!(
            contract["behavior_contract"]["terminal_state_fields"][2],
            "failed_entry"
        );
    }

    fn build_task(status: &str) -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 1,
            chapter_ids: json!(["chapter-1"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: status.to_string(),
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(1),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    fn build_terminal_task() -> batch_generation_task::Model {
        batch_generation_task::Model {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 1,
            chapter_ids: json!(["chapter-1"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: "running".to_string(),
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-1".to_string()),
            current_chapter_number: Some(1),
            current_retry_count: 1,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    #[test]
    fn should_resolve_single_generation_manual_review_label_from_analysis_payload() {
        let label = resolve_single_generation_manual_review_label_from_analysis_payload(&json!({
            "quality_metrics": {
                "quality_gate": {
                    "decision": "manual_review",
                    "label": "需要人工复核"
                }
            }
        }));

        assert_eq!(label.as_deref(), Some("需要人工复核"));
    }

    #[test]
    fn should_build_manual_review_terminal_state_from_quality_context() {
        let result = GeneratedChapterResult {
            chapter_id: "chapter-1".to_string(),
            chapter_number: 1,
            title: "第一章".to_string(),
            quality_gate_action: Some("manual_review".to_string()),
            quality_gate_message: Some("连续性需人工复核".to_string()),
            ..Default::default()
        };

        let terminal = resolve_single_generation_quality_gate_terminal_state(
            &Some(build_terminal_task()),
            &result,
            None,
        )
        .expect("manual review terminal");

        assert_eq!(
            terminal.checkpoint_payload["quality_gate_decision"],
            "manual_review"
        );
        assert_eq!(
            terminal.checkpoint_payload["quality_gate_label"],
            "连续性需人工复核"
        );
        assert_eq!(
            terminal.failed_entry["quality_gate_decision"],
            "manual_review"
        );
        assert_eq!(terminal.failed_entry["phase"], "quality_blocked");
        assert!(terminal.error_message.contains("需人工复核"));
    }

    #[test]
    fn should_build_retry_terminal_state_from_generated_retry_result() {
        let result = GeneratedChapterResult {
            chapter_id: "chapter-2".to_string(),
            chapter_number: 2,
            title: "第二章".to_string(),
            content_applied: false,
            provisional_draft_saved: true,
            attempt_state: "retry".to_string(),
            quality_gate_action: Some("retry".to_string()),
            quality_gate_message: Some("建议自动修复".to_string()),
            quality_metrics: Some(json!({
                "quality_gate": {
                    "failed_metrics": [{"label": "节奏"}]
                }
            })),
            ..Default::default()
        };

        let terminal = resolve_single_generation_quality_gate_terminal_state(
            &Some(build_terminal_task()),
            &result,
            None,
        )
        .expect("retry terminal");

        assert_eq!(
            terminal.checkpoint_payload["quality_gate_decision"],
            "auto_repair"
        );
        assert_eq!(terminal.checkpoint_payload["phase"], "quality_retry");
        assert_eq!(
            terminal.failed_entry["quality_gate_decision"],
            "auto_repair"
        );
        assert_eq!(
            terminal.failed_entry["quality_gate_failed_metrics"][0],
            "节奏"
        );
        assert!(terminal.error_message.contains("质量修复重试"));
    }

    #[test]
    fn should_build_error_terminal_state_for_runtime_failure() {
        let terminal = build_single_generation_error_terminal_state(
            &Some(build_terminal_task()),
            "chapter-3",
            Some(3),
            Some("第三章"),
            "generation failed",
        );

        assert_eq!(terminal.checkpoint_payload["phase"], "failed");
        assert_eq!(
            terminal.checkpoint_payload["analysis_last_error"],
            "generation failed"
        );
        assert_eq!(terminal.failed_entry["chapter_id"], "chapter-3");
        assert_eq!(terminal.failed_entry["retry_count"], 1);
    }

    #[test]
    fn should_merge_terminal_checkpoint_payload() {
        let merged = merge_single_generation_terminal_checkpoint_payload(
            json!({"phase": "failed", "status": "failed"}),
            json!({"quality_gate_decision": "manual_review"}),
        );

        assert_eq!(merged["phase"], "failed");
        assert_eq!(merged["status"], "failed");
        assert_eq!(merged["quality_gate_decision"], "manual_review");
    }

    #[test]
    fn should_append_single_generation_failed_chapter_entry() {
        let payload = append_single_generation_failed_chapter_entry(
            &json!([{"chapter_id": "old"}]),
            Some(&json!({"chapter_id": "new"})),
        );

        assert_eq!(payload.as_array().expect("array").len(), 2);
        assert_eq!(payload[1]["chapter_id"], "new");
    }

    #[test]
    fn should_resolve_single_generation_task_stage_mutation_contracts() {
        let preparing = SingleGenerationTaskStage::Preparing;
        assert_eq!(preparing.status(), "running");
        assert!(matches!(
            preparing.started_at_update(),
            TaskTimestampUpdate::Now
        ));
        assert!(matches!(
            preparing.completed_at_update(),
            TaskTimestampUpdate::Clear
        ));
        assert!(matches!(
            preparing.current_retry_count_update(),
            ModelFieldUpdate::Set(0)
        ));
        assert!(matches!(
            preparing.current_chapter_id_update("chapter-1"),
            ModelFieldUpdate::Set(Some(ref id)) if id == "chapter-1"
        ));

        let completed = SingleGenerationTaskStage::Completed;
        assert_eq!(completed.status(), "completed");
        assert!(matches!(
            completed.completed_at_update(),
            TaskTimestampUpdate::Now
        ));
        assert!(matches!(
            completed.completed_chapters_update(),
            ModelFieldUpdate::Set(1)
        ));
        assert!(matches!(
            completed.current_chapter_number_update(Some(2)),
            ModelFieldUpdate::Set(Some(2))
        ));

        let failed = SingleGenerationTaskStage::Failed;
        assert_eq!(failed.status(), "failed");
        assert!(matches!(
            failed.completed_at_update(),
            TaskTimestampUpdate::Now
        ));
        assert!(matches!(
            failed.current_chapter_id_update("chapter-3"),
            ModelFieldUpdate::Keep
        ));
    }

    #[test]
    fn should_apply_single_generation_task_mutation_plan() {
        let now = NaiveDate::from_ymd_opt(2026, 5, 21)
            .expect("valid date")
            .and_hms_opt(0, 20, 0)
            .expect("valid time");
        let mut active: batch_generation_task::ActiveModel = build_task("pending").into();

        SingleGenerationTaskStage::Completed.apply_to_active_model(
            &mut active,
            "chapter-8",
            Some(8),
            None,
            now,
        );

        assert_eq!(active.status, Set("completed".to_string()));
        assert_eq!(active.completed_at, Set(Some(now)));
        assert_eq!(active.error_message, Set(None));
        assert_eq!(active.completed_chapters, Set(1));
        assert_eq!(
            active.current_chapter_id,
            Set(Some("chapter-8".to_string()))
        );
        assert_eq!(active.current_chapter_number, Set(Some(8)));
    }

    #[test]
    fn should_build_single_generation_runtime_launch_input_contract() {
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-7".to_string(),
            user_id: "user-7".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2400,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: String::new(),
                        previous_chapter_summary: String::new(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                },
            },
        };

        assert_eq!(runtime_input.chapter_id, "chapter-7");
        assert_eq!(runtime_input.user_id, "user-7");
        assert_eq!(runtime_input.execution_input.target_word_count, 2400);
        assert!(runtime_input
            .execution_input
            .compat_options
            .enable_analysis());
        assert_eq!(
            runtime_input
                .execution_input
                .execution_config
                .provider_payload
                .characters_info,
            "[]"
        );
    }

    #[test]
    fn should_keep_single_generation_runtime_lifecycle_gateway_config_from_route() {
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-gateway".to_string(),
            user_id: "user-gateway".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2600,
                compat_options: empty_compat_options(),
                execution_config: crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: String::new(),
                        previous_chapter_summary: String::new(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                },
            },
        };
        let gateway_config = ChapterCandidateRouteGatewayConfig {
            rust_executor_enabled: true,
            fallback_on_rust_error: false,
            disabled_reason: Some("active route supplied rust gateway".to_string()),
            rollback_boundary: "active_route_gateway".to_string(),
        };

        let lifecycle =
            SingleGenerationRuntimeLifecyclePlan::from_runtime_launch_with_gateway_config(
                "task-gateway".to_string(),
                runtime_input,
                gateway_config.clone(),
            );

        assert_eq!(lifecycle.candidate_gateway_config, gateway_config);
    }

    #[test]
    fn should_keep_single_generation_runtime_persistence_contract_for_stage_owner() {
        assert_eq!(
            SingleGenerationSnapshotStage::Finalizing,
            SingleGenerationSnapshotStage::Finalizing
        );
        assert_eq!(
            SingleGenerationSnapshotStage::Completed,
            SingleGenerationSnapshotStage::Completed
        );
        assert_eq!(
            SingleGenerationSnapshotStage::Failed,
            SingleGenerationSnapshotStage::Failed
        );
        let completed_stage = SingleGenerationTaskStage::Completed;
        let failed_stage = SingleGenerationTaskStage::Failed;

        assert_eq!(completed_stage.status(), "completed");
        assert_eq!(failed_stage.status(), "failed");
    }

    #[test]
    fn should_keep_single_generation_runtime_preparation_persist_contract() {
        let chapter_id = "chapter-7";
        let preparing_checkpoint = build_single_generation_runtime_checkpoint_for_stage(
            SingleGenerationSnapshotStage::Preparing,
            chapter_id,
            None,
            None,
        );
        let generating_checkpoint = build_single_generation_runtime_checkpoint_for_stage(
            SingleGenerationSnapshotStage::Generating,
            chapter_id,
            None,
            None,
        );

        assert_eq!(preparing_checkpoint["phase"], "generating");
        assert_eq!(preparing_checkpoint["status"], "running");
        assert_eq!(preparing_checkpoint["progress"], 15);
        assert_eq!(preparing_checkpoint["current_chapter_id"], chapter_id);
        assert_eq!(generating_checkpoint["phase"], "generating");
        assert_eq!(generating_checkpoint["status"], "running");
        assert_eq!(generating_checkpoint["progress"], 65);
        assert_eq!(generating_checkpoint["current_chapter_id"], chapter_id);
    }

    #[test]
    fn should_attach_candidate_gateway_metadata_to_single_generation_runtime_checkpoint() {
        let checkpoint = build_single_generation_runtime_checkpoint_for_stage(
            SingleGenerationSnapshotStage::Completed,
            "chapter-9",
            Some(9),
            Some(2600),
        );
        let result = GeneratedChapterResult {
            chapter_id: "chapter-9".to_string(),
            chapter_number: 9,
            title: "第九章".to_string(),
            content: "正文".to_string(),
            word_count: 2600,
            candidate_gateway_metadata: Some(json!({
                "execution_path": "rust_candidate_executor",
                "fallback_applied": false,
                "fallback_reason": "rust executor completed",
                "rollback_boundary": "legacy_single_generation_direct_ai",
                "rust_error": null
            })),
            ..Default::default()
        };

        let checkpoint =
            attach_single_generation_candidate_gateway_checkpoint_metadata(checkpoint, &result);

        assert_eq!(checkpoint["phase"], "completed");
        assert_eq!(
            checkpoint["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(checkpoint["candidate_gateway"]["fallback_applied"], false);
        assert_eq!(
            checkpoint["candidate_gateway"]["rollback_boundary"],
            "legacy_single_generation_direct_ai"
        );
    }

    #[test]
    fn should_leave_single_generation_runtime_checkpoint_unchanged_without_candidate_gateway_metadata(
    ) {
        let checkpoint = build_single_generation_runtime_checkpoint_for_stage(
            SingleGenerationSnapshotStage::Finalizing,
            "chapter-10",
            Some(10),
            Some(1800),
        );
        let result = GeneratedChapterResult {
            chapter_id: "chapter-10".to_string(),
            chapter_number: 10,
            title: "第十章".to_string(),
            content: "正文".to_string(),
            word_count: 1800,
            candidate_gateway_metadata: None,
            ..Default::default()
        };

        let checkpoint =
            attach_single_generation_candidate_gateway_checkpoint_metadata(checkpoint, &result);

        assert_eq!(checkpoint["phase"], "finalizing");
        assert!(checkpoint.get("candidate_gateway").is_none());
    }

    #[test]
    fn should_project_single_generation_terminal_checkpoint_owner_contract() {
        let generated_result = GeneratedChapterResult {
            chapter_id: "chapter-11".to_string(),
            chapter_number: 11,
            title: "第十一章".to_string(),
            content: "正文".to_string(),
            word_count: 3120,
            candidate_gateway_metadata: Some(json!({
                "execution_path": "rust_candidate_executor",
                "fallback_applied": false,
                "fallback_reason": null,
                "rollback_boundary": "legacy_single_generation_direct_ai",
                "rust_error": null
            })),
            ..Default::default()
        };

        let completed_checkpoint = build_single_generation_runtime_terminal_checkpoint_projection(
            SingleGenerationSnapshotStage::Completed,
            &generated_result.chapter_id,
            Some(generated_result.chapter_number),
            Some(generated_result.word_count),
            None,
            Some(&generated_result),
        );

        assert_eq!(completed_checkpoint["phase"], "completed");
        assert_eq!(completed_checkpoint["status"], "completed");
        assert_eq!(completed_checkpoint["progress"], 100);
        assert_eq!(
            completed_checkpoint["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(
            completed_checkpoint["candidate_gateway"]["rollback_boundary"],
            "legacy_single_generation_direct_ai"
        );

        let failed_checkpoint = build_single_generation_runtime_terminal_checkpoint_projection(
            SingleGenerationSnapshotStage::Failed,
            &generated_result.chapter_id,
            Some(generated_result.chapter_number),
            Some(generated_result.word_count),
            Some(json!({
                "analysis_task_message": "单章生成触发质量门禁，需人工复核",
                "analysis_task_progress": 100,
                "analysis_last_error": null,
                "quality_gate_decision": "manual_review",
                "quality_gate_label": "连续性需人工复核",
                "phase": "quality_blocked"
            })),
            Some(&generated_result),
        );

        assert_eq!(failed_checkpoint["phase"], "quality_blocked");
        assert_eq!(failed_checkpoint["status"], "failed");
        assert_eq!(failed_checkpoint["quality_gate_decision"], "manual_review");
        assert_eq!(
            failed_checkpoint["analysis_task_message"],
            "单章生成触发质量门禁，需人工复核"
        );
        assert_eq!(
            failed_checkpoint["candidate_gateway"]["fallback_applied"],
            false
        );
        assert_eq!(failed_checkpoint["word_count"], 3120);
    }

    #[tokio::test]
    async fn should_keep_single_generation_runtime_dispatch_contract() {
        SingleGenerationRuntimeLifecyclePlan::from_runtime_launch(
            "task-7".to_string(),
            SingleGenerationRuntimeLaunchInput {
                chapter_id: "chapter-7".to_string(),
                user_id: "user-7".to_string(),
                execution_input: SingleChapterGenerationExecutionInput {
                    target_word_count: 2400,
                    compat_options: empty_compat_options(),
                    execution_config: crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                        ai_config: AIConfig::default(),
                        provider_payload: PromptContextProviderPayload {
                            recent_chapters_context: String::new(),
                            previous_chapter_summary: String::new(),
                            chapter_careers: "[]".to_string(),
                            characters_info: "[]".to_string(),
                            foreshadow_reminders: "[]".to_string(),
                            relevant_memories: "[]".to_string(),
                            research_query: String::new(),
                            research_assets: "[]".to_string(),
                            external_assets: "[]".to_string(),
                            reference_assets: "[]".to_string(),
                            mcp_references: String::new(),
                        },
                    },
                },
            },
        )
        .spawn(sea_orm::DatabaseConnection::Disconnected);
    }

    #[test]
    fn should_keep_single_generation_runtime_compat_options_on_launch_contract() {
        let launch = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-compat".to_string(),
            user_id: "user-compat".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 3100,
                compat_options: SingleChapterGenerationCompatOptions {
                    style_id: Some(12),
                    enable_analysis: false,
                    enable_mcp: false,
                    web_research_enabled: true,
                    web_research_query: Some("late qing trade routes".to_string()),
                    narrative_perspective: Some("omniscient".to_string()),
                    creative_mode: Some("suspense".to_string()),
                    story_focus: Some("reveal_mystery".to_string()),
                    plot_stage: Some("climax".to_string()),
                    story_creation_brief: Some("push toward reveal".to_string()),
                    quality_preset: Some("immersive".to_string()),
                    quality_notes: Some("lean prose".to_string()),
                    story_repair_summary: Some("repair pacing".to_string()),
                    story_repair_targets: vec!["tighten setup".to_string()],
                    story_preserve_strengths: vec!["voice".to_string()],
                },
                execution_config: crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: String::new(),
                        previous_chapter_summary: String::new(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                },
            },
        };

        assert_eq!(launch.execution_input.compat_options.style_id(), Some(12));
        assert!(!launch.execution_input.compat_options.enable_analysis());
        assert!(!launch.execution_input.compat_options.enable_mcp());
        assert!(launch.execution_input.compat_options.web_research_enabled());
        assert_eq!(
            launch.execution_input.compat_options.web_research_query(),
            Some("late qing trade routes")
        );
        assert_eq!(
            launch.execution_input.compat_options.creative_mode(),
            "suspense"
        );
        assert_eq!(
            launch.execution_input.compat_options.story_focus(),
            "reveal_mystery"
        );
        assert_eq!(launch.execution_input.compat_options.plot_stage(), "climax");
        assert_eq!(
            launch.execution_input.compat_options.quality_preset(),
            "immersive"
        );
    }

    #[test]
    fn should_build_single_generation_runtime_lifecycle_plan_from_runtime_launch() {
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-runtime".to_string(),
            user_id: "user-runtime".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2800,
                compat_options: SingleChapterGenerationCompatOptions {
                    style_id: None,
                    enable_analysis: false,
                    enable_mcp: true,
                    web_research_enabled: false,
                    web_research_query: None,
                    narrative_perspective: None,
                    creative_mode: None,
                    story_focus: None,
                    plot_stage: None,
                    story_creation_brief: None,
                    quality_preset: None,
                    quality_notes: None,
                    story_repair_summary: None,
                    story_repair_targets: Vec::new(),
                    story_preserve_strengths: Vec::new(),
                },
                execution_config: crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                    ai_config: AIConfig::default(),
                    provider_payload: PromptContextProviderPayload {
                        recent_chapters_context: String::new(),
                        previous_chapter_summary: String::new(),
                        chapter_careers: "[]".to_string(),
                        characters_info: "[]".to_string(),
                        foreshadow_reminders: "[]".to_string(),
                        relevant_memories: "[]".to_string(),
                        research_query: String::new(),
                        research_assets: "[]".to_string(),
                        external_assets: "[]".to_string(),
                        reference_assets: "[]".to_string(),
                        mcp_references: String::new(),
                    },
                },
            },
        };

        let plan = SingleGenerationRuntimeLifecyclePlan::from_runtime_launch(
            "task-runtime".to_string(),
            runtime_input.clone(),
        );

        assert_eq!(plan.task_id, "task-runtime");
        assert_eq!(plan.chapter_id, "chapter-runtime");
        assert_eq!(plan.runtime_user_id, "user-runtime");
        assert!(!plan.enable_analysis);
        assert_eq!(plan.runtime_input.chapter_id, runtime_input.chapter_id);
    }

    #[tokio::test]
    async fn should_skip_single_generation_follow_up_analysis_when_disabled() {
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-1".to_string(),
            user_id: "user-1".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2000,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: false,
                    ..empty_compat_options()
                },
                execution_config:
                    crate::services::chapter_generation_execution_contract_service::PreparedGenerationExecutionConfig {
                        ai_config: AIConfig::default(),
                        provider_payload: PromptContextProviderPayload {
                            recent_chapters_context: String::new(),
                            previous_chapter_summary: String::new(),
                            chapter_careers: "[]".to_string(),
                            characters_info: "[]".to_string(),
                            foreshadow_reminders: "[]".to_string(),
                            relevant_memories: "[]".to_string(),
                            research_query: String::new(),
                            research_assets: "[]".to_string(),
                            external_assets: "[]".to_string(),
                            reference_assets: "[]".to_string(),
                            mcp_references: String::new(),
                        },
                    },
            },
        };
        let outcome = SingleGenerationRuntimeOutcome::new(
            "task-1".to_string(),
            runtime_input.chapter_id.clone(),
            runtime_input.user_id.clone(),
            runtime_input
                .execution_input
                .compat_options
                .enable_analysis(),
        );
        let result = outcome
            .run_follow_up_analysis(
                &sea_orm::DatabaseConnection::Disconnected,
                &GeneratedChapterResult {
                    chapter_id: "chapter-1".to_string(),
                    chapter_number: 1,
                    title: "第一章".to_string(),
                    content: "正文".to_string(),
                    word_count: 2,
                    ..Default::default()
                },
            )
            .await;

        assert_eq!(result, None);
    }

    #[test]
    fn should_build_prompt_overrides_from_single_generation_compat_options() {
        let compat = SingleChapterGenerationCompatOptions {
            style_id: Some(5),
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: false,
            web_research_query: None,
            narrative_perspective: Some("第一人称".to_string()),
            creative_mode: Some("hook".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("development".to_string()),
            story_creation_brief: Some("本章集中推进逃亡计划".to_string()),
            quality_preset: Some("plot_drive".to_string()),
            quality_notes: Some("减少旁白解释".to_string()),
            story_repair_summary: None,
            story_repair_targets: Vec::new(),
            story_preserve_strengths: Vec::new(),
        };

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat);

        assert_eq!(
            prompt_overrides.narrative_perspective.as_deref(),
            Some("第一人称")
        );
        assert_eq!(prompt_overrides.creative_mode.as_deref(), Some("hook"));
        assert_eq!(
            prompt_overrides.story_focus.as_deref(),
            Some("advance_plot")
        );
        assert_eq!(prompt_overrides.plot_stage.as_deref(), Some("development"));
        assert_eq!(
            prompt_overrides.story_creation_brief.as_deref(),
            Some("本章集中推进逃亡计划")
        );
        assert_eq!(
            prompt_overrides.quality_preset.as_deref(),
            Some("plot_drive")
        );
        assert_eq!(
            prompt_overrides.quality_notes.as_deref(),
            Some("减少旁白解释")
        );
        assert!(!prompt_overrides.web_research_enabled);
        assert_eq!(prompt_overrides.web_research_query, None);
        assert_eq!(prompt_overrides.story_repair_summary, None);
        assert!(prompt_overrides.story_repair_targets.is_empty());
        assert!(prompt_overrides.story_preserve_strengths.is_empty());
    }

    #[test]
    fn should_include_story_repair_fields_in_prompt_overrides() {
        let compat = SingleChapterGenerationCompatOptions {
            style_id: Some(9),
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: false,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: Some("上一章后段信息重复，需要压缩".to_string()),
            story_repair_targets: vec!["收紧中段说明".to_string(), "让冲突更早落地".to_string()],
            story_preserve_strengths: vec!["角色张力".to_string(), "章节结尾钩子".to_string()],
        };

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat);

        assert_eq!(
            prompt_overrides.story_repair_summary.as_deref(),
            Some("上一章后段信息重复，需要压缩")
        );
        assert_eq!(
            prompt_overrides.story_repair_targets,
            vec!["收紧中段说明".to_string(), "让冲突更早落地".to_string()]
        );
        assert_eq!(
            prompt_overrides.story_preserve_strengths,
            vec!["角色张力".to_string(), "章节结尾钩子".to_string()]
        );
    }

    #[test]
    fn should_include_web_research_fields_in_prompt_overrides() {
        let compat = SingleChapterGenerationCompatOptions {
            style_id: Some(3),
            enable_analysis: true,
            enable_mcp: true,
            web_research_enabled: true,
            web_research_query: Some("民国报馆夜班排印流程".to_string()),
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: Vec::new(),
            story_preserve_strengths: Vec::new(),
        };

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat);

        assert!(prompt_overrides.web_research_enabled);
        assert_eq!(
            prompt_overrides.web_research_query.as_deref(),
            Some("民国报馆夜班排印流程")
        );
    }
}
