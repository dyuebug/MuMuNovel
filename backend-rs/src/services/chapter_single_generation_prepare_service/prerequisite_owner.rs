use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};

use crate::models::chapter;

use super::{
    MAX_SINGLE_GENERATION_QUALITY_NOTES_LENGTH, MAX_SINGLE_GENERATION_STORY_CREATION_BRIEF_LENGTH,
    SINGLE_GENERATION_CREATIVE_MODE_VALUES, SINGLE_GENERATION_PLOT_STAGE_VALUES,
    SINGLE_GENERATION_QUALITY_PRESET_VALUES, SINGLE_GENERATION_STORY_FOCUS_VALUES,
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterGenerationPrerequisiteCheck {
    pub(crate) can_generate: bool,
    pub(crate) error_message: String,
    pub(crate) previous_chapters: Vec<chapter::Model>,
}

pub(crate) fn build_chapter_generation_prerequisite_owner_contract() -> Value {
    json!({
        "owner": "chapter_single_generation_prepare_service",
        "scope": "prerequisite_owner",
        "python_source_map": [
            "backend/app/services/chapter_generation/prerequisite_service.py",
            "backend/app/api/chapter_analysis_task_routes.py",
            "backend/app/api/chapter_batch_generation_routes.py",
            "backend/app/services/compat/chapter_generation_route_compat_service.py",
            "backend/app/services/batch_generation_retry_service.py",
            "backend/app/models/chapter.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_single_generation_prepare_service.rs",
            "backend-rs/src/services/chapter_single_generation_prepare_service/prerequisite_owner.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/services/chapter_query_service.rs"
        ],
        "behavior_contract": {
            "entrypoint": "check_chapter_generation_prerequisites",
            "first_chapter": {
                "can_generate": true,
                "error_message": "",
                "previous_chapters": []
            },
            "previous_chapter_query": [
                "same_project_id",
                "chapter_number_lt_current",
                "ordered_by_chapter_number_asc"
            ],
            "incomplete_content_rule": "missing_or_trimmed_empty_content_blocks_generation",
            "error_message_template": "前置章节尚未完成: <numbers> 章",
            "route_payload_consumers": [
                "load_can_generate_payload",
                "load_single_chapter_generation_target"
            ],
            "request_normalization_helpers": [
                "normalize_optional_single_generation_request_string",
                "is_valid_optional_choice",
                "is_valid_optional_text_length",
                "normalize_single_generation_web_research_enabled"
            ],
            "request_bounds": {
                "story_creation_brief_max_chars": MAX_SINGLE_GENERATION_STORY_CREATION_BRIEF_LENGTH,
                "quality_notes_max_chars": MAX_SINGLE_GENERATION_QUALITY_NOTES_LENGTH,
                "creative_mode": SINGLE_GENERATION_CREATIVE_MODE_VALUES,
                "story_focus": SINGLE_GENERATION_STORY_FOCUS_VALUES,
                "plot_stage": SINGLE_GENERATION_PLOT_STAGE_VALUES,
                "quality_preset": SINGLE_GENERATION_QUALITY_PRESET_VALUES
            }
        },
        "validation_boundary": [
            "cargo test chapter_single_generation_prepare_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "rollback_boundary": "chapter_generation_prerequisite_service_python_source_map"
    })
}

pub(crate) async fn check_chapter_generation_prerequisites(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
) -> Result<ChapterGenerationPrerequisiteCheck, String> {
    if chapter_model.chapter_number == 1 {
        return Ok(ChapterGenerationPrerequisiteCheck {
            can_generate: true,
            error_message: String::new(),
            previous_chapters: Vec::new(),
        });
    }

    let previous_chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&chapter_model.project_id))
        .filter(chapter::Column::ChapterNumber.lt(chapter_model.chapter_number))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let incomplete_numbers = previous_chapters
        .iter()
        .filter(|chapter| {
            chapter
                .content
                .as_ref()
                .map(|content| content.trim().is_empty())
                .unwrap_or(true)
        })
        .map(|chapter| chapter.chapter_number.to_string())
        .collect::<Vec<_>>();

    if !incomplete_numbers.is_empty() {
        return Ok(ChapterGenerationPrerequisiteCheck {
            can_generate: false,
            error_message: format!("前置章节尚未完成: {} 章", incomplete_numbers.join(", ")),
            previous_chapters,
        });
    }

    Ok(ChapterGenerationPrerequisiteCheck {
        can_generate: true,
        error_message: String::new(),
        previous_chapters,
    })
}

pub(crate) fn normalize_optional_single_generation_request_string(
    value: Option<String>,
) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn is_valid_optional_choice(value: Option<&str>, allowed_values: &[&str]) -> bool {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| allowed_values.contains(&value))
        .unwrap_or(true)
}

pub(crate) fn is_valid_optional_text_length(value: Option<&str>, max_chars: usize) -> bool {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().count() <= max_chars)
        .unwrap_or(true)
}

pub(crate) fn normalize_single_generation_web_research_enabled(
    enabled: Option<bool>,
    default_enabled: bool,
) -> bool {
    enabled.unwrap_or(default_enabled)
}
