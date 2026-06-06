use chrono::NaiveDateTime;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::models::{batch_generation_task, chapter, generation_history};
use crate::services::chapter_analysis_read_context_service::load_chapter_analysis_read_context;
use crate::services::chapter_generation_execution_config_service::{
    prepare_generation_execution_config_with_provider_payload, PreparedGenerationExecutionConfig,
};
use crate::services::chapter_generation_prerequisite_service::check_chapter_generation_prerequisites;
use crate::services::chapter_generation_quality_runtime_context_service::{
    apply_generation_quality_runtime_context_to_payload,
    resolve_generation_quality_runtime_context_for_seed,
    resolve_generation_quality_runtime_context_from_persisted_sources,
};
use crate::services::chapter_generation_request_runtime_state_service::{
    active_story_repair_payload_from_runtime_state, BatchGenerationRequestRuntimeState,
};
use crate::services::chapter_generation_research_payload_service::build_single_chapter_research_provider_payload;
use crate::services::chapter_generation_target_word_count_service::normalize_chapter_generation_target_word_count;
use crate::services::chapter_quality_metrics_query_service::{
    build_chapter_analysis_quality_fragments, build_chapter_quality_metrics_fragments,
    ChapterQualityMetricsFragments,
};
use crate::services::chapter_single_generation_runtime_state_service::{
    build_single_generation_runtime_checkpoint_for_stage,
    dispatch_single_chapter_generation_runtime, SingleGenerationSnapshotStage,
};
use crate::services::chapter_single_generation_snapshot_service::SingleGenerationStartupSnapshotPlan;
use crate::services::chapter_single_generation_task_model_service::{
    build_single_generation_background_task_active_model,
    build_single_generation_background_task_persistence_seed, SingleGenerationTaskPersistenceSeed,
};
use crate::services::chapter_story_repair_quality_context_service::{
    aggregate_story_repair_quality_summaries,
    resolve_active_story_repair_payload_with_quality_fallback,
    restore_story_repair_compat_options_from_active_snapshot,
};
use crate::services::route_request_deserialize_service::deserialize_optional_non_null;
use crate::services::settings_service::SettingsService;

use super::chapter_generation_access_service::{
    load_accessible_chapter_for_generation, LoadAccessibleChapterForGenerationError,
};

const MIN_SINGLE_GENERATION_TARGET_WORD_COUNT: i32 = 500;
const MAX_SINGLE_GENERATION_TARGET_WORD_COUNT: i32 = 10_000;
const MAX_SINGLE_GENERATION_STORY_CREATION_BRIEF_LENGTH: usize = 1200;
const MAX_SINGLE_GENERATION_QUALITY_NOTES_LENGTH: usize = 600;
const SINGLE_GENERATION_CREATIVE_MODE_VALUES: &[&str] = &[
    "balanced",
    "hook",
    "emotion",
    "suspense",
    "relationship",
    "payoff",
];
const SINGLE_GENERATION_STORY_FOCUS_VALUES: &[&str] = &[
    "advance_plot",
    "deepen_character",
    "escalate_conflict",
    "reveal_mystery",
    "relationship_shift",
    "foreshadow_payoff",
];
const SINGLE_GENERATION_PLOT_STAGE_VALUES: &[&str] = &["development", "climax", "ending"];
const SINGLE_GENERATION_QUALITY_PRESET_VALUES: &[&str] = &[
    "balanced",
    "plot_drive",
    "immersive",
    "emotion_drama",
    "clean_prose",
];
const SINGLE_GENERATION_TASK_TYPE: &str = "chapter_single_generate";
const SINGLE_GENERATION_EXECUTION_MODE: &str = "interactive";
const SINGLE_GENERATION_ACTIVE_TASK_STATUSES: [&str; 2] = ["pending", "running"];

pub(crate) fn estimated_single_generation_task_minutes(
    target_word_count: i32,
    enable_analysis: bool,
) -> i32 {
    let generation_time = (target_word_count as f64 / 3000.0) * 2.0;
    let analysis_time = if enable_analysis { 1.0 } else { 0.0 };
    ((generation_time + analysis_time) as i32).max(1)
}

pub(crate) fn single_generation_pending_stage_code() -> &'static str {
    "6.writing.pending"
}

pub(crate) fn single_generation_active_task_statuses() -> [&'static str; 2] {
    SINGLE_GENERATION_ACTIVE_TASK_STATUSES
}

pub(crate) fn build_single_generation_runtime_payload_base(
    task_id: &str,
    project_id: &str,
    chapter_id: Option<&str>,
    status: &str,
    workflow_runtime_state: Option<&Value>,
    created_at: Option<NaiveDateTime>,
) -> Map<String, Value> {
    let stage_code = match status {
        "completed" => "6.writing.completed",
        "failed" => "6.writing.failed",
        "cancelled" => "6.writing.cancelled",
        "running" => "6.writing.generating",
        _ => single_generation_pending_stage_code(),
    };
    let mut checkpoint = workflow_runtime_state
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    checkpoint.insert("stage_code".to_string(), json!(stage_code));
    checkpoint.insert(
        "execution_mode".to_string(),
        json!(SINGLE_GENERATION_EXECUTION_MODE),
    );

    let mut payload = Map::new();
    payload.insert("batch_id".to_string(), json!(task_id));
    payload.insert("task_type".to_string(), json!(SINGLE_GENERATION_TASK_TYPE));
    payload.insert("project_id".to_string(), json!(project_id));
    payload.insert("status".to_string(), json!(status));
    payload.insert("stage_code".to_string(), json!(stage_code));
    payload.insert(
        "execution_mode".to_string(),
        json!(SINGLE_GENERATION_EXECUTION_MODE),
    );
    payload.insert(
        "current_chapter_id".to_string(),
        json!(chapter_id.map(str::to_string)),
    );
    payload.insert("checkpoint".to_string(), Value::Object(checkpoint));
    payload.insert(
        "created_at".to_string(),
        json!(created_at.map(|datetime| datetime.and_utc().to_rfc3339())),
    );

    payload
}

fn single_generation_task_chapter_id(task: &batch_generation_task::Model) -> Option<&str> {
    task.current_chapter_id.as_deref().or_else(|| {
        task.chapter_ids
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| {
                item.as_str()
                    .or_else(|| item.get("id").and_then(Value::as_str))
            })
    })
}

fn single_generation_to_iso(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.and_utc().to_rfc3339())
}

pub(crate) fn build_single_generation_task_view_payload_from_task_state(
    task: &batch_generation_task::Model,
    workflow_runtime_state: Option<&Value>,
) -> Map<String, Value> {
    let mut payload = build_single_generation_runtime_payload_base(
        &task.id,
        &task.project_id,
        single_generation_task_chapter_id(task),
        &task.status,
        workflow_runtime_state,
        task.created_at,
    );
    payload.insert("total".to_string(), json!(task.total_chapters));
    payload.insert("completed".to_string(), json!(task.completed_chapters));
    payload.insert(
        "current_chapter_number".to_string(),
        json!(task.current_chapter_number),
    );
    payload.insert(
        "started_at".to_string(),
        json!(single_generation_to_iso(task.started_at)),
    );
    payload.insert(
        "completed_at".to_string(),
        json!(single_generation_to_iso(task.completed_at)),
    );
    payload.insert("error_message".to_string(), json!(task.error_message));

    payload
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct SingleChapterGenerationRouteRequest {
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: Option<i32>,
    pub(crate) model: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub(crate) enable_analysis: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    pub(crate) enable_mcp: Option<bool>,
    #[serde(default)]
    pub(crate) enable_web_research: Option<bool>,
    pub(crate) web_research_query: Option<String>,
    pub(crate) narrative_perspective: Option<String>,
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

#[derive(Debug, Clone, Default)]
pub(crate) struct SingleChapterGenerationRequest {
    pub(crate) style_id: Option<i32>,
    pub(crate) target_word_count: Option<i32>,
    pub(crate) model: Option<String>,
    pub(crate) enable_analysis: Option<bool>,
    pub(crate) enable_mcp: Option<bool>,
    pub(crate) enable_web_research: Option<bool>,
    pub(crate) web_research_query: Option<String>,
    pub(crate) narrative_perspective: Option<String>,
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

impl SingleChapterGenerationRequest {
    fn from_route_request(route_request: SingleChapterGenerationRouteRequest) -> Self {
        Self {
            style_id: route_request.style_id,
            target_word_count: route_request.target_word_count,
            model: route_request.model,
            enable_analysis: route_request.enable_analysis,
            enable_mcp: route_request.enable_mcp,
            enable_web_research: route_request.enable_web_research,
            web_research_query: route_request.web_research_query,
            narrative_perspective: route_request.narrative_perspective,
            creative_mode: normalize_optional_single_generation_request_string(
                route_request.creative_mode,
            ),
            story_focus: normalize_optional_single_generation_request_string(
                route_request.story_focus,
            ),
            plot_stage: normalize_optional_single_generation_request_string(
                route_request.plot_stage,
            ),
            story_creation_brief: normalize_optional_single_generation_request_string(
                route_request.story_creation_brief,
            ),
            quality_preset: normalize_optional_single_generation_request_string(
                route_request.quality_preset,
            ),
            quality_notes: normalize_optional_single_generation_request_string(
                route_request.quality_notes,
            ),
            story_repair_summary: normalize_optional_single_generation_request_string(
                route_request.story_repair_summary,
            ),
            story_repair_targets: route_request.story_repair_targets,
            story_preserve_strengths: route_request.story_preserve_strengths,
        }
    }

    fn validate_request_bounds(&self) -> Result<(), PrepareSingleChapterGenerationRequestError> {
        if let Some(target_word_count) = self.target_word_count {
            if target_word_count < MIN_SINGLE_GENERATION_TARGET_WORD_COUNT {
                return Err(
                    PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooSmall,
                );
            }
            if target_word_count > MAX_SINGLE_GENERATION_TARGET_WORD_COUNT {
                return Err(
                    PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooLarge,
                );
            }
        }
        if !is_valid_optional_choice(
            self.creative_mode.as_deref(),
            SINGLE_GENERATION_CREATIVE_MODE_VALUES,
        ) {
            return Err(PrepareSingleChapterGenerationRequestError::InvalidCreativeMode);
        }
        if !is_valid_optional_choice(
            self.story_focus.as_deref(),
            SINGLE_GENERATION_STORY_FOCUS_VALUES,
        ) {
            return Err(PrepareSingleChapterGenerationRequestError::InvalidStoryFocus);
        }
        if !is_valid_optional_choice(
            self.plot_stage.as_deref(),
            SINGLE_GENERATION_PLOT_STAGE_VALUES,
        ) {
            return Err(PrepareSingleChapterGenerationRequestError::InvalidPlotStage);
        }
        if !is_valid_optional_choice(
            self.quality_preset.as_deref(),
            SINGLE_GENERATION_QUALITY_PRESET_VALUES,
        ) {
            return Err(PrepareSingleChapterGenerationRequestError::InvalidQualityPreset);
        }
        if !is_valid_optional_text_length(
            self.story_creation_brief.as_deref(),
            MAX_SINGLE_GENERATION_STORY_CREATION_BRIEF_LENGTH,
        ) {
            return Err(PrepareSingleChapterGenerationRequestError::StoryCreationBriefTooLong);
        }
        if !is_valid_optional_text_length(
            self.quality_notes.as_deref(),
            MAX_SINGLE_GENERATION_QUALITY_NOTES_LENGTH,
        ) {
            return Err(PrepareSingleChapterGenerationRequestError::QualityNotesTooLong);
        }

        Ok(())
    }

    pub(crate) fn compat_options_with_web_research_default(
        &self,
        web_research_default: bool,
    ) -> SingleChapterGenerationCompatOptions {
        SingleChapterGenerationCompatOptions {
            style_id: self.style_id,
            enable_analysis: self.enable_analysis.unwrap_or(true),
            enable_mcp: self.enable_mcp.unwrap_or(true),
            web_research_enabled: normalize_single_generation_web_research_enabled(
                self.enable_web_research,
                web_research_default,
            ),
            web_research_query: self.web_research_query.clone(),
            narrative_perspective: self.narrative_perspective.clone(),
            creative_mode: self.creative_mode.clone(),
            story_focus: self.story_focus.clone(),
            plot_stage: self.plot_stage.clone(),
            story_creation_brief: self.story_creation_brief.clone(),
            quality_preset: self.quality_preset.clone(),
            quality_notes: self.quality_notes.clone(),
            story_repair_summary: self.story_repair_summary.clone(),
            story_repair_targets: self.story_repair_targets.clone().unwrap_or_default(),
            story_preserve_strengths: self.story_preserve_strengths.clone().unwrap_or_default(),
        }
    }
}

pub(crate) fn build_single_chapter_generation_request_from_route_payload(
    route_request: SingleChapterGenerationRouteRequest,
) -> SingleChapterGenerationRequest {
    SingleChapterGenerationRequest::from_route_request(route_request)
}

fn normalize_optional_single_generation_request_string(value: Option<String>) -> Option<String> {
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

fn normalize_single_generation_web_research_enabled(
    enabled: Option<bool>,
    default_enabled: bool,
) -> bool {
    enabled.unwrap_or(default_enabled)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SingleChapterGenerationCompatOptions {
    pub(crate) style_id: Option<i32>,
    pub(crate) enable_analysis: bool,
    pub(crate) enable_mcp: bool,
    pub(crate) web_research_enabled: bool,
    pub(crate) web_research_query: Option<String>,
    pub(crate) narrative_perspective: Option<String>,
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

impl SingleChapterGenerationCompatOptions {
    pub(crate) fn style_id(&self) -> Option<i32> {
        self.style_id
    }

    pub(crate) fn enable_analysis(&self) -> bool {
        self.enable_analysis
    }

    pub(crate) fn enable_mcp(&self) -> bool {
        self.enable_mcp
    }

    pub(crate) fn web_research_enabled(&self) -> bool {
        self.web_research_enabled
    }

    pub(crate) fn web_research_query(&self) -> Option<&str> {
        self.web_research_query.as_deref()
    }

    pub(crate) fn narrative_perspective(&self) -> &str {
        self.narrative_perspective.as_deref().unwrap_or_default()
    }

    pub(crate) fn creative_mode(&self) -> &str {
        self.creative_mode.as_deref().unwrap_or_default()
    }

    pub(crate) fn story_focus(&self) -> &str {
        self.story_focus.as_deref().unwrap_or_default()
    }

    pub(crate) fn plot_stage(&self) -> &str {
        self.plot_stage.as_deref().unwrap_or_default()
    }

    pub(crate) fn story_creation_brief(&self) -> &str {
        self.story_creation_brief.as_deref().unwrap_or_default()
    }

    pub(crate) fn quality_preset(&self) -> &str {
        self.quality_preset.as_deref().unwrap_or_default()
    }

    pub(crate) fn quality_notes(&self) -> &str {
        self.quality_notes.as_deref().unwrap_or_default()
    }

    pub(crate) fn story_repair_summary(&self) -> &str {
        self.story_repair_summary.as_deref().unwrap_or_default()
    }

    pub(crate) fn story_repair_targets(&self) -> &[String] {
        &self.story_repair_targets
    }

    pub(crate) fn story_preserve_strengths(&self) -> &[String] {
        &self.story_preserve_strengths
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SingleChapterGenerationExecutionInput {
    pub(crate) target_word_count: i32,
    pub(crate) compat_options: SingleChapterGenerationCompatOptions,
    pub(crate) execution_config:
        crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SingleChapterGenerationTarget {
    pub(crate) project_id: String,
    pub(crate) chapter_id: String,
    pub(crate) chapter_number: i32,
    pub(crate) title: String,
}

impl SingleChapterGenerationTarget {
    pub(crate) fn from_model(chapter_model: &chapter::Model) -> Self {
        Self {
            project_id: chapter_model.project_id.clone(),
            chapter_id: chapter_model.id.clone(),
            chapter_number: chapter_model.chapter_number,
            title: chapter_model.title.clone(),
        }
    }

    pub(crate) fn pending_checkpoint(&self) -> Value {
        build_single_generation_runtime_checkpoint_for_stage(
            SingleGenerationSnapshotStage::Pending,
            &self.chapter_id,
            Some(self.chapter_number),
            None,
        )
    }

    pub(crate) fn background_response_payload(
        &self,
        task_id: &str,
        estimated_minutes: i32,
        active_story_repair_payload: Option<&Value>,
    ) -> Value {
        json!({
            "task_id": task_id,
            "chapter_id": self.chapter_id,
            "status": "pending",
            "message": "单章后台生成任务已创建",
            "estimated_time_minutes": estimated_minutes,
            "active_story_repair_payload": active_story_repair_payload.cloned(),
        })
    }

    pub(crate) fn background_task_active_model(
        &self,
        task_id: String,
        user_id: String,
        target_word_count: i32,
        now: chrono::NaiveDateTime,
    ) -> crate::models::batch_generation_task::ActiveModel {
        build_single_generation_background_task_active_model(
            task_id,
            &self.project_id,
            user_id,
            &self.chapter_id,
            self.chapter_number,
            &self.title,
            target_word_count,
            now,
        )
    }

    pub(crate) fn background_task_persistence_seed(
        &self,
        task_id: String,
        user_id: String,
        target_word_count: i32,
    ) -> SingleGenerationTaskPersistenceSeed {
        build_single_generation_background_task_persistence_seed(
            task_id,
            &self.project_id,
            user_id,
            &self.chapter_id,
            self.chapter_number,
            &self.title,
            target_word_count,
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PrepareSingleChapterGenerationRequestError {
    Chapter(LoadAccessibleChapterForGenerationError),
    Config(String),
    PrerequisitesBlocked(String),
    InvalidTargetWordCountTooSmall,
    InvalidTargetWordCountTooLarge,
    InvalidCreativeMode,
    InvalidStoryFocus,
    InvalidPlotStage,
    InvalidQualityPreset,
    StoryCreationBriefTooLong,
    QualityNotesTooLong,
    Internal(String),
}

impl PrepareSingleChapterGenerationRequestError {
    pub(crate) fn detail_message(&self) -> String {
        match self {
            Self::Chapter(LoadAccessibleChapterForGenerationError::ChapterNotFound) => {
                "Chapter not found".to_string()
            }
            Self::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
            ) => "Chapter not found or access denied".to_string(),
            Self::Chapter(LoadAccessibleChapterForGenerationError::Internal(detail))
            | Self::Config(detail)
            | Self::PrerequisitesBlocked(detail)
            | Self::Internal(detail) => detail.clone(),
            Self::InvalidTargetWordCountTooSmall => {
                "target_word_count must be greater than or equal to 500".to_string()
            }
            Self::InvalidTargetWordCountTooLarge => {
                "target_word_count must be less than or equal to 10000".to_string()
            }
            Self::InvalidCreativeMode => "creative_mode is invalid".to_string(),
            Self::InvalidStoryFocus => "story_focus is invalid".to_string(),
            Self::InvalidPlotStage => "plot_stage is invalid".to_string(),
            Self::InvalidQualityPreset => "quality_preset is invalid".to_string(),
            Self::StoryCreationBriefTooLong => {
                "story_creation_brief must be at most 1200 characters".to_string()
            }
            Self::QualityNotesTooLong => "quality_notes must be at most 600 characters".to_string(),
        }
    }
}

pub(crate) async fn load_single_chapter_generation_target(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<SingleChapterGenerationTarget, PrepareSingleChapterGenerationRequestError> {
    let chapter_model = load_accessible_chapter_for_generation(db, chapter_id, user_id)
        .await
        .map_err(PrepareSingleChapterGenerationRequestError::Chapter)?;
    let prerequisite = check_chapter_generation_prerequisites(db, &chapter_model)
        .await
        .map_err(PrepareSingleChapterGenerationRequestError::Internal)?;
    if !prerequisite.can_generate {
        return Err(
            PrepareSingleChapterGenerationRequestError::PrerequisitesBlocked(
                prerequisite.error_message,
            ),
        );
    }

    Ok(SingleChapterGenerationTarget::from_model(&chapter_model))
}

pub(crate) async fn prepare_single_chapter_generation_execution_config_from_runtime_state(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_target: &SingleChapterGenerationTarget,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
) -> Result<PreparedGenerationExecutionConfig, PrepareSingleChapterGenerationRequestError> {
    let provider_payload = build_single_chapter_research_provider_payload(
        db,
        user_id,
        chapter_target,
        &request_runtime_state.compat_options,
    )
    .await
    .map_err(PrepareSingleChapterGenerationRequestError::Config)?;

    prepare_generation_execution_config_with_provider_payload(
        db,
        user_id,
        request_runtime_state.model_override.as_deref(),
        provider_payload,
    )
    .await
    .map_err(PrepareSingleChapterGenerationRequestError::Config)
}

pub(crate) fn build_single_generation_runtime_launch_input_from_request_runtime_state(
    chapter_target: &SingleChapterGenerationTarget,
    user_id: &str,
    target_word_count: i32,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    execution_config: PreparedGenerationExecutionConfig,
) -> crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput
{
    crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput {
        chapter_id: chapter_target.chapter_id.clone(),
        user_id: user_id.to_string(),
        execution_input: SingleChapterGenerationExecutionInput {
            target_word_count,
            compat_options: request_runtime_state.compat_options.clone(),
            execution_config,
        },
    }
}

pub(crate) async fn prepare_single_chapter_runtime_launch_input_from_request_runtime_state(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_target: &SingleChapterGenerationTarget,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    target_word_count: i32,
) -> Result<
    crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput,
    PrepareSingleChapterGenerationRequestError,
>{
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
    runtime_input:
        crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedSingleGenerationBackgroundLaunchParts {
    pub(crate) task_seed: SingleGenerationTaskPersistenceSeed,
    pub(crate) startup_snapshot_plan: SingleGenerationStartupSnapshotPlan,
    pub(crate) response_payload: Value,
    pub(crate) runtime_input:
        crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput,
}

impl PreparedSingleGenerationBackgroundLaunchParts {
    pub(crate) async fn persist_and_dispatch(
        self,
        db: &DatabaseConnection,
        now: chrono::NaiveDateTime,
    ) -> Result<Value, PrepareSingleChapterGenerationRequestError> {
        let Self {
            task_seed,
            startup_snapshot_plan,
            response_payload,
            runtime_input,
        } = self;
        let task_id = task_seed.id.clone();
        let task = task_seed.into_active_model(now);

        task.insert(db).await.map_err(|error| {
            PrepareSingleChapterGenerationRequestError::Internal(error.to_string())
        })?;
        startup_snapshot_plan
            .persist(db, &task_id)
            .await
            .map_err(PrepareSingleChapterGenerationRequestError::Internal)?;
        dispatch_single_chapter_generation_runtime(db.clone(), task_id, runtime_input);

        Ok(response_payload)
    }
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
    ) -> Result<
        crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput,
        PrepareSingleChapterGenerationRequestError,
    >{
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
        crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput,
    ){
        (
            self.chapter_target,
            self.startup_snapshot_plan,
            self.runtime_input,
        )
    }

    pub(crate) fn into_runtime_launch_input(
        self,
    ) -> crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput
    {
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
        let task_seed = chapter_target.background_task_persistence_seed(
            task_id.clone(),
            runtime_input.user_id.clone(),
            runtime_input.execution_input.target_word_count,
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
        runtime_input: crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput,
    ) -> Self {
        Self {
            startup_snapshot_plan: SingleGenerationStartupSnapshotPlan::from_pending_checkpoint(
                chapter_target.pending_checkpoint(),
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
    runtime_input:
        &crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput,
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
    let compatibility_payload = chapter_target.background_response_payload(
        task_id,
        estimated_minutes,
        active_story_repair_payload.as_ref(),
    );
    if let Value::Object(compatibility_payload) = compatibility_payload {
        payload.extend(compatibility_payload);
    }

    Value::Object(payload)
}

pub(crate) fn build_single_generation_runtime_launch_input(
    chapter_id: String,
    user_id: String,
    execution_input: SingleChapterGenerationExecutionInput,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    runtime_state_payload: &Value,
) -> crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput{
    let resolved_compat_options = resolve_single_generation_runtime_compat_options_from_seed(
        request_runtime_state,
        runtime_state_payload,
    );
    let SingleChapterGenerationExecutionInput {
        target_word_count,
        execution_config,
        ..
    } = execution_input;

    crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput {
        chapter_id,
        user_id,
        execution_input: SingleChapterGenerationExecutionInput {
            target_word_count,
            compat_options: resolved_compat_options,
            execution_config,
        },
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
        crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput,
    ){
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

async fn restore_single_generation_runtime_state(
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
            chapter_target.pending_checkpoint(),
            request_runtime_state,
            quality_fragments,
            recent_history_summary,
        ),
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

#[cfg(test)]
mod tests {
    use super::{
        build_single_chapter_generation_request_from_route_payload,
        build_single_generation_runtime_launch_input_from_request_runtime_state,
        BatchGenerationRequestRuntimeState, PrepareSingleChapterGenerationRequestError,
        PreparedSingleChapterGenerationRestoredRuntimeLaunch, RestoredSingleGenerationRuntimeState,
        SingleChapterGenerationCompatOptions, SingleChapterGenerationExecutionInput,
        SingleChapterGenerationRequest, SingleChapterGenerationRouteRequest,
        SingleChapterGenerationTarget,
    };
    use crate::models::chapter;
    use crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig;
    use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
    use crate::services::chapter_generation_target_word_count_service::normalize_chapter_generation_target_word_count;
    use crate::services::chapter_quality_metrics_query_service::ChapterQualityMetricsFragments;
    use crate::services::chapter_single_generation_runtime_state_service::SingleGenerationRuntimeLaunchInput;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn should_normalize_single_chapter_generation_target_word_count() {
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
    fn should_load_single_chapter_generation_target_from_request() {
        let request = SingleChapterGenerationRequest {
            style_id: None,
            target_word_count: Some(1800),
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
        };

        assert_eq!(
            normalize_chapter_generation_target_word_count(request.target_word_count),
            1800
        );
    }

    #[test]
    fn should_reject_unknown_single_chapter_generation_route_fields_like_python_schema() {
        let error = serde_json::from_value::<SingleChapterGenerationRouteRequest>(json!({
            "target_word_count": 1800,
            "unexpected_field": true
        }))
        .expect_err("python ChapterGenerateRequest forbids extra fields");

        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn should_accept_known_single_chapter_generation_route_fields_with_strict_schema() {
        let request = serde_json::from_value::<SingleChapterGenerationRouteRequest>(json!({
            "target_word_count": 1800,
            "creative_mode": "hook",
            "quality_notes": "keep pacing tight"
        }))
        .expect("known python ChapterGenerateRequest fields should parse");

        assert_eq!(request.target_word_count, Some(1800));
        assert_eq!(request.creative_mode.as_deref(), Some("hook"));
        assert_eq!(request.quality_notes.as_deref(), Some("keep pacing tight"));
    }

    #[test]
    fn should_reject_single_chapter_generation_route_null_for_non_nullable_python_default_flags() {
        for (field_name, payload) in [
            ("enable_analysis", json!({"enable_analysis": null})),
            ("enable_mcp", json!({"enable_mcp": null})),
        ] {
            let error =
                serde_json::from_value::<SingleChapterGenerationRouteRequest>(payload).unwrap_err();

            assert!(
                error.to_string().contains("invalid type: null"),
                "{field_name} should reject explicit null like Python bool defaults"
            );
        }
    }

    #[test]
    fn should_keep_single_chapter_generation_route_nullable_fields_accepting_null() {
        let request = serde_json::from_value::<SingleChapterGenerationRouteRequest>(json!({
            "target_word_count": null,
            "enable_web_research": null
        }))
        .expect("Python Optional fields should keep accepting explicit null");

        assert_eq!(request.target_word_count, None);
        assert_eq!(request.enable_web_research, None);
    }

    #[test]
    fn should_apply_single_chapter_generation_python_defaults_when_flags_are_missing() {
        let route_request =
            serde_json::from_value::<SingleChapterGenerationRouteRequest>(json!({}))
                .expect("missing route fields should parse");
        assert_eq!(route_request.enable_analysis, None);
        assert_eq!(route_request.enable_mcp, None);

        let request = build_single_chapter_generation_request_from_route_payload(route_request);
        let compat = request.compat_options_with_web_research_default(false);

        assert!(compat.enable_analysis());
        assert!(compat.enable_mcp());
        assert!(!compat.web_research_enabled());
    }

    #[test]
    fn should_normalize_single_chapter_generation_fields_like_python_schema() {
        let request = build_single_chapter_generation_request_from_route_payload(
            SingleChapterGenerationRouteRequest {
                creative_mode: Some(" hook ".to_string()),
                story_focus: Some(" advance_plot ".to_string()),
                plot_stage: Some(" development ".to_string()),
                story_creation_brief: Some(" 强化开场钩子 ".to_string()),
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
            Some("强化开场钩子")
        );
        assert_eq!(request.quality_preset.as_deref(), Some("plot_drive"));
        assert_eq!(request.quality_notes.as_deref(), Some("压缩说明段"));
        assert_eq!(
            request.story_repair_summary.as_deref(),
            Some("修复中段节奏")
        );
    }

    #[test]
    fn should_convert_blank_single_chapter_generation_fields_to_none() {
        let request = build_single_chapter_generation_request_from_route_payload(
            SingleChapterGenerationRouteRequest {
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
    fn should_reject_single_chapter_generation_target_word_count_outside_python_bounds() {
        let too_low = SingleChapterGenerationRequest {
            target_word_count: Some(499),
            ..SingleChapterGenerationRequest::default()
        };
        let too_high = SingleChapterGenerationRequest {
            target_word_count: Some(10_001),
            ..SingleChapterGenerationRequest::default()
        };

        assert!(matches!(
            too_low
                .validate_request_bounds()
                .expect_err("target_word_count below python limit should fail"),
            PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooSmall
        ));
        assert!(matches!(
            too_high
                .validate_request_bounds()
                .expect_err("target_word_count above python limit should fail"),
            PrepareSingleChapterGenerationRequestError::InvalidTargetWordCountTooLarge
        ));
    }

    #[test]
    fn should_reject_single_chapter_generation_invalid_choice_fields() {
        let cases = [
            (
                SingleChapterGenerationRequest {
                    creative_mode: Some("too_fancy".to_string()),
                    ..SingleChapterGenerationRequest::default()
                },
                PrepareSingleChapterGenerationRequestError::InvalidCreativeMode,
            ),
            (
                SingleChapterGenerationRequest {
                    story_focus: Some("too_broad".to_string()),
                    ..SingleChapterGenerationRequest::default()
                },
                PrepareSingleChapterGenerationRequestError::InvalidStoryFocus,
            ),
            (
                SingleChapterGenerationRequest {
                    plot_stage: Some("middle".to_string()),
                    ..SingleChapterGenerationRequest::default()
                },
                PrepareSingleChapterGenerationRequestError::InvalidPlotStage,
            ),
            (
                SingleChapterGenerationRequest {
                    quality_preset: Some("max_quality".to_string()),
                    ..SingleChapterGenerationRequest::default()
                },
                PrepareSingleChapterGenerationRequestError::InvalidQualityPreset,
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
    fn should_reject_single_chapter_generation_text_fields_above_python_limits() {
        let long_brief = SingleChapterGenerationRequest {
            story_creation_brief: Some("a".repeat(1201)),
            ..SingleChapterGenerationRequest::default()
        };
        let long_quality_notes = SingleChapterGenerationRequest {
            quality_notes: Some("b".repeat(601)),
            ..SingleChapterGenerationRequest::default()
        };

        assert_eq!(
            long_brief
                .validate_request_bounds()
                .expect_err("story_creation_brief above python limit should fail"),
            PrepareSingleChapterGenerationRequestError::StoryCreationBriefTooLong
        );
        assert_eq!(
            long_quality_notes
                .validate_request_bounds()
                .expect_err("quality_notes above python limit should fail"),
            PrepareSingleChapterGenerationRequestError::QualityNotesTooLong
        );
    }

    #[test]
    fn should_accept_single_chapter_generation_python_request_bounds() {
        let lower_bound_request = SingleChapterGenerationRequest {
            target_word_count: Some(500),
            ..SingleChapterGenerationRequest::default()
        };
        let upper_bound_request = SingleChapterGenerationRequest {
            target_word_count: Some(10_000),
            ..SingleChapterGenerationRequest::default()
        };
        let choice_and_text_request = SingleChapterGenerationRequest {
            target_word_count: Some(3000),
            creative_mode: Some("hook".to_string()),
            story_focus: Some("advance_plot".to_string()),
            plot_stage: Some("development".to_string()),
            quality_preset: Some("plot_drive".to_string()),
            story_creation_brief: Some("a".repeat(1200)),
            quality_notes: Some("b".repeat(600)),
            ..SingleChapterGenerationRequest::default()
        };
        let blank_choice_and_text_request = SingleChapterGenerationRequest {
            creative_mode: Some("   ".to_string()),
            story_focus: Some("   ".to_string()),
            plot_stage: Some("   ".to_string()),
            quality_preset: Some("   ".to_string()),
            story_creation_brief: Some("   ".to_string()),
            quality_notes: Some("   ".to_string()),
            ..SingleChapterGenerationRequest::default()
        };

        lower_bound_request
            .validate_request_bounds()
            .expect("python lower target word count should pass");
        upper_bound_request
            .validate_request_bounds()
            .expect("python upper target word count should pass");
        choice_and_text_request
            .validate_request_bounds()
            .expect("valid python generation choices and text lengths should pass");
        blank_choice_and_text_request
            .validate_request_bounds()
            .expect("blank choices and texts normalize to None in python");
    }

    #[test]
    fn should_keep_single_chapter_generation_execution_input_contract() {
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2600,
            compat_options: SingleChapterGenerationCompatOptions {
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
        };

        assert_eq!(execution_input.target_word_count, 2600);
        assert_eq!(
            execution_input
                .execution_config
                .provider_payload
                .characters_info,
            "[]"
        );
        assert_eq!(
            execution_input
                .execution_config
                .provider_payload
                .external_assets,
            "[]"
        );
    }

    #[test]
    fn should_keep_single_chapter_generation_target_projection_contract() {
        let chapter_model = chapter::Model {
            id: "chapter-7".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
            content: Some("content".to_string()),
            summary: Some("summary".to_string()),
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        };

        let target = SingleChapterGenerationTarget::from_model(&chapter_model);

        assert_eq!(target.project_id, "project-1");
        assert_eq!(target.chapter_id, "chapter-7");
        assert_eq!(target.chapter_number, 7);
        assert_eq!(target.title, "Seven");
    }

    #[test]
    fn should_build_single_chapter_generation_target_payloads_from_owner() {
        let target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };

        let checkpoint = target.pending_checkpoint();
        let response_payload = target.background_response_payload("task-1", 2, None);
        let active_model = target.background_task_active_model(
            "task-1".to_string(),
            "user-1".to_string(),
            2600,
            chrono::NaiveDateTime::default(),
        );

        assert_eq!(checkpoint["chapter_id"], "chapter-7");
        assert_eq!(checkpoint["phase"], "pending");
        assert_eq!(response_payload["task_id"], "task-1");
        assert_eq!(response_payload["chapter_id"], "chapter-7");
        assert_eq!(response_payload["estimated_time_minutes"], 2);
        assert_eq!(active_model.target_word_count, sea_orm::Set(2600));
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
    fn should_build_single_chapter_generation_background_parts_from_target_owner() {
        let target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-7".to_string(),
            chapter_number: 7,
            title: "Seven".to_string(),
        };

        let checkpoint = target.pending_checkpoint();
        let response_payload = target.background_response_payload("task-1", 2, None);
        let task = target.background_task_active_model(
            "task-1".to_string(),
            "user-1".to_string(),
            2600,
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
    fn should_build_single_chapter_generation_request_parts_from_owner() {
        let request = build_single_chapter_generation_request_from_route_payload(
            SingleChapterGenerationRouteRequest {
                style_id: Some(7),
                target_word_count: Some(2200),
                model: Some("gpt-test".to_string()),
                enable_analysis: Some(true),
                enable_mcp: Some(true),
                enable_web_research: Some(true),
                web_research_query: Some("hero backstory".to_string()),
                narrative_perspective: Some("third_person".to_string()),
                creative_mode: Some("balanced".to_string()),
                story_focus: Some("advance_plot".to_string()),
                plot_stage: Some("development".to_string()),
                story_creation_brief: Some("brief".to_string()),
                quality_preset: Some("balanced".to_string()),
                quality_notes: Some("notes".to_string()),
                story_repair_summary: Some("repair".to_string()),
                story_repair_targets: Some(vec!["target-a".to_string()]),
                story_preserve_strengths: Some(vec!["strength-a".to_string()]),
            },
        );
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-8".to_string(),
            chapter_number: 8,
            title: "Eight".to_string(),
        };
        let execution_input = SingleChapterGenerationExecutionInput {
            target_word_count: 2200,
            compat_options: request.compat_options_with_web_research_default(false),
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

        assert_eq!(request.style_id, Some(7));
        assert_eq!(request.target_word_count, Some(2200));
        assert_eq!(request.model.as_deref(), Some("gpt-test"));
        assert_eq!(request.enable_analysis, Some(true));
        assert_eq!(request.enable_mcp, Some(true));
        assert_eq!(request.enable_web_research, Some(true));
        assert_eq!(
            request.web_research_query.as_deref(),
            Some("hero backstory")
        );
        assert_eq!(
            request.narrative_perspective.as_deref(),
            Some("third_person")
        );
        assert_eq!(request.creative_mode.as_deref(), Some("balanced"));
        assert_eq!(request.story_focus.as_deref(), Some("advance_plot"));
        assert_eq!(request.plot_stage.as_deref(), Some("development"));
        assert_eq!(request.story_creation_brief.as_deref(), Some("brief"));
        assert_eq!(request.quality_preset.as_deref(), Some("balanced"));
        assert_eq!(request.quality_notes.as_deref(), Some("notes"));
        assert_eq!(request.story_repair_summary.as_deref(), Some("repair"));
        assert_eq!(
            request.story_repair_targets.as_deref(),
            Some(&["target-a".to_string()][..])
        );
        assert_eq!(
            request.story_preserve_strengths.as_deref(),
            Some(&["strength-a".to_string()][..])
        );
        assert_eq!(chapter_target.chapter_id, "chapter-8");
        assert_eq!(execution_input.target_word_count, 2200);
        assert_eq!(execution_input.compat_options.style_id(), Some(7));
        assert!(execution_input.compat_options.enable_analysis());
        assert!(execution_input.compat_options.enable_mcp());
        assert!(execution_input.compat_options.web_research_enabled());
        assert_eq!(
            execution_input.compat_options.web_research_query(),
            Some("hero backstory")
        );
        assert_eq!(
            execution_input.compat_options.narrative_perspective(),
            "third_person"
        );
        assert_eq!(execution_input.compat_options.creative_mode(), "balanced");
        assert_eq!(execution_input.compat_options.story_focus(), "advance_plot");
        assert_eq!(execution_input.compat_options.plot_stage(), "development");
        assert_eq!(
            execution_input.compat_options.story_creation_brief(),
            "brief"
        );
        assert_eq!(execution_input.compat_options.quality_preset(), "balanced");
        assert_eq!(execution_input.compat_options.quality_notes(), "notes");
        assert_eq!(
            execution_input.compat_options.story_repair_summary(),
            "repair"
        );
        assert_eq!(
            execution_input.compat_options.story_repair_targets(),
            &["target-a".to_string()]
        );
        assert_eq!(
            execution_input.compat_options.story_preserve_strengths(),
            &["strength-a".to_string()]
        );
        assert_eq!(
            execution_input
                .execution_config
                .provider_payload
                .characters_info,
            "[]"
        );
        assert_eq!(
            execution_input
                .execution_config
                .provider_payload
                .research_query,
            ""
        );
    }

    #[test]
    fn should_project_prepared_single_chapter_generation_restored_launch_owner() {
        let runtime_input = SingleGenerationRuntimeLaunchInput {
            chapter_id: "chapter-8".to_string(),
            user_id: "user-1".to_string(),
            execution_input: SingleChapterGenerationExecutionInput {
                target_word_count: 2200,
                compat_options: SingleChapterGenerationCompatOptions {
                    style_id: Some(7),
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
        let restored_launch = PreparedSingleChapterGenerationRestoredRuntimeLaunch::from_parts(
            SingleChapterGenerationTarget {
                project_id: "project-1".to_string(),
                chapter_id: "chapter-8".to_string(),
                chapter_number: 8,
                title: "Eight".to_string(),
            },
            json!({
                "batch_request_runtime_state": {
                    "model_override": "gpt-test"
                }
            }),
            runtime_input,
        );

        assert_eq!(
            restored_launch.startup_snapshot_plan().runtime_state()["batch_request_runtime_state"]
                ["model_override"],
            "gpt-test"
        );

        let runtime_input = restored_launch.clone().into_runtime_launch_input();
        let (chapter_target, startup_snapshot_plan, runtime_input_again) =
            restored_launch.into_parts();

        assert!(matches!(
            runtime_input,
            SingleGenerationRuntimeLaunchInput {
                chapter_id,
                user_id,
                execution_input: SingleChapterGenerationExecutionInput {
                    target_word_count: 2200,
                    ..
                },
            } if chapter_id == "chapter-8" && user_id == "user-1"
        ));
        assert_eq!(chapter_target.chapter_number, 8);
        assert_eq!(
            startup_snapshot_plan.runtime_state()["batch_request_runtime_state"]["model_override"],
            "gpt-test"
        );
        assert_eq!(runtime_input_again.execution_input.target_word_count, 2200);
    }

    #[test]
    fn should_build_single_generation_runtime_launch_input_from_request_runtime_state_owner() {
        let chapter_target = SingleChapterGenerationTarget {
            project_id: "project-1".to_string(),
            chapter_id: "chapter-9".to_string(),
            chapter_number: 9,
            title: "第九章".to_string(),
        };
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                enable_analysis: true,
                story_repair_summary: Some("沿用恢复态摘要".to_string()),
                story_repair_targets: vec!["压缩说明".to_string()],
                ..Default::default()
            },
            Some("owner-model".to_string()),
        );

        let runtime_input = build_single_generation_runtime_launch_input_from_request_runtime_state(
            &chapter_target,
            "user-9",
            2800,
            &request_runtime_state,
            PreparedGenerationExecutionConfig {
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
        );

        assert_eq!(runtime_input.chapter_id, "chapter-9");
        assert_eq!(runtime_input.user_id, "user-9");
        assert_eq!(runtime_input.execution_input.target_word_count, 2800);
        assert_eq!(
            runtime_input
                .execution_input
                .compat_options
                .story_repair_summary(),
            "沿用恢复态摘要"
        );
        assert_eq!(
            runtime_input
                .execution_input
                .compat_options
                .story_repair_targets(),
            &["压缩说明".to_string()]
        );
        assert_eq!(
            runtime_input
                .execution_input
                .execution_config
                .ai_config
                .provider,
            crate::ai::AIConfig::default().provider
        );
    }

    #[test]
    fn should_project_restored_single_generation_runtime_state_into_startup_and_runtime_launch_owner(
    ) {
        let request_runtime_state = BatchGenerationRequestRuntimeState::new(
            SingleChapterGenerationCompatOptions {
                story_repair_summary: Some("request summary".to_string()),
                story_repair_targets: vec!["request-target".to_string()],
                story_preserve_strengths: vec!["request-strength".to_string()],
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
        let request = build_single_chapter_generation_request_from_route_payload(
            SingleChapterGenerationRouteRequest {
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
            },
        );

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
        let request = build_single_chapter_generation_request_from_route_payload(
            SingleChapterGenerationRouteRequest {
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
            },
        );

        let compat = request.compat_options_with_web_research_default(true);

        assert!(compat.web_research_enabled());
    }
}
