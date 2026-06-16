use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use crate::models::{chapter, generation_history};
use crate::services::chapter_analysis_service::load_chapter_analysis_read_context;
use crate::services::chapter_generation_execution_contract_service::{
    active_story_repair_payload_from_runtime_state, normalize_chapter_generation_target_word_count,
    BatchGenerationRequestRuntimeState, PreparedGenerationExecutionConfig,
    SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
};
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    apply_generation_quality_runtime_context_to_payload,
    resolve_generation_quality_runtime_context_for_seed,
    resolve_generation_quality_runtime_context_from_persisted_sources,
};
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::{
    aggregate_story_repair_quality_summaries,
    resolve_active_story_repair_payload_with_quality_fallback,
    restore_story_repair_compat_options_from_active_snapshot,
};
use crate::services::chapter_quality_metrics_query_service::{
    build_chapter_analysis_quality_fragments, build_chapter_quality_metrics_fragments,
    ChapterQualityMetricsFragments,
};
use crate::services::chapter_single_generation_background_launch_service::{
    build_single_generation_pending_checkpoint, SingleGenerationStartupSnapshotPlan,
};
use crate::services::chapter_single_generation_prepare_service::{
    prepare_single_chapter_generation_execution_config_from_runtime_state,
    PrepareSingleChapterGenerationRequestError, SingleChapterGenerationRequest,
    SingleChapterGenerationTarget,
};
use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;
use crate::services::settings_service::SettingsService;

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SingleGenerationRuntimeSeedSource {
    CurrentChapterQuality,
    RecentHistorySummary,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RestoredSingleGenerationRuntimeState {
    request_runtime_state: BatchGenerationRequestRuntimeState,
    startup_snapshot_plan: SingleGenerationStartupSnapshotPlan,
    #[cfg(test)]
    seed_source: SingleGenerationRuntimeSeedSource,
}

pub(crate) fn build_single_generation_runtime_launch_input_from_request_runtime_state(
    chapter_target: &SingleChapterGenerationTarget,
    user_id: &str,
    target_word_count: i32,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    execution_config: PreparedGenerationExecutionConfig,
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

fn build_single_generation_runtime_state_payload_from_sources(
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

fn build_single_generation_runtime_state_payload_from_parts(
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

fn resolve_single_generation_runtime_compat_options_from_seed(
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

fn build_single_generation_runtime_launch_input(
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
        #[cfg(test)]
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
            #[cfg(test)]
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
    pub(crate) fn request_runtime_state(&self) -> &BatchGenerationRequestRuntimeState {
        &self.request_runtime_state
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

pub(crate) async fn prepare_single_generation_restored_runtime_seed_from_target(
    db: &DatabaseConnection,
    user_id: &str,
    request: &SingleChapterGenerationRequest,
    chapter_target: SingleChapterGenerationTarget,
) -> Result<
    (
        SingleChapterGenerationTarget,
        SingleGenerationStartupSnapshotPlan,
        SingleGenerationRuntimeLaunchInput,
    ),
    PrepareSingleChapterGenerationRequestError,
> {
    request.validate_request_bounds()?;
    let normalized_target_word_count =
        normalize_chapter_generation_target_word_count(request.target_word_count);
    let web_research_default = SettingsService::resolve_web_research_enabled(db, user_id)
        .await
        .map_err(|error| PrepareSingleChapterGenerationRequestError::Config(error.to_string()))?;
    let compat_options = request.compat_options_with_web_research_default(web_research_default);
    let request_runtime_state =
        BatchGenerationRequestRuntimeState::new(compat_options.clone(), request.model.clone());
    let execution_config = prepare_single_chapter_generation_execution_config_from_runtime_state(
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

    Ok((chapter_target, startup_snapshot_plan, runtime_input))
}
