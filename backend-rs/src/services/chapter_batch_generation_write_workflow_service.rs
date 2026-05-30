use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::chapter;
use crate::models::generation_history;
use crate::services::chapter_batch_generation_owned_task_query_service::{
    load_owned_task, LoadOwnedBatchGenerationTaskError,
};
use crate::services::chapter_batch_generation_resume_semantics_service::ResumeExecutionSelection;
use crate::services::chapter_batch_generation_snapshot_service::persist_new_batch_generation_task_snapshot;
use crate::services::chapter_batch_generation_task_model_service::build_batch_generation_task_active_model;
use crate::services::chapter_batch_generation_access_service::load_accessible_chapter_for_generation;
use crate::services::chapter_generation_execution_config_service::{
    prepare_generation_execution_config, prepare_generation_execution_config_with_provider_payload,
    PreparedGenerationExecutionConfig,
};
use crate::services::chapter_generation_research_payload_service::build_single_chapter_research_provider_payload;
use crate::services::chapter_generation_target_word_count_service::normalize_chapter_generation_target_word_count;
use crate::services::project_access_query_service::{
    ensure_owned_project_access, ProjectAccessQueryError,
};
use crate::services::settings_service::SettingsService;
use crate::services::chapter_story_repair_quality_context_service::{
    aggregate_story_repair_quality_summaries, extract_quality_history_context,
    merge_active_story_repair_payloads,
    restore_active_story_repair_payload_from_quality_context,
    restore_story_repair_compat_options_from_active_snapshot,
};

use super::chapter_batch_generation_resume_semantics_service::ResumeBatchGenerationCommandState;
use super::chapter_batch_generation_resume_task_command_service::{
    prepare_batch_generation_resume, ResumeBatchGenerationDomainError,
};
use super::chapter_batch_generation_runtime_state_service::{
    dispatch_batch_generation_runtime, BatchGenerationExecutionInput,
};
use super::chapter_single_generation_prepare_service::{
    SingleChapterGenerationCompatOptions, SingleChapterGenerationTarget,
};
use super::chapter_quality_metrics_query_service::build_chapter_analysis_quality_fragments;

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
    pub(crate) fn from_route_payload(
        start_chapter_number: i32,
        count: i32,
        style_id: Option<i32>,
        target_word_count: Option<i32>,
        enable_analysis: Option<bool>,
        enable_mcp: Option<bool>,
        enable_web_research: Option<bool>,
        web_research_query: Option<String>,
        max_retries: Option<i32>,
        model_override: Option<String>,
        creative_mode: Option<String>,
        story_focus: Option<String>,
        plot_stage: Option<String>,
        story_creation_brief: Option<String>,
        quality_preset: Option<String>,
        quality_notes: Option<String>,
        story_repair_summary: Option<String>,
        story_repair_targets: Option<Vec<String>>,
        story_preserve_strengths: Option<Vec<String>>,
    ) -> Self {
        Self {
            start_chapter_number,
            count,
            style_id,
            target_word_count,
            enable_analysis: enable_analysis.unwrap_or(false),
            enable_mcp,
            enable_web_research,
            web_research_query,
            max_retries: max_retries.unwrap_or(3),
            model_override,
            creative_mode,
            story_focus,
            plot_stage,
            story_creation_brief,
            quality_preset,
            quality_notes,
            story_repair_summary,
            story_repair_targets: story_repair_targets.unwrap_or_default(),
            story_preserve_strengths: story_preserve_strengths.unwrap_or_default(),
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

    async fn prepare(
        &self,
        db: &DatabaseConnection,
        project_id: &str,
    ) -> Result<(i32, Vec<BatchGenerationCreateChapterTarget>), PrepareBatchGenerationCreateRequestError> {
        let chapters_to_generate = self
            .load_chapters_for_generation_range(db, project_id)
            .await?;

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

        let end_chapter_number = self.start_chapter_number + self.count - 1;
        let chapters_to_generate = chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(project_id))
            .filter(chapter::Column::ChapterNumber.gte(self.start_chapter_number))
            .filter(chapter::Column::ChapterNumber.lte(end_chapter_number))
            .order_by_asc(chapter::Column::ChapterNumber)
            .all(db)
            .await
            .map_err(|error| {
                PrepareBatchGenerationCreateRequestError::Internal(error.to_string())
            })?;

        if chapters_to_generate.is_empty() {
            return Err(PrepareBatchGenerationCreateRequestError::ChaptersNotFound);
        }

        Ok(chapters_to_generate)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BatchGenerationRequestRuntimeState {
    pub(crate) compat_options: SingleChapterGenerationCompatOptions,
    pub(crate) model_override: Option<String>,
}

impl BatchGenerationRequestRuntimeState {
    pub(crate) fn new(
        compat_options: SingleChapterGenerationCompatOptions,
        model_override: Option<String>,
    ) -> Self {
        Self {
            compat_options,
            model_override,
        }
    }

    pub(crate) fn from_create_request(
        request: &BatchGenerationCreateWorkflowRequest,
        web_research_default: bool,
    ) -> Self {
        Self::new(
            request.compat_options_with_web_research_default(web_research_default),
            request.model_override.clone(),
        )
    }

    pub(crate) fn active_story_repair_payload_with_scope(&self, scope: &str) -> Option<Value> {
        let summary = self.compat_options.story_repair_summary().trim();
        let repair_targets = self
            .compat_options
            .story_repair_targets()
            .iter()
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let preserve_strengths = self
            .compat_options
            .story_preserve_strengths()
            .iter()
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();

        if summary.is_empty() && repair_targets.is_empty() && preserve_strengths.is_empty() {
            return None;
        }

        Some(json!({
            "summary": if summary.is_empty() { Value::Null } else { json!(summary) },
            "repair_targets": repair_targets,
            "preserve_strengths": preserve_strengths,
            "focus_areas": Vec::<String>::new(),
            "weakest_metric_key": Value::Null,
            "weakest_metric_label": Value::Null,
            "weakest_metric_value": Value::Null,
            "quality_gate": Value::Null,
            "quality_gate_status": Value::Null,
            "quality_gate_decision": Value::Null,
            "quality_gate_label": Value::Null,
            "quality_gate_summary": Value::Null,
            "quality_gate_failed_metrics": Vec::<String>::new(),
            "source": "manual_request",
            "source_label": "Manual request",
            "scope": scope,
            "updated_at": Value::Null,
        }))
    }
}

const BATCH_REQUEST_RUNTIME_STATE_KEY: &str = "batch_request_runtime_state";

pub(crate) fn batch_generation_runtime_state_payload(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
) -> Value {
    let mut payload = serde_json::Map::from_iter([(
        BATCH_REQUEST_RUNTIME_STATE_KEY.to_string(),
        json!(request_runtime_state),
    )]);
    if let Some(active_story_repair_payload) =
        request_runtime_state.active_story_repair_payload_with_scope("batch")
    {
        payload.insert(
            "active_story_repair_payload".to_string(),
            active_story_repair_payload,
        );
    }

    Value::Object(payload)
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
        .map_err(|error| format!("load previous chapters for batch story repair failed: {error}"))?;

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
            .map_err(|error| format!("load generation histories for batch story repair failed: {error}"))?;
        let quality_fragments = build_chapter_analysis_quality_fragments(&histories, None);
        if let Some(summary) = quality_fragments.quality_metrics_summary {
            summaries.push(summary);
        }
    }

    Ok(aggregate_story_repair_quality_summaries(&summaries, "batch"))
}

async fn build_batch_generation_runtime_state_payload(
    db: &DatabaseConnection,
    project_id: &str,
    start_chapter_number: i32,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
) -> Result<Value, String> {
    let quality_summary =
        load_recent_batch_story_repair_quality_summary(db, project_id, start_chapter_number).await?;

    Ok(build_batch_generation_runtime_state_payload_from_parts(
        request_runtime_state,
        quality_summary.as_ref(),
    ))
}

pub(crate) fn parse_batch_generation_request_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> BatchGenerationRequestRuntimeState {
    workflow_runtime_state
        .and_then(|state| state.get(BATCH_REQUEST_RUNTIME_STATE_KEY).cloned())
        .and_then(|value| serde_json::from_value::<BatchGenerationRequestRuntimeState>(value).ok())
        .unwrap_or_default()
}

pub(crate) fn active_story_repair_payload_from_runtime_state(
    workflow_runtime_state: Option<&Value>,
) -> Option<Value> {
    workflow_runtime_state
        .and_then(Value::as_object)
        .and_then(|state| state.get("active_story_repair_payload"))
        .filter(|payload| payload.is_object())
        .cloned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PrepareBatchGenerationCreateRequestError {
    InvalidCount,
    ChaptersNotFound,
    Internal(String),
}

#[derive(Debug)]
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

fn estimated_task_minutes(total_chapters: usize) -> i32 {
    (total_chapters as i32).max(1) * 2
}

pub(crate) fn build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    explicit_story_repair_payload: Option<&Value>,
    quality_summary: Option<&Value>,
) -> Value {
    let mut payload = serde_json::Map::from_iter([(
        BATCH_REQUEST_RUNTIME_STATE_KEY.to_string(),
        json!(request_runtime_state),
    )]);

    let derived_story_repair_payload = restore_active_story_repair_payload_from_quality_context(
        quality_summary,
        None,
        "batch",
        "recent_history_summary",
        "Recent history summary",
    );
    let active_story_repair_payload = merge_active_story_repair_payloads(
        explicit_story_repair_payload,
        derived_story_repair_payload.as_ref(),
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
    payload.insert(
        "quality_metrics_summary".to_string(),
        quality_summary.cloned().unwrap_or(Value::Null),
    );
    payload.insert(
        "quality_history_context".to_string(),
        extract_quality_history_context(quality_summary).unwrap_or(Value::Null),
    );

    Value::Object(payload)
}

pub(crate) fn build_batch_generation_runtime_state_payload_from_parts(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    quality_summary: Option<&Value>,
) -> Value {
    build_batch_generation_runtime_state_payload_from_parts_with_explicit_payload(
        request_runtime_state,
        request_runtime_state.active_story_repair_payload_with_scope("batch").as_ref(),
        quality_summary,
    )
}

fn resolve_batch_generation_runtime_compat_options_from_seed(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    runtime_state_payload: &Value,
) -> SingleChapterGenerationCompatOptions {
    restore_story_repair_compat_options_from_active_snapshot(
        &request_runtime_state.compat_options,
        active_story_repair_payload_from_runtime_state(Some(runtime_state_payload)).as_ref(),
        runtime_state_payload.get("quality_metrics_summary"),
        None,
    )
}

fn batch_generation_create_response_payload(
    batch_id: &str,
    chapters_to_generate: &[BatchGenerationCreateChapterTarget],
) -> Value {
    let total_chapters = chapters_to_generate.len();
    json!({
        "batch_id": batch_id,
        "message": "Batch generation task created",
        "chapters_to_generate": chapters_to_generate
            .iter()
            .map(|target| {
                json!({
                    "id": target.id,
                    "chapter_number": target.chapter_number,
                    "title": target.title,
                })
            })
            .collect::<Vec<_>>(),
        "estimated_time_minutes": estimated_task_minutes(total_chapters),
    })
}

async fn dispatch_batch_generation_create_workflow(
    db: &DatabaseConnection,
    project_id: String,
    user_id: String,
    request: &BatchGenerationCreateWorkflowRequest,
    task_id: String,
    now: chrono::NaiveDateTime,
    normalized_target_word_count: i32,
    chapters_to_generate: Vec<BatchGenerationCreateChapterTarget>,
    request_runtime_state: BatchGenerationRequestRuntimeState,
    execution_config: crate::services::chapter_generation_execution_config_service::PreparedGenerationExecutionConfig,
) -> Result<Value, CreateBatchGenerationWriteWorkflowError> {
    let total_chapters = chapters_to_generate.len() as i32;
    let chapter_ids = chapters_to_generate
        .iter()
        .map(|target| target.id.clone())
        .collect::<Vec<_>>();
    let chapter_id_payload = Value::Array(
        chapter_ids
            .iter()
            .map(|chapter_id| json!(chapter_id))
            .collect(),
    );
    let response_payload = batch_generation_create_response_payload(&task_id, &chapters_to_generate);
    let background_task_id = task_id.clone();
    let runtime_state_payload = build_batch_generation_runtime_state_payload(
        db,
        &project_id,
        request.start_chapter_number,
        &request_runtime_state,
    )
    .await
    .map_err(CreateBatchGenerationWriteWorkflowError::Internal)?;
    let resolved_compat_options = resolve_batch_generation_runtime_compat_options_from_seed(
        &request_runtime_state,
        &runtime_state_payload,
    );
    let runtime_input = BatchGenerationExecutionInput {
        user_id: user_id.clone(),
        chapter_ids: chapter_ids.clone(),
        target_word_count: normalized_target_word_count,
        compat_options: resolved_compat_options,
        ai_config: execution_config.ai_config,
    };
    let task = build_batch_generation_task_active_model(
        background_task_id,
        project_id.clone(),
        user_id,
        request.start_chapter_number,
        total_chapters,
        chapter_id_payload,
        request.style_id,
        normalized_target_word_count,
        request.enable_analysis,
        total_chapters,
        None,
        None,
        request.max_retries,
        now,
    );

    task
        .insert(db)
        .await
        .map_err(|error| CreateBatchGenerationWriteWorkflowError::Internal(error.to_string()))?;
    persist_new_batch_generation_task_snapshot(
        db,
        &task_id,
        total_chapters,
        Some(runtime_state_payload),
    )
        .await
        .map_err(CreateBatchGenerationWriteWorkflowError::Internal)?;
    dispatch_batch_generation_runtime(db.clone(), task_id, runtime_input);

    Ok(response_payload)
}

fn should_rebuild_resume_provider_payload(
    execution_selection: &ResumeExecutionSelection,
) -> bool {
    matches!(
        execution_selection,
        ResumeExecutionSelection::SingleChapter { .. }
    )
}

async fn prepare_resume_generation_execution_config(
    db: &DatabaseConnection,
    user_id: &str,
    execution_selection: &ResumeExecutionSelection,
    request_runtime_state: &BatchGenerationRequestRuntimeState,
) -> Result<PreparedGenerationExecutionConfig, String> {
    if should_rebuild_resume_provider_payload(execution_selection) {
        match execution_selection {
            ResumeExecutionSelection::SingleChapter { chapter_id } => {
                let chapter_model = load_accessible_chapter_for_generation(db, chapter_id, user_id)
                    .await
                    .map_err(|error| match error {
                        crate::services::chapter_batch_generation_access_service::LoadAccessibleChapterForGenerationError::ChapterNotFound => {
                            "Chapter not found".to_string()
                        }
                        crate::services::chapter_batch_generation_access_service::LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied => {
                            "Chapter not found or access denied".to_string()
                        }
                        crate::services::chapter_batch_generation_access_service::LoadAccessibleChapterForGenerationError::Internal(detail) => detail,
                    })?;
                let chapter_target = SingleChapterGenerationTarget {
                    project_id: chapter_model.project_id.clone(),
                    chapter_id: chapter_model.id.clone(),
                    chapter_number: chapter_model.chapter_number,
                    title: chapter_model.title.clone(),
                };
                let provider_payload = build_single_chapter_research_provider_payload(
                    db,
                    user_id,
                    &chapter_target,
                    &request_runtime_state.compat_options,
                )
                .await?;

                prepare_generation_execution_config_with_provider_payload(
                    db,
                    user_id,
                    request_runtime_state.model_override.as_deref(),
                    provider_payload,
                )
                .await
            }
            ResumeExecutionSelection::Batch { .. } => unreachable!("checked by predicate"),
        }
    } else {
        prepare_generation_execution_config(
            db,
            user_id,
            request_runtime_state.model_override.as_deref(),
        )
        .await
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

pub(crate) async fn start_owned_batch_generation_write_workflow(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    request: BatchGenerationCreateWorkflowRequest,
) -> Result<Value, CreateBatchGenerationWriteWorkflowError> {
    ensure_owned_project_access(db, project_id, user_id)
        .await
        .map_err(CreateBatchGenerationWriteWorkflowError::ProjectAccess)?;

    let (normalized_target_word_count, chapters_to_generate) = request
        .prepare(db, project_id)
        .await
        .map_err(CreateBatchGenerationWriteWorkflowError::Prepare)?;
    let web_research_default = SettingsService::resolve_web_research_enabled(db, user_id)
        .await
        .map_err(|error| CreateBatchGenerationWriteWorkflowError::Config(error.to_string()))?;
    let request_runtime_state =
        BatchGenerationRequestRuntimeState::from_create_request(&request, web_research_default);

    let now = Utc::now().naive_utc();
    let execution_config =
        prepare_generation_execution_config(db, user_id, request.model_override.as_deref())
            .await
            .map_err(CreateBatchGenerationWriteWorkflowError::Config)?;
    dispatch_batch_generation_create_workflow(
        db,
        project_id.to_string(),
        user_id.to_string(),
        &request,
        Uuid::new_v4().to_string(),
        now,
        normalized_target_word_count,
        chapters_to_generate,
        request_runtime_state,
        execution_config,
    )
    .await
}

pub(crate) async fn resume_owned_batch_generation_write_workflow(
    db: &DatabaseConnection,
    batch_id: &str,
    user_id: &str,
) -> Result<Value, ResumeBatchGenerationWriteWorkflowError> {
    let task = load_owned_task(db, batch_id, user_id)
        .await
        .map_err(|error| {
            ResumeBatchGenerationWriteWorkflowError::Task(
                LoadOwnedBatchGenerationTaskError::Internal(error),
            )
        })?
        .ok_or(ResumeBatchGenerationWriteWorkflowError::Task(
            LoadOwnedBatchGenerationTaskError::TaskNotFound,
        ))?;
    let command_state = ResumeBatchGenerationCommandState::from_task(&task);

    let snapshot =
        crate::services::chapter_batch_generation_snapshot_service::load_batch_generation_snapshot(
            db, batch_id,
        )
        .await
        .map_err(ResumeBatchGenerationWriteWorkflowError::Config)?;
    let workflow_runtime_state = snapshot
        .as_ref()
        .and_then(|item| item.workflow_runtime_state.as_ref());
    let request_runtime_state = parse_batch_generation_request_runtime_state(
        workflow_runtime_state,
    );
    let (execution_selection, target_word_count, response_payload) =
        prepare_batch_generation_resume(
            db,
            command_state,
            user_id,
            workflow_runtime_state,
            snapshot.as_ref(),
            &request_runtime_state,
        )
        .await
        .map_err(ResumeBatchGenerationWriteWorkflowError::Domain)?;

    let execution_config = prepare_resume_generation_execution_config(
        db,
        user_id,
        &execution_selection,
        &request_runtime_state,
    )
        .await
        .map_err(ResumeBatchGenerationWriteWorkflowError::Config)?;
    crate::services::chapter_batch_generation_resume_task_command_service::dispatch_resumed_batch_generation_execution(
        db.clone(),
        batch_id.to_string(),
        user_id.to_string(),
        execution_selection,
        target_word_count,
        request_runtime_state.compat_options,
        execution_config,
    );
    Ok(response_payload)
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Utc};
    use serde_json::{json, Value};

    use super::{
        batch_generation_runtime_state_payload,
        build_batch_generation_runtime_state_payload_from_parts,
        resolve_batch_generation_runtime_compat_options_from_seed,
        BatchGenerationCreateChapterTarget, BatchGenerationCreateWorkflowRequest,
        BatchGenerationRequestRuntimeState,
        CreateBatchGenerationWriteWorkflowError, PrepareBatchGenerationCreateRequestError,
        should_rebuild_resume_provider_payload,
    };
    use crate::models::chapter;
    use crate::services::chapter_batch_generation_resume_semantics_service::ResumeExecutionSelection;
    use crate::services::chapter_batch_generation_owned_task_query_service::LoadOwnedBatchGenerationTaskError;
    use crate::services::chapter_generation_target_word_count_service::normalize_chapter_generation_target_word_count;
    use crate::services::chapter_story_repair_quality_context_service::aggregate_story_repair_quality_summaries;
    use crate::services::project_access_query_service::ProjectAccessQueryError;

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
        assert_eq!(super::estimated_task_minutes(0), 2);
        assert_eq!(super::estimated_task_minutes(1), 2);
        assert_eq!(super::estimated_task_minutes(3), 6);
    }

    #[test]
    fn should_build_batch_generation_create_response_payload() {
        let chapters = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let payload = super::batch_generation_create_response_payload("task-1", &chapters);

        assert_eq!(payload["batch_id"], "task-1");
        assert_eq!(payload["message"], "Batch generation task created");
        assert_eq!(payload["chapters_to_generate"][0]["id"], "chapter-1");
        assert_eq!(payload["chapters_to_generate"][1]["title"], "Second");
        assert_eq!(payload["estimated_time_minutes"], 4);
    }

    #[test]
    fn should_rebuild_provider_payload_only_for_single_chapter_resume() {
        assert!(should_rebuild_resume_provider_payload(
            &ResumeExecutionSelection::SingleChapter {
                chapter_id: "chapter-7".to_string(),
            }
        ));
        assert!(!should_rebuild_resume_provider_payload(
            &ResumeExecutionSelection::Batch {
                chapter_ids: vec!["chapter-7".to_string(), "chapter-8".to_string()],
            }
        ));
    }

    #[test]
    fn should_build_batch_generation_create_execution_input_from_create_parts() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let normalized_target_word_count = 2800;
        let chapter_ids = chapters_to_generate
            .iter()
            .map(|target| target.id.clone())
            .collect::<Vec<_>>();
        let response_payload =
            super::batch_generation_create_response_payload("task-1", &chapters_to_generate);
        let execution_input = super::BatchGenerationExecutionInput {
            user_id: "user-1".to_string(),
            chapter_ids,
            target_word_count: normalized_target_word_count,
            compat_options: crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default(),
            ai_config: crate::ai::AIConfig::default(),
        };

        assert_eq!(response_payload["batch_id"], "task-1");
        assert_eq!(response_payload["message"], "Batch generation task created");
        assert_eq!(
            response_payload["chapters_to_generate"][0]["id"],
            "chapter-1"
        );
        assert_eq!(execution_input.user_id, "user-1");
        assert_eq!(
            execution_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(execution_input.target_word_count, 2800);
        assert_eq!(execution_input.ai_config.provider, crate::ai::AIConfig::default().provider);
    }

    #[test]
    fn should_build_batch_generation_create_runtime_seed_from_create_parts() {
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
        let total_chapters = chapters_to_generate.len() as i32;
        let chapter_ids = chapters_to_generate
            .iter()
            .map(|target| target.id.clone())
            .collect::<Vec<_>>();
        let chapter_id_payload = Value::Array(
            chapters_to_generate
                .iter()
                .map(|target| target.id.clone())
                .into_iter()
                .map(|chapter_id| json!(chapter_id))
                .collect(),
        );
        let response_payload =
            super::batch_generation_create_response_payload("task-1", &chapters_to_generate);
        let task = super::build_batch_generation_task_active_model(
            "task-1".to_string(),
            "project-1".to_string(),
            "user-1".to_string(),
            request.start_chapter_number,
            total_chapters,
            chapter_id_payload,
            request.style_id,
            normalized_target_word_count,
            request.enable_analysis,
            total_chapters,
            None,
            None,
            request.max_retries,
            now,
        );

        assert_eq!(total_chapters, 2);
        assert_eq!(response_payload["batch_id"], "task-1");
        assert_eq!(task.id, sea_orm::Set("task-1".to_string()));
        assert_eq!(task.total_chapters, sea_orm::Set(2));
        assert_eq!(chapter_ids, vec!["chapter-1".to_string(), "chapter-2".to_string()]);
        assert_eq!(normalized_target_word_count, 2800);
    }

    #[test]
    fn should_keep_batch_generation_create_runtime_seed_contract() {
        let chapters_to_generate = vec![
            build_chapter_target("chapter-1", 1, "First"),
            build_chapter_target("chapter-2", 2, "Second"),
        ];
        let normalized_target_word_count = 2800;
        let runtime_input = super::BatchGenerationExecutionInput {
            user_id: "user-1".to_string(),
            chapter_ids: chapters_to_generate
                .iter()
                .map(|target| target.id.clone())
                .collect::<Vec<_>>(),
            target_word_count: normalized_target_word_count,
            compat_options: crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default(),
            ai_config: crate::ai::AIConfig::default(),
        };

        assert_eq!(runtime_input.user_id, "user-1");
        assert_eq!(
            runtime_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(runtime_input.target_word_count, 2800);
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

        let payload = batch_generation_runtime_state_payload(&runtime_state);

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
        let payload = batch_generation_runtime_state_payload(
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

        assert_eq!(payload["active_story_repair_payload"]["summary"], "手工摘要");
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
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 85);
        assert_eq!(
            payload["quality_history_context"],
            json!({
                "recent_metrics": [{"overall_score": 85}],
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

        assert_eq!(payload["active_story_repair_payload"]["summary"], "历史摘要");
        assert_eq!(
            payload["active_story_repair_payload"]["source"],
            "recent_history_summary"
        );
        assert_eq!(
            payload["quality_history_context"],
            json!({
                "recent_metrics": [{"overall_score": 88}]
            })
        );
        assert_eq!(payload["quality_metrics_summary"]["overall_score"], 88);
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
        let aggregated = aggregate_story_repair_quality_summaries(
            &[first_summary, second_summary],
            "batch",
        )
        .expect("aggregated batch summary");

        let payload = build_batch_generation_runtime_state_payload_from_parts(
            &BatchGenerationRequestRuntimeState::new(
                crate::services::chapter_single_generation_prepare_service::SingleChapterGenerationCompatOptions::default(),
                None,
            ),
            Some(&aggregated),
        );
        let compat = resolve_batch_generation_runtime_compat_options_from_seed(
            &BatchGenerationRequestRuntimeState::default(),
            &payload,
        );

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
        let compat = resolve_batch_generation_runtime_compat_options_from_seed(
            &runtime_state,
            &payload,
        );

        assert_eq!(compat.story_repair_summary(), "沿用批量历史修复建议");
        assert_eq!(
            compat.story_repair_targets(),
            &["压缩说明".to_string(), "提前冲突".to_string()]
        );
        assert_eq!(
            compat.story_preserve_strengths(),
            &["尾章钩子".to_string()]
        );
    }

    #[test]
    fn should_build_resume_batch_generation_execution_input_from_execution_owner() {
        let response_payload = serde_json::json!({
            "batch_id": "task-9",
            "message": "Batch generation resumed",
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
        assert_eq!(response_payload["message"], "Batch generation resumed");
        assert_eq!(execution_input.user_id, "user-1");
        assert_eq!(
            execution_input.chapter_ids,
            vec!["chapter-1".to_string(), "chapter-2".to_string()]
        );
        assert_eq!(execution_input.target_word_count, 2800);
        assert_eq!(execution_input.ai_config.provider, crate::ai::AIConfig::default().provider);
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
