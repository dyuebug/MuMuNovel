use chrono::{NaiveDateTime, Utc};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_generation_runtime_service::snapshot_persistence_owner::{
    build_chapter_generation_snapshot_owner_contract, upsert_chapter_generation_runtime_snapshot,
};
use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;

pub(crate) fn build_single_generation_runtime_checkpoint_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_runtime_state_service::runtime_checkpoint_owner",
        "scope": "runtime_checkpoint_projection_candidate_gateway_metadata_and_persisted_stage_updates",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_runtime_state_service/runtime_checkpoint_owner.rs",
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

pub(crate) fn merge_single_generation_terminal_checkpoint_payload(
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
