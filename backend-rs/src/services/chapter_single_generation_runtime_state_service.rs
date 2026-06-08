use chrono::{NaiveDateTime, Utc};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};

use crate::models::batch_generation_task;
use crate::services::chapter_analysis_runtime_service::analyze_generated_chapter_follow_up;
use crate::services::chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig;
use crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationExecutionInput;
use crate::services::chapter_generation_quality_gate_semantics_service::{
    manual_review_label_from_quality_context_with_retry_budget,
    retryable_repair_label_from_quality_context_with_retry_budget,
};
use crate::services::chapter_generation_runtime_service::{
    generate_and_persist_chapter_content_with_candidate_route_gateway, GeneratedChapterResult,
};
use crate::services::chapter_generation_snapshot_service::upsert_chapter_generation_runtime_snapshot;

pub(crate) use crate::services::chapter_generation_execution_contract_service::build_prompt_overrides_from_compat_options;

#[derive(Debug, Clone)]
pub(crate) struct SingleGenerationRuntimeLaunchInput {
    pub(crate) chapter_id: String,
    pub(crate) user_id: String,
    pub(crate) execution_input: SingleChapterGenerationExecutionInput,
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
    ) -> serde_json::Value {
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
        let mut checkpoint = serde_json::json!({
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
                object.insert("word_count".to_string(), serde_json::json!(value.max(0)));
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
}

pub(crate) fn build_single_generation_runtime_checkpoint_for_stage(
    stage: SingleGenerationSnapshotStage,
    chapter_id: &str,
    current_chapter_number: Option<i32>,
    word_count: Option<i32>,
) -> serde_json::Value {
    stage.build_checkpoint(chapter_id, current_chapter_number, word_count)
}

impl SingleGenerationTaskStage {
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

    pub(crate) async fn persist_with_checkpoint(
        self,
        db: &DatabaseConnection,
        task_id: &str,
        checkpoint_stage: SingleGenerationSnapshotStage,
        chapter_id: &str,
        chapter_number: Option<i32>,
        word_count: Option<i32>,
        error_message: Option<String>,
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
            build_single_generation_runtime_checkpoint_for_stage(
                checkpoint_stage,
                chapter_id,
                chapter_number,
                word_count,
            ),
            Utc::now().naive_utc(),
        )
        .await
    }
}

impl SingleGenerationRuntimeLaunchInput {
    pub(crate) async fn execute_generation_with_gateway_config(
        self,
        db: &DatabaseConnection,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Result<GeneratedChapterResult, String> {
        let Self {
            chapter_id,
            user_id,
            execution_input,
        } = self;
        let SingleChapterGenerationExecutionInput {
            target_word_count,
            compat_options,
            execution_config,
        } = execution_input;
        let crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
            ai_config,
            provider_payload,
        } = execution_config;

        let prompt_overrides = build_prompt_overrides_from_compat_options(&compat_options);
        generate_and_persist_chapter_content_with_candidate_route_gateway(
            db,
            &user_id,
            &chapter_id,
            target_word_count,
            provider_payload,
            &prompt_overrides,
            ai_config,
            candidate_gateway_config,
        )
        .await
    }
}

pub(crate) fn default_single_generation_candidate_gateway_config(
) -> ChapterCandidateRouteGatewayConfig {
    ChapterCandidateRouteGatewayConfig {
        rust_executor_enabled: false,
        fallback_on_rust_error: true,
        disabled_reason: Some("single generation direct AI fallback remains default".to_string()),
        rollback_boundary: "legacy_single_generation_direct_ai".to_string(),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SingleGenerationRuntimeLifecyclePlan {
    task_id: String,
    chapter_id: String,
    runtime_user_id: String,
    enable_analysis: bool,
    candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    runtime_input: SingleGenerationRuntimeLaunchInput,
}

impl SingleGenerationRuntimeLifecyclePlan {
    pub(crate) fn from_runtime_launch(
        task_id: String,
        runtime_input: SingleGenerationRuntimeLaunchInput,
    ) -> Self {
        Self::from_runtime_launch_with_gateway_config(
            task_id,
            runtime_input,
            default_single_generation_candidate_gateway_config(),
        )
    }

    pub(crate) fn from_runtime_launch_with_gateway_config(
        task_id: String,
        runtime_input: SingleGenerationRuntimeLaunchInput,
        candidate_gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Self {
        let chapter_id = runtime_input.chapter_id.clone();
        let runtime_user_id = runtime_input.user_id.clone();
        let enable_analysis = runtime_input
            .execution_input
            .compat_options
            .enable_analysis();

        Self {
            task_id,
            chapter_id,
            runtime_user_id,
            enable_analysis,
            candidate_gateway_config,
            runtime_input,
        }
    }

    pub(crate) fn spawn(self, db: DatabaseConnection) {
        tokio::spawn(async move {
            self.execute(&db).await;
        });
    }

    async fn execute(self, db: &DatabaseConnection) {
        let _ = SingleGenerationTaskStage::persist_runtime_preparation(
            db,
            &self.task_id,
            &self.chapter_id,
        )
        .await;
        let outcome = self.outcome();

        let _ = match self.execute_generation(db).await {
            Ok(generated_result) => {
                outcome
                    .persist_generated_result(db, &generated_result)
                    .await
            }
            Err(error) => outcome.persist_failed_generation(db, error).await,
        };
    }

    async fn execute_generation(
        &self,
        db: &DatabaseConnection,
    ) -> Result<GeneratedChapterResult, String> {
        self.runtime_input
            .clone()
            .execute_generation_with_gateway_config(db, self.candidate_gateway_config.clone())
            .await
    }

    fn outcome(&self) -> SingleGenerationRuntimeOutcome {
        SingleGenerationRuntimeOutcome::new(
            self.task_id.clone(),
            self.chapter_id.clone(),
            self.runtime_user_id.clone(),
            self.enable_analysis,
        )
    }
}

#[derive(Debug, Clone)]
struct SingleGenerationRuntimeOutcome {
    task_id: String,
    chapter_id: String,
    runtime_user_id: String,
    enable_analysis: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SingleGenerationFollowUpAnalysisDecision {
    manual_review_label: String,
    quality_metrics: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
struct SingleGenerationQualityGateTerminalState {
    checkpoint_payload: Value,
    error_message: String,
    failed_entry: Value,
}

impl SingleGenerationRuntimeOutcome {
    fn new(
        task_id: String,
        chapter_id: String,
        runtime_user_id: String,
        enable_analysis: bool,
    ) -> Self {
        Self {
            task_id,
            chapter_id,
            runtime_user_id,
            enable_analysis,
        }
    }

    async fn persist_generated_result(
        &self,
        db: &DatabaseConnection,
        generated_result: &GeneratedChapterResult,
    ) -> Result<(), String> {
        let persisted_task = self.load_task(db).await?;
        if let Some(terminal_state) =
            self.resolve_quality_gate_terminal_state(&persisted_task, generated_result, None)
        {
            return self
                .persist_quality_gate_terminal_generation(db, generated_result, terminal_state)
                .await;
        }

        if let Some(analysis_decision) = self.run_follow_up_analysis(db, generated_result).await {
            let terminal_state = self
                .resolve_quality_gate_terminal_state(
                    &persisted_task,
                    generated_result,
                    Some(&analysis_decision),
                )
                .unwrap_or_else(|| {
                    build_single_generation_manual_review_terminal_state(
                        &persisted_task,
                        generated_result,
                        &analysis_decision.manual_review_label,
                        analysis_decision.quality_metrics.as_ref(),
                    )
                });
            return self
                .persist_quality_gate_terminal_generation(db, generated_result, terminal_state)
                .await;
        }

        self.persist_completed_generation(db, generated_result)
            .await
    }

    async fn persist_failed_generation(
        &self,
        db: &DatabaseConnection,
        error: String,
    ) -> Result<(), String> {
        let persisted_task = self.load_task(db).await?;
        let terminal_state = build_single_generation_error_terminal_state(
            &persisted_task,
            &self.chapter_id,
            None,
            None,
            &error,
        );

        self.persist_quality_gate_terminal_generation_without_result(db, terminal_state)
            .await
    }

    async fn run_follow_up_analysis(
        &self,
        db: &DatabaseConnection,
        generated_result: &GeneratedChapterResult,
    ) -> Option<SingleGenerationFollowUpAnalysisDecision> {
        if !self.enable_analysis {
            return None;
        }

        analyze_generated_chapter_follow_up(db, &self.runtime_user_id, generated_result)
            .await
            .ok()
            .and_then(|payload| {
                resolve_single_generation_manual_review_label_from_analysis_payload(&payload).map(
                    |manual_review_label| SingleGenerationFollowUpAnalysisDecision {
                        manual_review_label,
                        quality_metrics: payload.get("quality_metrics").cloned(),
                    },
                )
            })
    }

    async fn persist_quality_gate_terminal_generation(
        &self,
        db: &DatabaseConnection,
        generated_result: &GeneratedChapterResult,
        terminal_state: SingleGenerationQualityGateTerminalState,
    ) -> Result<(), String> {
        let chapter_number = Some(generated_result.chapter_number);
        let word_count = Some(generated_result.word_count);
        self.persist_quality_gate_terminal_generation_inner(
            db,
            &generated_result.chapter_id,
            chapter_number,
            word_count,
            terminal_state,
        )
        .await
    }

    async fn persist_quality_gate_terminal_generation_without_result(
        &self,
        db: &DatabaseConnection,
        terminal_state: SingleGenerationQualityGateTerminalState,
    ) -> Result<(), String> {
        self.persist_quality_gate_terminal_generation_inner(
            db,
            &self.chapter_id,
            None,
            None,
            terminal_state,
        )
        .await
    }

    async fn persist_quality_gate_terminal_generation_inner(
        &self,
        db: &DatabaseConnection,
        chapter_id: &str,
        chapter_number: Option<i32>,
        word_count: Option<i32>,
        terminal_state: SingleGenerationQualityGateTerminalState,
    ) -> Result<(), String> {
        let SingleGenerationQualityGateTerminalState {
            checkpoint_payload,
            error_message,
            failed_entry,
        } = terminal_state;
        let merged_checkpoint_payload = merge_single_generation_terminal_checkpoint_payload(
            build_single_generation_runtime_checkpoint_for_stage(
                SingleGenerationSnapshotStage::Failed,
                chapter_id,
                chapter_number,
                word_count,
            ),
            checkpoint_payload,
        );
        upsert_chapter_generation_runtime_snapshot(
            db,
            &self.task_id,
            merged_checkpoint_payload,
            Utc::now().naive_utc(),
        )
        .await?;

        let Some(task_model) = self.load_task(db).await? else {
            return Ok(());
        };

        let mut active: batch_generation_task::ActiveModel = task_model.clone().into();
        let now = Utc::now().naive_utc();
        SingleGenerationTaskStage::Failed.apply_to_active_model(
            &mut active,
            chapter_id,
            chapter_number,
            Some(error_message),
            now,
        );
        active.failed_chapters = Set(append_single_generation_failed_chapter_entry(
            &task_model.failed_chapters,
            Some(&failed_entry),
        ));
        active.update(db).await.map_err(|error| error.to_string())?;
        Ok(())
    }

    async fn persist_completed_generation(
        &self,
        db: &DatabaseConnection,
        generated_result: &GeneratedChapterResult,
    ) -> Result<(), String> {
        let GeneratedChapterResult {
            chapter_id,
            chapter_number,
            word_count,
            ..
        } = generated_result;

        SingleGenerationTaskStage::Completed
            .persist_with_checkpoint(
                db,
                &self.task_id,
                SingleGenerationSnapshotStage::Finalizing,
                chapter_id,
                Some(*chapter_number),
                Some(*word_count),
                None,
            )
            .await?;

        SingleGenerationTaskStage::Completed
            .persist_with_checkpoint(
                db,
                &self.task_id,
                SingleGenerationSnapshotStage::Completed,
                chapter_id,
                Some(*chapter_number),
                Some(*word_count),
                None,
            )
            .await
    }

    async fn load_task(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Option<batch_generation_task::Model>, String> {
        batch_generation_task::Entity::find_by_id(&self.task_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())
    }

    fn resolve_quality_gate_terminal_state(
        &self,
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
}

fn resolve_single_generation_manual_review_label_from_analysis_payload(
    payload: &serde_json::Value,
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

fn append_single_generation_failed_chapter_entry(
    failed_chapters: &Value,
    failed_entry: Option<&Value>,
) -> Value {
    let mut items = failed_chapters.as_array().cloned().unwrap_or_default();
    if let Some(entry) = failed_entry.filter(|entry| entry.is_object()) {
        items.push(entry.clone());
    }
    Value::Array(items)
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

fn build_single_generation_error_terminal_state(
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

#[cfg(test)]
mod tests {
    use crate::ai::AIConfig;
    use chrono::NaiveDate;
    use sea_orm::Set;
    use serde_json::json;

    use super::{
        build_prompt_overrides_from_compat_options,
        build_single_generation_runtime_checkpoint_for_stage,
        resolve_single_generation_manual_review_label_from_analysis_payload,
        resolve_single_generation_manual_review_label_from_quality_context, ModelFieldUpdate,
        SingleGenerationRuntimeLaunchInput, SingleGenerationRuntimeLifecyclePlan,
        SingleGenerationRuntimeOutcome, SingleGenerationSnapshotStage, SingleGenerationTaskStage,
        TaskTimestampUpdate,
    };
    use crate::models::batch_generation_task;
    use crate::services::chapter_generation_execution_contract_service::{
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
    };
    use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
    use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
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
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
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
                    execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
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
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
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
                execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
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
                    crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig {
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
    fn should_resolve_manual_review_label_from_single_generation_quality_context() {
        let label = resolve_single_generation_manual_review_label_from_quality_context(
            &GeneratedChapterResult {
                quality_gate_action: Some("manual_review".to_string()),
                quality_gate_message: Some("连续性需人工复核".to_string()),
                ..Default::default()
            },
            Some(&json!({
                "quality_gate": {
                    "decision": "manual_review",
                    "label": "连续性需人工复核"
                }
            })),
            0,
            3,
        );

        assert_eq!(label.as_deref(), Some("连续性需人工复核"));
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
