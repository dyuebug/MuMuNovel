use chrono::NaiveDateTime;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::models::{batch_generation_task, chapter};
use crate::services::chapter_generation_execution_config_service::{
    prepare_generation_execution_config_with_provider_payload, PreparedGenerationExecutionConfig,
};
pub(crate) use crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions;
use crate::services::chapter_generation_prerequisite_service::check_chapter_generation_prerequisites;
use crate::services::chapter_generation_request_runtime_state_service::BatchGenerationRequestRuntimeState;
use crate::services::chapter_generation_research_payload_service::build_single_chapter_research_provider_payload;
use crate::services::route_request_deserialize_service::deserialize_optional_non_null;

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

impl SingleChapterGenerationRouteRequest {
    pub(crate) fn into_generation_request(self) -> SingleChapterGenerationRequest {
        SingleChapterGenerationRequest {
            style_id: self.style_id,
            target_word_count: self.target_word_count,
            model: self.model,
            enable_analysis: self.enable_analysis,
            enable_mcp: self.enable_mcp,
            enable_web_research: self.enable_web_research,
            web_research_query: self.web_research_query,
            narrative_perspective: self.narrative_perspective,
            creative_mode: normalize_optional_single_generation_request_string(self.creative_mode),
            story_focus: normalize_optional_single_generation_request_string(self.story_focus),
            plot_stage: normalize_optional_single_generation_request_string(self.plot_stage),
            story_creation_brief: normalize_optional_single_generation_request_string(
                self.story_creation_brief,
            ),
            quality_preset: normalize_optional_single_generation_request_string(
                self.quality_preset,
            ),
            quality_notes: normalize_optional_single_generation_request_string(self.quality_notes),
            story_repair_summary: normalize_optional_single_generation_request_string(
                self.story_repair_summary,
            ),
            story_repair_targets: self.story_repair_targets,
            story_preserve_strengths: self.story_preserve_strengths,
        }
    }
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
    pub(crate) fn validate_request_bounds(
        &self,
    ) -> Result<(), PrepareSingleChapterGenerationRequestError> {
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

#[cfg(test)]
mod tests {
    use super::{
        build_single_generation_runtime_payload_base,
        build_single_generation_task_view_payload_from_task_state,
        estimated_single_generation_task_minutes, single_generation_active_task_statuses,
        single_generation_pending_stage_code, PrepareSingleChapterGenerationRequestError,
        SingleChapterGenerationCompatOptions, SingleChapterGenerationRequest,
        SingleChapterGenerationRouteRequest, SingleChapterGenerationTarget,
    };
    use crate::models::{batch_generation_task, chapter};
    use crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig;
    use crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationExecutionInput;
    use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
    use crate::services::chapter_generation_request_runtime_state_service::BatchGenerationRequestRuntimeState;
    use crate::services::chapter_generation_target_word_count_service::normalize_chapter_generation_target_word_count;
    use crate::services::chapter_quality_metrics_query_service::ChapterQualityMetricsFragments;
    use crate::services::chapter_single_generation_runtime_restore_service::{
        build_single_generation_runtime_launch_input_from_request_runtime_state,
        PreparedSingleChapterGenerationRestoredRuntimeLaunch, RestoredSingleGenerationRuntimeState,
    };
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
    fn should_keep_single_generation_task_minutes_contract() {
        assert_eq!(estimated_single_generation_task_minutes(3000, false), 2);
        assert_eq!(estimated_single_generation_task_minutes(3000, true), 3);
        assert_eq!(estimated_single_generation_task_minutes(200, false), 1);
    }

    #[test]
    fn should_keep_single_generation_active_statuses_contract() {
        assert_eq!(
            single_generation_active_task_statuses(),
            ["pending", "running"]
        );
        assert_eq!(single_generation_pending_stage_code(), "6.writing.pending");
    }

    #[test]
    fn should_build_single_generation_runtime_payload_base_from_prepare_owner() {
        let payload = build_single_generation_runtime_payload_base(
            "task-1",
            "project-1",
            Some("chapter-1"),
            "pending",
            Some(&json!({"progress": 15})),
            None,
        );

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["project_id"], "project-1");
        assert_eq!(payload["current_chapter_id"], "chapter-1");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["stage_code"], "6.writing.pending");
        assert_eq!(payload["execution_mode"], "interactive");
        assert_eq!(payload["checkpoint"]["progress"], 15);
        assert_eq!(payload["checkpoint"]["stage_code"], "6.writing.pending");
    }

    #[test]
    fn should_build_single_generation_task_view_payload_from_prepare_owner() {
        let task = batch_generation_task::Model {
            id: "task-2".to_string(),
            project_id: "project-2".to_string(),
            user_id: "user-2".to_string(),
            start_chapter_number: 3,
            chapter_count: 1,
            chapter_ids: json!(["chapter-3"]),
            style_id: None,
            target_word_count: 2600,
            enable_analysis: true,
            status: "running".to_string(),
            total_chapters: 1,
            completed_chapters: 0,
            failed_chapters: json!([]),
            current_chapter_id: Some("chapter-3".to_string()),
            current_chapter_number: Some(3),
            current_retry_count: 0,
            max_retries: 3,
            created_at: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        };

        let payload = build_single_generation_task_view_payload_from_task_state(
            &task,
            Some(&json!({"phase": "generating", "progress": 42})),
        );

        assert_eq!(payload["batch_id"], "task-2");
        assert_eq!(payload["current_chapter_id"], "chapter-3");
        assert_eq!(payload["current_chapter_number"], 3);
        assert_eq!(payload["checkpoint"]["phase"], "generating");
        assert_eq!(payload["checkpoint"]["progress"], 42);
        assert_eq!(payload["total"], 1);
        assert_eq!(payload["completed"], 0);
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

        let request = route_request.into_generation_request();
        let compat = request.compat_options_with_web_research_default(false);

        assert!(compat.enable_analysis());
        assert!(compat.enable_mcp());
        assert!(!compat.web_research_enabled());
    }

    #[test]
    fn should_normalize_single_chapter_generation_fields_like_python_schema() {
        let request = SingleChapterGenerationRouteRequest {
            creative_mode: Some(" hook ".to_string()),
            story_focus: Some(" advance_plot ".to_string()),
            plot_stage: Some(" development ".to_string()),
            story_creation_brief: Some(" 强化开场钩子 ".to_string()),
            quality_preset: Some(" plot_drive ".to_string()),
            quality_notes: Some(" 压缩说明段 ".to_string()),
            story_repair_summary: Some(" 修复中段节奏 ".to_string()),
            ..Default::default()
        }
        .into_generation_request();

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
        let request = SingleChapterGenerationRouteRequest {
            creative_mode: Some("   ".to_string()),
            story_focus: Some("\t".to_string()),
            plot_stage: Some("\n".to_string()),
            story_creation_brief: Some("   ".to_string()),
            quality_preset: Some("   ".to_string()),
            quality_notes: Some("   ".to_string()),
            story_repair_summary: Some("   ".to_string()),
            ..Default::default()
        }
        .into_generation_request();

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
    fn should_build_single_chapter_generation_request_parts_from_owner() {
        let request = SingleChapterGenerationRouteRequest {
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
        }
        .into_generation_request();
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
}
