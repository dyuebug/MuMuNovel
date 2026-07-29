use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde_json::{json, Value};

use crate::models::{batch_generation_task, chapter};
use crate::services::business_checkpoint_service::{
    build_business_checkpoint, merge_business_checkpoint_runtime_state,
    read_business_checkpoint_runtime_state, BusinessCheckpointBoundary,
    BusinessCheckpointOutputReferenceV1, BusinessCheckpointRead, BusinessCheckpointV1,
};
use crate::services::chapter_batch_generation_task_payload_base_service::{
    BatchGenerationFailedTerminalKind, BatchGenerationFailedTerminalSemantics,
};
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::{
    build_chapter_generation_snapshot_owner_contract, load_chapter_generation_snapshot,
};
use crate::services::generation_contract_service::{
    read_generation_contract_runtime_snapshot, GenerationContractSnapshotRead,
};

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
        "python_source_map": [],
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
            "source_map_policy": "batch_generation_runtime_persistence_owner_is_rust_only_and_surviving_task_mutation_failed_entry_surfaces_are_tracked_by_external_persistence_contracts",
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

fn build_chapter_succeeded_business_checkpoint(
    task_id: &str,
    current_chapter_id: Option<&str>,
    completed_chapters: i32,
    workflow_runtime_state: &Value,
    recorded_at: DateTime<Utc>,
) -> Result<Option<BusinessCheckpointV1>, String> {
    let Some(chapter_id) = current_chapter_id.filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    let GenerationContractSnapshotRead::Valid(contract) =
        read_generation_contract_runtime_snapshot(workflow_runtime_state)
    else {
        return Ok(None);
    };
    let target = &contract.generation_intent.target;
    let chapter_is_in_contract = target.chapter_id.as_deref() == Some(chapter_id)
        || target
            .chapter_ids
            .iter()
            .any(|candidate| candidate == chapter_id);
    if !chapter_is_in_contract {
        return Ok(None);
    }

    let previous_revision = match read_business_checkpoint_runtime_state(workflow_runtime_state) {
        BusinessCheckpointRead::Valid(checkpoint)
            if checkpoint.input_digest == contract.input_digest =>
        {
            checkpoint.revision
        }
        _ => 0,
    };
    let revision = u64::try_from(completed_chapters.max(1))
        .unwrap_or(1)
        .max(previous_revision);
    build_business_checkpoint(
        task_id,
        BusinessCheckpointBoundary::ChapterDraftSaved,
        revision,
        &contract.input_digest,
        BusinessCheckpointOutputReferenceV1::Chapter {
            id: chapter_id.to_owned(),
        },
        recorded_at,
    )
    .map(Some)
    .map_err(|error| error.to_string())
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

    #[allow(dead_code)]
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
        let transaction = db.begin().await.map_err(|error| error.to_string())?;
        let recorded_at = Utc::now();
        let now = recorded_at.naive_utc();
        let existing_snapshot = load_chapter_generation_snapshot(&transaction, task_id).await?;
        let mut runtime_checkpoint = build_batch_generation_runtime_checkpoint_for_stage(
            self.checkpoint_stage,
            self.current_chapter_id.as_deref(),
            self.current_chapter_number,
            self.completed_chapters,
            self.total_chapters,
        );
        if matches!(self.task_stage, BatchGenerationTaskStage::ChapterSucceeded) {
            if let Some(workflow_runtime_state) = existing_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.workflow_runtime_state.as_ref())
            {
                if let Some(checkpoint) = build_chapter_succeeded_business_checkpoint(
                    task_id,
                    self.current_chapter_id.as_deref(),
                    self.completed_chapters,
                    workflow_runtime_state,
                    recorded_at,
                )? {
                    merge_business_checkpoint_runtime_state(&mut runtime_checkpoint, &checkpoint)
                        .map_err(|error| error.to_string())?;
                }
            }
        }

        let task_model = batch_generation_task::Entity::find_by_id(task_id)
            .one(&transaction)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| {
                "Batch generation task not found during runtime persistence".to_string()
            })?;
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

        let update_result = batch_generation_task::Entity::update_many()
            .set(active)
            .filter(batch_generation_task::Column::Id.eq(task_id))
            .filter(batch_generation_task::Column::Status.is_not_in([
                "completed",
                "failed",
                "cancelled",
            ]))
            .exec(&transaction)
            .await
            .map_err(|error| error.to_string())?;
        if update_result.rows_affected == 0 {
            return Err(
                "Batch generation runtime persistence rejected by terminal task status".to_string(),
            );
        }

        upsert_batch_generation_runtime_snapshot(&transaction, task_id, runtime_checkpoint).await?;
        transaction
            .commit()
            .await
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod business_checkpoint_tests {
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use crate::services::business_checkpoint_service::{
        merge_business_checkpoint_runtime_state, BusinessCheckpointRead,
    };
    use crate::services::generation_contract_service::{
        build_generation_contract_snapshot, merge_generation_contract_runtime_snapshot,
        GenerationIntentKind, GenerationIntentV1, GenerationTarget, StoryPacketV1,
    };

    use super::build_chapter_succeeded_business_checkpoint;

    fn contract_runtime_state() -> (serde_json::Value, String) {
        let target = GenerationTarget::chapter_batch(
            "project-1",
            vec!["chapter-1".to_owned(), "chapter-2".to_owned()],
        );
        let snapshot = build_generation_contract_snapshot(
            StoryPacketV1::new("project-1", target.clone()),
            GenerationIntentV1::new(GenerationIntentKind::BatchChapterGenerate, target),
        )
        .expect("build contract");
        let input_digest = snapshot.input_digest.clone();
        let mut runtime_state = json!({"phase": "generating"});
        merge_generation_contract_runtime_snapshot(&mut runtime_state, &snapshot)
            .expect("merge contract");
        (runtime_state, input_digest)
    }

    #[test]
    fn chapter_succeeded_checkpoint_should_use_contract_digest_and_monotonic_revision() {
        let (mut runtime_state, input_digest) = contract_runtime_state();
        let recorded_at = Utc
            .with_ymd_and_hms(2026, 7, 16, 2, 0, 0)
            .single()
            .expect("timestamp");
        let first = build_chapter_succeeded_business_checkpoint(
            "task-1",
            Some("chapter-1"),
            1,
            &runtime_state,
            recorded_at,
        )
        .expect("build first")
        .expect("first checkpoint");
        assert_eq!(first.revision, 1);
        assert_eq!(first.input_digest, input_digest);
        merge_business_checkpoint_runtime_state(&mut runtime_state, &first)
            .expect("merge first checkpoint");

        let repeated = build_chapter_succeeded_business_checkpoint(
            "task-1",
            Some("chapter-1"),
            0,
            &runtime_state,
            recorded_at,
        )
        .expect("build repeated")
        .expect("repeated checkpoint");
        assert_eq!(repeated.revision, 1);
        assert_eq!(repeated.idempotency_key, first.idempotency_key);

        let next = build_chapter_succeeded_business_checkpoint(
            "task-1",
            Some("chapter-2"),
            2,
            &runtime_state,
            recorded_at,
        )
        .expect("build next")
        .expect("next checkpoint");
        assert_eq!(next.revision, 2);
        assert_ne!(next.idempotency_key, first.idempotency_key);
    }

    #[test]
    fn chapter_succeeded_checkpoint_should_skip_legacy_or_out_of_contract_runtime_state() {
        let recorded_at = Utc
            .with_ymd_and_hms(2026, 7, 16, 2, 0, 0)
            .single()
            .expect("timestamp");
        assert_eq!(
            build_chapter_succeeded_business_checkpoint(
                "task-1",
                Some("chapter-1"),
                1,
                &json!({"checkpoint": {"stage": "chapter_succeeded"}}),
                recorded_at,
            )
            .expect("legacy result"),
            None
        );

        let (runtime_state, _) = contract_runtime_state();
        assert_eq!(
            build_chapter_succeeded_business_checkpoint(
                "task-1",
                Some("chapter-outside"),
                1,
                &runtime_state,
                recorded_at,
            )
            .expect("out of contract result"),
            None
        );
        assert!(matches!(
            crate::services::business_checkpoint_service::read_business_checkpoint_runtime_state(
                &runtime_state
            ),
            BusinessCheckpointRead::Missing
        ));
    }
}
