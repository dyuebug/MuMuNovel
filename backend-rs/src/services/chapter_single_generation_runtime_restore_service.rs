use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde_json::{json, Value};

use crate::models::{batch_generation_task, chapter, generation_history};
use crate::services::chapter_analysis_read_context_service::load_chapter_analysis_read_context;
use crate::services::chapter_generation_execution_contract_service::{
    SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
};
use crate::services::chapter_generation_quality_runtime_context_service::{
    apply_generation_quality_runtime_context_to_payload,
    resolve_generation_quality_runtime_context_for_seed,
    resolve_generation_quality_runtime_context_from_persisted_sources,
    GenerationQualityRuntimeContext,
};
use crate::services::chapter_generation_request_runtime_state_service::{
    active_story_repair_payload_from_runtime_state, BatchGenerationRequestRuntimeState,
};
use crate::services::chapter_generation_snapshot_service::upsert_chapter_generation_runtime_snapshot;
use crate::services::chapter_generation_target_word_count_service::normalize_chapter_generation_target_word_count;
use crate::services::chapter_quality_metrics_query_service::{
    build_chapter_analysis_quality_fragments, build_chapter_quality_metrics_fragments,
    ChapterQualityMetricsFragments,
};
use crate::services::chapter_story_repair_quality_context_service::{
    aggregate_story_repair_quality_summaries,
    resolve_active_story_repair_payload_with_quality_fallback,
    restore_story_repair_compat_options_from_active_snapshot,
};

use super::chapter_single_generation_prepare_service::{
    build_single_generation_runtime_payload_base, estimated_single_generation_task_minutes,
    load_single_chapter_generation_target,
    prepare_single_chapter_generation_execution_config_from_runtime_state,
    PrepareSingleChapterGenerationRequestError, SingleChapterGenerationRequest,
    SingleChapterGenerationTarget,
};
use super::chapter_single_generation_runtime_state_service::{
    build_single_generation_runtime_checkpoint_for_stage, SingleGenerationRuntimeLaunchInput,
    SingleGenerationSnapshotStage,
};
use super::settings_service::SettingsService;

const SINGLE_GENERATION_BACKGROUND_MAX_RETRIES: i32 = 3;

pub(crate) fn build_single_generation_runtime_launch_input_from_request_runtime_state(
    chapter_target: &SingleChapterGenerationTarget,
    user_id: &str,
    target_word_count: i32,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig,
) -> SingleGenerationRuntimeLaunchInput {
    SingleGenerationRuntimeLaunchInput {
        chapter_id: chapter_target.chapter_id.clone(),
        user_id: user_id.to_string(),
        execution_input: SingleChapterGenerationExecutionInput {
            target_word_count,
            compat_options: request_runtime_state.compat_options.clone(),
            execution_config,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SingleGenerationTaskPersistenceSeed {
    pub(crate) id: String,
    pub(crate) project_id: String,
    pub(crate) user_id: String,
    pub(crate) start_chapter_number: i32,
    pub(crate) chapter_count: i32,
    pub(crate) chapter_ids: Value,
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: i32,
    pub(crate) enable_analysis: bool,
    pub(crate) total_chapters: i32,
    pub(crate) current_chapter_id: Option<String>,
    pub(crate) current_chapter_number: Option<i32>,
    pub(crate) max_retries: i32,
}

impl SingleGenerationTaskPersistenceSeed {
    pub(crate) fn into_active_model(
        self,
        now: chrono::NaiveDateTime,
    ) -> batch_generation_task::ActiveModel {
        batch_generation_task::ActiveModel {
            id: Set(self.id),
            project_id: Set(self.project_id),
            user_id: Set(self.user_id),
            start_chapter_number: Set(self.start_chapter_number),
            chapter_count: Set(self.chapter_count),
            chapter_ids: Set(self.chapter_ids),
            style_id: Set(self.style_id),
            target_word_count: Set(self.target_word_count),
            enable_analysis: Set(self.enable_analysis),
            status: Set("pending".to_string()),
            total_chapters: Set(self.total_chapters),
            completed_chapters: Set(0),
            failed_chapters: Set(json!([])),
            current_chapter_id: Set(self.current_chapter_id),
            current_chapter_number: Set(self.current_chapter_number),
            current_retry_count: Set(0),
            max_retries: Set(self.max_retries),
            created_at: Set(Some(now)),
            started_at: Set(None),
            completed_at: Set(None),
            error_message: Set(None),
        }
    }
}

pub(crate) fn build_single_generation_pending_checkpoint(
    chapter_target: &SingleChapterGenerationTarget,
) -> Value {
    build_single_generation_runtime_checkpoint_for_stage(
        SingleGenerationSnapshotStage::Pending,
        &chapter_target.chapter_id,
        Some(chapter_target.chapter_number),
        None,
    )
}

fn build_single_generation_background_task_persistence_seed(
    task_id: String,
    chapter_target: &SingleChapterGenerationTarget,
    user_id: String,
    target_word_count: i32,
    enable_analysis: bool,
) -> SingleGenerationTaskPersistenceSeed {
    SingleGenerationTaskPersistenceSeed {
        id: task_id,
        project_id: chapter_target.project_id.clone(),
        user_id,
        start_chapter_number: chapter_target.chapter_number,
        chapter_count: 1,
        chapter_ids: json!([{
            "id": chapter_target.chapter_id,
            "chapter_number": chapter_target.chapter_number,
            "title": chapter_target.title,
        }]),
        style_id: None,
        target_word_count,
        enable_analysis,
        total_chapters: 1,
        current_chapter_id: Some(chapter_target.chapter_id.clone()),
        current_chapter_number: Some(chapter_target.chapter_number),
        max_retries: SINGLE_GENERATION_BACKGROUND_MAX_RETRIES,
    }
}

#[cfg(test)]
fn build_single_generation_background_task_active_model(
    task_id: String,
    chapter_target: &SingleChapterGenerationTarget,
    user_id: String,
    target_word_count: i32,
    enable_analysis: bool,
    now: chrono::NaiveDateTime,
) -> batch_generation_task::ActiveModel {
    build_single_generation_background_task_persistence_seed(
        task_id,
        chapter_target,
        user_id,
        target_word_count,
        enable_analysis,
    )
    .into_active_model(now)
}

fn merge_single_generation_runtime_state(
    current_workflow_runtime_state: Option<&Value>,
    incoming_workflow_runtime_state: &Value,
) -> Value {
    match (
        current_workflow_runtime_state.cloned(),
        incoming_workflow_runtime_state.clone(),
    ) {
        (Some(Value::Object(mut current)), Value::Object(incoming)) => {
            for (key, value) in incoming {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, incoming) => incoming,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SingleGenerationStartupSnapshotPlan {
    runtime_state: Value,
    quality_runtime_context: GenerationQualityRuntimeContext,
    active_story_repair_payload: Option<Value>,
    quality_history_context: Option<Value>,
}

impl SingleGenerationStartupSnapshotPlan {
    pub(crate) fn from_pending_checkpoint(
        pending_checkpoint: Value,
        runtime_state_seed: Value,
    ) -> Self {
        let runtime_state =
            merge_single_generation_runtime_state(Some(&pending_checkpoint), &runtime_state_seed);
        let quality_runtime_context =
            resolve_generation_quality_runtime_context_from_persisted_sources(
                "chapter",
                runtime_state.get("latest_quality_metrics"),
                runtime_state.get("quality_metrics_history"),
                runtime_state.get("quality_metrics_summary_state"),
                runtime_state.get("quality_metrics_summary"),
            );
        let active_story_repair_payload =
            active_story_repair_payload_from_runtime_state(Some(&runtime_state));
        let quality_history_context = runtime_state
            .get("quality_history_context")
            .cloned()
            .or_else(|| quality_runtime_context.quality_history_context.clone());

        Self {
            runtime_state,
            quality_runtime_context,
            active_story_repair_payload,
            quality_history_context,
        }
    }

    pub(crate) fn runtime_state(&self) -> &Value {
        &self.runtime_state
    }

    pub(crate) fn quality_runtime_context(&self) -> GenerationQualityRuntimeContext {
        self.quality_runtime_context.clone()
    }

    pub(crate) fn latest_quality_metrics(&self) -> Option<&Value> {
        self.quality_runtime_context.latest_quality_metrics.as_ref()
    }

    pub(crate) fn quality_metrics_history(&self) -> Option<&Value> {
        self.quality_runtime_context
            .quality_metrics_history
            .as_ref()
    }

    pub(crate) fn quality_metrics_summary_state(&self) -> Option<&Value> {
        self.quality_runtime_context
            .quality_metrics_summary_state
            .as_ref()
    }

    pub(crate) fn quality_metrics_summary(&self) -> Option<&Value> {
        self.quality_runtime_context
            .quality_metrics_summary
            .as_ref()
    }

    pub(crate) fn active_story_repair_payload(&self) -> Option<Value> {
        self.active_story_repair_payload.clone()
    }

    pub(crate) fn quality_history_context(&self) -> Option<Value> {
        self.quality_history_context.clone()
    }

    pub(crate) async fn persist(
        self,
        db: &DatabaseConnection,
        task_id: &str,
    ) -> Result<(), String> {
        upsert_chapter_generation_runtime_snapshot(
            db,
            task_id,
            self.runtime_state,
            chrono::Utc::now().naive_utc(),
        )
        .await
    }
}

pub(crate) fn build_single_generation_runtime_state_payload_from_sources(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    existing_quality_metrics_history: Option<&Value>,
    existing_quality_metrics_summary_state: Option<&Value>,
    derived_source: &str,
    derived_source_label: &str,
) -> Value {
    let mut payload = serde_json::Map::from_iter([(
        "batch_request_runtime_state".to_string(),
        json!(request_runtime_state),
    )]);

    let active_story_repair_payload = resolve_active_story_repair_payload_with_quality_fallback(
        request_runtime_state
            .active_story_repair_payload_with_scope("chapter")
            .as_ref(),
        quality_metrics_summary,
        latest_quality_metrics,
        "chapter",
        derived_source,
        derived_source_label,
    );

    if let Some(active_story_repair_payload) = active_story_repair_payload {
        payload.insert(
            "active_story_repair_payload".to_string(),
            active_story_repair_payload,
        );
    }
    let resolved_quality_context = resolve_generation_quality_runtime_context_for_seed(
        "chapter",
        existing_quality_metrics_summary_state,
        existing_quality_metrics_history,
        latest_quality_metrics,
        quality_metrics_summary,
        20,
    );
    apply_generation_quality_runtime_context_to_payload(
        &mut payload,
        resolved_quality_context,
        None,
        quality_metrics_summary.cloned(),
        None,
    );

    Value::Object(payload)
}

pub(crate) fn build_single_generation_runtime_state_payload_from_parts(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    existing_quality_metrics_history: Option<&Value>,
    existing_quality_metrics_summary_state: Option<&Value>,
) -> Value {
    build_single_generation_runtime_state_payload_from_sources(
        request_runtime_state,
        quality_metrics_summary,
        latest_quality_metrics,
        existing_quality_metrics_history,
        existing_quality_metrics_summary_state,
        "current_chapter_quality",
        "Current chapter quality",
    )
}

pub(crate) fn resolve_single_generation_runtime_compat_options_from_seed(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    runtime_state_payload: &Value,
) -> SingleChapterGenerationCompatOptions {
    let restored_quality_context =
        resolve_generation_quality_runtime_context_from_persisted_sources(
            "chapter",
            runtime_state_payload.get("latest_quality_metrics"),
            runtime_state_payload.get("quality_metrics_history"),
            runtime_state_payload.get("quality_metrics_summary_state"),
            runtime_state_payload.get("quality_metrics_summary"),
        );

    restore_story_repair_compat_options_from_active_snapshot(
        &request_runtime_state.compat_options,
        active_story_repair_payload_from_runtime_state(Some(runtime_state_payload)).as_ref(),
        restored_quality_context.quality_metrics_summary.as_ref(),
        restored_quality_context.latest_quality_metrics.as_ref(),
    )
}

pub(crate) fn build_single_generation_runtime_launch_input(
    chapter_id: String,
    user_id: String,
    execution_input: SingleChapterGenerationExecutionInput,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    runtime_state_payload: &Value,
) -> SingleGenerationRuntimeLaunchInput {
    let resolved_compat_options = resolve_single_generation_runtime_compat_options_from_seed(
        request_runtime_state,
        runtime_state_payload,
    );
    let SingleChapterGenerationExecutionInput {
        target_word_count,
        execution_config,
        ..
    } = execution_input;

    SingleGenerationRuntimeLaunchInput {
        chapter_id,
        user_id,
        execution_input: SingleChapterGenerationExecutionInput {
            target_word_count,
            compat_options: resolved_compat_options,
            execution_config,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SingleGenerationRuntimeSeedSource {
    CurrentChapterQuality,
    RecentHistorySummary,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RestoredSingleGenerationRuntimeState {
    request_runtime_state: BatchGenerationRequestRuntimeState,
    startup_snapshot_plan: SingleGenerationStartupSnapshotPlan,
    seed_source: SingleGenerationRuntimeSeedSource,
}

impl RestoredSingleGenerationRuntimeState {
    pub(crate) fn from_quality_fragments(
        pending_checkpoint: Value,
        request_runtime_state: &BatchGenerationRequestRuntimeState,
        quality_fragments: ChapterQualityMetricsFragments,
        recent_history_summary: Option<Value>,
    ) -> Self {
        let runtime_state_payload = if quality_fragments.quality_metrics_summary.is_some()
            || quality_fragments.latest_quality_metrics.is_some()
        {
            build_single_generation_runtime_state_payload_from_parts(
                request_runtime_state,
                quality_fragments.quality_metrics_summary.as_ref(),
                quality_fragments.latest_quality_metrics.as_ref(),
                quality_fragments.quality_metrics_history.as_ref(),
                quality_fragments.quality_metrics_summary_state.as_ref(),
            )
        } else {
            build_single_generation_runtime_state_payload_from_sources(
                request_runtime_state,
                recent_history_summary.as_ref(),
                None,
                None,
                None,
                "recent_history_summary",
                "Recent history summary",
            )
        };
        let seed_source = if quality_fragments.quality_metrics_summary.is_some()
            || quality_fragments.latest_quality_metrics.is_some()
        {
            SingleGenerationRuntimeSeedSource::CurrentChapterQuality
        } else {
            SingleGenerationRuntimeSeedSource::RecentHistorySummary
        };

        Self {
            request_runtime_state: request_runtime_state.clone(),
            startup_snapshot_plan: SingleGenerationStartupSnapshotPlan::from_pending_checkpoint(
                pending_checkpoint,
                runtime_state_payload,
            ),
            seed_source,
        }
    }

    pub(crate) fn into_startup_runtime_launch_parts(
        self,
        chapter_id: String,
        user_id: String,
        execution_input: SingleChapterGenerationExecutionInput,
    ) -> (
        SingleGenerationStartupSnapshotPlan,
        SingleGenerationRuntimeLaunchInput,
    ) {
        let Self {
            request_runtime_state,
            startup_snapshot_plan,
            ..
        } = self;
        let runtime_input = build_single_generation_runtime_launch_input(
            chapter_id,
            user_id,
            execution_input,
            &request_runtime_state,
            startup_snapshot_plan.runtime_state(),
        );

        (startup_snapshot_plan, runtime_input)
    }

    #[cfg(test)]
    pub(crate) fn runtime_state_payload(&self) -> &Value {
        self.startup_snapshot_plan.runtime_state()
    }

    #[cfg(test)]
    pub(crate) fn request_runtime_state(&self) -> &BatchGenerationRequestRuntimeState {
        &self.request_runtime_state
    }

    #[cfg(test)]
    pub(crate) fn seed_source(&self) -> SingleGenerationRuntimeSeedSource {
        self.seed_source
    }
}

async fn load_recent_single_generation_story_repair_quality_summary(
    db: &DatabaseConnection,
    project_id: &str,
    before_chapter_number: i32,
) -> Result<Option<Value>, String> {
    if before_chapter_number <= 1 {
        return Ok(None);
    }

    let previous_chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .filter(chapter::Column::ChapterNumber.lt(before_chapter_number))
        .order_by_desc(chapter::Column::ChapterNumber)
        .limit(3)
        .all(db)
        .await
        .map_err(|error| {
            format!("load previous chapters for single story repair failed: {error}")
        })?;

    if previous_chapters.is_empty() {
        return Ok(None);
    }

    let mut summaries = Vec::new();
    for previous_chapter in previous_chapters {
        let histories = generation_history::Entity::find()
            .filter(generation_history::Column::ChapterId.eq(Some(previous_chapter.id.clone())))
            .order_by_desc(generation_history::Column::CreatedAt)
            .limit(30)
            .all(db)
            .await
            .map_err(|error| {
                format!("load generation histories for single story repair failed: {error}")
            })?;
        let quality_fragments = build_chapter_analysis_quality_fragments(&histories, None);
        if let Some(summary) = quality_fragments.quality_metrics_summary {
            summaries.push(summary);
        }
    }

    Ok(aggregate_story_repair_quality_summaries(
        &summaries, "chapter",
    ))
}

pub(crate) async fn restore_single_generation_runtime_state(
    db: &DatabaseConnection,
    chapter_target: &SingleChapterGenerationTarget,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
) -> Result<RestoredSingleGenerationRuntimeState, String> {
    let read_context = load_chapter_analysis_read_context(db, &chapter_target.chapter_id).await?;
    let quality_fragments = build_chapter_quality_metrics_fragments(
        &read_context.histories,
        read_context.candidate_attempt.as_ref(),
    );
    let recent_history_summary = if quality_fragments.quality_metrics_summary.is_some()
        || quality_fragments.latest_quality_metrics.is_some()
    {
        None
    } else {
        load_recent_single_generation_story_repair_quality_summary(
            db,
            &chapter_target.project_id,
            chapter_target.chapter_number,
        )
        .await?
    };

    Ok(
        RestoredSingleGenerationRuntimeState::from_quality_fragments(
            build_single_generation_pending_checkpoint(chapter_target),
            request_runtime_state,
            quality_fragments,
            recent_history_summary,
        ),
    )
}

pub(crate) async fn prepare_single_chapter_runtime_launch_input_from_request_runtime_state(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_target: &SingleChapterGenerationTarget,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    target_word_count: i32,
) -> Result<SingleGenerationRuntimeLaunchInput, PrepareSingleChapterGenerationRequestError> {
    let execution_config = prepare_single_chapter_generation_execution_config_from_runtime_state(
        db,
        user_id,
        chapter_target,
        request_runtime_state,
    )
    .await?;

    Ok(
        build_single_generation_runtime_launch_input_from_request_runtime_state(
            chapter_target,
            user_id,
            target_word_count,
            request_runtime_state,
            execution_config,
        ),
    )
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedSingleChapterGenerationRestoredRuntimeLaunch {
    chapter_target: SingleChapterGenerationTarget,
    startup_snapshot_plan: SingleGenerationStartupSnapshotPlan,
    runtime_input: SingleGenerationRuntimeLaunchInput,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedSingleGenerationBackgroundLaunchParts {
    pub(crate) task_seed: SingleGenerationTaskPersistenceSeed,
    pub(crate) startup_snapshot_plan: SingleGenerationStartupSnapshotPlan,
    pub(crate) response_payload: Value,
    pub(crate) runtime_input: SingleGenerationRuntimeLaunchInput,
}

impl PreparedSingleChapterGenerationRestoredRuntimeLaunch {
    pub(crate) async fn prepare(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        request: &SingleChapterGenerationRequest,
    ) -> Result<Self, PrepareSingleChapterGenerationRequestError> {
        request.validate_request_bounds()?;

        let chapter_target = load_single_chapter_generation_target(db, chapter_id, user_id).await?;
        Self::prepare_from_target(db, user_id, request, chapter_target).await
    }

    pub(crate) async fn prepare_from_target(
        db: &DatabaseConnection,
        user_id: &str,
        request: &SingleChapterGenerationRequest,
        chapter_target: SingleChapterGenerationTarget,
    ) -> Result<Self, PrepareSingleChapterGenerationRequestError> {
        request.validate_request_bounds()?;
        let normalized_target_word_count =
            normalize_chapter_generation_target_word_count(request.target_word_count);
        let web_research_default = SettingsService::resolve_web_research_enabled(db, user_id)
            .await
            .map_err(|error| {
                PrepareSingleChapterGenerationRequestError::Config(error.to_string())
            })?;
        let compat_options = request.compat_options_with_web_research_default(web_research_default);
        let request_runtime_state =
            BatchGenerationRequestRuntimeState::new(compat_options.clone(), request.model.clone());
        let execution_config =
            prepare_single_chapter_generation_execution_config_from_runtime_state(
                db,
                user_id,
                &chapter_target,
                &request_runtime_state,
            )
            .await?;
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: normalized_target_word_count,
            compat_options,
            execution_config,
        };
        let restored_runtime_state =
            restore_single_generation_runtime_state(db, &chapter_target, &request_runtime_state)
                .await
                .map_err(PrepareSingleChapterGenerationRequestError::Internal)?;
        let (startup_snapshot_plan, runtime_input) = restored_runtime_state
            .into_startup_runtime_launch_parts(
                chapter_target.chapter_id.clone(),
                user_id.to_string(),
                execution_input,
            );

        Ok(Self {
            chapter_target,
            startup_snapshot_plan,
            runtime_input,
        })
    }

    pub(crate) async fn prepare_runtime_launch_input(
        db: &DatabaseConnection,
        chapter_id: &str,
        user_id: &str,
        request: &SingleChapterGenerationRequest,
    ) -> Result<SingleGenerationRuntimeLaunchInput, PrepareSingleChapterGenerationRequestError>
    {
        Ok(Self::prepare(db, chapter_id, user_id, request)
            .await?
            .into_runtime_launch_input())
    }

    pub(crate) async fn prepare_background_launch_parts_from_target(
        db: &DatabaseConnection,
        user_id: &str,
        request: &SingleChapterGenerationRequest,
        chapter_target: SingleChapterGenerationTarget,
        task_id: String,
    ) -> Result<
        PreparedSingleGenerationBackgroundLaunchParts,
        PrepareSingleChapterGenerationRequestError,
    > {
        Ok(
            Self::prepare_from_target(db, user_id, request, chapter_target)
                .await?
                .into_background_launch_parts(task_id),
        )
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        SingleChapterGenerationTarget,
        SingleGenerationStartupSnapshotPlan,
        SingleGenerationRuntimeLaunchInput,
    ) {
        (
            self.chapter_target,
            self.startup_snapshot_plan,
            self.runtime_input,
        )
    }

    pub(crate) fn into_runtime_launch_input(self) -> SingleGenerationRuntimeLaunchInput {
        self.runtime_input
    }

    pub(crate) fn into_background_launch_parts(
        self,
        task_id: String,
    ) -> PreparedSingleGenerationBackgroundLaunchParts {
        let Self {
            chapter_target,
            startup_snapshot_plan,
            runtime_input,
        } = self;
        let response_payload = build_single_generation_background_create_response_payload(
            &task_id,
            &chapter_target,
            &startup_snapshot_plan,
            &runtime_input,
        );
        let task_seed = build_single_generation_background_task_persistence_seed(
            task_id.clone(),
            &chapter_target,
            runtime_input.user_id.clone(),
            runtime_input.execution_input.target_word_count,
            runtime_input
                .execution_input
                .compat_options
                .enable_analysis(),
        );

        PreparedSingleGenerationBackgroundLaunchParts {
            task_seed,
            startup_snapshot_plan,
            response_payload,
            runtime_input,
        }
    }

    #[cfg(test)]
    pub(crate) fn startup_snapshot_plan(&self) -> &SingleGenerationStartupSnapshotPlan {
        &self.startup_snapshot_plan
    }

    #[cfg(test)]
    pub(crate) fn from_parts(
        chapter_target: SingleChapterGenerationTarget,
        runtime_state_payload: Value,
        runtime_input: SingleGenerationRuntimeLaunchInput,
    ) -> Self {
        Self {
            startup_snapshot_plan: SingleGenerationStartupSnapshotPlan::from_pending_checkpoint(
                build_single_generation_pending_checkpoint(&chapter_target),
                runtime_state_payload,
            ),
            chapter_target,
            runtime_input,
        }
    }
}

fn build_single_generation_background_create_response_payload(
    task_id: &str,
    chapter_target: &SingleChapterGenerationTarget,
    startup_snapshot_plan: &SingleGenerationStartupSnapshotPlan,
    runtime_input: &SingleGenerationRuntimeLaunchInput,
) -> Value {
    let workflow_runtime_state = startup_snapshot_plan.runtime_state();
    let mut payload = build_single_generation_runtime_payload_base(
        task_id,
        &chapter_target.project_id,
        Some(&chapter_target.chapter_id),
        "pending",
        Some(workflow_runtime_state),
        None,
    );
    let restored_quality_context = startup_snapshot_plan.quality_runtime_context();
    let active_story_repair_payload = startup_snapshot_plan.active_story_repair_payload();
    apply_generation_quality_runtime_context_to_payload(
        &mut payload,
        restored_quality_context,
        startup_snapshot_plan.latest_quality_metrics().cloned(),
        startup_snapshot_plan.quality_metrics_summary().cloned(),
        startup_snapshot_plan.quality_metrics_history().cloned(),
    );
    payload.insert(
        "active_story_repair_payload".to_string(),
        json!(active_story_repair_payload),
    );
    if let Some(quality_history_context) = startup_snapshot_plan.quality_history_context() {
        payload.insert(
            "quality_history_context".to_string(),
            quality_history_context,
        );
    }

    let estimated_minutes = estimated_single_generation_task_minutes(
        runtime_input.execution_input.target_word_count,
        runtime_input
            .execution_input
            .compat_options
            .enable_analysis(),
    );
    let compatibility_payload = build_single_generation_background_response_payload(
        task_id,
        chapter_target,
        estimated_minutes,
        active_story_repair_payload.as_ref(),
    );
    if let Value::Object(compatibility_payload) = compatibility_payload {
        payload.extend(compatibility_payload);
    }

    Value::Object(payload)
}

fn build_single_generation_background_response_payload(
    task_id: &str,
    chapter_target: &SingleChapterGenerationTarget,
    estimated_minutes: i32,
    active_story_repair_payload: Option<&Value>,
) -> Value {
    json!({
        "task_id": task_id,
        "chapter_id": chapter_target.chapter_id,
        "status": "pending",
        "message": "单章后台生成任务已创建",
        "estimated_time_minutes": estimated_minutes,
        "active_story_repair_payload": active_story_repair_payload.cloned(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_single_generation_background_create_response_payload,
        build_single_generation_background_response_payload,
        build_single_generation_background_task_active_model,
        build_single_generation_background_task_persistence_seed,
        build_single_generation_pending_checkpoint,
        build_single_generation_runtime_launch_input_from_request_runtime_state,
        PreparedSingleChapterGenerationRestoredRuntimeLaunch, RestoredSingleGenerationRuntimeState,
        SingleGenerationStartupSnapshotPlan, SingleGenerationTaskPersistenceSeed,
    };
    use crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig;
    use crate::services::chapter_generation_execution_contract_service::{
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
    };
    use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
    use crate::services::chapter_generation_request_runtime_state_service::BatchGenerationRequestRuntimeState;
    use crate::services::chapter_quality_metrics_query_service::ChapterQualityMetricsFragments;
    use crate::services::chapter_single_generation_prepare_service::{
        PrepareSingleChapterGenerationRequestError, SingleChapterGenerationRequest,
        SingleChapterGenerationRouteRequest, SingleChapterGenerationTarget,
    };
    use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;
    use serde_json::json;

    #[test]
    fn should_build_single_chapter_generation_target_background_launch_payloads_from_runtime_restore_owner(
    ) {
        let target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };

        let checkpoint = build_single_generation_pending_checkpoint(&target);
        let response_payload =
            build_single_generation_background_response_payload("task-1", &target, 2, None);
        let persistence_seed = build_single_generation_background_task_persistence_seed(
            "task-1".to_string(),
            &target,
            "user-1".to_string(),
            2600,
            true,
        );
        let active_model = build_single_generation_background_task_active_model(
            "task-1".to_string(),
            &target,
            "user-1".to_string(),
            2600,
            true,
            chrono::NaiveDateTime::default(),
        );

        assert_eq!(checkpoint["chapter_id"], "chapter-7");
        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(response_payload["task_id"], "task-1");
        assert_eq!(response_payload["chapter_id"], "chapter-7");
        assert_eq!(response_payload["estimated_time_minutes"], 2);
        assert_eq!(
            persistence_seed,
            SingleGenerationTaskPersistenceSeed {
                id: "task-1".to_string(),
                project_id: "project-1".to_string(),
                user_id: "user-1".to_string(),
                start_chapter_number: 7,
                chapter_count: 1,
                chapter_ids: json!([{
                    "id": "chapter-7",
                    "chapter_number": 7,
                    "title": "Seven",
                }]),
                style_id: None,
                target_word_count: 2600,
                enable_analysis: true,
                total_chapters: 1,
                current_chapter_id: Some("chapter-7".to_string()),
                current_chapter_number: Some(7),
                max_retries: 3,
            }
        );
        assert_eq!(active_model.target_word_count, sea_orm::Set(2600));
        assert_eq!(active_model.status, sea_orm::Set("pending".to_string()));
        assert_eq!(active_model.completed_chapters, sea_orm::Set(0));
        assert_eq!(active_model.failed_chapters, sea_orm::Set(json!([])));
        assert_eq!(active_model.current_retry_count, sea_orm::Set(0));
        assert_eq!(active_model.enable_analysis, sea_orm::Set(true));
        assert_eq!(active_model.max_retries, sea_orm::Set(3));
        assert_eq!(
            active_model.chapter_ids,
            sea_orm::Set(json!([{
                "id": "chapter-7",
                "chapter_number": 7,
                "title": "Seven",
            }]))
        );
        assert_eq!(
            active_model.current_chapter_id,
            sea_orm::Set(Some("chapter-7".to_string()))
        );
    }

    #[test]
    fn should_build_single_chapter_generation_background_parts_from_runtime_restore_owner() {
        let target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };

        let checkpoint = build_single_generation_pending_checkpoint(&target);
        let response_payload =
            build_single_generation_background_response_payload("task-1", &target, 2, None);
        let task = build_single_generation_background_task_active_model(
            "task-1".to_string(),
            &target,
            "user-1".to_string(),
            2600,
            true,
            chrono::NaiveDateTime::default(),
        );

        assert_eq!(checkpoint["chapter_id"], "chapter-7");
        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(response_payload["task_id"], "task-1");
        assert_eq!(response_payload["chapter_id"], "chapter-7");
        assert_eq!(response_payload["estimated_time_minutes"], 2);
        assert_eq!(task.target_word_count, sea_orm::Set(2600));
        assert_eq!(
            task.current_chapter_id,
            sea_orm::Set(Some("chapter-7".to_string()))
        );
    }

    #[test]
    fn should_keep_background_launch_owner_contract_from_restored_launch() {
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-12".to_string(),
            chapter_id: "chapter-12".to_string(),
            chapter_number: 12,
            title: "Twelve".to_string(),
        };
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                enable_analysis: true,
                ..Default::default()
            },
            Some("gpt-4.1".to_string()),
        );
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 3200,
            compat_options: request_runtime_state.compat_options.clone(),
            execution_config: PreparedGenerationExecutionConfig {
                ai_config: crate::ai::AIConfig::default(),
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
        };
        let runtime_input = build_single_generation_runtime_launch_input_from_request_runtime_state(
            &chapter_target,
            "user-1",
            execution_input.target_word_count,
            &request_runtime_state,
            execution_input.execution_config.clone(),
        );
        let restored_launch = PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
            chapter_target,
            json!({
                "quality_metrics_summary": {"chapter_count": 1},
                "active_story_repair_payload": {"mode": "repair"}
            }),
            runtime_input,
        );

        let launch_parts = restored_launch.into_background_launch_parts("task-12".to_string());

        assert_eq!(launch_parts.response_payload["task_id"], "task-12");
        assert_eq!(launch_parts.response_payload["chapter_id"], "chapter-12");
        assert_eq!(launch_parts.response_payload["estimated_time_minutes"], 3);
        assert_eq!(
            launch_parts.startup_snapshot_plan.runtime_state()["quality_metrics_summary"]
                ["chapter_count"],
            1
        );
        assert_eq!(launch_parts.task_seed.max_retries, 3);
        assert_eq!(launch_parts.task_seed.enable_analysis, true);
    }

    #[test]
    fn should_keep_background_response_payload_quality_context_fields() {
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-7".to_string(),
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-7".to_string(),
            user_id: "user-7".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2400,
                compat_options: SingleChapterGenerationCompatOptions {
                    enable_analysis: true,
                    ..Default::default()
                },
                execution_config: PreparedGenerationExecutionConfig {
                    ai_config: crate::ai::AIConfig::default(),
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
        let startup_snapshot_plan = SingleGenerationStartupSnapshotPlan::from_pending_checkpoint(
            build_single_generation_pending_checkpoint(&chapter_target),
            json!({
                "latest_quality_metrics": {"overall_score": 88},
                "quality_metrics_summary": {"chapter_count": 2},
                "quality_metrics_history": [{"overall_score": 82}, {"overall_score": 88}],
                "quality_history_context": {"source": "history"},
                "active_story_repair_payload": {"mode": "repair"}
            }),
        );

        let payload = build_single_generation_background_create_response_payload(
            "task-7",
            &chapter_target,
            &startup_snapshot_plan,
            &runtime_input,
        );

        assert_eq!(payload["task_id"], "task-7");
        assert_eq!(payload["chapter_id"], "chapter-7");
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 88);
        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(payload["quality_history_context"]["source"], "history");
        assert_eq!(payload["active_story_repair_payload"]["mode"], "repair");
    }

    #[tokio::test]
    async fn should_prepare_single_chapter_generation_request_from_target_without_reloading_chapter(
    ) {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite memory db");
        let request = SingleChapterGenerationRequest {
            target_word_count: Some(1800),
            ..SingleChapterGenerationRequest::default()
        };
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-9".to_string(),
            chapter_number: 9,
            title: "Nine".to_string(),
        };

        let error = PreparedSingleChapterGenerationRestoredRuntimeLaunch::prepare_from_target(
            &db,
            "user-1",
            &request,
            chapter_target,
        )
        .await
        .expect_err("sqlite memory db should fail before any chapter reload path is needed");

        assert!(matches!(
            error,
            PrepareSingleChapterGenerationRequestError::Config(_)
                | PrepareSingleChapterGenerationRequestError::Internal(_)
        ));
    }

    #[test]
    fn should_normalize_single_chapter_generation_compat_options_from_request_owner() {
        let request = SingleChapterGenerationRouteRequest {
            style_id: Some(9),
            target_word_count: Some(2800),
            model: None,
            enable_analysis: None,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: Some("hook".to_string()),
            story_focus: Some("reveal_mystery".to_string()),
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: Some("immersive".to_string()),
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: None,
            story_preserve_strengths: None,
        }
        .into_generation_request();

        let compat = request.compat_options_with_web_research_default(false);

        assert_eq!(compat.style_id(), Some(9));
        assert!(compat.enable_analysis());
        assert!(compat.enable_mcp());
        assert!(!compat.web_research_enabled());
        assert_eq!(compat.web_research_query(), None);
        assert_eq!(compat.creative_mode(), "hook");
        assert_eq!(compat.story_focus(), "reveal_mystery");
        assert_eq!(compat.quality_preset(), "immersive");
        assert_eq!(compat.story_repair_targets(), &[] as &[String]);
        assert_eq!(compat.story_preserve_strengths(), &[] as &[String]);
    }

    #[test]
    fn should_fallback_to_settings_default_for_single_generation_web_research() {
        let request = SingleChapterGenerationRouteRequest {
            style_id: None,
            target_word_count: Some(2800),
            model: None,
            enable_analysis: None,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            narrative_perspective: None,
            creative_mode: None,
            story_focus: None,
            plot_stage: None,
            story_creation_brief: None,
            quality_preset: None,
            quality_notes: None,
            story_repair_summary: None,
            story_repair_targets: None,
            story_preserve_strengths: None,
        }
        .into_generation_request();

        let compat = request.compat_options_with_web_research_default(true);

        assert!(compat.web_research_enabled());
    }

    #[test]
    fn should_restore_runtime_launch_parts_from_quality_fragments_owner() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("request summary".to_string()),
                story_repair_targets: vec!["compress".to_string()],
                ..Default::default()
            },
            Some("gpt-4.1".to_string()),
        );
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2400,
            compat_options: request_runtime_state.compat_options.clone(),
            execution_config: PreparedGenerationExecutionConfig {
                ai_config: crate::ai::AIConfig::default(),
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
        };
        let restored_runtime_state = RestoredSingleGenerationRuntimeState::from_quality_fragments(
            json!({
                "phase": "pending",
                "status": "pending",
                "chapter_id": "chapter-9"
            }),
            &request_runtime_state,
            ChapterQualityMetricsFragments {
                latest_quality_metrics: Some(json!({"overall_score": 84})),
                history_id: None,
                generated_at: None,
                quality_metrics_summary: Some(json!({
                    "chapter_count": 2,
                    "repair_guidance": {
                        "summary": "restored summary"
                    }
                })),
                quality_metrics_history: Some(json!([
                    {"overall_score": 80},
                    {"overall_score": 84}
                ])),
                quality_metrics_summary_state: Some(json!({"chapter_count": 2})),
            },
            None,
        );

        assert_eq!(
            restored_runtime_state.request_runtime_state(),
            &request_runtime_state
        );

        let (startup_snapshot_plan, runtime_input) = restored_runtime_state
            .into_startup_runtime_launch_parts(
                "chapter-9".to_string(),
                "user-9".to_string(),
                execution_input,
            );

        assert_eq!(
            startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["chapter_count"],
            2
        );
        assert_eq!(
            startup_snapshot_plan.runtime_state()["latest_quality_metrics"]["overall_score"],
            84
        );
        assert_eq!(runtime_input.chapter_id, "chapter-9");
        assert_eq!(runtime_input.user_id, "user-9");
        assert_eq!(runtime_input.execution_input.target_word_count, 2400);
        assert_eq!(
            runtime_input
                .execution_input
                .compat_options
                .story_repair_summary(),
            "request summary"
        );
    }
}
