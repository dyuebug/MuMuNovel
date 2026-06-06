use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::chapter;
use crate::models::generation_history;
use crate::models::project_default_style;
use crate::services::chapter_batch_generation_owned_task_query_service::LoadOwnedBatchGenerationTaskError;
use crate::services::chapter_batch_generation_quality_runtime_context_service::{
    apply_batch_quality_runtime_context_to_payload,
    resolve_batch_quality_runtime_context_for_startup_seed,
};
use crate::services::chapter_batch_generation_snapshot_service::BatchGenerationQueuedSnapshotPlan;
use crate::services::chapter_batch_generation_task_model_service::BatchGenerationTaskPersistenceSeed;
use crate::services::chapter_batch_generation_task_payload_base_service::{
    build_batch_generation_task_response_payload_from_runtime_parts, estimated_task_minutes,
    BatchGenerationTaskResponsePayloadOptions, BatchGenerationTaskResponseQualityPayload,
};
use crate::services::chapter_generation_execution_config_service::prepare_generation_execution_config;
use crate::services::chapter_generation_prerequisite_service::check_chapter_generation_prerequisites;
#[cfg(test)]
use crate::services::chapter_generation_request_runtime_state_service::parse_batch_generation_request_runtime_state;
use crate::services::chapter_generation_request_runtime_state_service::{
    batch_generation_request_runtime_state_payload, BatchGenerationRequestRuntimeState,
};
use crate::services::chapter_generation_target_word_count_service::normalize_chapter_generation_target_word_count;
use crate::services::chapter_story_repair_quality_context_service::{
    aggregate_story_repair_quality_summaries,
    resolve_active_story_repair_payload_with_quality_fallback,
};
use crate::services::project_access_query_service::{
    ensure_owned_project_access, ProjectAccessQueryError,
};
use crate::services::route_request_deserialize_service::deserialize_optional_non_null;
use crate::services::settings_service::SettingsService;

use super::chapter_batch_generation_owned_task_query_service::{
    load_owned_batch_generation_task_sources, LoadOwnedBatchGenerationTaskSourcesError,
};
use super::chapter_batch_generation_resume_task_command_service::{
    prepare_owned_batch_generation_resume, BatchGenerationResumeLaunchPersistencePlan,
    PrepareOwnedBatchGenerationResumeError, ResumeBatchGenerationDomainError,
};
#[cfg(test)]
use super::chapter_batch_generation_runtime_state_service::restore_batch_generation_runtime_compat_options_from_runtime_state_seed;
use super::chapter_batch_generation_runtime_state_service::{
    build_batch_generation_startup_snapshot_and_runtime_launch_input_from_runtime_state_seed,
    dispatch_batch_generation_runtime, BatchGenerationCancelledPersistencePlan,
    BatchGenerationExecutionInput,
};
use super::chapter_batch_generation_status_semantics_service::{
    batch_generation_task_type, BatchGenerationTaskKind,
};
use super::chapter_quality_metrics_query_service::build_chapter_analysis_quality_fragments;
use super::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions;

const MAX_BATCH_GENERATION_CREATE_COUNT: i32 = 20;
const MIN_BATCH_GENERATION_CREATE_TARGET_WORD_COUNT: i32 = 500;
const MAX_BATCH_GENERATION_CREATE_TARGET_WORD_COUNT: i32 = 10_000;
const MIN_BATCH_GENERATION_CREATE_RETRIES: i32 = 0;
const MAX_BATCH_GENERATION_CREATE_RETRIES: i32 = 5;
const MAX_BATCH_GENERATION_CREATE_STORY_CREATION_BRIEF_LENGTH: usize = 1200;
const MAX_BATCH_GENERATION_CREATE_QUALITY_NOTES_LENGTH: usize = 600;
const BATCH_GENERATION_CREATE_CREATIVE_MODE_VALUES: &[&str] = &[
    "balanced",
    "hook",
    "emotion",
    "suspense",
    "relationship",
    "payoff",
];
const BATCH_GENERATION_CREATE_STORY_FOCUS_VALUES: &[&str] = &[
    "advance_plot",
    "deepen_character",
    "escalate_conflict",
    "reveal_mystery",
    "relationship_shift",
    "foreshadow_payoff",
];
const BATCH_GENERATION_CREATE_PLOT_STAGE_VALUES: &[&str] = &["development", "climax", "ending"];
const BATCH_GENERATION_CREATE_QUALITY_PRESET_VALUES: &[&str] = &[
    "balanced",
    "plot_drive",
    "immersive",
    "emotion_drama",
    "clean_prose",
];

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct BatchGenerationCreateRouteRequest {
    pub(crate) start_chapter_number: i32,
    pub(crate) count: i32,
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub(crate) enable_analysis: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub(crate) enable_mcp: Option<bool>,
    pub(crate) enable_web_research: Option<bool>,
    pub(crate) web_research_query: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub(crate) max_retries: Option<i32>,
    pub(crate) model: Option<String>,
    pub(crate) creative_mode: Option<String>,
    pub(crate) story_focus: Option<String>,
    pub(crate) plot_stage: Option<String>,
    pub(crate) story_creation_brief: Option<String>,
    pub(crate) quality_preset: Option<String>,
    pub(crate) quality_notes: Option<String>,
    pub(crate) story_repair_summary: Option<String>,
    pub(crate) story_repair_targets: Option<Vec<String>>,
    pub(crate) story_preserve_strengths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BatchGenerationCreateWorkflowRequest {
    pub(crate) start_chapter_number: i32,
    pub(crate) count: i32,
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: Option<i32>,
    pub(crate) enable_analysis: bool,
    pub(crate) enable_mcp: Option<bool>,
    pub(crate) enable_web_research: Option<bool>,
    pub(crate) web_research_query: Option<String>,
    pub(crate) max_retries: i32,
    pub(crate) model_override: Option<String>,
    pub(crate) creative_mode: Option<String>,
    pub(crate) story_focus: Option<String>,
    pub(crate) plot_stage: Option<String>,
    pub(crate) story_creation_brief: Option<String>,
    pub(crate) quality_preset: Option<String>,
    pub(crate) quality_notes: Option<String>,
    pub(crate) story_repair_summary: Option<String>,
    pub(crate) story_repair_targets: Vec<String>,
    pub(crate) story_preserve_strengths: Vec<String>,
}

impl BatchGenerationCreateWorkflowRequest {
    fn from_route_request(route_request: BatchGenerationCreateRouteRequest) -> Self {
        Self {
            start_chapter_number: route_request.start_chapter_number,
            count: route_request.count,
            style_id: route_request.style_id,
            target_word_count: route_request.target_word_count,
            enable_analysis: route_request.enable_analysis.unwrap_or(false),
            enable_mcp: route_request.enable_mcp,
            enable_web_research: route_request.enable_web_research,
            web_research_query: route_request.web_research_query,
            max_retries: route_request.max_retries.unwrap_or(3),
            model_override: route_request.model,
            creative_mode: normalize_optional_create_request_string(route_request.creative_mode),
            story_focus: normalize_optional_create_request_string(route_request.story_focus),
            plot_stage: normalize_optional_create_request_string(route_request.plot_stage),
            story_creation_brief: normalize_optional_create_request_string(
                route_request.story_creation_brief,
            ),
            quality_preset: normalize_optional_create_request_string(route_request.quality_preset),
            quality_notes: normalize_optional_create_request_string(route_request.quality_notes),
            story_repair_summary: normalize_optional_create_request_string(
                route_request.story_repair_summary,
            ),
            story_repair_targets: route_request.story_repair_targets.unwrap_or_default(),
            story_preserve_strengths: route_request.story_preserve_strengths.unwrap_or_default(),
        }
    }

    fn compat_options_with_web_research_default(
        &self,
        web_research_default: bool,
    ) -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: self.style_id,
            enable_analysis: self.enable_analysis,
            enable_mcp: self.enable_mcp.unwrap_or(true),
            web_research_enabled: self.enable_web_research.unwrap_or(web_research_default),
            web_research_query: self.web_research_query.clone(),
            narrative_perspective: None,
            creative_mode: self.creative_mode.clone(),
            story_focus: self.story_focus.clone(),
            plot_stage: self.plot_stage.clone(),
            story_creation_brief: self.story_creation_brief.clone(),
            quality_preset: self.quality_preset.clone(),
            quality_notes: self.quality_notes.clone(),
            story_repair_summary: self.story_repair_summary.clone(),
            story_repair_targets: self.story_repair_targets.clone(),
            story_preserve_strengths: self.story_preserve_strengths.clone(),
        }
    }

    fn into_request_runtime_state(
        &self,
        web_research_default: bool,
    ) -> BatchGenerationRequestRuntimeState {
        BatchGenerationRequestRuntimeState::new(
            self.compat_options_with_web_research_default(web_research_default),
            self.model_override.clone(),
        )
    }

    fn task_spec(&self) -> BatchGenerationCreateTaskSpec {
        BatchGenerationCreateTaskSpec {
            start_chapter_number: self.start_chapter_number,
            style_id: self.style_id,
            enable_analysis: self.enable_analysis,
            max_retries: self.max_retries,
        }
    }

    async fn prepare(
        &self,
        db: &DatabaseConnection,
        project_id: &str,
    ) -> Result<
        (i32, Vec<BatchGenerationCreateChapterTarget>),
        PrepareBatchGenerationCreateRequestError,
    > {
        self.validate_request_bounds()?;

        let chapters_to_generate = self
            .load_chapters_for_generation_range(db, project_id)
            .await?;
        if let Some(first_chapter) = chapters_to_generate.first() {
            let prerequisite = check_chapter_generation_prerequisites(db, first_chapter)
                .await
                .map_err(PrepareBatchGenerationCreateRequestError::Internal)?;
            if !prerequisite.can_generate {
                return Err(
                    PrepareBatchGenerationCreateRequestError::PrerequisitesBlocked(
                        prerequisite.error_message,
                    ),
                );
            }
        }

        Ok((
            normalize_chapter_generation_target_word_count(self.target_word_count),
            chapters_to_generate
                .iter()
                .map(BatchGenerationCreateChapterTarget::from_model)
                .collect(),
        ))
    }

    async fn load_chapters_for_generation_range(
        &self,
        db: &DatabaseConnection,
        project_id: &str,
    ) -> Result<Vec<chapter::Model>, PrepareBatchGenerationCreateRequestError> {
        if self.count <= 0 {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidCount);
        }

        let project_chapters = chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(project_id))
            .order_by_asc(chapter::Column::ChapterNumber)
            .all(db)
            .await
            .map_err(|error| {
                PrepareBatchGenerationCreateRequestError::Internal(error.to_string())
            })?;

        self.select_chapters_for_generation_range(project_chapters)
    }

    fn validate_request_bounds(&self) -> Result<(), PrepareBatchGenerationCreateRequestError> {
        if self.count <= 0 {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidCount);
        }
        if self.count > MAX_BATCH_GENERATION_CREATE_COUNT {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidCountTooLarge);
        }
        if let Some(target_word_count) = self.target_word_count {
            if target_word_count < MIN_BATCH_GENERATION_CREATE_TARGET_WORD_COUNT {
                return Err(
                    PrepareBatchGenerationCreateRequestError::InvalidTargetWordCountTooSmall,
                );
            }
            if target_word_count > MAX_BATCH_GENERATION_CREATE_TARGET_WORD_COUNT {
                return Err(
                    PrepareBatchGenerationCreateRequestError::InvalidTargetWordCountTooLarge,
                );
            }
        }
        if self.max_retries < MIN_BATCH_GENERATION_CREATE_RETRIES
            || self.max_retries > MAX_BATCH_GENERATION_CREATE_RETRIES
        {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidMaxRetries);
        }
        if !is_valid_optional_choice(
            self.creative_mode.as_deref(),
            BATCH_GENERATION_CREATE_CREATIVE_MODE_VALUES,
        ) {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidCreativeMode);
        }
        if !is_valid_optional_choice(
            self.story_focus.as_deref(),
            BATCH_GENERATION_CREATE_STORY_FOCUS_VALUES,
        ) {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidStoryFocus);
        }
        if !is_valid_optional_choice(
            self.plot_stage.as_deref(),
            BATCH_GENERATION_CREATE_PLOT_STAGE_VALUES,
        ) {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidPlotStage);
        }
        if !is_valid_optional_choice(
            self.quality_preset.as_deref(),
            BATCH_GENERATION_CREATE_QUALITY_PRESET_VALUES,
        ) {
            return Err(PrepareBatchGenerationCreateRequestError::InvalidQualityPreset);
        }
        if !is_valid_optional_text_length(
            self.story_creation_brief.as_deref(),
            MAX_BATCH_GENERATION_CREATE_STORY_CREATION_BRIEF_LENGTH,
        ) {
            return Err(PrepareBatchGenerationCreateRequestError::StoryCreationBriefTooLong);
        }
        if !is_valid_optional_text_length(
            self.quality_notes.as_deref(),
            MAX_BATCH_GENERATION_CREATE_QUALITY_NOTES_LENGTH,
        ) {
            return Err(PrepareBatchGenerationCreateRequestError::QualityNotesTooLong);
        }

        Ok(())
    }

    fn select_chapters_for_generation_range(
        &self,
        project_chapters: Vec<chapter::Model>,
    ) -> Result<Vec<chapter::Model>, PrepareBatchGenerationCreateRequestError> {
        if project_chapters.is_empty() {
            return Err(PrepareBatchGenerationCreateRequestError::ProjectHasNoChapters);
        }

        let end_chapter_number = self.start_chapter_number + self.count - 1;
        let chapters_to_generate = project_chapters
            .into_iter()
            .filter(|chapter| {
                self.start_chapter_number <= chapter.chapter_number
                    && chapter.chapter_number <= end_chapter_number
            })
            .collect::<Vec<_>>();

        if chapters_to_generate.is_empty() {
            return Err(PrepareBatchGenerationCreateRequestError::ChaptersNotFound);
        }

        Ok(chapters_to_generate)
    }
}

fn normalize_optional_create_request_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn is_valid_optional_choice(value: Option<&str>, allowed_values: &[&str]) -> bool {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| allowed_values.contains(&value))
        .unwrap_or(true)
}

fn is_valid_optional_text_length(value: Option<&str>, max_chars: usize) -> bool {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().count() <= max_chars)
        .unwrap_or(true)
}

pub(crate) fn build_batch_generation_create_workflow_request_from_route_payload(
    route_request: BatchGenerationCreateRouteRequest,
) -> BatchGenerationCreateWorkflowRequest {
    BatchGenerationCreateWorkflowRequest::from_route_request(route_request)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BatchGenerationCreateTaskSpec {
    start_chapter_number: i32,
    style_id: Option<i32>,
    enable_analysis: bool,
    max_retries: i32,
}

impl BatchGenerationCreateTaskSpec {
    fn with_effective_style_id(self, style_id: Option<i32>) -> Self {
        Self { style_id, ..self }
    }
}

async fn resolve_batch_generation_create_effective_style_id(
    db: &DatabaseConnection,
    project_id: &str,
    requested_style_id: Option<i32>,
) -> Result<Option<i32>, String> {
    let default_style_id = match requested_style_id {
        Some(_) => None,
        None => load_batch_generation_project_default_style_id(db, project_id).await?,
    };

    Ok(select_batch_generation_create_effective_style_id(
        requested_style_id,
        default_style_id,
    ))
}

fn select_batch_generation_create_effective_style_id(
    requested_style_id: Option<i32>,
    default_style_id: Option<i32>,
) -> Option<i32> {
    requested_style_id.or(default_style_id)
}

async fn load_batch_generation_project_default_style_id(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<Option<i32>, String> {
    project_default_style::Entity::find()
        .filter(project_default_style::Column::ProjectId.eq(project_id))
        .one(db)
        .await
        .map(|default_style| default_style.map(|model| model.style_id))
        .map_err(|error| error.to_string())
}

pub(crate) async fn load_recent_batch_story_repair_quality_summary(
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
            format!("load previous chapters for batch story repair failed: {error}")
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
                format!("load generation histories for batch story repair failed: {error}")
            })?;
        let quality_fragments = build_chapter_analysis_quality_fragments(&histories, None);
        if let Some(summary) = quality_fragments.quality_metrics_summary {
            summaries.push(summary);
        }
    }

    Ok(aggregate_story_repair_quality_summaries(
        &summaries, "batch",
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchGenerationCreateStartupSeedSource {
    RequestOnly,
    RecentHistorySummary,
}

#[derive(Debug, Clone, PartialEq)]
struct BatchGenerationCreateStartupRuntimeState {
    request_runtime_state: BatchGenerationRequestRuntimeState,
    runtime_state_payload: Value,
    seed_source: BatchGenerationCreateStartupSeedSource,
}

impl BatchGenerationCreateStartupRuntimeState {
    async fn prepare(
        db: &DatabaseConnection,
        project_id: &str,
        start_chapter_number: i32,
        request_runtime_state: BatchGenerationRequestRuntimeState,
    ) -> Result<Self, String> {
        let recent_history_summary =
            load_recent_batch_story_repair_quality_summary(db, project_id, start_chapter_number)
                .await?;

        Ok(Self::from_recent_history_summary(
            request_runtime_state,
            recent_history_summary,
        ))
    }

    fn from_recent_history_summary(
        request_runtime_state: BatchGenerationRequestRuntimeState,
        recent_history_summary: Option<Value>,
    ) -> Self {
        let (runtime_state_payload, seed_source) = match recent_history_summary {
            Some(recent_history_summary) => (
                build_batch_generation_runtime_state_payload_from_parts(
                    &request_runtime_state,
                    Some(&recent_history_summary),
                ),
                BatchGenerationCreateStartupSeedSource::RecentHistorySummary,
            ),
            None => (
                build_batch_generation_runtime_state_payload_from_parts(
                    &request_runtime_state,
                    None,
                ),
                BatchGenerationCreateStartupSeedSource::RequestOnly,
            ),
        };

        Self {
            request_runtime_state,
            runtime_state_payload,
            seed_source,
        }
    }

    fn into_parts(self) -> (BatchGenerationRequestRuntimeState, Value) {
        (self.request_runtime_state, self.runtime_state_payload)
    }

    fn into_runtime_seed(self) -> BatchGenerationCreateRuntimeSeed {
        let (_, runtime_state_payload) = self.into_parts();

        BatchGenerationCreateRuntimeSeed {
            runtime_state_payload,
        }
    }

    #[cfg(test)]
    fn runtime_state_payload(&self) -> &Value {
        &self.runtime_state_payload
    }

    #[cfg(test)]
    fn request_runtime_state(&self) -> &BatchGenerationRequestRuntimeState {
        &self.request_runtime_state
    }

    #[cfg(test)]
    fn seed_source(&self) -> BatchGenerationCreateStartupSeedSource {
        self.seed_source
    }
}

#[derive(Debug, Clone, PartialEq)]
struct BatchGenerationCreateRuntimeSeed {
    runtime_state_payload: Value,
}

impl BatchGenerationCreateRuntimeSeed {
    async fn prepare(
        db: &DatabaseConnection,
        project_id: &str,
        start_chapter_number: i32,
        request_runtime_state: BatchGenerationRequestRuntimeState,
    ) -> Result<Self, String> {
        BatchGenerationCreateStartupRuntimeState::prepare(
            db,
            project_id,
            start_chapter_number,
            request_runtime_state,
        )
        .await
        .map(BatchGenerationCreateStartupRuntimeState::into_runtime_seed)
    }

    #[cfg(test)]
    fn from_runtime_state_payload(runtime_state_payload: Value) -> Self {
        Self {
            runtime_state_payload,
        }
    }

    #[cfg(test)]
    fn into_parts(self) -> (Value, SingleChapterGenerationCompatOptions) {
        let request_runtime_state =
            parse_batch_generation_request_runtime_state(Some(&self.runtime_state_payload));
        let resolved_compat_options =
            restore_batch_generation_runtime_compat_options_from_runtime_state_seed(
                &request_runtime_state.compat_options,
                Some(&self.runtime_state_payload),
            );

        (self.runtime_state_payload, resolved_compat_options)
    }

    fn into_workflow_launch_parts(
        self,
        user_id: String,
        chapter_ids: Vec<String>,
        total_chapters: i32,
        normalized_target_word_count: i32,
        execution_config:
            crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig,
    ) -> (
        BatchGenerationQueuedSnapshotPlan,
        BatchGenerationExecutionInput,
    ) {
        let runtime_state_payload = self.runtime_state_payload;

        build_batch_generation_startup_snapshot_and_runtime_launch_input_from_runtime_state_seed(
            user_id,
            chapter_ids,
            total_chapters,
            normalized_target_word_count,
            runtime_state_payload,
            execution_config,
        )
    }

    #[cfg(test)]
    fn startup_snapshot_plan(&self, total_chapters: i32) -> BatchGenerationQueuedSnapshotPlan {
        BatchGenerationQueuedSnapshotPlan::from_runtime_state_seed(
            total_chapters,
            Some(self.runtime_state_payload.clone()),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrepareBatchGenerationCreateRequestError {
    InvalidCount,
    InvalidCountTooLarge,
    InvalidTargetWordCountTooSmall,
    InvalidTargetWordCountTooLarge,
    InvalidMaxRetries,
    InvalidCreativeMode,
    InvalidStoryFocus,
    InvalidPlotStage,
    InvalidQualityPreset,
    StoryCreationBriefTooLong,
    QualityNotesTooLong,
    ProjectHasNoChapters,
    ChaptersNotFound,
    PrerequisitesBlocked(String),
    Internal(String),
}

#[derive(Debug, Clone)]
pub(crate) struct BatchGenerationCreateChapterTarget {
    pub(crate) id: String,
    pub(crate) chapter_number: i32,
    pub(crate) title: String,
}

impl BatchGenerationCreateChapterTarget {
    fn from_model(chapter_model: &chapter::Model) -> Self {
        Self {
            id: chapter_model.id.clone(),
            chapter_number: chapter_model.chapter_number,
            title: chapter_model.title.clone(),
        }
    }
}

pub(crate) fn build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    explicit_story_repair_payload: Option<&Value>,
    quality_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Value {
    let mut payload = batch_generation_request_runtime_state_payload(request_runtime_state)
        .as_object()
        .cloned()
        .unwrap_or_default();
    let resolved_quality_context = resolve_batch_quality_runtime_context_for_startup_seed(
        quality_summary,
        latest_quality_metrics,
    );
    let resolved_quality_summary = resolved_quality_context
        .quality_metrics_summary
        .clone()
        .unwrap_or(Value::Null);
    let active_story_repair_payload = resolve_active_story_repair_payload_with_quality_fallback(
        explicit_story_repair_payload,
        Some(&resolved_quality_summary),
        resolved_quality_context.latest_quality_metrics.as_ref(),
        "batch",
        "recent_history_summary",
        "Recent history summary",
    );

    if let Some(active_story_repair_payload) = active_story_repair_payload {
        payload.insert(
            "active_story_repair_payload".to_string(),
            active_story_repair_payload,
        );
    }
    apply_batch_quality_runtime_context_to_payload(
        &mut payload,
        resolved_quality_context,
        Some(resolved_quality_summary.clone()),
    );

    Value::Object(payload)
}

pub(crate) fn build_batch_generation_runtime_state_payload_from_parts(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    quality_summary: Option<&Value>,
) -> Value {
    build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload(
        request_runtime_state,
        request_runtime_state
            .active_story_repair_payload_with_scope("batch")
            .as_ref(),
        quality_summary,
        None,
    )
}
fn batch_generation_create_response_payload(
    batch_id: &str,
    project_id: &str,
    chapters_to_generate: &[BatchGenerationCreateChapterTarget],
    target_word_count: i32,
    enable_analysis: bool,
    startup_snapshot_plan: &BatchGenerationQueuedSnapshotPlan,
) -> Value {
    let total_chapters = chapters_to_generate.len();
    let task_kind = if total_chapters == 1 {
        BatchGenerationTaskKind::SingleChapter
    } else {
        BatchGenerationTaskKind::Batch
    };
    let payload = build_batch_generation_task_response_payload_from_runtime_parts(
        batch_id,
        batch_generation_task_type(task_kind),
        project_id,
        "pending",
        None,
        None,
        Some(startup_snapshot_plan.runtime_state()),
        BatchGenerationTaskResponsePayloadOptions {
            quality_payload: Some(BatchGenerationTaskResponseQualityPayload::Batch {
                quality_runtime_context: startup_snapshot_plan.quality_runtime_context(),
                quality_metrics_summary: startup_snapshot_plan.quality_metrics_summary().cloned(),
            }),
            active_story_repair_payload: startup_snapshot_plan.active_story_repair_payload(),
            quality_history_context: startup_snapshot_plan.quality_history_context(),
            extra_fields: vec![
                (
                    "message".to_string(),
                    json!(format!("已创建批量生成任务，共 {} 章", total_chapters)),
                ),
                (
                    "chapters_to_generate".to_string(),
                    Value::Array(
                        chapters_to_generate
                            .iter()
                            .map(|target| {
                                json!({
                                    "id": target.id,
                                    "chapter_number": target.chapter_number,
                                    "title": target.title,
                                })
                            })
                            .collect::<Vec<_>>(),
                    ),
                ),
                (
                    "estimated_time_minutes".to_string(),
                    json!(estimated_task_minutes(
                        total_chapters,
                        target_word_count,
                        enable_analysis,
                    )),
                ),
            ],
            ..Default::default()
        },
    );

    Value::Object(payload)
}

#[derive(Debug)]
struct BatchGenerationCreateLaunchPersistencePlan {
    task_seed: BatchGenerationTaskPersistenceSeed,
    startup_snapshot_plan: BatchGenerationQueuedSnapshotPlan,
    response_payload: Value,
    runtime_input: BatchGenerationExecutionInput,
}

#[derive(Debug, Clone)]
struct PreparedBatchGenerationResumeWorkflowLaunch {
    persistence_plan: BatchGenerationResumeLaunchPersistencePlan,
}

#[derive(Debug, Clone)]
struct PreparedBatchGenerationCancelWorkflowLaunch {
    persistence_plan: crate::services::chapter_batch_generation_runtime_state_service::BatchGenerationCancelledPersistencePlan,
}

#[derive(Debug, Clone)]
struct PreparedBatchGenerationCreateWorkflowLaunch {
    task_spec: BatchGenerationCreateTaskSpec,
    chapters_to_generate: Vec<BatchGenerationCreateChapterTarget>,
    startup_snapshot_plan: BatchGenerationQueuedSnapshotPlan,
    runtime_input: BatchGenerationExecutionInput,
}

#[derive(Debug, Clone)]
struct PreparedBatchGenerationCreateWorkflowPersistenceParts {
    task_seed: BatchGenerationTaskPersistenceSeed,
    startup_snapshot_plan: BatchGenerationQueuedSnapshotPlan,
    response_payload: Value,
    runtime_input: BatchGenerationExecutionInput,
}

impl PreparedBatchGenerationCreateWorkflowLaunch {
    async fn prepare(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        request: BatchGenerationCreateWorkflowRequest,
    ) -> Result<Self, CreateBatchGenerationWriteWorkflowError> {
        let (normalized_target_word_count, chapter_targets) = request
            .prepare(db, project_id)
            .await
            .map_err(CreateBatchGenerationWriteWorkflowError::Prepare)?;
        let task_spec = request.task_spec();
        let effective_style_id =
            resolve_batch_generation_create_effective_style_id(db, project_id, task_spec.style_id)
                .await
                .map_err(CreateBatchGenerationWriteWorkflowError::Internal)?;
        let task_spec = task_spec.with_effective_style_id(effective_style_id);
        let web_research_default = SettingsService::resolve_web_research_enabled(db, user_id)
            .await
            .map_err(|error| CreateBatchGenerationWriteWorkflowError::Config(error.to_string()))?;
        let request_runtime_state = request.into_request_runtime_state(web_research_default);
        let model_override = request_runtime_state.model_override.clone();
        let runtime_seed = BatchGenerationCreateRuntimeSeed::prepare(
            db,
            project_id,
            task_spec.start_chapter_number,
            request_runtime_state,
        )
        .await
        .map_err(CreateBatchGenerationWriteWorkflowError::Internal)?;
        let execution_config =
            prepare_generation_execution_config(db, user_id, model_override.as_deref())
                .await
                .map_err(CreateBatchGenerationWriteWorkflowError::Config)?;
        Ok(Self::from_runtime_seed(
            task_spec,
            normalized_target_word_count,
            chapter_targets,
            user_id,
            runtime_seed,
            execution_config,
        ))
    }

    fn from_runtime_seed(
        task_spec: BatchGenerationCreateTaskSpec,
        normalized_target_word_count: i32,
        chapters_to_generate: Vec<BatchGenerationCreateChapterTarget>,
        user_id: &str,
        runtime_seed: BatchGenerationCreateRuntimeSeed,
        execution_config:
            crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig,
    ) -> Self {
        let total_chapters = chapters_to_generate.len() as i32;
        let chapter_ids = chapters_to_generate
            .iter()
            .map(|target| target.id.clone())
            .collect();
        let (startup_snapshot_plan, runtime_input) = runtime_seed.into_workflow_launch_parts(
            user_id.to_string(),
            chapter_ids,
            total_chapters,
            normalized_target_word_count,
            execution_config,
        );

        Self {
            task_spec,
            chapters_to_generate,
            startup_snapshot_plan,
            runtime_input,
        }
    }

    async fn prepare_persistence_plan(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        request: BatchGenerationCreateWorkflowRequest,
        task_id: String,
    ) -> Result<BatchGenerationCreateLaunchPersistencePlan, CreateBatchGenerationWriteWorkflowError>
    {
        Ok(Self::prepare(db, project_id, user_id, request)
            .await?
            .into_persistence_plan(task_id, project_id.to_string()))
    }

    fn into_persistence_plan(
        self,
        task_id: String,
        project_id: String,
    ) -> BatchGenerationCreateLaunchPersistencePlan {
        BatchGenerationCreateLaunchPersistencePlan::from_workflow_persistence_parts(
            self.into_persistence_parts(task_id, project_id),
        )
    }

    fn into_persistence_parts(
        self,
        task_id: String,
        project_id: String,
    ) -> PreparedBatchGenerationCreateWorkflowPersistenceParts {
        let total_chapters = self.chapters_to_generate.len() as i32;
        let response_payload = batch_generation_create_response_payload(
            &task_id,
            &project_id,
            &self.chapters_to_generate,
            self.runtime_input.target_word_count,
            self.task_spec.enable_analysis,
            &self.startup_snapshot_plan,
        );
        let task_seed = BatchGenerationTaskPersistenceSeed {
            id: task_id,
            project_id,
            user_id: self.runtime_input.user_id.clone(),
            start_chapter_number: self.task_spec.start_chapter_number,
            chapter_count: total_chapters,
            chapter_ids: Value::Array(
                self.runtime_input
                    .chapter_ids
                    .iter()
                    .map(|chapter_id| json!(chapter_id))
                    .collect(),
            ),
            style_id: self.task_spec.style_id,
            target_word_count: self.runtime_input.target_word_count,
            enable_analysis: self.task_spec.enable_analysis,
            total_chapters,
            current_chapter_id: None,
            current_chapter_number: None,
            max_retries: self.task_spec.max_retries,
        };

        PreparedBatchGenerationCreateWorkflowPersistenceParts {
            task_seed,
            startup_snapshot_plan: self.startup_snapshot_plan,
            response_payload,
            runtime_input: self.runtime_input,
        }
    }
}

impl BatchGenerationCreateLaunchPersistencePlan {
    async fn start(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        request: BatchGenerationCreateWorkflowRequest,
        now: chrono::NaiveDateTime,
    ) -> Result<Value, CreateBatchGenerationWriteWorkflowError> {
        Self::prepare(db, project_id, user_id, request)
            .await?
            .persist_and_dispatch(db, now)
            .await
    }

    async fn prepare(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        request: BatchGenerationCreateWorkflowRequest,
    ) -> Result<Self, CreateBatchGenerationWriteWorkflowError> {
        PreparedBatchGenerationCreateWorkflowLaunch::prepare_persistence_plan(
            db,
            project_id,
            user_id,
            request,
            Uuid::new_v4().to_string(),
        )
        .await
    }

    #[cfg(test)]
    fn from_workflow_launch(
        task_id: String,
        project_id: String,
        workflow_launch: PreparedBatchGenerationCreateWorkflowLaunch,
    ) -> Self {
        Self::from_workflow_persistence_parts(
            workflow_launch.into_persistence_parts(task_id, project_id),
        )
    }

    fn from_workflow_persistence_parts(
        persistence_parts: PreparedBatchGenerationCreateWorkflowPersistenceParts,
    ) -> Self {
        let PreparedBatchGenerationCreateWorkflowPersistenceParts {
            task_seed,
            startup_snapshot_plan,
            response_payload,
            runtime_input,
        } = persistence_parts;

        Self {
            task_seed,
            startup_snapshot_plan,
            response_payload,
            runtime_input,
        }
    }

    #[cfg(test)]
    fn response_payload(&self) -> Value {
        self.response_payload.clone()
    }

    #[cfg(test)]
    fn background_task_active_model(
        &self,
        now: chrono::NaiveDateTime,
    ) -> crate::models::batch_generation_task::ActiveModel {
        self.task_seed.clone().into_active_model(now)
    }

    async fn persist_and_dispatch(
        self,
        db: &DatabaseConnection,
        now: chrono::NaiveDateTime,
    ) -> Result<Value, CreateBatchGenerationWriteWorkflowError> {
        let BatchGenerationCreateLaunchPersistencePlan {
            task_seed,
            startup_snapshot_plan,
            response_payload,
            runtime_input,
        } = self;
        let task_id = task_seed.id.clone();
        let task = task_seed.into_active_model(now);

        task.insert(db).await.map_err(|error| {
            CreateBatchGenerationWriteWorkflowError::Internal(error.to_string())
        })?;
        startup_snapshot_plan
            .persist(db, &task_id)
            .await
            .map_err(CreateBatchGenerationWriteWorkflowError::Internal)?;
        dispatch_batch_generation_runtime(db.clone(), task_id, runtime_input);

        Ok(response_payload)
    }
}

impl PreparedBatchGenerationResumeWorkflowLaunch {
    async fn start(
        db: &DatabaseConnection,
        batch_id: &str,
        user_id: &str,
    ) -> Result<Value, ResumeBatchGenerationWriteWorkflowError> {
        Self::prepare(db, batch_id, user_id)
            .await?
            .persist_and_dispatch(db)
            .await
    }

    async fn prepare(
        db: &DatabaseConnection,
        batch_id: &str,
        user_id: &str,
    ) -> Result<Self, ResumeBatchGenerationWriteWorkflowError> {
        Ok(Self {
            persistence_plan: prepare_owned_batch_generation_resume(db, batch_id, user_id)
                .await
                .map_err(ResumeBatchGenerationWriteWorkflowError::from)?,
        })
    }

    async fn persist_and_dispatch(
        self,
        db: &DatabaseConnection,
    ) -> Result<Value, ResumeBatchGenerationWriteWorkflowError> {
        self.persistence_plan
            .persist_and_dispatch(db)
            .await
            .map_err(ResumeBatchGenerationWriteWorkflowError::Config)
    }

    #[cfg(test)]
    fn from_persistence_plan(persistence_plan: BatchGenerationResumeLaunchPersistencePlan) -> Self {
        Self { persistence_plan }
    }

    #[cfg(test)]
    fn persistence_plan(&self) -> &BatchGenerationResumeLaunchPersistencePlan {
        &self.persistence_plan
    }
}

impl PreparedBatchGenerationCancelWorkflowLaunch {
    async fn start(
        db: &DatabaseConnection,
        batch_id: &str,
        user_id: &str,
    ) -> Result<Value, CancelBatchGenerationWriteWorkflowError> {
        Self::prepare(db, batch_id, user_id)
            .await?
            .persist_and_dispatch(db)
            .await
    }

    async fn prepare(
        db: &DatabaseConnection,
        batch_id: &str,
        user_id: &str,
    ) -> Result<Self, CancelBatchGenerationWriteWorkflowError> {
        Ok(Self {
            persistence_plan: prepare_owned_batch_generation_cancel_workflow(db, batch_id, user_id)
                .await
                .map_err(CancelBatchGenerationWriteWorkflowError::from)?,
        })
    }

    async fn persist_and_dispatch(
        self,
        db: &DatabaseConnection,
    ) -> Result<Value, CancelBatchGenerationWriteWorkflowError> {
        self.persistence_plan
            .persist(db)
            .await
            .map_err(CancelBatchGenerationWriteWorkflowError::Domain)
    }

    #[cfg(test)]
    fn from_persistence_plan(
        persistence_plan: crate::services::chapter_batch_generation_runtime_state_service::BatchGenerationCancelledPersistencePlan,
    ) -> Self {
        Self { persistence_plan }
    }

    #[cfg(test)]
    fn persistence_plan(
        &self,
    ) -> &crate::services::chapter_batch_generation_runtime_state_service::BatchGenerationCancelledPersistencePlan{
        &self.persistence_plan
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CreateBatchGenerationWriteWorkflowError {
    ProjectAccess(ProjectAccessQueryError),
    Prepare(PrepareBatchGenerationCreateRequestError),
    Config(String),
    Internal(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResumeBatchGenerationWriteWorkflowError {
    Task(LoadOwnedBatchGenerationTaskError),
    Domain(ResumeBatchGenerationDomainError),
    Config(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CancelBatchGenerationWriteWorkflowError {
    Task(LoadOwnedBatchGenerationTaskError),
    Domain(String),
}

impl From<PrepareOwnedBatchGenerationResumeError> for ResumeBatchGenerationWriteWorkflowError {
    fn from(error: PrepareOwnedBatchGenerationResumeError) -> Self {
        match error {
            PrepareOwnedBatchGenerationResumeError::Task(error) => Self::Task(error),
            PrepareOwnedBatchGenerationResumeError::Domain(error) => Self::Domain(error),
            PrepareOwnedBatchGenerationResumeError::Config(error) => Self::Config(error),
        }
    }
}

fn validate_cancel_batch_generation_task_status(
    task: &crate::models::batch_generation_task::Model,
) -> Result<(), CancelBatchGenerationWriteWorkflowError> {
    if matches!(task.status.as_str(), "completed" | "failed" | "cancelled") {
        return Err(CancelBatchGenerationWriteWorkflowError::Domain(format!(
            "Cannot cancel task in status {}",
            task.status
        )));
    }

    Ok(())
}

fn map_prepare_owned_batch_generation_cancel_sources_error(
    error: LoadOwnedBatchGenerationTaskSourcesError,
) -> CancelBatchGenerationWriteWorkflowError {
    match error {
        LoadOwnedBatchGenerationTaskSourcesError::Task(error) => {
            CancelBatchGenerationWriteWorkflowError::Task(error)
        }
        LoadOwnedBatchGenerationTaskSourcesError::Snapshot(error) => {
            CancelBatchGenerationWriteWorkflowError::Domain(error)
        }
    }
}

fn prepare_cancel_batch_generation_persistence_plan_from_owned_sources(
    task: crate::models::batch_generation_task::Model,
    snapshot: Option<crate::models::batch_generation_snapshot::Model>,
) -> Result<BatchGenerationCancelledPersistencePlan, CancelBatchGenerationWriteWorkflowError> {
    validate_cancel_batch_generation_task_status(&task)?;
    Ok(BatchGenerationCancelledPersistencePlan::from_sources(
        &task,
        snapshot.as_ref(),
    ))
}

async fn prepare_owned_batch_generation_cancel_workflow(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<BatchGenerationCancelledPersistencePlan, CancelBatchGenerationWriteWorkflowError> {
    let (task, snapshot) = load_owned_batch_generation_task_sources(db, batch_id, user_id)
        .await
        .map_err(map_prepare_owned_batch_generation_cancel_sources_error)?
        .into_parts();

    prepare_cancel_batch_generation_persistence_plan_from_owned_sources(task, snapshot)
}

pub(crate) async fn start_owned_batch_generation_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    request: BatchGenerationCreateWorkflowRequest,
) -> Result<Value, CreateBatchGenerationWriteWorkflowError> {
    ensure_owned_project_access(db, project_id, user_id)
        .await
        .map_err(CreateBatchGenerationWriteWorkflowError::ProjectAccess)?;
    BatchGenerationCreateLaunchPersistencePlan::start(
        db,
        project_id,
        user_id,
        request,
        Utc::now().naive_utc(),
    )
    .await
}

pub(crate) async fn start_owned_batch_generation_write_workflow_from_route_payload(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    route_request: BatchGenerationCreateRouteRequest,
) -> Result<Value, CreateBatchGenerationWriteWorkflowError> {
    start_owned_batch_generation_write_workflow(
        db,
        project_id,
        user_id,
        build_batch_generation_create_workflow_request_from_route_payload(route_request),
    )
    .await
}

pub(crate) async fn resume_owned_batch_generation_write_workflow(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Value, ResumeBatchGenerationWriteWorkflowError> {
    PreparedBatchGenerationResumeWorkflowLaunch::start(db, batch_id, user_id).await
}

pub(crate) async fn cancel_owned_batch_generation_write_workflow(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Value, CancelBatchGenerationWriteWorkflowError> {
    PreparedBatchGenerationCancelWorkflowLaunch::start(db, batch_id, user_id).await
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};
    use serde_json::{json, Value};

    use super::{
        build_batch_generation_create_workflow_request_from_route_payload,
        build_batch_generation_runtime_state_payload_from_parts,
        build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload,
        BatchGenerationCreateChapterTarget, BatchGenerationCreateLaunchPersistencePlan,
        BatchGenerationCreateRouteRequest, BatchGenerationCreateRuntimeSeed,
        BatchGenerationCreateStartupRuntimeState, BatchGenerationCreateStartupSeedSource,
        BatchGenerationCreateTaskSpec, BatchGenerationCreateWorkflowRequest,
        CancelBatchGenerationWriteWorkflowError, CreateBatchGenerationWriteWorkflowError,
        PrepareBatchGenerationCreateRequestError, PreparedBatchGenerationCancelWorkflowLaunch,
        PreparedBatchGenerationCreateWorkflowLaunch, PreparedBatchGenerationResumeWorkflowLaunch,
    };
    use crate::models::chapter;
    use crate::models::{batch_generation_snapshot, batch_generation_task};
    use crate::services::chapter_batch_generation_owned_task_query_service::LoadOwnedBatchGenerationTaskError;
    use crate::services::chapter_batch_generation_resume_semantics_service::ResumeBatchGenerationCommandState;
    use crate::services::chapter_batch_generation_resume_task_command_service::{
        BatchGenerationResumeLaunchPersistencePlan, ResumeExecutionDispatchPlan,
    };
    use crate::services::chapter_batch_generation_runtime_state_service::{
        build_batch_generation_execution_input, BatchGenerationCancelledPersistencePlan,
        BatchGenerationResumeResetPersistencePlan,
    };
    use crate::services::chapter_batch_generation_task_model_service::BatchGenerationTaskPersistenceSeed;
    use crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig;
    use crate::services::chapter_generation_request_runtime_state_service::{
        batch_generation_request_runtime_state_payload, BatchGenerationRequestRuntimeState,
    };
    use crate::services::chapter_generation_target_word_count_service::normalize_chapter_generation_target_word_count;
    use crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions;
    use crate::services::chapter_story_repair_quality_context_service::aggregate_story_repair_quality_summaries;
    use crate::services::project_access_query_service::ProjectAccessQueryError;

    fn build_resume_task(status: &str) -> crate::models::batch_generation_task::Model {
        crate::models::batch_generation_task::Model {
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

    fn chapter_model() -> chapter::Model {
        chapter::Model {
            id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
            content: Some("正文".to_string()),
            summary: Some("摘要".to_string()),
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    fn chapter_model_with_number(chapter_number: i32) -> chapter::Model {
        chapter::Model {
            id: format!("chapter-{chapter_number}"),
            chapter_number,
            title: format!("第{chapter_number}章"),
            ..chapter_model()
        }
    }

    fn build_chapter_target(
        id: &str,
        chapter_number: i32,
        title: &str,
    ) -> BatchGenerationCreateChapterTarget {
        BatchGenerationCreateChapterTarget {
            id: id.to_string(),
            chapter_number,
            title: title.to_string(),
        }
    }

    fn build_test_generation_execution_config() -> PreparedGenerationExecutionConfig {
        PreparedGenerationExecutionConfig {
            ai_config: crate::ai::AIConfig::default(),
            provider_payload: crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload {
                recent_chapters_context: String::new(),
                previous_chapter_summary: String::new(),
                chapter_careers: "[]".to_string(),
                characters_info: "[]".to_string(),
                foreshadow_reminders: "[]".to_string(),
                relevant_memories: String::new(),
                research_query: String::new(),
                research_assets: "[]".to_string(),
                external_assets: "[]".to_string(),
                reference_assets: "[]".to_string(),
                mcp_references: String::new(),
            },
        }
    }

    fn build_test_batch_generation_create_workflow_launch(
        task_spec: BatchGenerationCreateTaskSpec,
        normalized_target_word_count: i32,
        chapters_to_generate: Vec<BatchGenerationCreateChapterTarget>,
        user_id: &str,
        runtime_seed: BatchGenerationCreateRuntimeSeed,
    ) -> PreparedBatchGenerationCreateWorkflowLaunch {
        super::PreparedBatchGenerationCreateWorkflowLaunch::from_runtime_seed(
            task_spec,
            normalized_target_word_count,
            chapters_to_generate,
            user_id,
            runtime_seed,
            build_test_generation_execution_config(),
        )
    }

    fn build_test_batch_generation_create_workflow_entry(
        task_id: &str,
        project_id: &str,
        task_spec: BatchGenerationCreateTaskSpec,
        normalized_target_word_count: i32,
        chapters_to_generate: Vec<BatchGenerationCreateChapterTarget>,
        user_id: &str,
        runtime_seed: BatchGenerationCreateRuntimeSeed,
    ) -> BatchGenerationCreateLaunchPersistencePlan {
        BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            task_id.to_string(),
            project_id.to_string(),
            build_test_batch_generation_create_workflow_launch(
                task_spec,
                normalized_target_word_count,
                chapters_to_generate,
                user_id,
                runtime_seed,
            ),
        )
    }

    fn build_test_batch_generation_resume_workflow_launch(
        command_state: ResumeBatchGenerationCommandState,
        user_id: &str,
        chapter_ids: Vec<String>,
        enable_analysis: bool,
    ) -> PreparedBatchGenerationResumeWorkflowLaunch {
        let dispatch_plan = ResumeExecutionDispatchPlan::Batch {
            runtime_input: build_batch_generation_execution_input(
                user_id.to_string(),
                chapter_ids,
                command_state.target_word_count,
                SingleChapterGenerationCompatOptions {
                    enable_analysis,
                    ..Default::default()
                },
                build_test_generation_execution_config(),
            ),
        };
        let reset_persistence_plan =
            BatchGenerationResumeResetPersistencePlan::from_resume_task(&command_state, None);

        PreparedBatchGenerationResumeWorkflowLaunch::from_persistence_plan(
            BatchGenerationResumeLaunchPersistencePlan::from_contract_for_test(
                command_state,
                dispatch_plan,
                reset_persistence_plan,
            ),
        )
    }

    #[test]
    fn should_normalize_batch_generation_target_word_count() {
        assert_eq!(normalize_chapter_generation_target_word_count(None), 3000);
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(-100)),
            1
        );
        assert_eq!(normalize_chapter_generation_target_word_count(Some(0)), 1);
        assert_eq!(
            normalize_chapter_generation_target_word_count(Some(2500)),
            2500
        );
    }

    #[test]
    fn should_build_batch_generation_create_chapter_target_projection() {
        let target = BatchGenerationCreateChapterTarget::from_model(&chapter_model());

        assert_eq!(target.id, "chapter-7");
        assert_eq!(target.chapter_number, 7);
        assert_eq!(target.title, "第七章");
    }

    #[test]
    fn should_reject_unknown_batch_generation_create_route_fields_like_python_schema() {
        let error = serde_json::from_value::<BatchGenerationCreateRouteRequest>(json!({
            "start_chapter_number": 1,
            "count": 2,
            "unexpected_field": true
        }))
        .expect_err("python BatchGenerateRequest forbids extra fields");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn should_accept_known_batch_generation_create_route_fields_with_strict_schema() {
        let request = serde_json::from_value::<BatchGenerationCreateRouteRequest>(json!({
            "start_chapter_number": 1,
            "count": 2,
            "target_word_count": 3000,
            "creative_mode": "hook",
            "quality_notes": "keep pacing tight"
        }))
        .expect("known python BatchGenerateRequest fields should parse");

        assert_eq!(request.start_chapter_number, 1);
        assert_eq!(request.count, 2);
        assert_eq!(request.target_word_count, Some(3000));
        assert_eq!(request.creative_mode.as_deref(), Some("hook"));
        assert_eq!(request.quality_notes.as_deref(), Some("keep pacing tight"));
    }

    #[test]
    fn should_reject_batch_generation_create_route_null_for_non_nullable_python_default_fields() {
        for (field_name, payload) in [
            (
                "enable_analysis",
                json!({
                    "start_chapter_number": 1,
                    "count": 2,
                    "enable_analysis": null
                }),
            ),
            (
                "enable_mcp",
                json!({
                    "start_chapter_number": 1,
                    "count": 2,
                    "enable_mcp": null
                }),
            ),
            (
                "max_retries",
                json!({
                    "start_chapter_number": 1,
                    "count": 2,
                    "max_retries": null
                }),
            ),
        ] {
            let error =
                serde_json::from_value::<BatchGenerationCreateRouteRequest>(payload).unwrap_err();

            assert!(
                error.to_string().contains("invalid type: null"),
                "{field_name} should reject explicit null like Python defaulted fields"
            );
        }
    }

    #[test]
    fn should_keep_batch_generation_create_route_nullable_fields_accepting_null() {
        let request = serde_json::from_value::<BatchGenerationCreateRouteRequest>(json!({
            "start_chapter_number": 1,
            "count": 2,
            "target_word_count": null,
            "enable_web_research": null
        }))
        .expect("Python Optional fields should keep accepting explicit null");

        assert_eq!(request.target_word_count, None);
        assert_eq!(request.enable_web_research, None);
    }

    #[test]
    fn should_apply_batch_generation_create_python_defaults_when_fields_are_missing() {
        let route_request = serde_json::from_value::<BatchGenerationCreateRouteRequest>(json!({
            "start_chapter_number": 1,
            "count": 2
        }))
        .expect("missing defaulted route fields should parse");
        assert_eq!(route_request.enable_analysis, None);
        assert_eq!(route_request.enable_mcp, None);
        assert_eq!(route_request.max_retries, None);

        let request = BatchGenerationCreateWorkflowRequest::from_route_request(route_request);
        let compat = request.compat_options_with_web_research_default(false);

        assert!(!request.enable_analysis);
        assert_eq!(request.max_retries, 3);
        assert!(compat.enable_mcp());
        assert!(!compat.web_research_enabled());
    }

    #[test]
    fn should_keep_batch_generation_create_route_payload_request_contract() {
        let request = build_batch_generation_create_workflow_request_from_route_payload(
            BatchGenerationCreateRouteRequest {
                start_chapter_number: 5,
                count: 3,
                style_id: Some(9),
                target_word_count: Some(3200),
                enable_analysis: Some(true),
                enable_mcp: Some(true),
                enable_web_research: Some(false),
                web_research_query: Some("ignored".to_string()),
                max_retries: Some(6),
                model: Some("gpt-4.1-mini".to_string()),
                creative_mode: Some("hook".to_string()),
                story_focus: Some("advance_plot".to_string()),
                plot_stage: Some("climax".to_string()),
                story_creation_brief: Some("brief".to_string()),
                quality_preset: Some("plot_drive".to_string()),
                quality_notes: Some("notes".to_string()),
                story_repair_summary: Some("repair".to_string()),
                story_repair_targets: Some(vec!["target-a".to_string()]),
                story_preserve_strengths: Some(vec!["strength-a".to_string()]),
            },
        );

        assert_eq!(request.start_chapter_number, 5);
        assert_eq!(request.count, 3);
        assert_eq!(request.style_id, Some(9));
        assert_eq!(request.target_word_count, Some(3200));
        assert!(request.enable_analysis);
        assert_eq!(request.enable_mcp, Some(true));
        assert_eq!(request.enable_web_research, Some(false));
        assert_eq!(request.max_retries, 6);
        assert_eq!(request.model_override.as_deref(), Some("gpt-4.1-mini"));
        assert_eq!(request.creative_mode.as_deref(), Some("hook"));
        assert_eq!(request.story_focus.as_deref(), Some("advance_plot"));
        assert_eq!(request.plot_stage.as_deref(), Some("climax"));
        assert_eq!(request.quality_preset.as_deref(), Some("plot_drive"));
    }

    #[test]
    fn should_normalize_batch_generation_create_generation_fields_like_python_schema() {
        let request = BatchGenerationCreateWorkflowRequest::from_route_request(
            BatchGenerationCreateRouteRequest {
                start_chapter_number: 1,
                count: 2,
                creative_mode: Some(" hook ".to_string()),
                story_focus: Some(" advance_plot ".to_string()),
                plot_stage: Some(" development ".to_string()),
                story_creation_brief: Some(" 本轮强化开场钩子 ".to_string()),
                quality_preset: Some(" plot_drive ".to_string()),
                quality_notes: Some(" 压缩说明段 ".to_string()),
                story_repair_summary: Some(" 修复中段节奏 ".to_string()),
                ..Default::default()
            },
        );

        assert_eq!(request.creative_mode.as_deref(), Some("hook"));
        assert_eq!(request.story_focus.as_deref(), Some("advance_plot"));
        assert_eq!(request.plot_stage.as_deref(), Some("development"));
        assert_eq!(
            request.story_creation_brief.as_deref(),
            Some("本轮强化开场钩子")
        );
        assert_eq!(request.quality_preset.as_deref(), Some("plot_drive"));
        assert_eq!(request.quality_notes.as_deref(), Some("压缩说明段"));
        assert_eq!(
            request.story_repair_summary.as_deref(),
            Some("修复中段节奏")
        );
    }

    #[test]
    fn should_convert_blank_batch_generation_create_generation_fields_to_none() {
        let request = BatchGenerationCreateWorkflowRequest::from_route_request(
            BatchGenerationCreateRouteRequest {
                start_chapter_number: 1,
                count: 2,
                creative_mode: Some("   ".to_string()),
                story_focus: Some("\t".to_string()),
                plot_stage: Some("\n".to_string()),
                story_creation_brief: Some("   ".to_string()),
                quality_preset: Some("   ".to_string()),
                quality_notes: Some("   ".to_string()),
                story_repair_summary: Some("   ".to_string()),
                ..Default::default()
            },
        );

        assert!(request.creative_mode.is_none());
        assert!(request.story_focus.is_none());
        assert!(request.plot_stage.is_none());
        assert!(request.story_creation_brief.is_none());
        assert!(request.quality_preset.is_none());
        assert!(request.quality_notes.is_none());
        assert!(request.story_repair_summary.is_none());
    }

    #[test]
    fn should_seed_batch_runtime_state_with_normalized_generation_fields() {
        let request = BatchGenerationCreateWorkflowRequest::from_route_request(
            BatchGenerationCreateRouteRequest {
                start_chapter_number: 1,
                count: 2,
                creative_mode: Some(" hook ".to_string()),
                story_focus: Some(" advance_plot ".to_string()),
                plot_stage: Some(" development ".to_string()),
                story_creation_brief: Some(" 强化开场悬念 ".to_string()),
                quality_preset: Some(" plot_drive ".to_string()),
                quality_notes: Some(" 保持短句推进 ".to_string()),
                story_repair_summary: Some(" 修复伏笔衔接 ".to_string()),
                ..Default::default()
            },
        );

        let runtime_state = request.into_request_runtime_state(false);
        let payload = batch_generation_request_runtime_state_payload(&runtime_state);
        let compat_payload = &payload["batch_request_runtime_state"]["compat_options"];

        assert_eq!(compat_payload["creative_mode"], "hook");
        assert_eq!(compat_payload["story_focus"], "advance_plot");
        assert_eq!(compat_payload["plot_stage"], "development");
        assert_eq!(compat_payload["story_creation_brief"], "强化开场悬念");
        assert_eq!(compat_payload["quality_preset"], "plot_drive");
        assert_eq!(compat_payload["quality_notes"], "保持短句推进");
        assert_eq!(compat_payload["story_repair_summary"], "修复伏笔衔接");
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "修复伏笔衔接"
        );
    }

    #[test]
    fn should_project_batch_generation_create_targets_directly() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        assert_eq!(
            chapters_to_generate
                .iter()
                .map(|target| target.id.clone())
                .collect::<Vec<_>>(),
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        let chapter_id_payload = Value::Array(
            chapters_to_generate
                .iter()
                .map(|target| target.id.clone())
                .into_iter()
                .map(|chapter_id| json!(chapter_id))
                .collect(),
        );
        assert_eq!(chapter_id_payload, json!(["chapter-1", "chapter-2"]));
        let chapters_to_generate_payload = chapters_to_generate
            .iter()
            .map(|target| {
                json!({
                    "id": target.id,
                    "chapter_number": target.chapter_number,
                    "title": target.title,
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(chapters_to_generate_payload[0]["id"], "chapter-1");
        assert_eq!(chapters_to_generate_payload[1]["title"], "Second");
        assert_eq!(chapters_to_generate.len() as i32, 2);
    }

    #[test]
    fn should_select_batch_generation_create_range_from_project_chapters() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 2,
            count: 2,
            ..Default::default()
        };

        let selected = request
            .select_chapters_for_generation_range(vec![
                chapter_model_with_number(1),
                chapter_model_with_number(2),
                chapter_model_with_number(3),
                chapter_model_with_number(4),
            ])
            .expect("selected chapters");

        assert_eq!(
            selected
                .iter()
                .map(|chapter| chapter.chapter_number)
                .collect::<Vec<_>>(),
            vec![2, 3]
        );
    }

    #[test]
    fn should_distinguish_empty_project_from_empty_batch_generation_range() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 5,
            count: 2,
            ..Default::default()
        };

        let empty_project_error = request
            .select_chapters_for_generation_range(Vec::new())
            .expect_err("empty project should fail");
        let empty_range_error = request
            .select_chapters_for_generation_range(vec![
                chapter_model_with_number(1),
                chapter_model_with_number(2),
            ])
            .expect_err("empty range should fail");

        assert!(matches!(
            empty_project_error,
            PrepareBatchGenerationCreateRequestError::ProjectHasNoChapters
        ));
        assert!(matches!(
            empty_range_error,
            PrepareBatchGenerationCreateRequestError::ChaptersNotFound
        ));
    }

    #[test]
    fn should_reject_batch_generation_create_count_above_python_limit() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 21,
            max_retries: 3,
            ..Default::default()
        };

        let error = request
            .validate_request_bounds()
            .expect_err("count above python limit should fail");

        assert!(matches!(
            error,
            PrepareBatchGenerationCreateRequestError::InvalidCountTooLarge
        ));
    }

    #[test]
    fn should_reject_batch_generation_create_target_word_count_below_python_limit() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            target_word_count: Some(499),
            max_retries: 3,
            ..Default::default()
        };

        let error = request
            .validate_request_bounds()
            .expect_err("target word count below python limit should fail");

        assert!(matches!(
            error,
            PrepareBatchGenerationCreateRequestError::InvalidTargetWordCountTooSmall
        ));
    }

    #[test]
    fn should_reject_batch_generation_create_target_word_count_above_python_limit() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            target_word_count: Some(10_001),
            max_retries: 3,
            ..Default::default()
        };

        let error = request
            .validate_request_bounds()
            .expect_err("target word count above python limit should fail");

        assert!(matches!(
            error,
            PrepareBatchGenerationCreateRequestError::InvalidTargetWordCountTooLarge
        ));
    }

    #[test]
    fn should_reject_batch_generation_create_max_retries_outside_python_bounds() {
        let too_low = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            max_retries: -1,
            ..Default::default()
        };
        let too_high = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            max_retries: 6,
            ..Default::default()
        };

        assert!(matches!(
            too_low
                .validate_request_bounds()
                .expect_err("negative max_retries should fail"),
            PrepareBatchGenerationCreateRequestError::InvalidMaxRetries
        ));
        assert!(matches!(
            too_high
                .validate_request_bounds()
                .expect_err("max_retries above python limit should fail"),
            PrepareBatchGenerationCreateRequestError::InvalidMaxRetries
        ));
    }

    #[test]
    fn should_reject_batch_generation_create_invalid_generation_choice_fields() {
        let cases = [
            (
                BatchGenerationCreateWorkflowRequest {
                    start_chapter_number: 1,
                    count: 2,
                    max_retries: 3,
                    creative_mode: Some("too_fancy".to_string()),
                    ..Default::default()
                },
                PrepareBatchGenerationCreateRequestError::InvalidCreativeMode,
            ),
            (
                BatchGenerationCreateWorkflowRequest {
                    start_chapter_number: 1,
                    count: 2,
                    max_retries: 3,
                    story_focus: Some("too_broad".to_string()),
                    ..Default::default()
                },
                PrepareBatchGenerationCreateRequestError::InvalidStoryFocus,
            ),
            (
                BatchGenerationCreateWorkflowRequest {
                    start_chapter_number: 1,
                    count: 2,
                    max_retries: 3,
                    plot_stage: Some("middle".to_string()),
                    ..Default::default()
                },
                PrepareBatchGenerationCreateRequestError::InvalidPlotStage,
            ),
            (
                BatchGenerationCreateWorkflowRequest {
                    start_chapter_number: 1,
                    count: 2,
                    max_retries: 3,
                    quality_preset: Some("max_quality".to_string()),
                    ..Default::default()
                },
                PrepareBatchGenerationCreateRequestError::InvalidQualityPreset,
            ),
        ];

        for (request, expected_error) in cases {
            assert_eq!(
                request
                    .validate_request_bounds()
                    .expect_err("invalid generation choice should fail"),
                expected_error
            );
        }
    }

    #[test]
    fn should_reject_batch_generation_create_generation_text_fields_above_python_limits() {
        let long_brief = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            max_retries: 3,
            story_creation_brief: Some("a".repeat(1201)),
            ..Default::default()
        };
        let long_quality_notes = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            max_retries: 3,
            quality_notes: Some("b".repeat(601)),
            ..Default::default()
        };

        assert_eq!(
            long_brief
                .validate_request_bounds()
                .expect_err("story_creation_brief above python limit should fail"),
            PrepareBatchGenerationCreateRequestError::StoryCreationBriefTooLong
        );
        assert_eq!(
            long_quality_notes
                .validate_request_bounds()
                .expect_err("quality_notes above python limit should fail"),
            PrepareBatchGenerationCreateRequestError::QualityNotesTooLong
        );
    }

    #[test]
    fn should_accept_batch_generation_create_python_request_bounds() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 20,
            target_word_count: Some(10_000),
            max_retries: 5,
            ..Default::default()
        };
        let lower_bound_request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 1,
            target_word_count: Some(500),
            max_retries: 0,
            ..Default::default()
        };
        let default_target_request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 1,
            target_word_count: None,
            max_retries: 3,
            ..Default::default()
        };
        let choice_and_text_request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 1,
            target_word_count: Some(3000),
            max_retries: 3,
            creative_mode: Some("hook".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("development".to_string()),
            quality_preset: Some("plot_drive".to_string()),
            story_creation_brief: Some("a".repeat(1200)),
            quality_notes: Some("b".repeat(600)),
            ..Default::default()
        };
        let blank_choice_and_text_request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 1,
            max_retries: 3,
            creative_mode: Some("   ".to_string()),
            story_focus: Some("   ".to_string()),
            plot_stage: Some("   ".to_string()),
            quality_preset: Some("   ".to_string()),
            story_creation_brief: Some("   ".to_string()),
            quality_notes: Some("   ".to_string()),
            ..Default::default()
        };

        request
            .validate_request_bounds()
            .expect("python upper bounds should pass");
        lower_bound_request
            .validate_request_bounds()
            .expect("python lower bounds should pass");
        default_target_request
            .validate_request_bounds()
            .expect("default target word count should pass");
        choice_and_text_request
            .validate_request_bounds()
            .expect("valid python generation choices and text lengths should pass");
        blank_choice_and_text_request
            .validate_request_bounds()
            .expect("blank choices and texts normalize to None in python");
    }

    #[test]
    fn should_keep_batch_generation_route_write_workflow_project_access_error_shape() {
        let error = CreateBatchGenerationWriteWorkflowError::ProjectAccess(
            ProjectAccessQueryError::NotFoundOrAccessDenied,
        );

        assert!(matches!(
            error,
            CreateBatchGenerationWriteWorkflowError::ProjectAccess(
                ProjectAccessQueryError::NotFoundOrAccessDenied
            )
        ));
    }

    #[test]
    fn should_keep_batch_generation_route_write_workflow_error_shape() {
        let error = CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::InvalidCount,
        );

        assert!(matches!(
            error,
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::InvalidCount
            )
        ));
    }

    #[test]
    fn should_apply_effective_style_id_to_batch_generation_create_task_spec() {
        let task_spec = BatchGenerationCreateTaskSpec {
            start_chapter_number: 1,
            style_id: None,
            enable_analysis: false,
            max_retries: 3,
        }
        .with_effective_style_id(Some(12));

        assert_eq!(task_spec.style_id, Some(12));
        assert_eq!(task_spec.start_chapter_number, 1);
        assert!(!task_spec.enable_analysis);
        assert_eq!(task_spec.max_retries, 3);
    }

    #[test]
    fn should_select_explicit_batch_generation_create_style_before_default_style() {
        assert_eq!(
            super::select_batch_generation_create_effective_style_id(Some(9), Some(12)),
            Some(9)
        );
        assert_eq!(
            super::select_batch_generation_create_effective_style_id(None, Some(12)),
            Some(12)
        );
        assert_eq!(
            super::select_batch_generation_create_effective_style_id(None, None),
            None
        );
    }

    #[test]
    fn should_keep_batch_generation_create_prerequisite_error_shape() {
        let error = CreateBatchGenerationWriteWorkflowError::Prepare(
            PrepareBatchGenerationCreateRequestError::PrerequisitesBlocked(
                "前置章节尚未完成: 2 章".to_string(),
            ),
        );

        assert!(matches!(
            error,
            CreateBatchGenerationWriteWorkflowError::Prepare(
                PrepareBatchGenerationCreateRequestError::PrerequisitesBlocked(detail)
            ) if detail == "前置章节尚未完成: 2 章"
        ));
    }

    #[test]
    fn should_keep_batch_generation_route_write_workflow_config_error_shape() {
        let error = CreateBatchGenerationWriteWorkflowError::Config("model missing".to_string());

        assert!(matches!(
            error,
            CreateBatchGenerationWriteWorkflowError::Config(detail) if detail == "model missing"
        ));
    }

    #[test]
    fn should_keep_resume_batch_generation_write_workflow_config_error_shape() {
        let error =
            super::ResumeBatchGenerationWriteWorkflowError::Config("model missing".to_string());

        assert!(matches!(
            error,
            super::ResumeBatchGenerationWriteWorkflowError::Config(detail)
                if detail == "model missing"
        ));
    }

    #[test]
    fn should_keep_resume_batch_generation_write_workflow_task_error_shape() {
        let error = super::ResumeBatchGenerationWriteWorkflowError::Task(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
        );

        assert!(matches!(
            error,
            super::ResumeBatchGenerationWriteWorkflowError::Task(
                LoadOwnedBatchGenerationTaskError::TaskNotFound
            )
        ));
    }

    #[test]
    fn should_keep_cancel_batch_generation_write_workflow_task_error_shape() {
        let error = CancelBatchGenerationWriteWorkflowError::Task(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
        );

        assert!(matches!(
            error,
            CancelBatchGenerationWriteWorkflowError::Task(
                LoadOwnedBatchGenerationTaskError::TaskNotFound
            )
        ));
    }

    #[test]
    fn should_keep_cancel_batch_generation_write_workflow_domain_error_shape() {
        let error =
            CancelBatchGenerationWriteWorkflowError::Domain("Cannot cancel task".to_string());

        assert!(matches!(
            error,
            CancelBatchGenerationWriteWorkflowError::Domain(detail)
                if detail == "Cannot cancel task"
        ));
    }

    #[test]
    fn should_keep_cancel_batch_generation_prepare_owner_contract() {
        let task = batch_generation_task::Model {
            id: "task-cancel-owner-1".to_string(),
            project_id: "project-cancel-1".to_string(),
            user_id: "user-cancel-1".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-1", "chapter-2"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: true,
            status: "running".to_string(),
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-2".to_string()),
            current_chapter_number: Some(2),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };
        let snapshot = batch_generation_snapshot::Model {
            id: "snapshot-cancel-owner-1".to_string(),
            batch_task_id: "task-cancel-owner-1".to_string(),
            latest_quality_metrics: None,
            quality_metrics_history: None,
            quality_metrics_summary: None,
            workflow_runtime_state: Some(json!({
                "progress": 55,
                "phase": "generating",
                "status": "running"
            })),
            created_at: None,
            updated_at: None,
        };

        let persistence_plan =
            super::prepare_cancel_batch_generation_persistence_plan_from_owned_sources(
                task.clone(),
                Some(snapshot.clone()),
            )
            .expect("running task should prepare cancel persistence plan");

        let payload = persistence_plan.response_payload_for_test(batch_generation_task::Model {
            status: "cancelled".to_string(),
            ..task
        });

        assert_eq!(payload["batch_id"], "task-cancel-owner-1");
        assert_eq!(payload["status"], "cancelled");
        assert_eq!(payload["message"], "Batch generation cancelled");
        assert_eq!(payload["checkpoint"]["phase"], "cancelled");
    }

    #[test]
    fn should_reject_terminal_status_inside_cancel_prepare_owner() {
        let task = batch_generation_task::Model {
            id: "task-cancel-owner-2".to_string(),
            project_id: "project-cancel-2".to_string(),
            user_id: "user-cancel-2".to_string(),
            start_chapter_number: 1,
            chapter_count: 1,
            chapter_ids: json!(["chapter-1"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: false,
            status: "cancelled".to_string(),
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
        };

        let error =
            super::prepare_cancel_batch_generation_persistence_plan_from_owned_sources(task, None)
                .expect_err("cancelled task should fail cancel preparation");

        assert!(matches!(
            error,
            CancelBatchGenerationWriteWorkflowError::Domain(detail)
                if detail == "Cannot cancel task in status cancelled"
        ));
    }

    #[test]
    fn should_keep_batch_generation_resume_workflow_launch_owner_contract() {
        let mut task = build_resume_task("failed");
        task.user_id = "user-7".to_string();
        task.target_word_count = 3100;
        task.enable_analysis = true;
        task.chapter_ids = json!(["chapter-1", "chapter-2"]);

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let workflow_launch = build_test_batch_generation_resume_workflow_launch(
            command_state.clone(),
            "user-7",
            vec!["chapter-1".to_string(), "chapter-2".to_string()],
            true,
        );

        match workflow_launch.persistence_plan().dispatch_plan() {
            ResumeExecutionDispatchPlan::Batch { runtime_input } => {
                assert_eq!(runtime_input.user_id, "user-7");
                assert_eq!(
                    runtime_input.chapter_ids,
                    vec!["chapter-1".to_string(), "chapter-2".to_string()]
                );
                assert_eq!(runtime_input.target_word_count, 3100);
                assert!(runtime_input.compat_options.enable_analysis);
            }
            ResumeExecutionDispatchPlan::SingleChapter { .. } => {
                panic!("expected batch dispatch plan");
            }
        }

        assert_eq!(
            workflow_launch.persistence_plan().response_payload()["batch_id"],
            command_state.batch_id
        );
    }

    #[test]
    fn should_keep_batch_generation_resume_workflow_launch_persistence_owner_contract() {
        let mut task = build_resume_task("cancelled");
        task.chapter_count = 2;
        task.total_chapters = 2;
        task.target_word_count = 2800;
        task.chapter_ids = json!(["chapter-3", "chapter-4"]);

        let command_state = ResumeBatchGenerationCommandState::from_task(&task);
        let workflow_launch = build_test_batch_generation_resume_workflow_launch(
            command_state,
            "user-1",
            vec!["chapter-3".to_string(), "chapter-4".to_string()],
            false,
        );
        let persistence_plan = workflow_launch.persistence_plan();

        assert_eq!(persistence_plan.response_payload()["status"], "pending");
        assert_eq!(
            persistence_plan.response_payload()["message"],
            "Task resumed and queued"
        );
        assert_eq!(
            persistence_plan.response_payload()["checkpoint"]["phase"],
            "pending"
        );
    }

    #[test]
    fn should_keep_batch_generation_write_workflow_request_contract_transport_free() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 3,
            count: 2,
            style_id: Some(7),
            target_word_count: Some(2800),
            enable_analysis: false,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            max_retries: 3,
            model_override: Some("gpt-4.1".to_string()),
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

        assert_eq!(request.start_chapter_number, 3);
        assert_eq!(request.count, 2);
        assert_eq!(request.style_id, Some(7));
        assert_eq!(request.target_word_count, Some(2800));
        assert!(!request.enable_analysis);
        assert_eq!(request.enable_mcp, None);
        assert_eq!(request.max_retries, 3);
        assert_eq!(request.model_override.as_deref(), Some("gpt-4.1"));
    }

    #[test]
    fn should_estimate_batch_generation_task_minutes_with_minimum_floor() {
        assert_eq!(super::estimated_task_minutes(0, 3000, false), 1);
        assert_eq!(super::estimated_task_minutes(1, 3000, false), 2);
        assert_eq!(super::estimated_task_minutes(1, 6000, false), 4);
        assert_eq!(super::estimated_task_minutes(3, 3000, true), 9);
        assert_eq!(super::estimated_task_minutes(2, 2800, true), 5);
    }

    #[test]
    fn should_build_batch_generation_create_response_payload() {
        let chapters = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let queued_runtime_state =
            super::BatchGenerationQueuedSnapshotPlan::from_runtime_state_seed(
                2,
                Some(json!({
                    "quality_metrics_summary": {
                        "chapter_count": 2,
                        "overall_score": 86.0,
                        "quality_runtime_context": {
                            "recent_metrics": [
                                {"overall_score": 86}
                            ],
                            "history_scope": "batch"
                        }
                    },
                    "quality_metrics_summary_state": {
                        "scope": "batch",
                        "chapter_count": 2,
                        "first_overall_score": 82.0,
                        "last_overall_score": 86.0
                    },
                    "quality_metrics_history": [
                        {"overall_score": 82},
                        {"overall_score": 86}
                    ],
                    "latest_quality_metrics": {
                        "overall_score": 86,
                        "quality_gate": {
                            "decision": "repair"
                        }
                    },
                    "quality_history_context": {
                        "scope": "batch",
                        "source": "create_response_test"
                    },
                    "active_story_repair_payload": {
                        "summary": "沿用批量修复建议",
                        "repair_targets": ["压缩说明"],
                        "source": "recent_history_summary",
                        "scope": "batch"
                    }
                })),
            );
        let payload = super::batch_generation_create_response_payload(
            "task-1",
            "project-1",
            &chapters,
            3000,
            false,
            &queued_runtime_state,
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["task_type"], "chapters_batch_generate");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["stage_code"], "6.writing.pending");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["checkpoint"]["last_event"], "queued");
        assert_eq!(payload["checkpoint"]["phase"], "pending");
        assert_eq!(payload["checkpoint"]["total"], 2);
        assert_eq!(payload["message"], "已创建批量生成任务，共 2 章");
        assert_eq!(payload["chapters_to_generate"][0]["id"], "chapter-1");
        assert_eq!(payload["chapters_to_generate"][1]["title"], "Second");
        assert_eq!(payload["estimated_time_minutes"], 4);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 86);
        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_history_context"]["source"],
            "create_response_test"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "沿用批量修复建议"
        );
    }

    #[test]
    fn should_build_batch_generation_create_launch_persistence_plan_from_create_parts() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            style_id: Some(9),
            target_word_count: Some(2800),
            enable_analysis: true,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            max_retries: 5,
            model_override: Some("gpt-4.1".to_string()),
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
        let normalized_target_word_count = 2800;
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default(),
            Some("gpt-4.1".to_string()),
        );
        let runtime_state_payload = json!({
            "batch_request_runtime_state": request_runtime_state.clone(),
            "quality_metrics_summary": {
                "chapter_count": 2,
                "overall_score": 86.0,
                "quality_runtime_context": {
                    "recent_metrics": [{"overall_score": 86}],
                    "history_scope": "batch"
                }
            },
            "quality_metrics_summary_state": {
                "scope": "batch",
                "chapter_count": 2,
                "first_overall_score": 82.0,
                "last_overall_score": 86.0
            },
            "quality_metrics_history": [
                {"overall_score": 82},
                {"overall_score": 86}
            ],
            "latest_quality_metrics": {
                "overall_score": 86,
                "quality_gate": {
                    "decision": "repair"
                }
            },
            "quality_history_context": {
                "scope": "batch",
                "source": "plan_response"
            },
            "active_story_repair_payload": {
                "summary": "沿用批量修复建议",
                "repair_targets": ["压缩说明"],
                "source": "recent_history_summary",
                "scope": "batch"
            }
        });
        let runtime_seed =
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(runtime_state_payload);
        let plan = BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            "task-1".to_string(),
            "project-1".to_string(),
            build_test_batch_generation_create_workflow_launch(
                request.task_spec(),
                normalized_target_word_count,
                chapters_to_generate,
                "user-1",
                runtime_seed,
            ),
        );
        let response_payload = plan.response_payload();

        assert_eq!(response_payload["batch_id"], "task-1");
        assert_eq!(response_payload["message"], "已创建批量生成任务，共 2 章");
        assert_eq!(response_payload["project_id"], "project-1");
        assert_eq!(response_payload["task_type"], "chapters_batch_generate");
        assert_eq!(response_payload["status"], "pending");
        assert_eq!(response_payload["checkpoint"]["last_event"], "queued");
        assert_eq!(response_payload["checkpoint"]["total"], 2);
        assert_eq!(
            response_payload["latest_quality_metrics"]["overall_score"],
            86
        );
        assert_eq!(
            response_payload["quality_metrics_summary_state"]["chapter_count"],
            2
        );
        assert_eq!(
            response_payload["quality_history_context"]["source"],
            "plan_response"
        );
        assert_eq!(
            response_payload["active_story_repair_payload"]["summary"],
            "沿用批量修复建议"
        );
        assert_eq!(
            response_payload["chapters_to_generate"][0]["id"],
            "chapter-1"
        );
        assert_eq!(plan.runtime_input.user_id, "user-1");
        assert_eq!(
            plan.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(plan.runtime_input.target_word_count, 2800);
        assert_eq!(
            plan.runtime_input.ai_config.provider,
            crate::ai::AIConfig::default().provider
        );
    }

    #[test]
    fn should_build_batch_generation_create_launch_task_from_create_parts() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 1,
            count: 2,
            style_id: Some(9),
            target_word_count: Some(2800),
            enable_analysis: true,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            max_retries: 5,
            model_override: Some("gpt-4.1".to_string()),
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
        let now = NaiveDate::from_ymd_opt(2026, 5, 28)
            .expect("valid date")
            .and_hms_opt(22, 20, 0)
            .expect("valid time");
        let normalized_target_word_count = 2800;
        let runtime_seed = BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
            json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
        );
        let plan = BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            "task-1".to_string(),
            "project-1".to_string(),
            build_test_batch_generation_create_workflow_launch(
                request.task_spec(),
                normalized_target_word_count,
                chapters_to_generate,
                "user-1",
                runtime_seed,
            ),
        );
        let response_payload = plan.response_payload();
        let task = plan.background_task_active_model(now);

        assert_eq!(plan.task_seed.total_chapters, 2);
        assert_eq!(response_payload["batch_id"], "task-1");
        assert_eq!(response_payload["message"], "已创建批量生成任务，共 2 章");
        assert_eq!(task.id, sea_orm::Set("task-1".to_string()));
        assert_eq!(task.total_chapters, sea_orm::Set(2));
        assert_eq!(
            plan.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(normalized_target_word_count, 2800);
    }

    #[test]
    fn should_keep_batch_generation_create_task_spec_owner_contract_explicit() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 9,
            count: 2,
            style_id: Some(4),
            target_word_count: Some(3200),
            enable_analysis: true,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            max_retries: 4,
            model_override: Some("gpt-4.1".to_string()),
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
        let task_spec = request.task_spec();

        assert_eq!(
            task_spec,
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 9,
                style_id: Some(4),
                enable_analysis: true,
                max_retries: 4,
            }
        );
    }

    #[test]
    fn should_keep_batch_generation_create_persistence_plan_owner_contract_explicit() {
        let request = BatchGenerationCreateWorkflowRequest {
            start_chapter_number: 9,
            count: 2,
            style_id: Some(4),
            target_word_count: Some(3200),
            enable_analysis: true,
            enable_mcp: None,
            enable_web_research: None,
            web_research_query: None,
            max_retries: 4,
            model_override: Some("gpt-4.1".to_string()),
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
        let persistence_plan = BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            "task-9".to_string(),
            "project-9".to_string(),
            build_test_batch_generation_create_workflow_launch(
                request.task_spec(),
                3200,
                vec![
                    build_chapter_target("chapter-9", 9, "Ninth"),
                    build_chapter_target("chapter-10", 10, "Tenth"),
                ],
                "user-9",
                BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                    json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
                ),
            ),
        );

        assert_eq!(persistence_plan.task_seed.id, "task-9");
        assert_eq!(persistence_plan.task_seed.project_id, "project-9");
        assert_eq!(persistence_plan.task_seed.total_chapters, 2);
        assert_eq!(persistence_plan.runtime_input.target_word_count, 3200);
        assert_eq!(
            persistence_plan.task_seed.chapter_ids,
            json!(["chapter-9", "chapter-10"])
        );
        assert_eq!(persistence_plan.task_seed.start_chapter_number, 9);
        assert_eq!(persistence_plan.task_seed.style_id, Some(4));
        assert!(persistence_plan.task_seed.enable_analysis);
        assert_eq!(persistence_plan.task_seed.max_retries, 4);
    }

    #[test]
    fn should_keep_batch_generation_create_persistence_plan_contract_from_create_launch_owner() {
        let persistence_plan = build_test_batch_generation_create_workflow_entry(
            "task-11",
            "project-11",
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 11,
                style_id: Some(6),
                enable_analysis: true,
                max_retries: 4,
            },
            3100,
            vec![
                build_chapter_target("chapter-11", 11, "Eleventh"),
                build_chapter_target("chapter-12", 12, "Twelfth"),
            ],
            "user-11",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
            ),
        );

        assert_eq!(persistence_plan.task_seed.id, "task-11");
        assert_eq!(persistence_plan.task_seed.project_id, "project-11");
        assert_eq!(persistence_plan.task_seed.start_chapter_number, 11);
        assert_eq!(persistence_plan.task_seed.total_chapters, 2);
        assert_eq!(persistence_plan.runtime_input.user_id, "user-11");
        assert_eq!(persistence_plan.runtime_input.target_word_count, 3100);
        assert_eq!(
            persistence_plan.runtime_input.chapter_ids,
            vec!["chapter-11".to_string(), "chapter-12".to_string()]
        );
    }

    #[test]
    fn should_keep_batch_generation_create_persistence_plan_payload_owner_contract() {
        let persistence_plan = build_test_batch_generation_create_workflow_entry(
            "task-21",
            "project-21",
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 21,
                style_id: Some(8),
                enable_analysis: false,
                max_retries: 3,
            },
            2800,
            vec![
                build_chapter_target("chapter-21", 21, "Twenty-first"),
                build_chapter_target("chapter-22", 22, "Twenty-second"),
            ],
            "user-21",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
                "batch_request_runtime_state": {"model_override": "gpt-4.1"},
                "quality_metrics_summary": {"overall_score": 88}
            })),
        );

        assert_eq!(persistence_plan.response_payload()["batch_id"], "task-21");
        assert_eq!(
            persistence_plan.response_payload()["message"],
            "已创建批量生成任务，共 2 章"
        );
        assert_eq!(
            persistence_plan.response_payload()["quality_metrics_summary"]["overall_score"],
            88
        );
        assert_eq!(
            persistence_plan.task_seed.chapter_ids,
            json!(["chapter-21", "chapter-22"])
        );
    }

    #[test]
    fn should_keep_batch_generation_create_persistence_plan_start_owner_contract() {
        let persistence_plan = build_test_batch_generation_create_workflow_entry(
            "task-31",
            "project-31",
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 31,
                style_id: Some(5),
                enable_analysis: true,
                max_retries: 4,
            },
            3600,
            vec![
                build_chapter_target("chapter-31", 31, "Thirty-first"),
                build_chapter_target("chapter-32", 32, "Thirty-second"),
            ],
            "user-31",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
                "batch_request_runtime_state": {"model_override": "gpt-4.1"},
                "quality_metrics_summary": {"chapter_count": 2}
            })),
        );

        assert_eq!(persistence_plan.task_seed.id, "task-31");
        assert_eq!(persistence_plan.task_seed.project_id, "project-31");
        assert_eq!(persistence_plan.runtime_input.user_id, "user-31");
        assert_eq!(persistence_plan.runtime_input.target_word_count, 3600);
        assert_eq!(
            persistence_plan.response_payload()["quality_metrics_summary"]["chapter_count"],
            2
        );
    }

    #[test]
    fn should_keep_batch_generation_resume_workflow_launch_start_owner_contract() {
        let mut task = build_resume_task("failed");
        task.user_id = "user-41".to_string();
        task.target_word_count = 2900;
        task.chapter_ids = json!(["chapter-41", "chapter-42"]);

        let workflow_launch = build_test_batch_generation_resume_workflow_launch(
            ResumeBatchGenerationCommandState::from_task(&task),
            "user-41",
            vec!["chapter-41".to_string(), "chapter-42".to_string()],
            true,
        );

        let persistence_plan = workflow_launch.persistence_plan();

        match persistence_plan.dispatch_plan() {
            ResumeExecutionDispatchPlan::Batch { runtime_input } => {
                assert_eq!(runtime_input.user_id, "user-41");
                assert_eq!(
                    runtime_input.chapter_ids,
                    vec!["chapter-41".to_string(), "chapter-42".to_string()]
                );
                assert_eq!(runtime_input.target_word_count, 2900);
                assert!(runtime_input.compat_options.enable_analysis);
            }
            ResumeExecutionDispatchPlan::SingleChapter { .. } => {
                panic!("expected batch dispatch plan");
            }
        }

        assert_eq!(persistence_plan.response_payload()["batch_id"], "task-1");
        assert_eq!(persistence_plan.response_payload()["status"], "pending");
    }

    #[test]
    fn should_keep_batch_generation_cancel_workflow_launch_start_owner_contract() {
        let task = batch_generation_task::Model {
            id: "task-51".to_string(),
            project_id: "project-51".to_string(),
            user_id: "user-51".to_string(),
            start_chapter_number: 1,
            chapter_count: 2,
            chapter_ids: json!(["chapter-51", "chapter-52"]),
            style_id: None,
            target_word_count: 3000,
            enable_analysis: true,
            status: "running".to_string(),
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-52".to_string()),
            current_chapter_number: Some(2),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };
        let snapshot = batch_generation_snapshot::Model {
            id: "snapshot-51".to_string(),
            batch_task_id: "task-51".to_string(),
            latest_quality_metrics: None,
            quality_metrics_history: None,
            quality_metrics_summary: None,
            workflow_runtime_state: Some(json!({
                "progress": 55,
                "phase": "generating",
                "status": "running"
            })),
            created_at: None,
            updated_at: None,
        };

        let workflow_launch = PreparedBatchGenerationCancelWorkflowLaunch::from_persistence_plan(
            BatchGenerationCancelledPersistencePlan::from_sources(&task, Some(&snapshot)),
        );

        let persistence_plan = workflow_launch.persistence_plan();
        let payload = persistence_plan.response_payload_for_test(batch_generation_task::Model {
            status: "cancelled".to_string(),
            ..task
        });

        assert_eq!(payload["batch_id"], "task-51");
        assert_eq!(payload["status"], "cancelled");
        assert_eq!(payload["message"], "Batch generation cancelled");
        assert_eq!(payload["checkpoint"]["phase"], "cancelled");
    }

    #[test]
    fn should_keep_batch_generation_create_runtime_seed_contract() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("沿用历史修复建议".to_string()),
                story_repair_targets: vec!["压缩说明".to_string()],
                ..crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default()
            },
            Some("gpt-4.1".to_string()),
        );
        let runtime_state_payload = json!({
            "batch_request_runtime_state": request_runtime_state.clone(),
            "active_story_repair_payload": {
                "summary": "沿用历史修复建议",
                "repair_targets": ["压缩说明"],
                "scope": "batch"
            },
            "quality_metrics_summary": {
                "overall_score": 84
            }
        });

        let runtime_seed =
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(runtime_state_payload);
        let (runtime_state_payload, resolved_compat_options) = runtime_seed.into_parts();

        assert_eq!(
            runtime_state_payload["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            runtime_state_payload["quality_metrics_summary"]["overall_score"],
            84
        );
        assert_eq!(
            resolved_compat_options.story_repair_summary(),
            "沿用历史修复建议"
        );
        assert_eq!(
            resolved_compat_options.story_repair_targets(),
            &["压缩说明".to_string()]
        );
    }

    #[test]
    fn should_build_batch_generation_create_workflow_runtime_parts_from_runtime_seed() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("沿用历史修复建议".to_string()),
                story_repair_targets: vec!["压缩说明".to_string()],
                ..crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default()
            },
            Some("gpt-4.1".to_string()),
        );
        let runtime_seed = BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
            "batch_request_runtime_state": request_runtime_state,
            "active_story_repair_payload": {
                "summary": "沿用历史修复建议",
                "repair_targets": ["压缩说明"],
                "scope": "batch"
            },
            "quality_metrics_summary": {
                "overall_score": 84
            }
        }));

        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 1,
                style_id: Some(9),
                enable_analysis: true,
                max_retries: 5,
            },
            2800,
            chapters_to_generate,
            "user-1",
            runtime_seed,
        );

        assert_eq!(
            workflow_launch.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(workflow_launch.runtime_input.user_id, "user-1");
        assert_eq!(workflow_launch.runtime_input.target_word_count, 2800);
        assert_eq!(
            workflow_launch
                .runtime_input
                .compat_options
                .story_repair_summary(),
            "沿用历史修复建议"
        );
        assert_eq!(
            workflow_launch.startup_snapshot_plan.runtime_state()["quality_metrics_summary"]
                ["overall_score"],
            84
        );
    }

    #[test]
    fn should_materialize_batch_generation_create_workflow_launch_parts_inside_runtime_seed_owner()
    {
        let runtime_seed = BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
            "batch_request_runtime_state": {
                "compat_options": {
                    "enable_analysis": true,
                    "story_repair_summary": "沿用历史修复建议",
                    "story_repair_targets": ["压缩说明"]
                },
                "model_override": "gpt-4.1"
            },
            "active_story_repair_payload": {
                "summary": "沿用历史修复建议",
                "repair_targets": ["压缩说明"],
                "scope": "batch"
            },
            "quality_metrics_summary": {
                "overall_score": 84
            }
        }));

        let (startup_snapshot_plan, runtime_input) = runtime_seed.into_workflow_launch_parts(
            "user-1".to_string(),
            vec!["chapter-1".to_string(), "chapter-2".to_string()],
            2,
            2800,
            build_test_generation_execution_config(),
        );

        assert_eq!(
            startup_snapshot_plan.runtime_state()["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["overall_score"],
            84
        );
        assert_eq!(runtime_input.user_id, "user-1");
        assert_eq!(runtime_input.target_word_count, 2800);
        assert_eq!(
            runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(
            runtime_input.compat_options.story_repair_summary(),
            "沿用历史修复建议"
        );
        assert_eq!(
            runtime_input.compat_options.story_repair_targets(),
            &["压缩说明".to_string()]
        );
    }

    #[test]
    fn should_build_batch_generation_create_workflow_launch_into_persistence_plan() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 1,
                style_id: Some(9),
                enable_analysis: true,
                max_retries: 5,
            },
            2800,
            chapters_to_generate.clone(),
            "user-1",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
            ),
        );

        let plan =
            workflow_launch.into_persistence_plan("task-1".to_string(), "project-1".to_string());

        assert_eq!(plan.task_seed.id, "task-1");
        assert_eq!(plan.task_seed.project_id, "project-1");
        assert_eq!(plan.runtime_input.user_id, "user-1");
        assert_eq!(plan.task_seed.start_chapter_number, 1);
        assert_eq!(plan.task_seed.total_chapters, 2);
        assert_eq!(plan.runtime_input.target_word_count, 2800);
        assert_eq!(
            plan.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
    }

    #[test]
    fn should_materialize_batch_generation_create_persistence_parts_inside_workflow_launch_owner() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 1,
                style_id: Some(9),
                enable_analysis: true,
                max_retries: 5,
            },
            2800,
            chapters_to_generate.clone(),
            "user-1",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
                "batch_request_runtime_state": {
                    "model_override": "gpt-4.1"
                },
                "quality_metrics_summary": {
                    "overall_score": 86
                }
            })),
        );

        let persistence_parts =
            workflow_launch.into_persistence_parts("task-1".to_string(), "project-1".to_string());

        assert_eq!(persistence_parts.task_seed.id, "task-1");
        assert_eq!(persistence_parts.task_seed.project_id, "project-1");
        assert_eq!(persistence_parts.task_seed.start_chapter_number, 1);
        assert_eq!(persistence_parts.task_seed.total_chapters, 2);
        assert_eq!(persistence_parts.response_payload["batch_id"], "task-1");
        assert_eq!(
            persistence_parts.response_payload["message"],
            "已创建批量生成任务，共 2 章"
        );
        assert_eq!(
            persistence_parts.response_payload["quality_metrics_summary"]["overall_score"],
            86
        );
        assert_eq!(persistence_parts.runtime_input.user_id, "user-1");
        assert_eq!(persistence_parts.runtime_input.target_word_count, 2800);
        assert_eq!(
            persistence_parts.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(
            persistence_parts.startup_snapshot_plan.runtime_state()["quality_metrics_summary"]
                ["overall_score"],
            86
        );
    }

    #[test]
    fn should_materialize_batch_generation_create_task_seed_inside_workflow_launch_owner() {
        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 3,
                style_id: Some(4),
                enable_analysis: true,
                max_retries: 4,
            },
            3200,
            vec![
                build_chapter_target("chapter-3", 3, "Third"),
                build_chapter_target("chapter-4", 4, "Fourth"),
            ],
            "user-3",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
            ),
        );

        let persistence_parts =
            workflow_launch.into_persistence_parts("task-3".to_string(), "project-3".to_string());

        assert_eq!(
            persistence_parts.task_seed,
            BatchGenerationTaskPersistenceSeed {
                id: "task-3".to_string(),
                project_id: "project-3".to_string(),
                user_id: "user-3".to_string(),
                start_chapter_number: 3,
                chapter_count: 2,
                chapter_ids: json!(["chapter-3", "chapter-4"]),
                style_id: Some(4),
                target_word_count: 3200,
                enable_analysis: true,
                total_chapters: 2,
                current_chapter_id: None,
                current_chapter_number: None,
                max_retries: 4,
            }
        );
    }

    #[test]
    fn should_keep_batch_generation_create_workflow_launch_owner_contract_explicit() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 1,
                style_id: Some(9),
                enable_analysis: true,
                max_retries: 5,
            },
            2800,
            chapters_to_generate.clone(),
            "user-1",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
            ),
        );

        assert_eq!(workflow_launch.task_spec.start_chapter_number, 1);
        assert_eq!(workflow_launch.task_spec.style_id, Some(9));
        assert!(workflow_launch.task_spec.enable_analysis);
        assert_eq!(workflow_launch.task_spec.max_retries, 5);
        assert_eq!(
            workflow_launch.runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(workflow_launch.runtime_input.target_word_count, 2800);
        assert_eq!(workflow_launch.runtime_input.user_id, "user-1");
        assert_eq!(workflow_launch.chapters_to_generate.len(), 2);
    }

    #[test]
    fn should_keep_batch_generation_create_workflow_launch_runtime_seed_owner_contract() {
        let workflow_launch = super::PreparedBatchGenerationCreateWorkflowLaunch::from_runtime_seed(
            BatchGenerationCreateTaskSpec {
                start_chapter_number: 3,
                style_id: Some(4),
                enable_analysis: true,
                max_retries: 4,
            },
            3200,
            vec![
                build_chapter_target("chapter-3", 3, "Third"),
                build_chapter_target("chapter-4", 4, "Fourth"),
            ],
            "user-3",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
                "batch_request_runtime_state": {
                    "model_override": "gpt-4.1"
                },
                "quality_metrics_summary": {
                    "overall_score": 86
                }
            })),
            build_test_generation_execution_config(),
        );

        let super::PreparedBatchGenerationCreateWorkflowLaunch {
            task_spec,
            chapters_to_generate,
            startup_snapshot_plan,
            runtime_input,
        } = workflow_launch;

        assert_eq!(task_spec.start_chapter_number, 3);
        assert_eq!(task_spec.style_id, Some(4));
        assert!(task_spec.enable_analysis);
        assert_eq!(task_spec.max_retries, 4);
        assert_eq!(chapters_to_generate.len(), 2);
        assert_eq!(
            startup_snapshot_plan.runtime_state()["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["overall_score"],
            86
        );
        assert_eq!(runtime_input.user_id, "user-3");
        assert_eq!(runtime_input.target_word_count, 3200);
        assert_eq!(
            runtime_input.chapter_ids,
            vec!["chapter-3".to_string(), "chapter-4".to_string()]
        );
    }

    #[test]
    fn should_keep_explicit_batch_generation_create_style_over_default_style() {
        let task_spec = BatchGenerationCreateTaskSpec {
            start_chapter_number: 1,
            style_id: Some(9),
            enable_analysis: false,
            max_retries: 3,
        };
        let effective_style_id =
            super::select_batch_generation_create_effective_style_id(task_spec.style_id, Some(12));
        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            task_spec.with_effective_style_id(effective_style_id),
            2800,
            vec![build_chapter_target("chapter-1", 1, "First")],
            "user-1",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                json!({"batch_request_runtime_state": {}}),
            ),
        );

        assert_eq!(workflow_launch.task_spec.style_id, Some(9));
    }

    #[test]
    fn should_apply_project_default_style_to_batch_generation_create_workflow_launch() {
        let task_spec = BatchGenerationCreateTaskSpec {
            start_chapter_number: 1,
            style_id: None,
            enable_analysis: false,
            max_retries: 3,
        };
        let workflow_launch = build_test_batch_generation_create_workflow_launch(
            task_spec.with_effective_style_id(Some(12)),
            2800,
            vec![build_chapter_target("chapter-1", 1, "First")],
            "user-1",
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                json!({"batch_request_runtime_state": {}}),
            ),
        );

        assert_eq!(workflow_launch.task_spec.style_id, Some(12));
    }

    #[test]
    fn should_build_batch_generation_create_persistence_plan_task_and_response_payload() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let now = NaiveDate::from_ymd_opt(2026, 5, 31)
            .expect("valid date")
            .and_hms_opt(21, 40, 0)
            .expect("valid time");
        let plan = BatchGenerationCreateLaunchPersistencePlan::from_workflow_launch(
            "task-1".to_string(),
            "project-1".to_string(),
            build_test_batch_generation_create_workflow_launch(
                BatchGenerationCreateTaskSpec {
                    start_chapter_number: 1,
                    style_id: Some(9),
                    enable_analysis: true,
                    max_retries: 5,
                },
                2800,
                chapters_to_generate.clone(),
                "user-1",
                BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(
                    json!({"batch_request_runtime_state": {"model_override": "gpt-4.1"}}),
                ),
            ),
        );
        let response_payload = plan.response_payload();
        let task = plan.background_task_active_model(now);

        assert_eq!(plan.task_seed.id, "task-1");
        assert_eq!(response_payload["batch_id"], "task-1");
        assert_eq!(response_payload["estimated_time_minutes"], 5);
        assert_eq!(task.id, sea_orm::Set("task-1".to_string()));
        assert_eq!(plan.runtime_input.user_id, "user-1");
        assert_eq!(plan.runtime_input.target_word_count, 2800);
    }

    #[test]
    fn should_build_batch_generation_create_startup_runtime_state_from_recent_history_summary() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let recent_history_summary = json!({
            "repair_guidance": {
                "summary": "沿用最近三章修复建议",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["尾章钩子"],
                "focus_areas": ["pacing"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复"
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 83}]
            },
            "overall_score": 83
        });

        let startup_runtime_state =
            BatchGenerationCreateStartupRuntimeState::from_recent_history_summary(
                request_runtime_state.clone(),
                Some(recent_history_summary),
            );

        assert_eq!(
            startup_runtime_state.seed_source(),
            BatchGenerationCreateStartupSeedSource::RecentHistorySummary
        );
        assert_eq!(
            startup_runtime_state.request_runtime_state(),
            &request_runtime_state
        );
        assert_eq!(
            startup_runtime_state.runtime_state_payload()["active_story_repair_payload"]["summary"],
            "沿用最近三章修复建议"
        );
        assert_eq!(
            startup_runtime_state.runtime_state_payload()["quality_metrics_summary"]
                ["overall_score"],
            83.0
        );
    }

    #[test]
    fn should_build_batch_generation_create_startup_runtime_state_from_request_only() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("保留手工修复目标".to_string()),
                story_repair_targets: vec!["补强动机".to_string()],
                ..crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default()
            },
            Some("gpt-4.1".to_string()),
        );

        let startup_runtime_state =
            BatchGenerationCreateStartupRuntimeState::from_recent_history_summary(
                request_runtime_state.clone(),
                None,
            );

        assert_eq!(
            startup_runtime_state.seed_source(),
            BatchGenerationCreateStartupSeedSource::RequestOnly
        );
        assert_eq!(
            startup_runtime_state.request_runtime_state(),
            &request_runtime_state
        );
        assert_eq!(
            startup_runtime_state.runtime_state_payload()["active_story_repair_payload"]["summary"],
            "保留手工修复目标"
        );
        assert_eq!(
            startup_runtime_state.runtime_state_payload()["batch_request_runtime_state"]
                ["model_override"],
            "gpt-4.1"
        );
        assert!(startup_runtime_state.runtime_state_payload()["quality_metrics_summary"].is_null());
    }

    #[test]
    fn should_build_batch_generation_create_runtime_seed_inside_startup_owner() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("沿用最近三章修复建议".to_string()),
                story_repair_targets: vec!["压缩说明".to_string()],
                ..crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default()
            },
            Some("gpt-4.1".to_string()),
        );
        let startup_runtime_state =
            BatchGenerationCreateStartupRuntimeState::from_recent_history_summary(
                request_runtime_state.clone(),
                Some(json!({
                    "repair_guidance": {
                        "summary": "沿用最近三章修复建议",
                        "repair_targets": ["压缩说明"],
                        "preserve_strengths": ["尾章钩子"],
                        "focus_areas": ["pacing"]
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "repair",
                        "label": "需修复"
                    },
                    "overall_score": 83
                })),
            );

        let runtime_seed = startup_runtime_state.into_runtime_seed();
        let (runtime_state_payload, resolved_compat_options) = runtime_seed.into_parts();
        assert_eq!(
            runtime_state_payload["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            resolved_compat_options.story_repair_summary(),
            "沿用最近三章修复建议"
        );
        assert_eq!(
            resolved_compat_options.story_repair_targets(),
            &["压缩说明".to_string()]
        );
    }

    #[test]
    fn should_keep_batch_generation_create_runtime_seed_dispatch_ready_contract() {
        let runtime_seed = BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
            "batch_request_runtime_state": {
                "compat_options": {
                    "enable_analysis": true,
                    "enable_mcp": true,
                    "web_research_enabled": false,
                    "story_repair_summary": "沿用最近三章修复建议",
                    "story_repair_targets": ["压缩说明"],
                    "story_preserve_strengths": []
                },
                "model_override": "gpt-4.1"
            },
            "active_story_repair_payload": {
                "summary": "沿用最近三章修复建议",
                "repair_targets": ["压缩说明"],
                "scope": "batch"
            }
        }));
        let (runtime_state_payload, resolved_compat_options) = runtime_seed.into_parts();

        assert_eq!(
            runtime_state_payload["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            resolved_compat_options.story_repair_summary(),
            "沿用最近三章修复建议"
        );
        assert_eq!(
            resolved_compat_options.story_repair_targets(),
            &["压缩说明".to_string()]
        );
    }

    #[test]
    fn should_materialize_batch_generation_queued_snapshot_inside_runtime_seed_owner() {
        let runtime_seed = BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(json!({
            "batch_request_runtime_state": {
                "model_override": "gpt-4.1"
            },
            "quality_metrics_summary": {
                "overall_score": 86
            },
            "active_story_repair_payload": {
                "summary": "沿用最近三章修复建议",
                "repair_targets": ["压缩说明"],
                "scope": "batch"
            }
        }));

        let startup_snapshot_plan = runtime_seed.startup_snapshot_plan(2);

        assert_eq!(
            startup_snapshot_plan.runtime_state()["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            startup_snapshot_plan.runtime_state()["quality_metrics_summary"]["overall_score"],
            86
        );
        assert_eq!(
            startup_snapshot_plan.active_story_repair_payload(),
            Some(json!({
                "summary": "沿用最近三章修复建议",
                "repair_targets": ["压缩说明"],
                "scope": "batch"
            }))
        );
    }

    #[test]
    fn should_seed_manual_story_repair_payload_into_batch_runtime_state() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions {
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
                story_repair_summary: Some("中段节奏需要压缩".to_string()),
                story_repair_targets: vec!["提前冲突触发".to_string()],
                story_preserve_strengths: vec!["尾章钩子".to_string()],
            },
            Some("gpt-4.1".to_string()),
        );

        let payload = batch_generation_request_runtime_state_payload(&runtime_state);

        assert_eq!(
            payload["batch_request_runtime_state"]["model_override"],
            "gpt-4.1"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "中段节奏需要压缩"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["提前冲突触发"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["尾章钩子"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "manual_request"
        );
        assert_eq!(payload["active_story_repair_payload"]["scope"], "batch");
    }

    #[test]
    fn should_skip_empty_manual_story_repair_payload_in_batch_runtime_state() {
        let payload = batch_generation_request_runtime_state_payload(
            &BatchGenerationRequestRuntimeState::new(
                crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default(),
                None,
            ),
        );

        assert!(payload.get("active_story_repair_payload").is_none());
    }

    #[test]
    fn should_merge_manual_and_recent_history_story_repair_state_into_create_runtime_seed() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions {
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
                story_repair_summary: Some("手工摘要".to_string()),
                story_repair_targets: vec!["手工目标".to_string(), "共同目标".to_string()],
                story_preserve_strengths: vec!["手工优点".to_string()],
            },
            Some("gpt-4.1".to_string()),
        );

        let quality_summary = json!({
            "repair_guidance": {
                "summary": "历史摘要",
                "repair_targets": ["共同目标", "历史目标"],
                "preserve_strengths": ["历史优点"],
                "focus_areas": ["历史焦点"],
                "weakest_metric_key": "continuity",
                "weakest_metric_label": "Continuity",
                "weakest_metric_value": 0.62
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复",
                "summary": "近期质量波动",
                "failed_metrics": [{"label": "Continuity"}]
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 85}],
                "history_scope": "batch"
            },
            "overall_score": 85
        });

        let payload = build_batch_generation_runtime_state_payload_from_parts(
            &runtime_state,
            Some(&quality_summary),
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "手工摘要"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["手工目标", "共同目标", "历史目标"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["手工优点", "历史优点"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["focus_areas"],
            json!(["历史焦点"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "manual_plus_recent_history_summary"
        );
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 85.0);
        assert_eq!(
            payload["quality_history_context"],
            json!({
                "scope": "batch",
                "recent_metrics": [{
                    "history_index": 0,
                    "overall_score": 85,
                    "repair_guidance": {
                        "summary": "历史摘要",
                        "repair_targets": ["共同目标", "历史目标"],
                        "preserve_strengths": ["历史优点"],
                        "focus_areas": ["历史焦点"],
                        "weakest_metric_key": "continuity",
                        "weakest_metric_label": "Continuity",
                        "weakest_metric_value": 0.62
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "repair",
                        "label": "需修复",
                        "summary": "近期质量波动",
                        "failed_metrics": [{"label": "Continuity"}]
                    }
                }],
                "history_scope": "batch"
            })
        );
    }

    #[test]
    fn should_write_recent_history_quality_state_into_create_runtime_seed_without_manual_input() {
        let quality_summary = json!({
            "repair_guidance": {
                "summary": "历史摘要",
                "repair_targets": ["历史目标"],
                "preserve_strengths": ["历史优点"],
                "focus_areas": ["历史焦点"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "需修复",
                "summary": "近期质量波动",
                "failed_metrics": [{"label": "Continuity"}]
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 88}]
            },
            "overall_score": 88
        });

        let payload = build_batch_generation_runtime_state_payload_from_parts(
            &BatchGenerationRequestRuntimeState::new(
                crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default(),
                None,
            ),
            Some(&quality_summary),
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "历史摘要"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "recent_history_summary"
        );
        assert_eq!(
            payload["quality_history_context"],
            json!({
                "scope": "batch",
                "recent_metrics": [{
                    "history_index": 0,
                    "overall_score": 88,
                    "repair_guidance": {
                        "summary": "历史摘要",
                        "repair_targets": ["历史目标"],
                        "preserve_strengths": ["历史优点"],
                        "focus_areas": ["历史焦点"]
                    },
                    "quality_gate": {
                        "status": "warning",
                        "decision": "repair",
                        "label": "需修复",
                        "summary": "近期质量波动",
                        "failed_metrics": [{"label": "Continuity"}]
                    }
                }]
            })
        );
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 88.0);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 88);
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 88);
        assert_eq!(payload["quality_metrics_summary_state"]["scope"], "batch");
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 1);
    }

    #[test]
    fn should_seed_batch_runtime_state_with_latest_quality_metrics_context() {
        let request_runtime_state = BatchGenerationRequestRuntimeState::default();
        let quality_summary = json!({
            "repair_guidance": {
                "summary": "历史摘要",
                "repair_targets": ["历史目标"],
                "preserve_strengths": ["历史优点"],
                "focus_areas": ["历史焦点"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "repair",
                "label": "历史门禁"
            },
            "quality_runtime_context": {
                "recent_metrics": [{"overall_score": 88}]
            },
            "overall_score": 88
        });
        let latest_quality_metrics = json!({
            "repair_guidance": {
                "summary": "最新摘要",
                "repair_targets": ["最新目标"],
                "preserve_strengths": ["最新优点"],
                "focus_areas": ["最新焦点"]
            },
            "quality_gate": {
                "status": "warning",
                "decision": "auto_repair",
                "label": "最新门禁",
                "summary": "继续修复"
            },
            "overall_score": 81
        });

        let payload = build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload(
            &request_runtime_state,
            None,
            Some(&quality_summary),
            Some(&latest_quality_metrics),
        );

        assert_eq!(
            payload["active_story_repair_payload"]["summary"],
            "最新摘要"
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["最新目标", "历史目标"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["preserve_strengths"],
            json!(["最新优点", "历史优点"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["quality_gate_label"],
            "最新门禁"
        );
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 81);
    }

    #[test]
    fn should_aggregate_recent_history_quality_summaries_before_seeding_batch_runtime_state() {
        let first_summary = json!({
            "overall_score": 86,
            "repair_guidance": {
                "summary": "先处理节奏拖沓",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["尾章钩子"],
                "focus_areas": ["pacing", "conflict"]
            },
            "quality_gate": {
                "decision": "repair",
                "failed_metrics": [{"label": "Pacing"}]
            }
        });
        let second_summary = json!({
            "overall_score": 81,
            "repair_guidance": {
                "summary": "补角色动机",
                "repair_targets": ["强化动机", "提前冲突"],
                "preserve_strengths": ["人物口吻"],
                "focus_areas": ["character", "pacing"]
            },
            "quality_gate": {
                "decision": "manual_review",
                "failed_metrics": [{"label": "Character"}]
            }
        });
        let aggregated =
            aggregate_story_repair_quality_summaries(&[first_summary, second_summary], "batch")
                .expect("aggregated batch summary");

        let payload = build_batch_generation_runtime_state_payload_from_parts(
            &BatchGenerationRequestRuntimeState::new(
                crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default(),
                None,
            ),
            Some(&aggregated),
        );
        let runtime_seed =
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(payload.clone());
        let (_, compat) = runtime_seed.into_parts();

        assert_eq!(payload["quality_metrics_summary"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_metrics_summary"]["recent_focus_areas"],
            json!(["pacing", "conflict", "character"])
        );
        assert_eq!(
            payload["active_story_repair_payload"]["repair_targets"],
            json!(["压缩说明", "提前冲突", "强化动机"])
        );
        assert_eq!(
            payload["quality_history_context"]["recent_metrics"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(
            payload["quality_metrics_history"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(payload["quality_metrics_history"][0]["overall_score"], 81);
        assert_eq!(payload["quality_metrics_history"][1]["overall_score"], 86);
        assert_eq!(payload["latest_quality_metrics"]["overall_score"], 86);
        assert_eq!(payload["quality_metrics_summary_state"]["chapter_count"], 2);
        assert_eq!(
            payload["quality_metrics_summary_state"]["first_overall_score"],
            81.0
        );
        assert_eq!(
            payload["quality_metrics_summary_state"]["last_overall_score"],
            86.0
        );
        assert_eq!(compat.story_repair_summary(), "先处理节奏拖沓");
        assert_eq!(
            compat.story_repair_targets(),
            &[
                "压缩说明".to_string(),
                "提前冲突".to_string(),
                "强化动机".to_string()
            ]
        );
    }

    #[test]
    fn should_restore_batch_runtime_compat_options_from_seeded_story_repair_payload() {
        let runtime_state = BatchGenerationRequestRuntimeState::new(
            crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default(),
            None,
        );
        let quality_summary = json!({
            "repair_guidance": {
                "summary": "沿用批量历史修复建议",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["尾章钩子"]
            }
        });

        let payload = build_batch_generation_runtime_state_payload_from_parts(
            &runtime_state,
            Some(&quality_summary),
        );
        let runtime_seed =
            BatchGenerationCreateRuntimeSeed::from_runtime_state_payload(payload.clone());
        let (_, compat) = runtime_seed.into_parts();

        assert_eq!(compat.story_repair_summary(), "沿用批量历史修复建议");
        assert_eq!(
            compat.story_repair_targets(),
            &["压缩说明".to_string(), "提前冲突".to_string()]
        );
        assert_eq!(compat.story_preserve_strengths(), &["尾章钩子".to_string()]);
    }

    #[test]
    fn should_build_resume_batch_generation_execution_input_from_execution_owner() {
        let response_payload = serde_json::json!({
            "batch_id": "task-9",
            "message": "Task resumed and queued",
            "project_id": "project-1",
            "task_type": "chapters_batch_generate",
            "status": "pending",
            "stage_code": "6.writing.pending",
            "execution_mode": "interactive",
            "current_chapter_id": null,
            "created_at": null,
            "checkpoint": {
                "stage_code": "6.writing.pending",
                "execution_mode": "interactive"
            },
            "completed_chapters": 0,
            "total_chapters": 2
        });
        let execution_input = super::BatchGenerationExecutionInput {
            user_id: "user-1".to_string(),
            chapter_ids: vec!["chapter-1".to_string(), "chapter-2".to_string()],
            target_word_count: 2800,
            compat_options: crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default(),
            ai_config: crate::ai::AIConfig::default(),
        };

        assert_eq!(response_payload["batch_id"], "task-9");
        assert_eq!(response_payload["message"], "Task resumed and queued");
        assert_eq!(execution_input.user_id, "user-1");
        assert_eq!(
            execution_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(execution_input.target_word_count, 2800);
        assert_eq!(
            execution_input.ai_config.provider,
            crate::ai::AIConfig::default().provider
        );
    }

    #[test]
    fn should_keep_resume_batch_generation_owned_prepare_owner_contract_explicit() {
        let task = crate::models::batch_generation_task::Model {
            id: "task-5".to_string(),
            project_id: "project-5".to_string(),
            user_id: "user-5".to_string(),
            start_chapter_number: 5,
            chapter_count: 2,
            chapter_ids: json!(["chapter-5", "chapter-6"]),
            style_id: None,
            target_word_count: 3400,
            enable_analysis: true,
            status: "failed".to_string(),
            total_chapters: 2,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-5".to_string()),
            current_chapter_number: Some(5),
            current_retry_count: 1,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };
        let workflow_runtime_state = json!({
            "batch_request_runtime_state": {
                "compat_options": {
                    "enable_analysis": true,
                    "enable_mcp": true,
                    "web_research_enabled": false,
                    "story_repair_targets": [],
                    "story_preserve_strengths": []
                },
                "model_override": null
            }
        });
        let snapshot = crate::models::batch_generation_snapshot::Model {
            id: "snapshot-5".to_string(),
            batch_task_id: "task-5".to_string(),
            workflow_runtime_state: Some(workflow_runtime_state.clone()),
            latest_quality_metrics: None,
            quality_metrics_history: None,
            quality_metrics_summary: None,
            created_at: Some(Utc::now().naive_utc()),
            updated_at: Some(Utc::now().naive_utc()),
        };
        let command_state = ResumeBatchGenerationCommandState::from_task(&task);

        assert_eq!(command_state.batch_id, "task-5");
        assert_eq!(command_state.chapter_ids, json!(["chapter-5", "chapter-6"]));
        assert_eq!(
            snapshot
                .workflow_runtime_state
                .as_ref()
                .and_then(|state| state.get("batch_request_runtime_state"))
                .and_then(|state| state.get("model_override")),
            Some(&Value::Null)
        );
    }

    #[test]
    fn should_project_batch_generation_create_chapter_ids_in_order() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-5", 5, "Chapter 5"),
            build_chapter_target("chapter-6", 6, "Chapter 6"),
        ];
        let chapter_ids = chapters_to_generate
            .iter()
            .map(|target| target.id.clone())
            .collect::<Vec<_>>();

        assert_eq!(
            chapter_ids,
            vec!["chapter-5".to_string(), "chapter-6".to_string()]
        );
    }

    #[test]
    fn should_build_batch_generation_task_chapter_id_payload_from_create_parts() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-5", 5, "Chapter 5"),
            build_chapter_target("chapter-6", 6, "Chapter 6"),
        ];
        let chapter_id_payload = Value::Array(
            chapters_to_generate
                .iter()
                .map(|target| target.id.clone())
                .into_iter()
                .map(|chapter_id| json!(chapter_id))
                .collect(),
        );

        assert_eq!(chapter_id_payload, json!(["chapter-5", "chapter-6"]));
    }

    #[test]
    fn should_build_batch_generation_create_response_chapters_to_generate_payload() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-5", 5, "Chapter 5"),
            build_chapter_target("chapter-6", 6, "Chapter 6"),
        ];
        let payload = chapters_to_generate
            .iter()
            .map(|target| {
                json!({
                    "id": target.id,
                    "chapter_number": target.chapter_number,
                    "title": target.title,
                })
            })
            .collect::<Vec<_>>();

        assert_eq!(payload.len(), 2);
        assert_eq!(payload[0]["id"], "chapter-5");
        assert_eq!(payload[0]["chapter_number"], 5);
        assert_eq!(payload[1]["title"], "Chapter 6");
    }
}
