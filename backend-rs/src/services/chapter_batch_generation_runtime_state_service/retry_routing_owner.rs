use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};

use crate::models::{batch_generation_task, chapter};
use crate::services::chapter_batch_generation_task_payload_base_service::{
    BatchGenerationFailedTerminalKind, BatchGenerationFailedTerminalSemantics,
};

use super::{
    build_batch_generation_runtime_checkpoint_for_stage,
    build_quality_gate_blocked_runtime_state_patch_from_workflow_state,
    build_retry_quality_runtime_patch_contract_from_workflow_state,
    persist_batch_generation_runtime_plan, upsert_batch_generation_runtime_snapshot,
    BatchGenerationAttemptProgression, BatchGenerationFailureKind,
    BatchGenerationRuntimeDriverProgression, BatchGenerationRuntimePersistencePlan,
    BatchGenerationSnapshotStage, BatchGenerationStepProgress,
};

pub(crate) fn build_batch_generation_retry_routing_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::retry_failure_quality_gate_routing",
        "scope": "retry_persistence_generic_failure_quality_gate_retry_progression_and_terminal_stop_routing",
        "python_source_map": [
            "backend/app/services/batch_generation_orchestration_service.py",
            "backend/app/services/batch_generation_retry_service.py",
            "backend/app/services/batch_generation_candidate_service.py",
            "backend/app/services/task_workflow_runtime_service.py",
            "backend/app/api/chapter_batch_generation_routes.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/retry_routing_owner.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "retry_persistence_entrypoints": [
                "BatchGenerationRetryPersistencePlan::new",
                "BatchGenerationRetryPersistencePlan::from_step_context",
                "BatchGenerationRetryPersistencePlan::build_waiting_snapshot",
                "BatchGenerationRetryPersistencePlan::persist"
            ],
            "routing_entrypoints": [
                "BatchGenerationGenericFailureRoutingPlan::from_step_error",
                "BatchGenerationGenericFailureRoutingPlan::from_step_context",
                "BatchGenerationGenericFailureRoutingPlan::persist_and_resolve",
                "BatchGenerationQualityGateRoutingPlan::from_terminal_semantics",
                "BatchGenerationQualityGateRoutingPlan::persist_and_resolve"
            ],
            "progression_entrypoints": [
                "BatchGenerationRetryProgressionPlan::new",
                "BatchGenerationRetryProgressionPlan::execute"
            ],
            "retry_policy_helpers": [
                "should_retry_batch_generation_attempt",
                "batch_generation_retry_backoff_seconds",
                "build_batch_generation_failed_task_error_message"
            ],
            "routing_contract": {
                "generic_failure_retry_contract": "retry while next_retry_count <= max_retries, otherwise persist failed terminal snapshot",
                "quality_gate_retry_contract": "retry only when terminal_semantics.kind=Retry and retry budget remains",
                "quality_gate_manual_review_contract": "manual review writes blocked runtime patch and stops driver progression",
                "retry_progression_contract": "retry path sleeps with bounded backoff and resumes same runtime driver"
            },
            "runtime_state_patch_dependencies": [
                "build_quality_gate_blocked_runtime_state_patch_from_workflow_state",
                "build_retry_quality_runtime_patch_contract_from_workflow_state"
            ]
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "terminal_runtime_patch_owner_contract": crate::services::chapter_batch_generation_runtime_state_service::build_generation_terminal_runtime_patch_owner_contract(),
        "snapshot_persistence_owner_contract": crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::build_chapter_generation_snapshot_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_retry_failure_quality_gate_routing_shells_as_source_map_until_explicit_freeze_delete_round",
            "runtime_state_keys": [
                "current_retry_count",
                "max_retries",
                "last_event",
                "last_message",
                "last_error",
                "retry_backoff_seconds",
                "quality_gate_decision",
                "quality_gate_label",
                "active_story_repair_payload"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_retry_gateway_smoke"
        }
    })
}

pub(crate) fn should_retry_batch_generation_attempt(
    next_retry_count: i32,
    max_retries: i32,
) -> bool {
    next_retry_count >= 0 && next_retry_count <= max_retries.max(0)
}

pub(crate) fn batch_generation_retry_backoff_seconds(next_retry_count: i32) -> u64 {
    let exponent = next_retry_count.clamp(0, 4) as u32;
    2_u64.pow(exponent).min(10)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BatchGenerationRetryPersistenceContract {
    Generic,
    QualityGate {
        terminal_semantics: BatchGenerationFailedTerminalSemantics,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationRetryPersistencePlan {
    pub(crate) current_chapter_id: String,
    pub(crate) current_chapter_number: Option<i32>,
    pub(crate) completed_chapters: i32,
    pub(crate) total_chapters: i32,
    pub(crate) next_retry_count: i32,
    pub(crate) max_retries: i32,
    pub(crate) wait_seconds: u64,
    pub(crate) error_message: String,
    pub(crate) retry_contract: BatchGenerationRetryPersistenceContract,
}

impl BatchGenerationRetryPersistencePlan {
    pub(crate) fn new(
        chapter_model: &chapter::Model,
        progress: &BatchGenerationStepProgress,
        next_retry_count: i32,
        max_retries: i32,
        error_message: &str,
        retry_contract: BatchGenerationRetryPersistenceContract,
    ) -> Self {
        Self::from_step_context(
            &chapter_model.id,
            Some(chapter_model.chapter_number),
            progress,
            next_retry_count,
            max_retries,
            error_message,
            retry_contract,
        )
    }

    pub(crate) fn from_step_context(
        chapter_id: &str,
        chapter_number: Option<i32>,
        progress: &BatchGenerationStepProgress,
        next_retry_count: i32,
        max_retries: i32,
        error_message: &str,
        retry_contract: BatchGenerationRetryPersistenceContract,
    ) -> Self {
        Self {
            current_chapter_id: chapter_id.to_string(),
            current_chapter_number: chapter_number,
            completed_chapters: progress.completed,
            total_chapters: progress.total_chapters,
            next_retry_count: next_retry_count.max(0),
            max_retries,
            wait_seconds: batch_generation_retry_backoff_seconds(next_retry_count),
            error_message: error_message.to_string(),
            retry_contract,
        }
    }

    pub(crate) fn build_waiting_snapshot(&self) -> Value {
        let mut checkpoint = build_batch_generation_runtime_checkpoint_for_stage(
            BatchGenerationSnapshotStage::ChapterStarted,
            Some(&self.current_chapter_id),
            self.current_chapter_number,
            self.completed_chapters,
            self.total_chapters,
        );
        if let Some(checkpoint_object) = checkpoint.as_object_mut() {
            checkpoint_object.insert(
                "last_event".to_string(),
                Value::String("chapter_retry".to_string()),
            );
            checkpoint_object.insert(
                "last_message".to_string(),
                Value::String(match self.current_chapter_number {
                    Some(chapter_number) => format!(
                        "第 {} 章生成失败，{} 秒后进行第 {} 次重试",
                        chapter_number, self.wait_seconds, self.next_retry_count
                    ),
                    None => format!(
                        "章节生成失败，{} 秒后进行第 {} 次重试",
                        self.wait_seconds, self.next_retry_count
                    ),
                }),
            );
            checkpoint_object.insert(
                "current_retry_count".to_string(),
                Value::Number(self.next_retry_count.into()),
            );
            checkpoint_object.insert(
                "max_retries".to_string(),
                Value::Number(self.max_retries.into()),
            );
            checkpoint_object.insert(
                "retry_backoff_seconds".to_string(),
                Value::Number((self.wait_seconds as i64).into()),
            );
            checkpoint_object.insert(
                "last_error".to_string(),
                Value::String(self.error_message.clone()),
            );
            if let BatchGenerationRetryPersistenceContract::QualityGate { terminal_semantics } =
                &self.retry_contract
            {
                checkpoint_object.insert(
                    "terminal_reason".to_string(),
                    json!(terminal_semantics.reason),
                );
                checkpoint_object.insert(
                    "terminal_label".to_string(),
                    json!(terminal_semantics.label.clone()),
                );
                checkpoint_object.insert(
                    "review_required".to_string(),
                    json!(terminal_semantics.review_required),
                );
                checkpoint_object.insert(
                    "can_resume".to_string(),
                    json!(terminal_semantics.can_resume),
                );
                if terminal_semantics.kind == BatchGenerationFailedTerminalKind::Retry {
                    checkpoint_object
                        .insert("quality_gate_decision".to_string(), json!("auto_repair"));
                    checkpoint_object.insert(
                        "quality_gate_label".to_string(),
                        json!(terminal_semantics.label.clone()),
                    );
                    checkpoint_object.insert("phase".to_string(), json!("repair_pending"));
                }
            }
        }
        checkpoint
    }

    pub(crate) fn apply_to_active_model(&self, active: &mut batch_generation_task::ActiveModel) {
        active.status = Set("running".to_string());
        active.error_message = Set(None);
        active.current_chapter_id = Set(Some(self.current_chapter_id.clone()));
        active.current_chapter_number = Set(self.current_chapter_number);
        active.current_retry_count = Set(self.next_retry_count);
    }

    pub(crate) async fn persist(self, db: &DatabaseConnection, task_id: &str) {
        if let Ok(Some(task_model)) = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
        {
            let mut active: batch_generation_task::ActiveModel = task_model.into();
            self.apply_to_active_model(&mut active);
            let _ = active.update(db).await;
        }

        let _ =
            upsert_batch_generation_runtime_snapshot(db, task_id, self.build_waiting_snapshot())
                .await;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BatchGenerationGenericFailureRoutingPlan {
    Retry(BatchGenerationRetryPersistencePlan),
    Stop(BatchGenerationRuntimePersistencePlan),
}

impl BatchGenerationGenericFailureRoutingPlan {
    pub(crate) fn from_step_error(
        chapter_model: &chapter::Model,
        progress: &BatchGenerationStepProgress,
        current_retry_count: i32,
        max_retries: i32,
        failure_kind: BatchGenerationFailureKind,
        error_message: &str,
    ) -> Self {
        Self::from_step_context(
            &chapter_model.id,
            Some(chapter_model.chapter_number),
            Some(&chapter_model.title),
            progress,
            current_retry_count,
            max_retries,
            failure_kind,
            error_message,
        )
    }

    pub(crate) fn from_step_context(
        chapter_id: &str,
        chapter_number: Option<i32>,
        chapter_title: Option<&str>,
        progress: &BatchGenerationStepProgress,
        current_retry_count: i32,
        max_retries: i32,
        failure_kind: BatchGenerationFailureKind,
        error_message: &str,
    ) -> Self {
        let next_retry_count = current_retry_count + 1;
        if should_retry_batch_generation_attempt(next_retry_count, max_retries) {
            return Self::Retry(BatchGenerationRetryPersistencePlan::from_step_context(
                chapter_id,
                chapter_number,
                progress,
                next_retry_count,
                max_retries,
                error_message,
                BatchGenerationRetryPersistenceContract::Generic,
            ));
        }

        Self::Stop(BatchGenerationRuntimePersistencePlan::failed(
            Some(chapter_id),
            chapter_number,
            chapter_title,
            progress.completed,
            progress.total_chapters,
            failure_kind,
            next_retry_count - 1,
            error_message.to_string(),
            build_batch_generation_failed_task_error_message(
                chapter_number,
                next_retry_count - 1,
                error_message,
            ),
        ))
    }

    pub(crate) async fn persist_and_resolve(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> BatchGenerationAttemptProgression {
        match self {
            BatchGenerationGenericFailureRoutingPlan::Retry(plan) => {
                let next_retry_count = plan.next_retry_count;
                plan.persist(db, task_id).await;
                BatchGenerationRetryProgressionPlan::new(next_retry_count)
                    .execute()
                    .await
            }
            BatchGenerationGenericFailureRoutingPlan::Stop(plan) => {
                persist_batch_generation_runtime_plan(db, task_id, plan).await;
                BatchGenerationAttemptProgression::Driver(
                    BatchGenerationRuntimeDriverProgression::Stop,
                )
            }
        }
    }
}

pub(crate) fn build_batch_generation_failed_task_error_message(
    chapter_number: Option<i32>,
    retry_count: i32,
    error_message: &str,
) -> String {
    match chapter_number {
        Some(chapter_number) => format!(
            "第{}章生成失败(重试{}次): {}",
            chapter_number,
            retry_count.max(0),
            error_message
        ),
        None => format!(
            "章节生成失败(重试{}次): {}",
            retry_count.max(0),
            error_message
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BatchGenerationQualityGateRoutingPlan {
    Retry {
        runtime_state_patch: Value,
        persistence_plan: BatchGenerationRetryPersistencePlan,
        next_retry_count: i32,
    },
    Stop {
        runtime_state_patch: Value,
        persistence_plan: BatchGenerationRuntimePersistencePlan,
    },
}

impl BatchGenerationQualityGateRoutingPlan {
    pub(crate) fn from_terminal_semantics(
        chapter_model: &chapter::Model,
        progress: &BatchGenerationStepProgress,
        workflow_runtime_state: Option<&Value>,
        current_retry_count: i32,
        max_retries: i32,
        terminal_semantics: BatchGenerationFailedTerminalSemantics,
    ) -> Option<Self> {
        match terminal_semantics.kind {
            BatchGenerationFailedTerminalKind::ManualReview => {
                let manual_review_label = terminal_semantics.label.clone();
                let failure_message = format!(
                    "第{}章触发质量门禁，需人工复核: {}",
                    chapter_model.chapter_number, manual_review_label
                );
                Some(Self::Stop {
                    runtime_state_patch:
                        build_quality_gate_blocked_runtime_state_patch_from_workflow_state(
                            workflow_runtime_state,
                            chapter_model.chapter_number,
                            &manual_review_label,
                        ),
                    persistence_plan:
                        BatchGenerationRuntimePersistencePlan::failed_quality_gate_blocked(
                            Some(&chapter_model.id),
                            Some(chapter_model.chapter_number),
                            Some(&chapter_model.title),
                            progress.completed,
                            progress.total_chapters,
                            current_retry_count,
                            &terminal_semantics,
                            workflow_runtime_state,
                            failure_message,
                        ),
                })
            }
            BatchGenerationFailedTerminalKind::Retry => {
                let next_retry_count = current_retry_count + 1;
                if !should_retry_batch_generation_attempt(next_retry_count, max_retries) {
                    return None;
                }

                let retry_label = terminal_semantics.label.clone();
                let retry_message = format!(
                    "第{}章触发质量修复重试: {}",
                    chapter_model.chapter_number, retry_label
                );
                Some(Self::Retry {
                    runtime_state_patch: Value::Object(
                        build_retry_quality_runtime_patch_contract_from_workflow_state(
                            workflow_runtime_state,
                            chapter_model.chapter_number,
                            &retry_label,
                        ),
                    ),
                    persistence_plan: BatchGenerationRetryPersistencePlan::new(
                        chapter_model,
                        progress,
                        next_retry_count,
                        max_retries,
                        &retry_message,
                        BatchGenerationRetryPersistenceContract::QualityGate { terminal_semantics },
                    ),
                    next_retry_count,
                })
            }
            BatchGenerationFailedTerminalKind::Error => None,
        }
    }

    pub(crate) async fn persist_and_resolve(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> BatchGenerationAttemptProgression {
        match self {
            BatchGenerationQualityGateRoutingPlan::Retry {
                runtime_state_patch,
                persistence_plan,
                next_retry_count,
            } => {
                persistence_plan.persist(db, task_id).await;
                let _ = upsert_batch_generation_runtime_snapshot(db, task_id, runtime_state_patch)
                    .await;
                BatchGenerationRetryProgressionPlan::new(next_retry_count)
                    .execute()
                    .await
            }
            BatchGenerationQualityGateRoutingPlan::Stop {
                runtime_state_patch,
                persistence_plan,
            } => {
                let _ = upsert_batch_generation_runtime_snapshot(db, task_id, runtime_state_patch)
                    .await;
                persist_batch_generation_runtime_plan(db, task_id, persistence_plan).await;
                BatchGenerationAttemptProgression::Driver(
                    BatchGenerationRuntimeDriverProgression::Stop,
                )
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BatchGenerationRetryProgressionPlan {
    next_retry_count: i32,
}

impl BatchGenerationRetryProgressionPlan {
    pub(crate) fn new(next_retry_count: i32) -> Self {
        Self { next_retry_count }
    }

    pub(crate) async fn execute(self) -> BatchGenerationAttemptProgression {
        sleep(Duration::from_secs(batch_generation_retry_backoff_seconds(
            self.next_retry_count,
        )))
        .await;
        BatchGenerationAttemptProgression::Retry(self.next_retry_count)
    }
}
