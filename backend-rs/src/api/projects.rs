use axum::{
    extract::{Extension, Multipart, Path, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::{de, Deserialize, Deserializer};
use serde_json::{json, Value};

use crate::services::auth::Claims;
use crate::services::project_consistency_query_service::LoadProjectConsistencyContextError;
use crate::services::project_consistency_write_workflow_service::{
    check_project_consistency_write_workflow, fix_project_member_counts_write_workflow,
    fix_project_organizations_write_workflow, normalize_project_consistency_auto_fix,
    ProjectConsistencyWriteWorkflowError,
};
use crate::services::project_export_payload_adapter_service::{
    build_project_export_data_payload, build_project_export_txt_content,
    build_safe_project_export_json_filename, build_safe_project_export_txt_filename,
};
use crate::services::project_export_query_service::{
    load_project_export_context, load_project_export_context_with_non_empty_chapters,
    LoadProjectExportContextError, ProjectExportOptions,
};
use crate::services::project_import_workflow_service::{
    import_project_write_workflow, validate_project_import_payload,
    ImportProjectWriteWorkflowError, ValidateProjectImportPayloadError,
};
use crate::services::project_service::ProjectService;
use crate::services::route_request_deserialize_service::deserialize_optional_non_null;

const MAX_STORY_CREATION_BRIEF_LEN: usize = 1200;
const MAX_QUALITY_NOTES_LEN: usize = 600;

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
        .route("/projects", post(create_project).get(list_projects))
        .route(
            "/projects/{project_id}",
            get(get_project).put(update_project).delete(delete_project),
        )
        .route("/projects/{project_id}/export", get(export_project_txt))
        .route(
            "/projects/{project_id}/export-data",
            post(export_project_data),
        )
        .route(
            "/projects/{project_id}/check-consistency",
            post(check_project_consistency),
        )
        .route(
            "/projects/{project_id}/fix-organizations",
            post(fix_project_organizations),
        )
        .route(
            "/projects/{project_id}/fix-member-counts",
            post(fix_project_member_counts),
        )
        .route("/projects/validate-import", post(validate_import))
        .route("/projects/import", post(import_project))
}

#[cfg(test)]
mod tests {
    use super::{
        build_project_list_payload, map_import_project_write_workflow_error,
        map_project_consistency_write_workflow_error, map_project_export_context_error,
        map_validate_project_import_payload_error, validate_project_import_filename, CreateRequest,
        CreativeModePreference, ExportOptions, ListQuery, PlotStagePreference,
        ProjectListQueryRequest, QualityPresetPreference, StoryFocusPreference, UpdateRequest,
    };
    use crate::models::project;
    use crate::services::project_consistency_query_service::LoadProjectConsistencyContextError;
    use crate::services::project_consistency_write_workflow_service::ProjectConsistencyWriteWorkflowError;
    use crate::services::project_export_query_service::LoadProjectExportContextError;
    use crate::services::project_import_workflow_service::{
        ImportProjectWriteWorkflowError, ValidateProjectImportPayloadError,
    };
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
