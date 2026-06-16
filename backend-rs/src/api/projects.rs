use axum::{
    extract::{Extension, Multipart, Path, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::{de, Deserialize, Deserializer};
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

mod import_workflow_owner;

use self::import_workflow_owner::{
    import_project_write_workflow, validate_project_import_payload,
    ImportProjectWriteWorkflowError, ValidateProjectImportPayloadError,
};
use crate::models::{
    career, chapter, character, character_career, generation_history, organization,
    organization_member, outline, plot_analysis, project, project_default_style, relationship,
    story_memory, writing_style,
};
use crate::services::auth::Claims;
use crate::services::project_service::ProjectService;

const MAX_STORY_CREATION_BRIEF_LEN: usize = 1200;
const MAX_QUALITY_NOTES_LEN: usize = 600;

const PROJECTS_LIST_CREATE_ROUTE: &str = "/projects";
const PROJECTS_DETAIL_ROUTE: &str = "/projects/{project_id}";
const PROJECTS_EXPORT_TXT_ROUTE: &str = "/projects/{project_id}/export";
const PROJECTS_EXPORT_DATA_ROUTE: &str = "/projects/{project_id}/export-data";
const PROJECTS_CHECK_CONSISTENCY_ROUTE: &str = "/projects/{project_id}/check-consistency";
const PROJECTS_FIX_ORGANIZATIONS_ROUTE: &str = "/projects/{project_id}/fix-organizations";
const PROJECTS_FIX_MEMBER_COUNTS_ROUTE: &str = "/projects/{project_id}/fix-member-counts";
const PROJECTS_VALIDATE_IMPORT_ROUTE: &str = "/projects/validate-import";
const PROJECTS_IMPORT_ROUTE: &str = "/projects/import";

#[cfg(test)]
fn build_projects_route_owner_contract() -> Value {
    json!({
        "owner": "projects",
        "rust_owner": "backend-rs/src/api/projects.rs",
        "route_prefix": "/api",
        "routes": {
            "list": PROJECTS_LIST_CREATE_ROUTE,
            "create": PROJECTS_LIST_CREATE_ROUTE,
            "detail": PROJECTS_DETAIL_ROUTE,
            "update": PROJECTS_DETAIL_ROUTE,
            "delete": PROJECTS_DETAIL_ROUTE,
            "export_txt": PROJECTS_EXPORT_TXT_ROUTE,
            "export_data": PROJECTS_EXPORT_DATA_ROUTE,
            "check_consistency": PROJECTS_CHECK_CONSISTENCY_ROUTE,
            "fix_organizations": PROJECTS_FIX_ORGANIZATIONS_ROUTE,
            "fix_member_counts": PROJECTS_FIX_MEMBER_COUNTS_ROUTE,
            "validate_import": PROJECTS_VALIDATE_IMPORT_ROUTE,
            "import": PROJECTS_IMPORT_ROUTE
        },
        "method_contract": {
            "list_create": ["GET", "POST"],
            "detail": ["GET", "PUT", "DELETE"],
            "export_txt": ["GET"],
            "export_data": ["POST"],
            "check_consistency": ["POST"],
            "fix_organizations": ["POST"],
            "fix_member_counts": ["POST"],
            "validate_import": ["POST"],
            "import": ["POST"]
        },
        "service_handoffs": {
            "crud_owner": "backend-rs/src/services/project_service.rs",
            "export_payload_owner": "backend-rs/src/api/projects.rs",
            "export_query_owner": "backend-rs/src/api/projects.rs",
            "import_workflow_owner": "backend-rs/src/api/projects/import_workflow_owner.rs",
            "consistency_workflow_owner": "backend-rs/src/api/projects.rs",
            "consistency_query_owner": "backend-rs/src/api/projects.rs"
        },
        "readiness_probes": [
            "projects-list-auth-guard-rust",
            "projects-detail-auth-guard-rust",
            "projects-create-auth-guard-rust",
            "projects-update-auth-guard-rust",
            "projects-delete-auth-guard-rust",
            "projects-export-txt-auth-guard-rust",
            "projects-validate-import-public-rust",
            "projects-import-auth-guard-rust",
            "projects-export-data-auth-guard-rust",
            "projects-check-consistency-auth-guard-rust",
            "projects-fix-organizations-auth-guard-rust",
            "projects-fix-member-counts-auth-guard-rust",
            "projects-create-business-rust",
            "projects-list-business-rust",
            "projects-detail-business-rust",
            "projects-update-business-rust",
            "projects-export-data-business-rust",
            "projects-check-consistency-business-rust",
            "projects-fix-organizations-business-rust",
            "projects-fix-member-counts-business-rust",
            "projects-delete-business-rust"
        ],
        "source_map_files": [
            "backend/app/api/projects.py",
            "backend/app/models/project.py",
            "backend/app/models/project_default_style.py",
            "backend/app/schemas/project.py"
        ],
        "owner_profile": {
            "name": "phase5-projects-business-owner",
            "business_probes": [
                "projects-create-business-rust",
                "projects-list-business-rust",
                "projects-detail-business-rust",
                "projects-update-business-rust",
                "projects-export-data-business-rust",
                "projects-check-consistency-business-rust",
                "projects-fix-organizations-business-rust",
                "projects-fix-member-counts-business-rust",
                "projects-delete-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "rollback_boundary": {
            "source_map_policy": "keep_python_projects_route_model_schema_files_as_source_map_until_explicit_freeze_delete_round",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": false,
            "remaining_blockers": [
                "explicit source-map freeze/delete/repoint approval"
            ],
            "freeze_reason": "Rust projects route group has dedicated phase5-projects-business-owner probes for CRUD, export-data, consistency check, organization/member-count fixes, and delete; final Python source-map freeze/delete/repoint still requires explicit approval and rollback policy."
        },
        "business_smoke_status": {
            "owner_profile": "phase5-projects-business-owner",
            "business_probe_count": 9,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Projects route business smoke is covered by phase5-projects-business-owner; final completion now requires explicit source-map freeze/delete/repoint approval with same-round rollback policy."
    })
}

#[derive(Deserialize)]
enum ProjectOutlineMode {
    #[serde(rename = "one-to-one")]
    OneToOne,
    #[serde(rename = "one-to-many")]
    OneToMany,
}

#[derive(Deserialize)]
enum CreativeModePreference {
    #[serde(rename = "balanced")]
    Balanced,
    #[serde(rename = "hook")]
    Hook,
    #[serde(rename = "emotion")]
    Emotion,
    #[serde(rename = "suspense")]
    Suspense,
    #[serde(rename = "relationship")]
    Relationship,
    #[serde(rename = "payoff")]
    Payoff,
}

#[derive(Deserialize)]
enum StoryFocusPreference {
    #[serde(rename = "advance_plot")]
    AdvancePlot,
    #[serde(rename = "deepen_character")]
    DeepenCharacter,
    #[serde(rename = "escalate_conflict")]
    EscalateConflict,
    #[serde(rename = "reveal_mystery")]
    RevealMystery,
    #[serde(rename = "relationship_shift")]
    RelationshipShift,
    #[serde(rename = "foreshadow_payoff")]
    ForeshadowPayoff,
}

#[derive(Deserialize)]
enum PlotStagePreference {
    #[serde(rename = "development")]
    Development,
    #[serde(rename = "climax")]
    Climax,
    #[serde(rename = "ending")]
    Ending,
}

#[derive(Deserialize)]
enum QualityPresetPreference {
    #[serde(rename = "balanced")]
    Balanced,
    #[serde(rename = "plot_drive")]
    PlotDrive,
    #[serde(rename = "immersive")]
    Immersive,
    #[serde(rename = "emotion_drama")]
    EmotionDrama,
    #[serde(rename = "clean_prose")]
    CleanProse,
}

impl ProjectOutlineMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::OneToOne => "one-to-one",
            Self::OneToMany => "one-to-many",
        }
    }
}

impl CreativeModePreference {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Hook => "hook",
            Self::Emotion => "emotion",
            Self::Suspense => "suspense",
            Self::Relationship => "relationship",
            Self::Payoff => "payoff",
        }
    }
}

impl StoryFocusPreference {
    fn as_str(&self) -> &'static str {
        match self {
            Self::AdvancePlot => "advance_plot",
            Self::DeepenCharacter => "deepen_character",
            Self::EscalateConflict => "escalate_conflict",
            Self::RevealMystery => "reveal_mystery",
            Self::RelationshipShift => "relationship_shift",
            Self::ForeshadowPayoff => "foreshadow_payoff",
        }
    }
}

impl PlotStagePreference {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Climax => "climax",
            Self::Ending => "ending",
        }
    }
}

impl QualityPresetPreference {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::PlotDrive => "plot_drive",
            Self::Immersive => "immersive",
            Self::EmotionDrama => "emotion_drama",
            Self::CleanProse => "clean_prose",
        }
    }
}

fn deserialize_optional_creative_mode<'de, D>(
    deserializer: D,
) -> Result<Option<CreativeModePreference>, D::Error>
where
    D: Deserializer<'de>,
{
    match optional_trimmed_string(deserializer)? {
        Some(value) => match value.as_str() {
            "balanced" => Ok(Some(CreativeModePreference::Balanced)),
            "hook" => Ok(Some(CreativeModePreference::Hook)),
            "emotion" => Ok(Some(CreativeModePreference::Emotion)),
            "suspense" => Ok(Some(CreativeModePreference::Suspense)),
            "relationship" => Ok(Some(CreativeModePreference::Relationship)),
            "payoff" => Ok(Some(CreativeModePreference::Payoff)),
            _ => Err(de::Error::unknown_variant(
                &value,
                &[
                    "balanced",
                    "hook",
                    "emotion",
                    "suspense",
                    "relationship",
                    "payoff",
                ],
            )),
        },
        None => Ok(None),
    }
}

fn deserialize_optional_story_focus<'de, D>(
    deserializer: D,
) -> Result<Option<StoryFocusPreference>, D::Error>
where
    D: Deserializer<'de>,
{
    match optional_trimmed_string(deserializer)? {
        Some(value) => match value.as_str() {
            "advance_plot" => Ok(Some(StoryFocusPreference::AdvancePlot)),
            "deepen_character" => Ok(Some(StoryFocusPreference::DeepenCharacter)),
            "escalate_conflict" => Ok(Some(StoryFocusPreference::EscalateConflict)),
            "reveal_mystery" => Ok(Some(StoryFocusPreference::RevealMystery)),
            "relationship_shift" => Ok(Some(StoryFocusPreference::RelationshipShift)),
            "foreshadow_payoff" => Ok(Some(StoryFocusPreference::ForeshadowPayoff)),
            _ => Err(de::Error::unknown_variant(
                &value,
                &[
                    "advance_plot",
                    "deepen_character",
                    "escalate_conflict",
                    "reveal_mystery",
                    "relationship_shift",
                    "foreshadow_payoff",
                ],
            )),
        },
        None => Ok(None),
    }
}

fn deserialize_optional_plot_stage<'de, D>(
    deserializer: D,
) -> Result<Option<PlotStagePreference>, D::Error>
where
    D: Deserializer<'de>,
{
    match optional_trimmed_string(deserializer)? {
        Some(value) => match value.as_str() {
            "development" => Ok(Some(PlotStagePreference::Development)),
            "climax" => Ok(Some(PlotStagePreference::Climax)),
            "ending" => Ok(Some(PlotStagePreference::Ending)),
            _ => Err(de::Error::unknown_variant(
                &value,
                &["development", "climax", "ending"],
            )),
        },
        None => Ok(None),
    }
}

fn deserialize_optional_quality_preset<'de, D>(
    deserializer: D,
) -> Result<Option<QualityPresetPreference>, D::Error>
where
    D: Deserializer<'de>,
{
    match optional_trimmed_string(deserializer)? {
        Some(value) => match value.as_str() {
            "balanced" => Ok(Some(QualityPresetPreference::Balanced)),
            "plot_drive" => Ok(Some(QualityPresetPreference::PlotDrive)),
            "immersive" => Ok(Some(QualityPresetPreference::Immersive)),
            "emotion_drama" => Ok(Some(QualityPresetPreference::EmotionDrama)),
            "clean_prose" => Ok(Some(QualityPresetPreference::CleanProse)),
            _ => Err(de::Error::unknown_variant(
                &value,
                &[
                    "balanced",
                    "plot_drive",
                    "immersive",
                    "emotion_drama",
                    "clean_prose",
                ],
            )),
        },
        None => Ok(None),
    }
}

fn deserialize_optional_story_creation_brief<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_trimmed_text::<D, MAX_STORY_CREATION_BRIEF_LEN>(deserializer)
}

fn deserialize_optional_quality_notes<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_optional_trimmed_text::<D, MAX_QUALITY_NOTES_LEN>(deserializer)
}

fn deserialize_optional_non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn deserialize_optional_trimmed_text<'de, D, const MAX_LEN: usize>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(value) = optional_trimmed_string(deserializer)? else {
        return Ok(None);
    };
    if value.chars().count() > MAX_LEN {
        return Err(de::Error::custom(format!(
            "string should have at most {MAX_LEN} characters"
        )));
    }
    Ok(Some(value))
}

fn optional_trimmed_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(|value| {
        value.and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

fn default_true() -> bool {
    true
}

fn format_export_datetime(value: NaiveDateTime) -> String {
    value.format("%Y-%m-%dT%H:%M:%S").to_string()
}

fn format_optional_export_datetime(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(format_export_datetime)
}

fn current_export_time() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string()
}

#[derive(Debug, Clone)]
pub struct ProjectExportOptions {
    pub include_generation_history: bool,
    pub include_writing_styles: bool,
    pub include_careers: bool,
    pub include_memories: bool,
    pub include_plot_analysis: bool,
}

#[derive(Debug, Clone)]
pub struct ProjectExportContext {
    pub project: project::Model,
    pub chapters: Vec<chapter::Model>,
    pub characters: Vec<character::Model>,
    pub outlines: Vec<outline::Model>,
    pub relationships: Vec<relationship::Model>,
    pub organizations: Vec<organization::Model>,
    pub organization_members: Vec<organization_member::Model>,
    pub writing_styles: Vec<writing_style::Model>,
    pub generation_history: Vec<generation_history::Model>,
    pub careers: Vec<career::Model>,
    pub character_careers: Vec<character_career::Model>,
    pub story_memories: Vec<story_memory::Model>,
    pub plot_analysis: Vec<plot_analysis::Model>,
    pub project_default_style: Option<project_default_style::Model>,
    pub project_default_style_style: Option<writing_style::Model>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadProjectExportContextError {
    Context(ProjectQueryContextError),
    ProjectHasNoChapters,
}

async fn load_project_export_context(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    options: &ProjectExportOptions,
) -> Result<ProjectExportContext, LoadProjectExportContextError> {
    let project = ProjectService::get(db, project_id, user_id)
        .await
        .map_err(ProjectQueryContextError::Internal)
        .map_err(LoadProjectExportContextError::Context)?
        .ok_or(LoadProjectExportContextError::Context(
            ProjectQueryContextError::ProjectNotFound,
        ))?;

    let chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(db)
        .await
        .map_err(map_project_export_internal_error)?;

    let characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(map_project_export_internal_error)?;

    let outlines = outline::Entity::find()
        .filter(outline::Column::ProjectId.eq(project_id))
        .order_by_asc(outline::Column::OrderIndex)
        .all(db)
        .await
        .map_err(map_project_export_internal_error)?;

    let relationships = relationship::Entity::find()
        .filter(relationship::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(map_project_export_internal_error)?;

    let organizations = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(map_project_export_internal_error)?;

    let organization_ids = organizations
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let organization_members = if organization_ids.is_empty() {
        Vec::new()
    } else {
        organization_member::Entity::find()
            .filter(organization_member::Column::OrganizationId.is_in(organization_ids))
            .all(db)
            .await
            .map_err(map_project_export_internal_error)?
    };

    let writing_styles = if options.include_writing_styles {
        writing_style::Entity::find()
            .filter(writing_style::Column::UserId.eq(project.user_id.clone()))
            .order_by_asc(writing_style::Column::OrderIndex)
            .all(db)
            .await
            .map_err(map_project_export_internal_error)?
    } else {
        Vec::new()
    };

    let generation_history = if options.include_generation_history {
        generation_history::Entity::find()
            .filter(generation_history::Column::ProjectId.eq(project_id))
            .order_by_desc(generation_history::Column::CreatedAt)
            .limit(100)
            .all(db)
            .await
            .map_err(map_project_export_internal_error)?
    } else {
        Vec::new()
    };

    let careers = if options.include_careers {
        career::Entity::find()
            .filter(career::Column::ProjectId.eq(project_id))
            .order_by_asc(career::Column::CareerType)
            .order_by_asc(career::Column::CreatedAt)
            .all(db)
            .await
            .map_err(map_project_export_internal_error)?
    } else {
        Vec::new()
    };

    let character_ids = characters
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let character_careers = if options.include_careers && !character_ids.is_empty() {
        character_career::Entity::find()
            .filter(character_career::Column::CharacterId.is_in(character_ids))
            .all(db)
            .await
            .map_err(map_project_export_internal_error)?
    } else {
        Vec::new()
    };

    let story_memories = if options.include_memories {
        story_memory::Entity::find()
            .filter(story_memory::Column::ProjectId.eq(project_id))
            .order_by_asc(story_memory::Column::StoryTimeline)
            .order_by_asc(story_memory::Column::ChapterPosition)
            .all(db)
            .await
            .map_err(map_project_export_internal_error)?
    } else {
        Vec::new()
    };

    let plot_analysis = if options.include_plot_analysis {
        plot_analysis::Entity::find()
            .filter(plot_analysis::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(map_project_export_internal_error)?
    } else {
        Vec::new()
    };

    let project_default_style = project_default_style::Entity::find()
        .filter(project_default_style::Column::ProjectId.eq(project_id))
        .one(db)
        .await
        .map_err(map_project_export_internal_error)?;
    let project_default_style_style = if let Some(default_style) = project_default_style.as_ref() {
        writing_style::Entity::find_by_id(default_style.style_id)
            .one(db)
            .await
            .map_err(map_project_export_internal_error)?
    } else {
        None
    };

    Ok(ProjectExportContext {
        project,
        chapters,
        characters,
        outlines,
        relationships,
        organizations,
        organization_members,
        writing_styles,
        generation_history,
        careers,
        character_careers,
        story_memories,
        plot_analysis,
        project_default_style,
        project_default_style_style,
    })
}

async fn load_project_export_context_with_non_empty_chapters(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<ProjectExportContext, LoadProjectExportContextError> {
    let context = load_project_export_context(
        db,
        project_id,
        user_id,
        &ProjectExportOptions {
            include_generation_history: false,
            include_writing_styles: false,
            include_careers: false,
            include_memories: false,
            include_plot_analysis: false,
        },
    )
    .await?;
    if context.chapters.is_empty() {
        return Err(LoadProjectExportContextError::ProjectHasNoChapters);
    }

    Ok(context)
}

fn map_project_export_internal_error(error: sea_orm::DbErr) -> LoadProjectExportContextError {
    LoadProjectExportContextError::Context(ProjectQueryContextError::Internal(error.to_string()))
}

fn build_project_export_data_payload(
    context: &ProjectExportContext,
    options: &ProjectExportOptions,
) -> Value {
    let outline_title_mapping = context
        .outlines
        .iter()
        .map(|outline| (outline.id.clone(), outline.title.clone()))
        .collect::<HashMap<_, _>>();
    let chapter_title_mapping = context
        .chapters
        .iter()
        .map(|chapter| (chapter.id.clone(), chapter.title.clone()))
        .collect::<HashMap<_, _>>();
    let character_name_mapping = context
        .characters
        .iter()
        .map(|character| (character.id.clone(), character.name.clone()))
        .collect::<HashMap<_, _>>();
    let organization_by_id = context
        .organizations
        .iter()
        .map(|organization| (organization.id.clone(), organization))
        .collect::<HashMap<_, _>>();
    let character_by_id = context
        .characters
        .iter()
        .map(|character| (character.id.clone(), character))
        .collect::<HashMap<_, _>>();
    let career_name_mapping = context
        .careers
        .iter()
        .map(|career| (career.id.clone(), career.name.clone()))
        .collect::<HashMap<_, _>>();
    let style_name_mapping = context
        .writing_styles
        .iter()
        .map(|style| (style.id, style.name.clone()))
        .collect::<HashMap<_, _>>();

    json!({
        "version": "1.1.0",
        "export_time": current_export_time(),
        "project": build_project_payload(&context.project),
        "chapters": context
            .chapters
            .iter()
            .map(|chapter| build_chapter_payload(chapter, &outline_title_mapping))
            .collect::<Vec<_>>(),
        "characters": context
            .characters
            .iter()
            .map(build_character_payload)
            .collect::<Vec<_>>(),
        "outlines": context
            .outlines
            .iter()
            .map(build_outline_payload)
            .collect::<Vec<_>>(),
        "relationships": context
            .relationships
            .iter()
            .filter_map(|relationship| {
                build_relationship_payload(relationship, &character_name_mapping)
            })
            .collect::<Vec<_>>(),
        "organizations": context
            .organizations
            .iter()
            .filter_map(|organization| {
                build_organization_payload(organization, &organization_by_id, &character_by_id)
            })
            .collect::<Vec<_>>(),
        "organization_members": context
            .organization_members
            .iter()
            .filter_map(|member| {
                build_organization_member_payload(member, &organization_by_id, &character_by_id)
            })
            .collect::<Vec<_>>(),
        "writing_styles": if options.include_writing_styles {
            context
                .writing_styles
                .iter()
                .filter(|style| style.user_id.as_ref() == Some(&context.project.user_id))
                .map(build_writing_style_payload)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        "generation_history": if options.include_generation_history {
            context
                .generation_history
                .iter()
                .map(|history| build_generation_history_payload(history, &chapter_title_mapping))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        "careers": if options.include_careers {
            context
                .careers
                .iter()
                .map(build_career_payload)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        "character_careers": if options.include_careers {
            context
                .character_careers
                .iter()
                .filter_map(|item| {
                    build_character_career_payload(
                        item,
                        &character_name_mapping,
                        &career_name_mapping,
                    )
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        "story_memories": if options.include_memories {
            context
                .story_memories
                .iter()
                .map(|memory| {
                    build_story_memory_payload(
                        memory,
                        &chapter_title_mapping,
                        &character_name_mapping,
                    )
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        "plot_analysis": if options.include_plot_analysis {
            context
                .plot_analysis
                .iter()
                .filter_map(|analysis| {
                    build_plot_analysis_payload(analysis, &chapter_title_mapping)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        },
        "project_default_style": build_project_default_style_payload(
            context.project_default_style_style.as_ref(),
            &style_name_mapping,
            context.project_default_style.as_ref().map(|item| item.style_id),
        ),
    })
}

fn build_project_payload(project: &crate::models::project::Model) -> Value {
    json!({
        "title": project.title,
        "description": project.description,
        "theme": project.theme,
        "genre": project.genre,
        "target_words": project.target_words,
        "current_words": project.current_words,
        "status": project.status,
        "world_time_period": project.world_time_period,
        "world_location": project.world_location,
        "world_atmosphere": project.world_atmosphere,
        "world_rules": project.world_rules,
        "chapter_count": project.chapter_count,
        "narrative_perspective": project.narrative_perspective,
        "character_count": project.character_count,
        "outline_mode": project.outline_mode,
        "user_id": project.user_id,
        "created_at": format_export_datetime(project.created_at),
        "wizard_status": project.wizard_status,
        "wizard_step": project.wizard_step,
        "default_creative_mode": project.default_creative_mode,
        "default_story_focus": project.default_story_focus,
        "default_plot_stage": project.default_plot_stage,
        "default_story_creation_brief": project.default_story_creation_brief,
        "default_quality_preset": project.default_quality_preset,
        "default_quality_notes": project.default_quality_notes,
    })
}

fn build_chapter_payload(
    chapter: &crate::models::chapter::Model,
    outline_title_mapping: &HashMap<String, String>,
) -> Value {
    json!({
        "title": chapter.title,
        "content": chapter.content,
        "summary": chapter.summary,
        "chapter_number": chapter.chapter_number,
        "word_count": chapter.word_count,
        "status": chapter.status,
        "created_at": format_export_datetime(chapter.created_at),
        "outline_title": chapter
            .outline_id
            .as_ref()
            .and_then(|outline_id| outline_title_mapping.get(outline_id))
            .cloned(),
        "sub_index": chapter.sub_index,
        "expansion_plan": parse_json_text(chapter.expansion_plan.as_deref()),
    })
}

fn build_character_payload(character: &crate::models::character::Model) -> Value {
    json!({
        "name": character.name,
        "age": character.age,
        "gender": character.gender,
        "is_organization": character.is_organization,
        "role_type": character.role_type,
        "personality": character.personality,
        "background": character.background,
        "appearance": character.appearance,
        "traits": parse_json_text(character.traits.as_deref()),
        "organization_type": character.organization_type,
        "organization_purpose": character.organization_purpose,
        "avatar_url": character.avatar_url,
        "main_career_id": character.main_career_id,
        "main_career_stage": character.main_career_stage,
        "sub_careers": character.sub_careers,
        "created_at": format_export_datetime(character.created_at),
    })
}

fn build_outline_payload(outline: &crate::models::outline::Model) -> Value {
    json!({
        "title": outline.title,
        "content": outline.content,
        "structure": outline.structure,
        "order_index": outline.order_index,
        "created_at": format_export_datetime(outline.created_at),
    })
}

fn build_relationship_payload(
    relationship: &crate::models::relationship::Model,
    character_name_mapping: &HashMap<String, String>,
) -> Option<Value> {
    let source_name = character_name_mapping
        .get(&relationship.character_from_id)
        .cloned()?;
    let target_name = character_name_mapping
        .get(&relationship.character_to_id)
        .cloned()?;

    Some(json!({
        "source_name": source_name,
        "target_name": target_name,
        "relationship_name": relationship.relationship_name,
        "intimacy_level": relationship.intimacy_level,
        "status": relationship.status,
        "description": relationship.description,
        "started_at": relationship.started_at,
    }))
}

fn build_organization_payload(
    organization: &crate::models::organization::Model,
    organization_by_id: &HashMap<String, &crate::models::organization::Model>,
    character_by_id: &HashMap<String, &crate::models::character::Model>,
) -> Option<Value> {
    let org_character = character_by_id.get(&organization.character_id)?;
    let parent_org_name = organization
        .parent_org_id
        .as_ref()
        .and_then(|parent_id| organization_by_id.get(parent_id))
        .and_then(|parent_org| character_by_id.get(&parent_org.character_id))
        .map(|character| character.name.clone());

    Some(json!({
        "character_name": org_character.name,
        "parent_org_name": parent_org_name,
        "power_level": organization.power_level,
        "member_count": organization.member_count,
        "location": organization.location,
        "motto": organization.motto,
        "color": organization.color,
    }))
}

fn build_organization_member_payload(
    member: &crate::models::organization_member::Model,
    organization_by_id: &HashMap<String, &crate::models::organization::Model>,
    character_by_id: &HashMap<String, &crate::models::character::Model>,
) -> Option<Value> {
    let organization = organization_by_id.get(&member.organization_id)?;
    let organization_character = character_by_id.get(&organization.character_id)?;
    let member_character = character_by_id.get(&member.character_id)?;

    Some(json!({
        "organization_name": organization_character.name,
        "character_name": member_character.name,
        "position": member.position,
        "rank": member.rank,
        "status": member.status,
        "joined_at": member.joined_at,
        "loyalty": member.loyalty,
        "contribution": member.contribution,
        "notes": member.notes,
    }))
}

fn build_writing_style_payload(style: &crate::models::writing_style::Model) -> Value {
    json!({
        "name": style.name,
        "style_type": style.style_type,
        "preset_id": style.preset_id,
        "description": style.description,
        "prompt_content": style.prompt_content,
        "order_index": style.order_index,
    })
}

fn build_generation_history_payload(
    history: &crate::models::generation_history::Model,
    chapter_title_mapping: &HashMap<String, String>,
) -> Value {
    json!({
        "chapter_title": history
            .chapter_id
            .as_ref()
            .and_then(|chapter_id| chapter_title_mapping.get(chapter_id))
            .cloned(),
        "prompt": history.prompt,
        "generated_content": history.generated_content,
        "model": history.model,
        "tokens_used": history.tokens_used,
        "generation_time": history.generation_time,
        "created_at": format_optional_export_datetime(history.created_at),
    })
}

fn build_career_payload(career: &crate::models::career::Model) -> Value {
    json!({
        "name": career.name,
        "type": career.career_type,
        "description": career.description,
        "category": career.category,
        "stages": career.stages,
        "max_stage": career.max_stage,
        "requirements": career.requirements,
        "special_abilities": career.special_abilities,
        "worldview_rules": career.worldview_rules,
        "attribute_bonuses": career.attribute_bonuses,
        "source": career.source,
        "created_at": format_export_datetime(career.created_at),
    })
}

fn build_character_career_payload(
    item: &crate::models::character_career::Model,
    character_name_mapping: &HashMap<String, String>,
    career_name_mapping: &HashMap<String, String>,
) -> Option<Value> {
    Some(json!({
        "character_name": character_name_mapping.get(&item.character_id)?.clone(),
        "career_name": career_name_mapping.get(&item.career_id)?.clone(),
        "career_type": item.career_type,
        "current_stage": item.current_stage,
        "stage_progress": item.stage_progress.unwrap_or(0),
        "started_at": item.started_at,
        "reached_current_stage_at": item.reached_current_stage_at,
        "notes": item.notes,
    }))
}

fn build_story_memory_payload(
    memory: &crate::models::story_memory::Model,
    chapter_title_mapping: &HashMap<String, String>,
    character_name_mapping: &HashMap<String, String>,
) -> Value {
    let related_characters = memory.related_characters.as_ref().and_then(|value| {
        value.as_array().map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(|character_id| {
                    character_name_mapping
                        .get(character_id)
                        .cloned()
                        .unwrap_or_else(|| character_id.to_string())
                })
                .collect::<Vec<_>>()
        })
    });

    json!({
        "chapter_title": memory
            .chapter_id
            .as_ref()
            .and_then(|chapter_id| chapter_title_mapping.get(chapter_id))
            .cloned(),
        "memory_type": memory.memory_type,
        "title": memory.title,
        "content": memory.content,
        "full_context": memory.full_context,
        "related_characters": related_characters,
        "related_locations": memory.related_locations,
        "tags": memory.tags,
        "importance_score": memory.importance_score.unwrap_or(0.5),
        "story_timeline": memory.story_timeline,
        "chapter_position": memory.chapter_position,
        "text_length": memory.text_length,
        "is_foreshadow": memory.is_foreshadow,
        "foreshadow_strength": memory.foreshadow_strength,
        "created_at": format_optional_export_datetime(memory.created_at),
    })
}

fn build_plot_analysis_payload(
    analysis: &crate::models::plot_analysis::Model,
    chapter_title_mapping: &HashMap<String, String>,
) -> Option<Value> {
    let chapter_title = chapter_title_mapping.get(&analysis.chapter_id).cloned()?;

    Some(json!({
        "chapter_title": chapter_title,
        "plot_stage": analysis.plot_stage,
        "conflict_level": analysis.conflict_level,
        "conflict_types": analysis.conflict_types,
        "emotional_tone": analysis.emotional_tone,
        "emotional_intensity": analysis.emotional_intensity,
        "emotional_curve": analysis.emotional_curve,
        "hooks": analysis.hooks,
        "hooks_count": analysis.hooks_count,
        "hooks_avg_strength": analysis.hooks_avg_strength,
        "foreshadows": analysis.foreshadows,
        "foreshadows_planted": analysis.foreshadows_planted,
        "foreshadows_resolved": analysis.foreshadows_resolved,
        "plot_points": analysis.plot_points,
        "plot_points_count": analysis.plot_points_count,
        "character_states": analysis.character_states,
        "scenes": analysis.scenes,
        "pacing": analysis.pacing,
        "overall_quality_score": analysis.overall_quality_score,
        "pacing_score": analysis.pacing_score,
        "engagement_score": analysis.engagement_score,
        "coherence_score": analysis.coherence_score,
        "analysis_report": analysis.analysis_report,
        "suggestions": analysis.suggestions,
        "word_count": analysis.word_count,
        "dialogue_ratio": analysis.dialogue_ratio,
        "description_ratio": analysis.description_ratio,
        "created_at": format_optional_export_datetime(analysis.created_at),
    }))
}

fn build_project_default_style_payload(
    style: Option<&crate::models::writing_style::Model>,
    style_name_mapping: &HashMap<i32, String>,
    style_id: Option<i32>,
) -> Value {
    if let Some(style) = style {
        json!({ "style_name": style.name })
    } else if let Some(style_id) = style_id {
        json!({ "style_name": style_name_mapping.get(&style_id).cloned() })
    } else {
        Value::Null
    }
}

fn parse_json_text(raw: Option<&str>) -> Option<Value> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }

    serde_json::from_str(raw).ok()
}

fn build_project_export_txt_content(
    project: &crate::models::project::Model,
    chapters: &[crate::models::chapter::Model],
) -> String {
    let mut text = String::new();
    text.push_str(&format!("项目：{}\n", project.title));
    if let Some(ref desc) = project.description {
        if !desc.is_empty() {
            text.push_str(&format!("简介：{}\n", desc));
        }
    }
    if let Some(ref theme) = project.theme {
        if !theme.is_empty() {
            text.push_str(&format!("主题：{}\n", theme));
        }
    }
    if let Some(ref genre) = project.genre {
        if !genre.is_empty() {
            text.push_str(&format!("类型：{}\n", genre));
        }
    }
    text.push_str("\n\n");

    for ch in chapters {
        text.push_str(&format!("第 {} 章：{}\n\n", ch.chapter_number, ch.title));
        if let Some(ref content) = ch.content {
            text.push_str(content);
        }
        text.push_str("\n\n---\n\n");
    }

    text
}

fn build_safe_project_export_json_filename(title: &str) -> String {
    let safe_title: String = title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == ' ' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    format!("project_{}.json", safe_title.trim().replace(' ', "_"))
}

fn build_safe_project_export_txt_filename(title: &str) -> String {
    let safe_title: String = title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    format!("{}.txt", safe_title)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateRequest {
    title: String,
    description: Option<String>,
    theme: Option<String>,
    genre: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_non_null")]
    outline_mode: Option<ProjectOutlineMode>,
    target_words: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_optional_creative_mode")]
    default_creative_mode: Option<CreativeModePreference>,
    #[serde(default, deserialize_with = "deserialize_optional_story_focus")]
    default_story_focus: Option<StoryFocusPreference>,
    #[serde(default, deserialize_with = "deserialize_optional_plot_stage")]
    default_plot_stage: Option<PlotStagePreference>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_story_creation_brief"
    )]
    default_story_creation_brief: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_quality_preset")]
    default_quality_preset: Option<QualityPresetPreference>,
    #[serde(default, deserialize_with = "deserialize_optional_quality_notes")]
    default_quality_notes: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateRequest {
    title: Option<String>,
    description: Option<String>,
    theme: Option<String>,
    genre: Option<String>,
    status: Option<String>,
    target_words: Option<i32>,
    world_time_period: Option<String>,
    world_location: Option<String>,
    world_atmosphere: Option<String>,
    world_rules: Option<String>,
    chapter_count: Option<i32>,
    narrative_perspective: Option<String>,
    character_count: Option<i32>,
    #[serde(default, deserialize_with = "deserialize_optional_creative_mode")]
    default_creative_mode: Option<CreativeModePreference>,
    #[serde(default, deserialize_with = "deserialize_optional_story_focus")]
    default_story_focus: Option<StoryFocusPreference>,
    #[serde(default, deserialize_with = "deserialize_optional_plot_stage")]
    default_plot_stage: Option<PlotStagePreference>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_story_creation_brief"
    )]
    default_story_creation_brief: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_quality_preset")]
    default_quality_preset: Option<QualityPresetPreference>,
    #[serde(default, deserialize_with = "deserialize_optional_quality_notes")]
    default_quality_notes: Option<String>,
}

#[derive(Deserialize)]
struct ExportOptions {
    #[serde(default)]
    include_generation_history: bool,
    #[serde(default = "default_true")]
    include_writing_styles: bool,
    #[serde(default = "default_true")]
    include_careers: bool,
    #[serde(default)]
    include_memories: bool,
    #[serde(default)]
    include_plot_analysis: bool,
}

#[derive(Deserialize)]
struct ListQuery {
    skip: Option<i64>,
    limit: Option<i64>,
}

#[derive(Deserialize)]
struct CheckProjectConsistencyQuery {
    #[serde(default)]
    auto_fix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProjectQueryContextError {
    ProjectNotFound,
    Internal(String),
}

type LoadProjectConsistencyContextError = ProjectQueryContextError;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectConsistencyCounts {
    organization_character_total: usize,
    organization_total: usize,
}

#[derive(Debug)]
enum ProjectConsistencyWriteWorkflowError {
    Context(LoadProjectConsistencyContextError),
    Internal(String),
}

fn normalize_project_consistency_auto_fix(raw: Option<&str>) -> bool {
    raw.map(|value| value != "false" && value != "0")
        .unwrap_or(true)
}

async fn ensure_project_consistency_access(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<(), LoadProjectConsistencyContextError> {
    ProjectService::get(db, project_id, user_id)
        .await
        .map_err(ProjectQueryContextError::Internal)?
        .ok_or(ProjectQueryContextError::ProjectNotFound)?;
    Ok(())
}

async fn load_project_consistency_counts(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<ProjectConsistencyCounts, LoadProjectConsistencyContextError> {
    ensure_project_consistency_access(db, project_id, user_id).await?;

    let organization_character_total = character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .filter(character::Column::IsOrganization.eq(true))
        .count(db)
        .await
        .map_err(|error| ProjectQueryContextError::Internal(error.to_string()))?
        as usize;

    let organization_total = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(project_id))
        .count(db)
        .await
        .map_err(|error| ProjectQueryContextError::Internal(error.to_string()))?
        as usize;

    Ok(ProjectConsistencyCounts {
        organization_character_total,
        organization_total,
    })
}

async fn fix_missing_organization_records(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<(usize, usize), String> {
    let organization_characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .filter(character::Column::IsOrganization.eq(true))
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let mut fixed = 0usize;
    for character_model in &organization_characters {
        let existing = organization::Entity::find()
            .filter(organization::Column::CharacterId.eq(&character_model.id))
            .one(db)
            .await
            .map_err(|error| error.to_string())?;
        if existing.is_some() {
            continue;
        }

        let now = Utc::now().naive_utc();
        organization::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            character_id: Set(character_model.id.clone()),
            project_id: Set(project_id.to_string()),
            parent_org_id: Set(None),
            level: Set(1),
            power_level: Set(50),
            member_count: Set(0),
            location: Set(None),
            motto: Set(None),
            color: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .map_err(|error| error.to_string())?;
        fixed += 1;
    }

    Ok((fixed, organization_characters.len()))
}

async fn fix_organization_member_counts(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<(usize, usize), String> {
    let organizations = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let mut fixed = 0usize;
    for organization_model in &organizations {
        let actual_count = organization_member::Entity::find()
            .filter(organization_member::Column::OrganizationId.eq(&organization_model.id))
            .filter(organization_member::Column::Status.eq("active"))
            .count(db)
            .await
            .map_err(|error| error.to_string())? as i32;

        if organization_model.member_count == actual_count {
            continue;
        }

        let mut active_model: organization::ActiveModel = organization_model.clone().into();
        active_model.member_count = Set(actual_count);
        active_model.updated_at = Set(Some(Utc::now().naive_utc()));
        active_model
            .update(db)
            .await
            .map_err(|error| error.to_string())?;
        fixed += 1;
    }

    Ok((fixed, organizations.len()))
}

async fn fix_project_organizations_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, ProjectConsistencyWriteWorkflowError> {
    ensure_project_consistency_access(db, project_id, user_id)
        .await
        .map_err(ProjectConsistencyWriteWorkflowError::Context)?;

    let (fixed, total) = fix_missing_organization_records(db, project_id)
        .await
        .map_err(ProjectConsistencyWriteWorkflowError::Internal)?;

    Ok(json!({
        "message": "组织记录修复完成",
        "fixed": fixed,
        "total": total,
    }))
}

async fn fix_project_member_counts_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<Value, ProjectConsistencyWriteWorkflowError> {
    ensure_project_consistency_access(db, project_id, user_id)
        .await
        .map_err(ProjectConsistencyWriteWorkflowError::Context)?;

    let (fixed, total) = fix_organization_member_counts(db, project_id)
        .await
        .map_err(ProjectConsistencyWriteWorkflowError::Internal)?;

    Ok(json!({
        "message": "成员计数修复完成",
        "fixed": fixed,
        "total": total,
    }))
}

async fn check_project_consistency_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    auto_fix: bool,
) -> Result<Value, ProjectConsistencyWriteWorkflowError> {
    let counts_without_fix = if auto_fix {
        None
    } else {
        Some(
            load_project_consistency_counts(db, project_id, user_id)
                .await
                .map_err(ProjectConsistencyWriteWorkflowError::Context)?,
        )
    };

    let (organization_fixed, organization_total) = if auto_fix {
        ensure_project_consistency_access(db, project_id, user_id)
            .await
            .map_err(ProjectConsistencyWriteWorkflowError::Context)?;
        fix_missing_organization_records(db, project_id)
            .await
            .map_err(ProjectConsistencyWriteWorkflowError::Internal)?
    } else {
        let counts = counts_without_fix
            .as_ref()
            .expect("counts_without_fix should exist when auto_fix is false");
        (0, counts.organization_character_total)
    };

    let (member_fixed, member_total) = if auto_fix {
        fix_organization_member_counts(db, project_id)
            .await
            .map_err(ProjectConsistencyWriteWorkflowError::Internal)?
    } else {
        let counts = counts_without_fix
            .as_ref()
            .expect("counts_without_fix should exist when auto_fix is false");
        (0, counts.organization_total)
    };

    Ok(json!({
        "project_id": project_id,
        "checks": {
            "organization_records": {
                "checked": organization_total,
                "fixed": organization_fixed,
                "status": if organization_fixed == 0 { "ok" } else { "fixed" },
            },
            "member_counts": {
                "checked": member_total,
                "fixed": member_fixed,
                "status": if member_fixed == 0 { "ok" } else { "fixed" },
            },
        },
    }))
}

fn map_project_export_context_error(
    error: LoadProjectExportContextError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadProjectExportContextError::Context(error) => map_project_query_context_error(error),
        LoadProjectExportContextError::ProjectHasNoChapters => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project has no chapters"})),
        ),
    }
}

fn map_project_query_context_error(
    error: LoadProjectConsistencyContextError,
) -> (StatusCode, Json<Value>) {
    match error {
        LoadProjectConsistencyContextError::ProjectNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found"})),
        ),
        LoadProjectConsistencyContextError::Internal(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
    }
}

fn map_project_consistency_context_error(
    error: LoadProjectConsistencyContextError,
) -> (StatusCode, Json<Value>) {
    map_project_query_context_error(error)
}

fn map_validate_project_import_payload_error(
    error: ValidateProjectImportPayloadError,
) -> (StatusCode, Json<Value>) {
    match error {
        ValidateProjectImportPayloadError::InvalidJson(detail) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": format!("无效的JSON格式: {}", detail)})),
        ),
    }
}

fn map_import_project_write_workflow_error(
    error: ImportProjectWriteWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        ImportProjectWriteWorkflowError::PayloadTooLarge => (
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(json!({"detail": "文件大小超过50MB限制"})),
        ),
        ImportProjectWriteWorkflowError::InvalidJson(detail) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": format!("无效的JSON格式: {}", detail)})),
        ),
        ImportProjectWriteWorkflowError::Internal(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
    }
}

fn validate_project_import_filename(
    filename: Option<&str>,
) -> Result<(), (StatusCode, Json<Value>)> {
    match filename {
        Some(filename) if filename.ends_with(".json") => Ok(()),
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"detail": "只支持JSON格式文件"})),
        )),
    }
}

async fn read_project_import_file_from_multipart(
    multipart: &mut Multipart,
) -> Result<Vec<u8>, (StatusCode, Json<Value>)> {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            validate_project_import_filename(field.file_name())?;
            let bytes = field.bytes().await.map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"detail": format!("Failed to read uploaded file: {}", error)})),
                )
            })?;
            return Ok(bytes.to_vec());
        }
    }

    Err((
        StatusCode::BAD_REQUEST,
        Json(json!({"detail": "Missing file field"})),
    ))
}

fn map_project_consistency_write_workflow_error(
    error: ProjectConsistencyWriteWorkflowError,
) -> (StatusCode, Json<Value>) {
    match error {
        ProjectConsistencyWriteWorkflowError::Context(error) => {
            map_project_consistency_context_error(error)
        }
        ProjectConsistencyWriteWorkflowError::Internal(detail) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": detail})),
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProjectListQueryRequest {
    skip: usize,
    limit: usize,
}

impl ProjectListQueryRequest {
    fn from_route_query(query: &ListQuery) -> Self {
        Self {
            skip: non_negative_query_usize(query.skip, 0),
            limit: non_negative_query_usize(query.limit, 100),
        }
    }
}

fn non_negative_query_usize(value: Option<i64>, default_value: usize) -> usize {
    value
        .filter(|value| *value >= 0)
        .map(|value| value as usize)
        .unwrap_or(default_value)
}

fn build_project_list_payload(
    projects: Vec<crate::models::project::Model>,
    request: ProjectListQueryRequest,
) -> Value {
    let total = projects.len();
    let items = projects
        .into_iter()
        .skip(request.skip)
        .take(request.limit)
        .collect::<Vec<_>>();

    json!({
        "total": total,
        "items": items,
    })
}

async fn create_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match ProjectService::create(
        &db,
        &claims.sub,
        &body.title,
        body.description.as_deref(),
        body.theme.as_deref(),
        body.genre.as_deref(),
        body.outline_mode.as_ref().map(ProjectOutlineMode::as_str),
        body.target_words,
        body.default_creative_mode
            .as_ref()
            .map(CreativeModePreference::as_str),
        body.default_story_focus
            .as_ref()
            .map(StoryFocusPreference::as_str),
        body.default_plot_stage
            .as_ref()
            .map(PlotStagePreference::as_str),
        body.default_story_creation_brief.as_deref(),
        body.default_quality_preset
            .as_ref()
            .map(QualityPresetPreference::as_str),
        body.default_quality_notes.as_deref(),
    )
    .await
    {
        Ok(project) => Ok((StatusCode::CREATED, Json(json!(project)))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn list_projects(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = ProjectListQueryRequest::from_route_query(&query);
    match ProjectService::list(&db, &claims.sub).await {
        Ok(projects) => Ok(Json(build_project_list_payload(projects, request))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn get_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ProjectService::get(&db, &project_id, &claims.sub).await {
        Ok(Some(project)) => Ok(Json(json!(project))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Project not found"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn update_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ProjectService::update(
        &db,
        &project_id,
        &claims.sub,
        body.title.as_deref(),
        body.description.as_deref(),
        body.theme.as_deref(),
        body.genre.as_deref(),
        body.status.as_deref(),
        body.target_words,
        body.world_time_period.as_deref(),
        body.world_location.as_deref(),
        body.world_atmosphere.as_deref(),
        body.world_rules.as_deref(),
        body.chapter_count,
        body.narrative_perspective.as_deref(),
        body.character_count,
        body.default_creative_mode
            .as_ref()
            .map(CreativeModePreference::as_str),
        body.default_story_focus
            .as_ref()
            .map(StoryFocusPreference::as_str),
        body.default_plot_stage
            .as_ref()
            .map(PlotStagePreference::as_str),
        body.default_story_creation_brief.as_deref(),
        body.default_quality_preset
            .as_ref()
            .map(QualityPresetPreference::as_str),
        body.default_quality_notes.as_deref(),
    )
    .await
    {
        Ok(Some(project)) => Ok(Json(json!(project))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Project not found"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn delete_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match ProjectService::delete(&db, &project_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(
            json!({"success": true, "message": "Project deleted successfully"}),
        )),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "Project not found"})),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e})),
        )),
    }
}

async fn export_project_data(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Json(options): Json<ExportOptions>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let export_options = ProjectExportOptions {
        include_generation_history: options.include_generation_history,
        include_writing_styles: options.include_writing_styles,
        include_careers: options.include_careers,
        include_memories: options.include_memories,
        include_plot_analysis: options.include_plot_analysis,
    };
    let context = load_project_export_context(&db, &project_id, &claims.sub, &export_options)
        .await
        .map_err(map_project_export_context_error)?;
    let filename = build_safe_project_export_json_filename(&context.project.title);
    let export_payload = build_project_export_data_payload(&context, &export_options);
    let encoded_filename = filename.clone();
    let body = serde_json::to_vec_pretty(&export_payload).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"detail": format!("{}", e)})),
        )
    })?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename*=UTF-8''{}", encoded_filename),
        )
        .body(axum::body::Body::from(body))
        .unwrap())
}

async fn export_project_txt(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    let context =
        load_project_export_context_with_non_empty_chapters(&db, &project_id, &claims.sub)
            .await
            .map_err(map_project_export_context_error)?;
    let project = context.project;
    let chapters = context.chapters;

    let text = build_project_export_txt_content(&project, &chapters);
    let filename = build_safe_project_export_txt_filename(&project.title);
    let headers = [
        (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
        (
            header::CONTENT_DISPOSITION,
            &format!("attachment; filename=\"{}\"", filename),
        ),
    ];

    Ok((headers, text).into_response())
}

async fn validate_import(
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let file_data = read_project_import_file_from_multipart(&mut multipart).await?;
    let payload = validate_project_import_payload(&file_data)
        .map_err(map_validate_project_import_payload_error)?;

    Ok(Json(payload))
}

async fn import_project(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let file_data = read_project_import_file_from_multipart(&mut multipart).await?;
    let payload = import_project_write_workflow(&db, &claims.sub, &file_data)
        .await
        .map_err(map_import_project_write_workflow_error)?;

    Ok(Json(payload))
}

async fn fix_project_organizations(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = fix_project_organizations_write_workflow(&db, &project_id, &claims.sub)
        .await
        .map_err(map_project_consistency_write_workflow_error)?;

    Ok(Json(payload))
}

async fn fix_project_member_counts(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = fix_project_member_counts_write_workflow(&db, &project_id, &claims.sub)
        .await
        .map_err(map_project_consistency_write_workflow_error)?;

    Ok(Json(payload))
}

async fn check_project_consistency(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<CheckProjectConsistencyQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let payload = check_project_consistency_write_workflow(
        &db,
        &project_id,
        &claims.sub,
        normalize_project_consistency_auto_fix(query.auto_fix.as_deref()),
    )
    .await
    .map_err(map_project_consistency_write_workflow_error)?;

    Ok(Json(payload))
}

pub fn routes() -> Router {
    Router::new()
        .route(
            PROJECTS_LIST_CREATE_ROUTE,
            post(create_project).get(list_projects),
        )
        .route(
            PROJECTS_DETAIL_ROUTE,
            get(get_project).put(update_project).delete(delete_project),
        )
        .route(PROJECTS_EXPORT_TXT_ROUTE, get(export_project_txt))
        .route(PROJECTS_EXPORT_DATA_ROUTE, post(export_project_data))
        .route(
            PROJECTS_CHECK_CONSISTENCY_ROUTE,
            post(check_project_consistency),
        )
        .route(
            PROJECTS_FIX_ORGANIZATIONS_ROUTE,
            post(fix_project_organizations),
        )
        .route(
            PROJECTS_FIX_MEMBER_COUNTS_ROUTE,
            post(fix_project_member_counts),
        )
        .route(PROJECTS_VALIDATE_IMPORT_ROUTE, post(validate_import))
        .route(PROJECTS_IMPORT_ROUTE, post(import_project))
}

#[cfg(test)]
mod tests {
    use super::{
        build_project_export_data_payload, build_project_export_txt_content,
        build_project_list_payload, build_projects_route_owner_contract,
        build_safe_project_export_json_filename, build_safe_project_export_txt_filename,
        map_import_project_write_workflow_error, map_project_consistency_write_workflow_error,
        map_project_export_context_error, map_validate_project_import_payload_error,
        validate_project_import_filename, CreateRequest, CreativeModePreference, ExportOptions,
        ListQuery, LoadProjectConsistencyContextError, LoadProjectExportContextError,
        PlotStagePreference, ProjectConsistencyWriteWorkflowError, ProjectExportContext,
        ProjectExportOptions, ProjectListQueryRequest, QualityPresetPreference,
        StoryFocusPreference, UpdateRequest, PROJECTS_CHECK_CONSISTENCY_ROUTE,
        PROJECTS_DETAIL_ROUTE, PROJECTS_EXPORT_DATA_ROUTE, PROJECTS_EXPORT_TXT_ROUTE,
        PROJECTS_FIX_MEMBER_COUNTS_ROUTE, PROJECTS_FIX_ORGANIZATIONS_ROUTE, PROJECTS_IMPORT_ROUTE,
        PROJECTS_LIST_CREATE_ROUTE, PROJECTS_VALIDATE_IMPORT_ROUTE,
    };
    use crate::api::projects::import_workflow_owner::{
        ImportProjectWriteWorkflowError, ValidateProjectImportPayloadError,
    };
    use crate::models::project;
    use axum::http::StatusCode;
    use chrono::NaiveDateTime;
    use serde_json::json;

    fn project_model(id: &str, title: &str) -> project::Model {
        project::Model {
            id: id.to_string(),
            user_id: "user-1".to_string(),
            title: title.to_string(),
            description: None,
            theme: None,
            genre: None,
            target_words: 0,
            current_words: 0,
            status: "planning".to_string(),
            wizard_status: "incomplete".to_string(),
            wizard_step: 0,
            outline_mode: "one-to-many".to_string(),
            world_time_period: None,
            world_location: None,
            world_atmosphere: None,
            world_rules: None,
            chapter_count: None,
            narrative_perspective: None,
            character_count: 5,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    #[test]
    fn should_publish_projects_route_owner_contract() {
        let contract = build_projects_route_owner_contract();

        assert_eq!(contract["owner"], "projects");
        assert_eq!(contract["rust_owner"], "backend-rs/src/api/projects.rs");
        assert_eq!(contract["routes"]["list"], PROJECTS_LIST_CREATE_ROUTE);
        assert_eq!(contract["routes"]["create"], PROJECTS_LIST_CREATE_ROUTE);
        assert_eq!(contract["routes"]["detail"], PROJECTS_DETAIL_ROUTE);
        assert_eq!(contract["routes"]["export_txt"], PROJECTS_EXPORT_TXT_ROUTE);
        assert_eq!(
            contract["routes"]["export_data"],
            PROJECTS_EXPORT_DATA_ROUTE
        );
        assert_eq!(
            contract["routes"]["validate_import"],
            PROJECTS_VALIDATE_IMPORT_ROUTE
        );
        assert_eq!(contract["routes"]["import"], PROJECTS_IMPORT_ROUTE);
        assert_eq!(
            contract["service_handoffs"]["export_payload_owner"],
            "backend-rs/src/api/projects.rs"
        );
        assert_eq!(contract["readiness_probes"].as_array().unwrap().len(), 21);
        assert_eq!(
            contract["readiness_probes"][20],
            "projects-delete-business-rust"
        );
        assert_eq!(contract["source_map_files"].as_array().unwrap().len(), 4);
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-projects-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"][4],
            "projects-export-data-business-rust"
        );
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            false
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(contract["business_smoke_status"]["business_probe_count"], 9);
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("business smoke is covered"));
        assert!(!contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("requires source-map freeze/delete/repoint evidence or business smoke"));
    }

    fn test_datetime() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
            .expect("test datetime should parse")
    }

    fn chapter_model() -> crate::models::chapter::Model {
        crate::models::chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 1,
            title: "第一章".to_string(),
            content: Some("这里是正文".to_string()),
            summary: Some("章节摘要".to_string()),
            word_count: 5,
            status: "draft".to_string(),
            outline_id: Some("outline-1".to_string()),
            sub_index: 0,
            expansion_plan: Some("{\"beats\":[\"a\"]}".to_string()),
            created_at: test_datetime(),
            updated_at: Some(test_datetime()),
        }
    }

    fn export_context() -> ProjectExportContext {
        ProjectExportContext {
            project: crate::models::project::Model {
                id: "project-1".to_string(),
                user_id: "user-1".to_string(),
                title: "测试 项目/Title".to_string(),
                description: Some("项目简介".to_string()),
                theme: Some("主题测试".to_string()),
                genre: Some("奇幻".to_string()),
                target_words: 100000,
                current_words: 1234,
                status: "draft".to_string(),
                wizard_status: "completed".to_string(),
                wizard_step: 0,
                outline_mode: "traditional".to_string(),
                world_time_period: Some("架空近古".to_string()),
                world_location: Some("北境王城".to_string()),
                world_atmosphere: Some("冷峻".to_string()),
                world_rules: Some("月相影响法术".to_string()),
                chapter_count: Some(1),
                narrative_perspective: Some("third_person".to_string()),
                character_count: 1,
                default_creative_mode: Some("balanced".to_string()),
                default_story_focus: Some("advance_plot".to_string()),
                default_plot_stage: Some("development".to_string()),
                default_story_creation_brief: Some("保持主线推进".to_string()),
                default_quality_preset: Some("balanced".to_string()),
                default_quality_notes: Some("偏重节奏".to_string()),
                created_at: test_datetime(),
                updated_at: Some(test_datetime()),
            },
            chapters: vec![chapter_model()],
            characters: vec![crate::models::character::Model {
                id: "character-1".to_string(),
                project_id: "project-1".to_string(),
                name: "林青".to_string(),
                age: Some("19".to_string()),
                gender: Some("女".to_string()),
                is_organization: false,
                role_type: Some("supporting".to_string()),
                personality: Some("冷静".to_string()),
                background: Some("山门弃徒".to_string()),
                appearance: Some("青衣长剑".to_string()),
                relationships: None,
                organization_type: None,
                organization_purpose: None,
                organization_members: None,
                status: "active".to_string(),
                status_changed_chapter: None,
                current_state: None,
                state_updated_chapter: None,
                main_career_id: Some("career-1".to_string()),
                main_career_stage: Some(2),
                sub_careers: None,
                avatar_url: None,
                traits: Some("[\"敏锐\",\"谨慎\"]".to_string()),
                created_at: test_datetime(),
                updated_at: Some(test_datetime()),
            }],
            outlines: vec![crate::models::outline::Model {
                id: "outline-1".to_string(),
                project_id: "project-1".to_string(),
                title: "第一卷总纲".to_string(),
                content: Some("大纲内容".to_string()),
                structure: Some("三幕式".to_string()),
                order_index: Some(1),
                created_at: test_datetime(),
                updated_at: Some(test_datetime()),
            }],
            relationships: vec![crate::models::relationship::Model {
                id: "rel-1".to_string(),
                project_id: "project-1".to_string(),
                character_from_id: "character-1".to_string(),
                character_to_id: "character-1".to_string(),
                relationship_type_id: None,
                relationship_name: Some("镜像".to_string()),
                intimacy_level: 50,
                status: "active".to_string(),
                description: Some("自我映照".to_string()),
                started_at: Some("chapter-1".to_string()),
                ended_at: None,
                source: "imported".to_string(),
                created_at: test_datetime(),
                updated_at: Some(test_datetime()),
            }],
            organizations: Vec::new(),
            organization_members: Vec::new(),
            writing_styles: vec![crate::models::writing_style::Model {
                id: 7,
                user_id: Some("user-1".to_string()),
                name: "冷峻风格".to_string(),
                style_type: "custom".to_string(),
                preset_id: None,
                description: Some("风格描述".to_string()),
                prompt_content: "提示词".to_string(),
                order_index: 0,
                created_at: test_datetime(),
                updated_at: test_datetime(),
            }],
            generation_history: vec![crate::models::generation_history::Model {
                id: "history-1".to_string(),
                project_id: "project-1".to_string(),
                chapter_id: Some("chapter-1".to_string()),
                prompt: Some("提示".to_string()),
                generated_content: Some("生成内容".to_string()),
                model: Some("gpt-test".to_string()),
                tokens_used: Some(128),
                generation_time: Some(1.5),
                created_at: Some(test_datetime()),
            }],
            careers: vec![crate::models::career::Model {
                id: "career-1".to_string(),
                project_id: "project-1".to_string(),
                name: "剑修".to_string(),
                career_type: "main".to_string(),
                description: Some("职业描述".to_string()),
                category: Some("战斗".to_string()),
                stages: "[\"入门\",\"大成\"]".to_string(),
                max_stage: 10,
                requirements: None,
                special_abilities: None,
                worldview_rules: Some("以剑证道".to_string()),
                attribute_bonuses: None,
                source: "ai".to_string(),
                created_at: test_datetime(),
                updated_at: Some(test_datetime()),
            }],
            character_careers: vec![crate::models::character_career::Model {
                id: "cc-1".to_string(),
                character_id: "character-1".to_string(),
                career_id: "career-1".to_string(),
                career_type: "main".to_string(),
                current_stage: 2,
                stage_progress: Some(30),
                started_at: None,
                reached_current_stage_at: None,
                notes: Some("主修本命剑".to_string()),
                created_at: test_datetime(),
                updated_at: Some(test_datetime()),
            }],
            story_memories: vec![crate::models::story_memory::Model {
                id: "memory-1".to_string(),
                project_id: "project-1".to_string(),
                chapter_id: Some("chapter-1".to_string()),
                memory_type: "foreshadow".to_string(),
                title: Some("初遇".to_string()),
                content: "雨夜初见".to_string(),
                full_context: None,
                related_characters: Some(json!(["character-1"])),
                related_locations: Some(json!(["后山"])),
                tags: Some(json!(["伏笔"])),
                importance_score: Some(0.9),
                story_timeline: 1,
                chapter_position: 20,
                text_length: 4,
                is_foreshadow: 1,
                foreshadow_resolved_at: None,
                foreshadow_strength: Some(0.8),
                vector_id: None,
                embedding_model: None,
                created_at: Some(test_datetime()),
                updated_at: Some(test_datetime()),
            }],
            plot_analysis: vec![crate::models::plot_analysis::Model {
                id: "analysis-1".to_string(),
                project_id: "project-1".to_string(),
                chapter_id: "chapter-1".to_string(),
                plot_stage: Some("opening".to_string()),
                conflict_level: Some(3),
                conflict_types: Some(json!(["external"])),
                emotional_tone: Some("tense".to_string()),
                emotional_intensity: Some(0.7),
                emotional_curve: Some(json!({"start": 0.2, "end": 0.7})),
                hooks: Some(json!([{"text": "悬念"}])),
                hooks_count: 1,
                hooks_avg_strength: Some(0.8),
                foreshadows: Some(json!([{"text": "暗线"}])),
                foreshadows_planted: 1,
                foreshadows_resolved: 0,
                plot_points: Some(json!([{"text": "转折"}])),
                plot_points_count: 1,
                character_states: Some(json!([{"name": "林青"}])),
                scenes: Some(json!([{"name": "雨夜"}])),
                pacing: Some("fast".to_string()),
                overall_quality_score: Some(88.0),
                pacing_score: Some(86.0),
                engagement_score: Some(87.0),
                coherence_score: Some(89.0),
                analysis_report: Some("分析报告".to_string()),
                suggestions: Some(json!(["加强冲突"])),
                word_count: Some(1200),
                dialogue_ratio: Some(0.3),
                description_ratio: Some(0.7),
                created_at: Some(test_datetime()),
            }],
            project_default_style: Some(crate::models::project_default_style::Model {
                id: 1,
                project_id: "project-1".to_string(),
                style_id: 7,
                created_at: test_datetime(),
                updated_at: test_datetime(),
            }),
            project_default_style_style: Some(crate::models::writing_style::Model {
                id: 7,
                user_id: Some("user-1".to_string()),
                name: "冷峻风格".to_string(),
                style_type: "custom".to_string(),
                preset_id: None,
                description: None,
                prompt_content: "提示词".to_string(),
                order_index: 0,
                created_at: test_datetime(),
                updated_at: test_datetime(),
            }),
        }
    }

    #[test]
    fn build_project_export_data_payload_matches_python_style_shape() {
        let payload = build_project_export_data_payload(
            &export_context(),
            &ProjectExportOptions {
                include_generation_history: true,
                include_writing_styles: true,
                include_careers: true,
                include_memories: true,
                include_plot_analysis: true,
            },
        );

        assert_eq!(payload["version"], "1.1.0");
        assert_eq!(payload["project"]["title"], "测试 项目/Title");
        assert_eq!(payload["project"]["default_story_focus"], "advance_plot");
        assert_eq!(payload["chapters"][0]["outline_title"], "第一卷总纲");
        assert_eq!(payload["chapters"][0]["expansion_plan"]["beats"][0], "a");
        assert_eq!(payload["characters"][0]["name"], "林青");
        assert_eq!(payload["characters"][0]["traits"][0], "敏锐");
        assert_eq!(payload["relationships"][0]["relationship_name"], "镜像");
        assert_eq!(payload["writing_styles"][0]["name"], "冷峻风格");
        assert_eq!(payload["generation_history"][0]["chapter_title"], "第一章");
        assert_eq!(payload["careers"][0]["name"], "剑修");
        assert_eq!(payload["character_careers"][0]["career_name"], "剑修");
        assert_eq!(
            payload["story_memories"][0]["related_characters"][0],
            "林青"
        );
        assert_eq!(payload["plot_analysis"][0]["chapter_title"], "第一章");
        assert_eq!(payload["project_default_style"]["style_name"], "冷峻风格");
    }

    #[test]
    fn build_project_export_data_payload_respects_optional_flags() {
        let payload = build_project_export_data_payload(
            &export_context(),
            &ProjectExportOptions {
                include_generation_history: false,
                include_writing_styles: false,
                include_careers: false,
                include_memories: false,
                include_plot_analysis: false,
            },
        );

        assert_eq!(payload["generation_history"], json!([]));
        assert_eq!(payload["writing_styles"], json!([]));
        assert_eq!(payload["careers"], json!([]));
        assert_eq!(payload["character_careers"], json!([]));
        assert_eq!(payload["story_memories"], json!([]));
        assert_eq!(payload["plot_analysis"], json!([]));
        assert_eq!(payload["project_default_style"]["style_name"], "冷峻风格");
    }

    #[test]
    fn build_project_export_txt_content_keeps_existing_text_format() {
        let project = export_context().project;
        let chapters = vec![chapter_model()];

        let text = build_project_export_txt_content(&project, &chapters);

        assert!(text.contains("项目：测试 项目/Title"));
        assert!(text.contains("简介：项目简介"));
        assert!(text.contains("主题：主题测试"));
        assert!(text.contains("类型：奇幻"));
        assert!(text.contains("第 1 章：第一章"));
        assert!(text.contains("这里是正文"));
        assert!(text.contains("\n\n---\n\n"));
    }

    #[test]
    fn build_safe_project_export_filenames_keep_existing_normalization() {
        assert_eq!(
            build_safe_project_export_json_filename("测试 项目/Title"),
            "project_______Title.json"
        );
        assert_eq!(
            build_safe_project_export_txt_filename("测试 项目/Title"),
            "______Title.txt"
        );
    }

    #[test]
    fn should_keep_projects_route_group_paths_stable() {
        assert_eq!(PROJECTS_LIST_CREATE_ROUTE, "/projects");
        assert_eq!(PROJECTS_DETAIL_ROUTE, "/projects/{project_id}");
        assert_eq!(PROJECTS_EXPORT_TXT_ROUTE, "/projects/{project_id}/export");
        assert_eq!(
            PROJECTS_EXPORT_DATA_ROUTE,
            "/projects/{project_id}/export-data"
        );
        assert_eq!(
            PROJECTS_CHECK_CONSISTENCY_ROUTE,
            "/projects/{project_id}/check-consistency"
        );
        assert_eq!(
            PROJECTS_FIX_ORGANIZATIONS_ROUTE,
            "/projects/{project_id}/fix-organizations"
        );
        assert_eq!(
            PROJECTS_FIX_MEMBER_COUNTS_ROUTE,
            "/projects/{project_id}/fix-member-counts"
        );
        assert_eq!(PROJECTS_VALIDATE_IMPORT_ROUTE, "/projects/validate-import");
        assert_eq!(PROJECTS_IMPORT_ROUTE, "/projects/import");
    }

    #[test]
    fn project_list_payload_matches_python_total_items_shape() {
        let payload = build_project_list_payload(
            vec![
                project_model("project-1", "项目一"),
                project_model("project-2", "项目二"),
                project_model("project-3", "项目三"),
            ],
            ProjectListQueryRequest { skip: 1, limit: 1 },
        );

        assert_eq!(payload["total"], 3);
        assert_eq!(payload["items"].as_array().map(Vec::len), Some(1));
        assert_eq!(payload["items"][0]["id"], "project-2");
        assert_eq!(payload["items"][0]["title"], "项目二");
    }

    #[test]
    fn project_list_payload_keeps_empty_items_when_skip_exceeds_total() {
        let payload = build_project_list_payload(
            vec![project_model("project-1", "项目一")],
            ProjectListQueryRequest {
                skip: 20,
                limit: 100,
            },
        );

        assert_eq!(payload["total"], 1);
        assert_eq!(payload["items"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn project_list_query_request_matches_python_defaults_without_user_override() {
        assert_eq!(
            ProjectListQueryRequest::from_route_query(&ListQuery {
                skip: None,
                limit: None,
            }),
            ProjectListQueryRequest {
                skip: 0,
                limit: 100,
            }
        );
        assert_eq!(
            ProjectListQueryRequest::from_route_query(&ListQuery {
                skip: Some(5),
                limit: Some(20),
            }),
            ProjectListQueryRequest { skip: 5, limit: 20 }
        );
        assert_eq!(
            ProjectListQueryRequest::from_route_query(&ListQuery {
                skip: Some(-1),
                limit: Some(-1),
            }),
            ProjectListQueryRequest {
                skip: 0,
                limit: 100,
            }
        );
        assert_eq!(
            ProjectListQueryRequest::from_route_query(&ListQuery {
                skip: Some(0),
                limit: Some(0),
            }),
            ProjectListQueryRequest { skip: 0, limit: 0 }
        );
    }

    #[test]
    fn create_project_route_request_accepts_python_generation_preference_fields() {
        let request: CreateRequest = serde_json::from_value(json!({
            "title": "项目",
            "description": "描述",
            "theme": "主题",
            "genre": "奇幻",
            "outline_mode": "one-to-many",
            "target_words": 100000,
            "default_creative_mode": "balanced",
            "default_story_focus": "advance_plot",
            "default_plot_stage": "development",
            "default_story_creation_brief": "保持主线推进",
            "default_quality_preset": "balanced",
            "default_quality_notes": "偏重节奏"
        }))
        .expect("ProjectCreate-compatible payload should deserialize");

        assert_eq!(
            request
                .default_creative_mode
                .as_ref()
                .map(CreativeModePreference::as_str),
            Some("balanced")
        );
        assert_eq!(
            request
                .default_story_focus
                .as_ref()
                .map(StoryFocusPreference::as_str),
            Some("advance_plot")
        );
        assert_eq!(
            request
                .default_plot_stage
                .as_ref()
                .map(PlotStagePreference::as_str),
            Some("development")
        );
        assert_eq!(
            request.default_story_creation_brief.as_deref(),
            Some("保持主线推进")
        );
        assert_eq!(
            request
                .default_quality_preset
                .as_ref()
                .map(QualityPresetPreference::as_str),
            Some("balanced")
        );
        assert_eq!(request.default_quality_notes.as_deref(), Some("偏重节奏"));
    }

    #[test]
    fn create_project_route_request_normalizes_blank_generation_preferences_like_python() {
        let request: CreateRequest = serde_json::from_value(json!({
            "title": "项目",
            "default_creative_mode": "   ",
            "default_story_focus": "",
            "default_plot_stage": "  ",
            "default_story_creation_brief": "  保持主线推进  ",
            "default_quality_preset": "",
            "default_quality_notes": "  偏重节奏  "
        }))
        .expect("blank generation preference fields should normalize like Python");

        assert!(request.default_creative_mode.is_none());
        assert!(request.default_story_focus.is_none());
        assert!(request.default_plot_stage.is_none());
        assert_eq!(
            request.default_story_creation_brief.as_deref(),
            Some("保持主线推进")
        );
        assert!(request.default_quality_preset.is_none());
        assert_eq!(request.default_quality_notes.as_deref(), Some("偏重节奏"));
    }

    #[test]
    fn create_project_route_request_rejects_invalid_generation_preference_literals_like_python() {
        let result = serde_json::from_value::<CreateRequest>(json!({
            "title": "项目",
            "default_creative_mode": "chaos"
        }));
        let error = match result {
            Ok(_) => panic!("ProjectCreate should reject invalid creative mode literals"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("unknown variant `chaos`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn create_project_route_request_rejects_overlong_generation_text_like_python() {
        let result = serde_json::from_value::<CreateRequest>(json!({
            "title": "项目",
            "default_quality_notes": "x".repeat(601)
        }));
        let error = match result {
            Ok(_) => panic!("ProjectCreate should reject overlong quality notes"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("at most 600 characters"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn create_project_route_request_rejects_invalid_outline_mode_like_python_literal() {
        let result = serde_json::from_value::<CreateRequest>(json!({
            "title": "项目",
            "outline_mode": "many-to-one"
        }));
        let error = match result {
            Ok(_) => panic!("ProjectCreate outline_mode should match Python Literal values"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("unknown variant `many-to-one`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn create_project_route_request_rejects_null_outline_mode_like_python_defaulted_literal() {
        let result = serde_json::from_value::<CreateRequest>(json!({
            "title": "项目",
            "outline_mode": null
        }));
        let error = match result {
            Ok(_) => panic!("ProjectCreate outline_mode should reject explicit null like Python"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("invalid type: null"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn create_project_route_request_rejects_unknown_fields_like_python() {
        let result = serde_json::from_value::<CreateRequest>(json!({
            "title": "项目",
            "unsupported": true
        }));
        let error = match result {
            Ok(_) => panic!("ProjectCreate extra=forbid should reject unknown fields"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("unknown field"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn update_project_route_request_accepts_python_world_and_count_fields() {
        let request: UpdateRequest = serde_json::from_value(json!({
            "title": "更新项目",
            "world_time_period": "架空中古",
            "world_location": "群山王国",
            "world_atmosphere": "阴郁史诗",
            "world_rules": "魔法受月相限制",
            "chapter_count": 80,
            "character_count": 12
        }))
        .expect("ProjectUpdate-compatible payload should deserialize");

        assert_eq!(request.world_time_period.as_deref(), Some("架空中古"));
        assert_eq!(request.world_location.as_deref(), Some("群山王国"));
        assert_eq!(request.world_atmosphere.as_deref(), Some("阴郁史诗"));
        assert_eq!(request.world_rules.as_deref(), Some("魔法受月相限制"));
        assert_eq!(request.chapter_count, Some(80));
        assert_eq!(request.character_count, Some(12));
    }

    #[test]
    fn update_project_route_request_rejects_unknown_fields_like_python() {
        let result = serde_json::from_value::<UpdateRequest>(json!({
            "title": "更新项目",
            "unsupported": true
        }));
        let error = match result {
            Ok(_) => panic!("ProjectUpdate extra=forbid should reject unknown fields"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("unknown field"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn update_project_route_request_rejects_outline_mode_like_python_schema() {
        let result = serde_json::from_value::<UpdateRequest>(json!({
            "title": "更新项目",
            "outline_mode": "one-to-one"
        }));
        let error = match result {
            Ok(_) => panic!("ProjectUpdate should not accept outline_mode"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("unknown field `outline_mode`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn update_project_route_request_normalizes_generation_preferences_like_python() {
        let request: UpdateRequest = serde_json::from_value(json!({
            "default_story_focus": " relationship_shift ",
            "default_story_creation_brief": "   ",
            "default_quality_preset": " clean_prose ",
            "default_quality_notes": "  减少解释性旁白  "
        }))
        .expect("ProjectUpdate generation preferences should normalize like Python");

        assert_eq!(
            request
                .default_story_focus
                .as_ref()
                .map(StoryFocusPreference::as_str),
            Some("relationship_shift")
        );
        assert!(request.default_story_creation_brief.is_none());
        assert_eq!(
            request
                .default_quality_preset
                .as_ref()
                .map(QualityPresetPreference::as_str),
            Some("clean_prose")
        );
        assert_eq!(
            request.default_quality_notes.as_deref(),
            Some("减少解释性旁白")
        );
    }

    #[test]
    fn update_project_route_request_rejects_invalid_generation_preference_literals_like_python() {
        let result = serde_json::from_value::<UpdateRequest>(json!({
            "default_plot_stage": "opening"
        }));
        let error = match result {
            Ok(_) => panic!("ProjectUpdate should reject invalid plot stage literals"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("unknown variant `opening`"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn export_options_match_python_defaults() {
        let options: ExportOptions = serde_json::from_value(json!({}))
            .expect("omitted export options should use Python defaults");

        assert!(!options.include_generation_history);
        assert!(options.include_writing_styles);
        assert!(options.include_careers);
        assert!(!options.include_memories);
        assert!(!options.include_plot_analysis);
    }

    #[test]
    fn export_options_keep_explicit_false_values() {
        let options: ExportOptions = serde_json::from_value(json!({
            "include_writing_styles": false,
            "include_careers": false
        }))
        .expect("explicit false export options should remain false");

        assert!(!options.include_writing_styles);
        assert!(!options.include_careers);
    }

    #[test]
    fn fix_project_organizations_not_found_keeps_existing_transport_detail() {
        let response = map_project_consistency_write_workflow_error(
            ProjectConsistencyWriteWorkflowError::Context(
                LoadProjectConsistencyContextError::ProjectNotFound,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "Project not found" }));
    }

    #[test]
    fn fix_project_member_counts_internal_keeps_detail_passthrough() {
        let response = map_project_consistency_write_workflow_error(
            ProjectConsistencyWriteWorkflowError::Internal("member count failed".to_string()),
        );

        assert_eq!(response.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.1 .0, json!({ "detail": "member count failed" }));
    }

    #[test]
    fn check_project_consistency_not_found_keeps_existing_transport_detail() {
        let response = map_project_consistency_write_workflow_error(
            ProjectConsistencyWriteWorkflowError::Context(
                LoadProjectConsistencyContextError::ProjectNotFound,
            ),
        );

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "Project not found" }));
    }

    #[test]
    fn export_project_context_not_found_keeps_existing_transport_detail() {
        let response = map_project_export_context_error(LoadProjectExportContextError::Context(
            LoadProjectConsistencyContextError::ProjectNotFound,
        ));

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(response.1 .0, json!({ "detail": "Project not found" }));
    }

    #[test]
    fn export_project_context_no_chapters_keeps_specific_transport_detail() {
        let response =
            map_project_export_context_error(LoadProjectExportContextError::ProjectHasNoChapters);

        assert_eq!(response.0, StatusCode::NOT_FOUND);
        assert_eq!(
            response.1 .0,
            json!({ "detail": "Project has no chapters" })
        );
    }

    #[test]
    fn validate_project_import_invalid_json_matches_python_detail() {
        let response = map_validate_project_import_payload_error(
            ValidateProjectImportPayloadError::InvalidJson("boom".to_string()),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "无效的JSON格式: boom" }));
    }

    #[test]
    fn import_project_payload_too_large_keeps_existing_chinese_detail() {
        let response = map_import_project_write_workflow_error(
            ImportProjectWriteWorkflowError::PayloadTooLarge,
        );

        assert_eq!(response.0, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(response.1 .0, json!({ "detail": "文件大小超过50MB限制" }));
    }

    #[test]
    fn import_project_invalid_json_matches_python_detail() {
        let response = map_import_project_write_workflow_error(
            ImportProjectWriteWorkflowError::InvalidJson("boom".to_string()),
        );

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "无效的JSON格式: boom" }));
    }

    #[test]
    fn project_import_filename_accepts_json_like_python() {
        assert!(validate_project_import_filename(Some("project.json")).is_ok());
    }

    #[test]
    fn project_import_filename_rejects_non_json_like_python() {
        let response = validate_project_import_filename(Some("project.txt"))
            .expect_err("Python import routes reject non-json filenames");

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "只支持JSON格式文件" }));
    }

    #[test]
    fn project_import_filename_rejects_missing_filename_like_python_upload_file() {
        let response = validate_project_import_filename(None)
            .expect_err("missing filename should not bypass json filename check");

        assert_eq!(response.0, StatusCode::BAD_REQUEST);
        assert_eq!(response.1 .0, json!({ "detail": "只支持JSON格式文件" }));
    }
}
