use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};

pub(crate) mod request_contract_owner;

pub(crate) use self::request_contract_owner::{
    build_batch_generation_create_workflow_request_from_route_payload,
    build_batch_generation_request_contract_owner_contract, BatchGenerationCreateRouteRequest,
};
use crate::models::chapter;
use crate::services::chapter_generation_execution_contract_service::normalize_chapter_generation_target_word_count;
use crate::services::chapter_single_generation_prepare_service::check_chapter_generation_prerequisites;

pub(crate) const MAX_BATCH_GENERATION_CREATE_COUNT: i32 = 20;
pub(crate) const MIN_BATCH_GENERATION_CREATE_TARGET_WORD_COUNT: i32 = 500;
pub(crate) const MAX_BATCH_GENERATION_CREATE_TARGET_WORD_COUNT: i32 = 10_000;
pub(crate) const MIN_BATCH_GENERATION_CREATE_RETRIES: i32 = 0;
pub(crate) const MAX_BATCH_GENERATION_CREATE_RETRIES: i32 = 5;
pub(crate) const MAX_BATCH_GENERATION_CREATE_STORY_CREATION_BRIEF_LENGTH: usize = 1200;
pub(crate) const MAX_BATCH_GENERATION_CREATE_QUALITY_NOTES_LENGTH: usize = 600;
pub(crate) const BATCH_GENERATION_CREATE_CREATIVE_MODE_VALUES: &[&str] = &[
    "balanced",
    "hook",
    "emotion",
    "suspense",
    "relationship",
    "payoff",
];
pub(crate) const BATCH_GENERATION_CREATE_STORY_FOCUS_VALUES: &[&str] = &[
    "advance_plot",
    "deepen_character",
    "escalate_conflict",
    "reveal_mystery",
    "relationship_shift",
    "foreshadow_payoff",
];
pub(crate) const BATCH_GENERATION_CREATE_PLOT_STAGE_VALUES: &[&str] =
    &["development", "climax", "ending"];
pub(crate) const BATCH_GENERATION_CREATE_QUALITY_PRESET_VALUES: &[&str] = &[
    "balanced",
    "plot_drive",
    "immersive",
    "emotion_drama",
    "clean_prose",
];

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
    pub(crate) async fn prepare(
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

    pub(crate) async fn load_chapters_for_generation_range(
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

    pub(crate) fn select_chapters_for_generation_range(
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BatchGenerationCreateTaskSpec {
    pub(crate) start_chapter_number: i32,
    pub(crate) style_id: Option<i32>,
    pub(crate) enable_analysis: bool,
    pub(crate) max_retries: i32,
}

impl BatchGenerationCreateTaskSpec {
    pub(crate) fn with_effective_style_id(self, style_id: Option<i32>) -> Self {
        Self { style_id, ..self }
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
    pub(crate) fn from_model(chapter_model: &chapter::Model) -> Self {
        Self {
            id: chapter_model.id.clone(),
            chapter_number: chapter_model.chapter_number,
            title: chapter_model.title.clone(),
        }
    }
}

pub(crate) fn build_batch_generation_request_prepare_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_write_workflow_service::request_prepare_owner",
        "scope": "batch_generation_create_request_normalization_validation_and_target_selection",
        "python_source_map": [
            "backend/app/api/chapter_batch_generation_routes.py",
            "backend/app/api/chapters.py",
            "backend/app/services/batch_generation/create_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service/request_prepare_owner.rs",
            "backend-rs/src/api/chapter_batch_generation.rs"
        ],
        "behavior_contract": {
            "request_entrypoints": [
                "build_batch_generation_create_workflow_request_from_route_payload",
                "BatchGenerationCreateWorkflowRequest::from_route_request",
                "BatchGenerationCreateWorkflowRequest::compat_options_with_web_research_default",
                "BatchGenerationCreateWorkflowRequest::into_request_runtime_state"
            ],
            "prepare_entrypoints": [
                "BatchGenerationCreateWorkflowRequest::validate_request_bounds",
                "BatchGenerationCreateWorkflowRequest::load_chapters_for_generation_range",
                "BatchGenerationCreateWorkflowRequest::select_chapters_for_generation_range",
                "BatchGenerationCreateWorkflowRequest::prepare"
            ],
            "request_contract_fields": [
                "start_chapter_number",
                "count",
                "style_id",
                "target_word_count",
                "enable_analysis",
                "enable_mcp",
                "enable_web_research",
                "web_research_query",
                "max_retries",
                "model",
                "creative_mode",
                "story_focus",
                "plot_stage",
                "story_creation_brief",
                "quality_preset",
                "quality_notes",
                "story_repair_summary",
                "story_repair_targets",
                "story_preserve_strengths"
            ]
        },
        "request_contract_owner_contract": build_batch_generation_request_contract_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_write_workflow_service",
            "cargo check"
        ]
    })
}
