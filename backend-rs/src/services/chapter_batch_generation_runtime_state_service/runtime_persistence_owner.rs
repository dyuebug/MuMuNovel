use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};

use crate::models::{batch_generation_task, chapter};
use crate::services::chapter_batch_generation_task_payload_base_service::{
    BatchGenerationFailedTerminalKind, BatchGenerationFailedTerminalSemantics,
};
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::build_chapter_generation_snapshot_owner_contract;

use super::{
    apply_manual_review_terminal_fields, build_batch_generation_runtime_checkpoint_for_stage,
    build_generation_terminal_runtime_patch_owner_contract,
    upsert_batch_generation_runtime_snapshot, BatchGenerationFailureKind,
    BatchGenerationSnapshotStage, ResumeResetSemantics,
};

pub(crate) fn build_batch_generation_runtime_persistence_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::runtime_persistence_task_mutation_projection",
        "scope": "task_stage_mutation_failed_entry_quality_gate_metrics_and_runtime_checkpoint_persistence",
        "python_source_map": [
            "backend/app/services/batch_generation_orchestration_service.py",
            "backend/app/services/task_workflow_runtime_service.py",
            "backend/app/services/batch_generation_retry_service.py",
            "backend/app/services/batch_generation_candidate_service.py",
            "backend/app/api/chapter_batch_generation_routes.py",
            "backend/app/api/chapters.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_persistence_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/runtime_driver_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/retry_routing_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/startup_and_command_projection_owner.rs"
        ],
        "behavior_contract": {
            "task_mutation_entrypoints": [
                "BatchGenerationResumeTaskResetMutationPlan::from_reset_semantics",
                "BatchGenerationResumeTaskResetMutationPlan::apply_to_active_model",
                "BatchGenerationTaskStage::apply_to_active_model"
            ],
            "task_stage_helpers": [
                "BatchGenerationTaskStage::status",
                "BatchGenerationTaskStage::started_at_update",
                "BatchGenerationTaskStage::completed_at_update",
                "BatchGenerationTaskStage::error_message_update",
                "BatchGenerationTaskStage::completed_chapters_update",
                "BatchGenerationTaskStage::current_retry_count_update",
                "BatchGenerationTaskStage::current_chapter_id_update",
                "BatchGenerationTaskStage::current_chapter_number_update",
                "BatchGenerationTaskStage::total_chapters_update"
            ],
            "failed_entry_entrypoints": [
                "append_failed_chapter_entry",
                "build_batch_generation_failed_chapter_entry",
                "build_quality_gate_blocked_failed_chapter_entry",
                "extract_quality_gate_failed_metrics_from_runtime_state",
                "extract_quality_gate_failed_metrics_from_payload"
            ],
            "runtime_checkpoint_entrypoints": [
                "BatchGenerationRuntimePersistencePlan::preparing",
                "BatchGenerationRuntimePersistencePlan::cancelled",
                "BatchGenerationRuntimePersistencePlan::chapter_started",
                "BatchGenerationRuntimePersistencePlan::chapter_succeeded",
                "BatchGenerationRuntimePersistencePlan::failed",
                "BatchGenerationRuntimePersistencePlan::failed_quality_gate_blocked",
                "BatchGenerationRuntimePersistencePlan::persist"
            ],
            "state_contract": {
                "task_stage_owner": "task status / timestamps / chapter progress mutation stays coupled to runtime snapshot persistence",
                "failed_entry_owner": "failed_chapters payload append and quality-gate terminal projection stay in the same runtime persistence owner",
                "checkpoint_owner": "task row mutation and runtime checkpoint update are persisted together per runtime stage"
            }
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_runtime_state_service::runtime_driver_owner",
            "chapter_batch_generation_runtime_state_service::retry_routing_owner",
            "chapter_batch_generation_runtime_state_service::startup_cancel_resume_task_payload_projection"
        ],
        "snapshot_persistence_owner_contract": build_chapter_generation_snapshot_owner_contract(),
        "terminal_runtime_patch_owner_contract": build_generation_terminal_runtime_patch_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_runtime_persistence_and_failed_entry_shells_as_source_map_until_explicit_freeze_delete_round",
            "task_fields": [
                "status",
                "started_at",
                "completed_at",
                "error_message",
                "completed_chapters",
                "current_retry_count",
                "current_chapter_id",
                "current_chapter_number",
                "total_chapters",
                "failed_chapters"
            ],
            "runtime_state_keys": [
                "checkpoint",
                "last_event",
                "last_message",
                "quality_gate_status",
                "quality_gate_failed_metrics"
            ]
        }
    })
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BatchGenerationResumeTaskResetMutationPlan {
    failed_chapters: Value,
    current_chapter_id: Option<String>,
    current_chapter_number: Option<i32>,
    completed_chapters: i32,
    total_chapters: i32,
}

impl BatchGenerationResumeTaskResetMutationPlan {
    pub(crate) fn from_reset_semantics(
        total_chapters: i32,
        reset_semantics: &ResumeResetSemantics,
    ) -> Self {
        Self {
            failed_chapters: reset_semantics.failed_chapters.clone(),
            current_chapter_id: reset_semantics.current_chapter_id.clone(),
            current_chapter_number: reset_semantics.current_chapter_number,
            completed_chapters: reset_semantics.completed_chapters,
            total_chapters,
        }
    }

    pub(crate) fn apply_to_active_model(
        self,
        active: &mut batch_generation_task::ActiveModel,
        now: chrono::NaiveDateTime,
    ) {
        active.failed_chapters = Set(self.failed_chapters);
        BatchGenerationTaskStage::ResumeReset.apply_to_active_model(
            active,
            self.current_chapter_id.as_deref(),
            self.current_chapter_number,
            self.completed_chapters,
            self.total_chapters,
            None,
            now,
        );
    }
}

pub(crate) fn append_failed_chapter_entry(
    failed_chapters: &Value,
    failed_entry: Option<&Value>,
) -> Value {
    let mut items = failed_chapters.as_array().cloned().unwrap_or_default();
    if let Some(entry) = failed_entry.filter(|entry| entry.is_object()) {
        items.push(entry.clone());
    }
    Value::Array(items)
}

pub(crate) fn build_batch_generation_failed_chapter_entry(
    chapter_id: Option<&str>,
    chapter_number: Option<i32>,
    chapter_title: Option<&str>,
    task_error_message: &str,
    retry_count: i32,
) -> Value {
    json!({
        "chapter_id": chapter_id,
        "chapter_number": chapter_number,
        "title": chapter_title,
        "error": task_error_message,
        "retry_count": retry_count.max(0),
    })
}

pub(crate) fn build_quality_gate_blocked_failed_chapter_entry(
    chapter_id: Option<&str>,
    chapter_number: Option<i32>,
    chapter_title: Option<&str>,
    task_error_message: &str,
    retry_count: i32,
    terminal_semantics: &BatchGenerationFailedTerminalSemantics,
    workflow_runtime_state: Option<&Value>,
) -> Value {
    let mut entry = build_batch_generation_failed_chapter_entry(
        chapter_id,
        chapter_number,
        chapter_title,
        task_error_message,
        retry_count,
    );
    if let Some(object) = entry.as_object_mut() {
        if terminal_semantics.kind == BatchGenerationFailedTerminalKind::ManualReview {
            apply_manual_review_terminal_fields(object, &terminal_semantics.label);
        }
        object.insert("quality_gate_status".to_string(), json!("failed"));
        object.insert(
            "quality_gate_failed_metrics".to_string(),
            json!(extract_quality_gate_failed_metrics_from_runtime_state(
                workflow_runtime_state
            )),
        );
    }
    entry
}

pub(crate) fn extract_quality_gate_failed_metrics_from_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> Vec<String> {
    let mut collected = Vec::new();

    for candidate in [
        workflow_runtime_state.and_then(|state| state.get("active_story_repair_payload")),
        workflow_runtime_state.and_then(|state| state.get("quality_metrics_summary")),
        workflow_runtime_state.and_then(|state| state.get("latest_quality_metrics")),
    ] {
        collected.extend(extract_quality_gate_failed_metrics_from_payload(candidate));
        if !collected.is_empty() {
            break;
        }
    }

    let mut seen = std::collections::HashSet::new();
    collected
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .filter(|item| seen.insert(item.clone()))
        .collect()
}

pub(crate) fn extract_quality_gate_failed_metrics_from_payload(
    value: Option<&Value>,
) -> Vec<String> {
    let Some(payload) = value.and_then(Value::as_object) else {
        return Vec::new();
    };

    let direct_items = payload
        .get("quality_gate_failed_metrics")
        .and_then(Value::as_array);
    let nested_items = payload
        .get("quality_gate")
        .and_then(Value::as_object)
        .and_then(|gate| gate.get("failed_metrics"))
        .and_then(Value::as_array);

    direct_items
        .or(nested_items)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str().map(str::to_string).or_else(|| {
                        item.as_object()
                            .and_then(|entry| entry.get("label"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchGenerationTaskStage {
    ResumeReset,
    Preparing,
    ChapterStarted,
    ChapterSucceeded,
    Cancelled,
    Failed,
}

impl BatchGenerationTaskStage {
    pub(crate) fn status(self, completed_chapters: i32, total_chapters: i32) -> &'static str {
        match self {
            Self::ResumeReset => "pending",
            Self::Preparing | Self::ChapterStarted => "running",
            Self::ChapterSucceeded => {
                if completed_chapters >= total_chapters {
                    "completed"
                } else {
                    "running"
                }
            }
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn started_at_update(self) -> TaskTimestampUpdate {
        match self {
            Self::ResumeReset => TaskTimestampUpdate::Clear,
            Self::Preparing => TaskTimestampUpdate::Now,
            Self::ChapterStarted | Self::ChapterSucceeded | Self::Cancelled | Self::Failed => {
                TaskTimestampUpdate::Keep
            }
        }
    }

    pub(crate) fn completed_at_update(
        self,
        completed_chapters: i32,
        total_chapters: i32,
    ) -> TaskTimestampUpdate {
        match self {
            Self::ResumeReset | Self::Preparing => TaskTimestampUpdate::Clear,
            Self::ChapterStarted => TaskTimestampUpdate::Keep,
            Self::ChapterSucceeded => {
                if completed_chapters >= total_chapters {
                    TaskTimestampUpdate::Now
                } else {
                    TaskTimestampUpdate::Keep
                }
            }
            Self::Cancelled | Self::Failed => TaskTimestampUpdate::Now,
        }
    }

    pub(crate) fn error_message_update(
        self,
        error_message: Option<String>,
    ) -> ModelFieldUpdate<Option<String>> {
        match self {
            Self::ResumeReset | Self::Preparing | Self::ChapterStarted | Self::ChapterSucceeded => {
                ModelFieldUpdate::Set(None)
            }
            Self::Cancelled => ModelFieldUpdate::Keep,
            Self::Failed => ModelFieldUpdate::Set(error_message),
        }
    }

    pub(crate) fn completed_chapters_update(
        self,
        completed_chapters: i32,
    ) -> ModelFieldUpdate<i32> {
        match self {
            Self::ResumeReset | Self::ChapterStarted | Self::ChapterSucceeded | Self::Failed => {
                ModelFieldUpdate::Set(completed_chapters)
            }
            Self::Preparing | Self::Cancelled => ModelFieldUpdate::Keep,
        }
    }

    pub(crate) fn current_retry_count_update(self) -> ModelFieldUpdate<i32> {
        match self {
            Self::ResumeReset | Self::Preparing => ModelFieldUpdate::Set(0),
            Self::ChapterStarted | Self::ChapterSucceeded | Self::Cancelled | Self::Failed => {
                ModelFieldUpdate::Keep
            }
        }
    }

    pub(crate) fn current_chapter_id_update(
        self,
        current_chapter_id: Option<&str>,
    ) -> ModelFieldUpdate<Option<String>> {
        match self {
            Self::ResumeReset | Self::ChapterStarted | Self::ChapterSucceeded | Self::Failed => {
                ModelFieldUpdate::Set(current_chapter_id.map(str::to_string))
            }
            Self::Preparing | Self::Cancelled => ModelFieldUpdate::Keep,
        }
    }

    pub(crate) fn current_chapter_number_update(
        self,
        current_chapter_number: Option<i32>,
    ) -> ModelFieldUpdate<Option<i32>> {
        match self {
            Self::ResumeReset | Self::ChapterStarted | Self::ChapterSucceeded | Self::Failed => {
                ModelFieldUpdate::Set(current_chapter_number)
            }
            Self::Preparing | Self::Cancelled => ModelFieldUpdate::Keep,
        }
    }

    pub(crate) fn total_chapters_update(self, total_chapters: i32) -> ModelFieldUpdate<i32> {
        match self {
            Self::ChapterStarted => ModelFieldUpdate::Set(total_chapters),
            Self::ResumeReset
            | Self::Preparing
            | Self::ChapterSucceeded
            | Self::Cancelled
            | Self::Failed => ModelFieldUpdate::Keep,
        }
    }

    pub(crate) fn apply_to_active_model(
        self,
        active: &mut batch_generation_task::ActiveModel,
        current_chapter_id: Option<&str>,
        current_chapter_number: Option<i32>,
        completed_chapters: i32,
        total_chapters: i32,
        error_message: Option<String>,
        now: chrono::NaiveDateTime,
    ) {
        active.status = Set(self.status(completed_chapters, total_chapters).to_string());

        match self.started_at_update() {
            TaskTimestampUpdate::Keep => {}
            TaskTimestampUpdate::Clear => active.started_at = Set(None),
            TaskTimestampUpdate::Now => active.started_at = Set(Some(now)),
        }

        match self.completed_at_update(completed_chapters, total_chapters) {
            TaskTimestampUpdate::Keep => {}
            TaskTimestampUpdate::Clear => active.completed_at = Set(None),
            TaskTimestampUpdate::Now => active.completed_at = Set(Some(now)),
        }

        match self.error_message_update(error_message) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.error_message = Set(value),
        }

        match self.completed_chapters_update(completed_chapters) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.completed_chapters = Set(value),
        }

        match self.current_retry_count_update() {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_retry_count = Set(value),
        }

        match self.current_chapter_id_update(current_chapter_id) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_chapter_id = Set(value),
        }

        match self.current_chapter_number_update(current_chapter_number) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.current_chapter_number = Set(value),
        }

        match self.total_chapters_update(total_chapters) {
            ModelFieldUpdate::Keep => {}
            ModelFieldUpdate::Set(value) => active.total_chapters = Set(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationRuntimePersistencePlan {
    pub(crate) task_stage: BatchGenerationTaskStage,
    pub(crate) checkpoint_stage: BatchGenerationSnapshotStage,
    pub(crate) current_chapter_id: Option<String>,
    pub(crate) current_chapter_number: Option<i32>,
    pub(crate) completed_chapters: i32,
    pub(crate) total_chapters: i32,
    pub(crate) current_retry_count: Option<i32>,
    pub(crate) error_message: Option<String>,
    pub(crate) failed_chapter_entry: Option<Value>,
}

impl BatchGenerationRuntimePersistencePlan {
    pub(crate) fn preparing(total_chapters: i32) -> Self {
        Self {
            task_stage: BatchGenerationTaskStage::Preparing,
            checkpoint_stage: BatchGenerationSnapshotStage::Preparing,
            current_chapter_id: None,
            current_chapter_number: None,
            completed_chapters: 0,
            total_chapters,
            current_retry_count: Some(0),
            error_message: None,
            failed_chapter_entry: None,
        }
    }

    pub(crate) fn cancelled(completed_chapters: i32, total_chapters: i32) -> Self {
        Self {
            task_stage: BatchGenerationTaskStage::Cancelled,
            checkpoint_stage: BatchGenerationSnapshotStage::Cancelled,
            current_chapter_id: None,
            current_chapter_number: None,
            completed_chapters,
            total_chapters,
            current_retry_count: None,
            error_message: None,
            failed_chapter_entry: None,
        }
    }

    pub(crate) fn chapter_started(
        chapter_model: &chapter::Model,
        completed_chapters: i32,
        total_chapters: i32,
        retry_count: i32,
    ) -> Self {
        Self {
            task_stage: BatchGenerationTaskStage::ChapterStarted,
            checkpoint_stage: BatchGenerationSnapshotStage::ChapterStarted,
            current_chapter_id: Some(chapter_model.id.clone()),
            current_chapter_number: Some(chapter_model.chapter_number),
            completed_chapters,
            total_chapters,
            current_retry_count: Some(retry_count.max(0)),
            error_message: None,
            failed_chapter_entry: None,
        }
    }

    pub(crate) fn chapter_succeeded(
        chapter_model: &chapter::Model,
        completed_chapters: i32,
        total_chapters: i32,
    ) -> Self {
        Self {
            task_stage: BatchGenerationTaskStage::ChapterSucceeded,
            checkpoint_stage: BatchGenerationSnapshotStage::ChapterSucceeded,
            current_chapter_id: Some(chapter_model.id.clone()),
            current_chapter_number: Some(chapter_model.chapter_number),
            completed_chapters,
            total_chapters,
            current_retry_count: Some(0),
            error_message: None,
            failed_chapter_entry: None,
        }
    }

    pub(crate) fn failed(
        chapter_id: Option<&str>,
        chapter_number: Option<i32>,
        chapter_title: Option<&str>,
        completed_chapters: i32,
        total_chapters: i32,
        failure_kind: BatchGenerationFailureKind,
        failed_retry_count: i32,
        failed_entry_error: String,
        task_error_message: String,
    ) -> Self {
        let failed_chapter_entry = Some(build_batch_generation_failed_chapter_entry(
            chapter_id,
            chapter_number,
            chapter_title,
            &failed_entry_error,
            failed_retry_count,
        ));
        Self {
            task_stage: BatchGenerationTaskStage::Failed,
            checkpoint_stage: BatchGenerationSnapshotStage::Failed(failure_kind),
            current_chapter_id: chapter_id.map(str::to_string),
            current_chapter_number: chapter_number,
            completed_chapters,
            total_chapters,
            current_retry_count: Some(failed_retry_count.max(0)),
            error_message: Some(task_error_message),
            failed_chapter_entry,
        }
    }

    pub(crate) fn failed_quality_gate_blocked(
        chapter_id: Option<&str>,
        chapter_number: Option<i32>,
        chapter_title: Option<&str>,
        completed_chapters: i32,
        total_chapters: i32,
        retry_count: i32,
        terminal_semantics: &BatchGenerationFailedTerminalSemantics,
        workflow_runtime_state: Option<&Value>,
        task_error_message: String,
    ) -> Self {
        let failed_chapter_entry = Some(build_quality_gate_blocked_failed_chapter_entry(
            chapter_id,
            chapter_number,
            chapter_title,
            &task_error_message,
            retry_count,
            terminal_semantics,
            workflow_runtime_state,
        ));
        Self {
            task_stage: BatchGenerationTaskStage::Failed,
            checkpoint_stage: BatchGenerationSnapshotStage::Failed(
                BatchGenerationFailureKind::QualityGateBlocked,
            ),
            current_chapter_id: chapter_id.map(str::to_string),
            current_chapter_number: chapter_number,
            completed_chapters,
            total_chapters,
            current_retry_count: Some(retry_count.max(0)),
            error_message: Some(task_error_message),
            failed_chapter_entry,
        }
    }

    pub(crate) async fn persist(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> Result<(), String> {
        let now = Utc::now().naive_utc();
        if let Some(task_model) = batch_generation_task::Entity::find_by_id(task_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?
        {
            let existing_failed_chapters = task_model.failed_chapters.clone();
            let mut active: batch_generation_task::ActiveModel = task_model.into();
            self.task_stage.apply_to_active_model(
                &mut active,
                self.current_chapter_id.as_deref(),
                self.current_chapter_number,
                self.completed_chapters,
                self.total_chapters,
                self.error_message.clone(),
                now,
            );
            if let Some(retry_count) = self.current_retry_count {
                active.current_retry_count = Set(retry_count.max(0));
            }
            if matches!(self.task_stage, BatchGenerationTaskStage::Failed) {
                active.failed_chapters = Set(append_failed_chapter_entry(
                    &existing_failed_chapters,
                    self.failed_chapter_entry.as_ref(),
                ));
            }
            active.update(db).await.map_err(|error| error.to_string())?;
        }

        upsert_batch_generation_runtime_snapshot(
            db,
            task_id,
            build_batch_generation_runtime_checkpoint_for_stage(
                self.checkpoint_stage,
                self.current_chapter_id.as_deref(),
                self.current_chapter_number,
                self.completed_chapters,
                self.total_chapters,
            ),
        )
        .await
    }
}
