use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::ai::config::AIConfig;
use crate::ai::service::AIService;
use crate::ai::types::AIResponse;
use crate::models::{chapter, chapter_draft_attempt, generation_history, project};
use crate::services::chapter_candidate_executor_production_adapter_service::{
    chapter_candidate_production_execution_path_name, ChapterCandidateProductionAdapterOutput,
    ChapterCandidateProductionFallbackContext,
};
use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
use crate::services::chapter_candidate_quality_adapter_service::{
    build_chapter_candidate_quality_adapter, ChapterCandidateQualityAdapterContext,
};
use crate::services::chapter_candidate_route_gateway_service::{
    execute_chapter_candidate_route_gateway, ChapterCandidateRouteGatewayConfig,
};
use crate::services::chapter_draft_view_payload_service::build_candidate_draft_payload;
use crate::services::chapter_generation_access_service::{
    load_accessible_chapter_for_generation, LoadAccessibleChapterForGenerationError,
};
use crate::services::chapter_generation_context_compaction_service::compact_generation_context;
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
use crate::services::chapter_generation_prompt_service::{
    build_previous_chapter_prompt_context, build_prompt_with_provider_payload,
    ChapterGenerationPromptOverrides, PreviousChapterPromptContext,
};
use crate::services::chapter_narrative_cleaner_service::{
    contains_chapter_workflow_meta_text, sanitize_generated_narrative_text,
};
use crate::services::chapter_single_generation_candidate_quality_service::{
    build_single_generation_quality_runtime_context,
    compute_single_generation_story_quality_metrics, resolve_single_generation_quality_gate_plan,
};
use crate::services::chapter_story_repair_quality_context_service::resolve_active_story_repair_payload_with_quality_fallback;

const CHAPTER_GENERATION_HISTORY_MODEL: &str = "chapter_generation_v1";
const CHAPTER_GENERATION_HISTORY_LOG_TYPE: &str = "chapter_generation_quality_v1";
const CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH: usize = 500;
const SINGLE_GENERATION_CANDIDATE_MAX_CANDIDATES: i64 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
enum LoadGenerationContextError {
    Chapter(LoadAccessibleChapterForGenerationError),
    ProjectNotFound,
    Internal(String),
}

impl LoadGenerationContextError {
    fn into_runtime_message(self) -> String {
        match self {
            LoadGenerationContextError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFound,
            ) => "Chapter not found".to_string(),
            LoadGenerationContextError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
            ) => "Chapter not found or access denied".to_string(),
            LoadGenerationContextError::Chapter(
                LoadAccessibleChapterForGenerationError::Internal(error),
            )
            | LoadGenerationContextError::Internal(error) => error,
            LoadGenerationContextError::ProjectNotFound => "Project not found".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedChapterResult {
    pub(crate) chapter_id: String,
    pub(crate) chapter_number: i32,
    pub(crate) title: String,
    pub(crate) content: String,
    pub(crate) word_count: i32,
    pub(crate) saved_word_count: i32,
    pub(crate) chapter_status: String,
    pub(crate) content_applied: bool,
    pub(crate) provisional_draft_saved: bool,
    pub(crate) attempt_state: String,
    pub(crate) quality_metrics: Option<Value>,
    pub(crate) quality_gate_action: Option<String>,
    pub(crate) quality_gate_message: Option<String>,
    pub(crate) candidate_draft: Option<Value>,
    pub(crate) candidate_gateway_metadata: Option<Value>,
}

impl Default for GeneratedChapterResult {
    fn default() -> Self {
        Self {
            chapter_id: String::new(),
            chapter_number: 0,
            title: String::new(),
            content: String::new(),
            word_count: 0,
            saved_word_count: 0,
            chapter_status: String::new(),
            content_applied: true,
            provisional_draft_saved: false,
            attempt_state: "generated_from_runtime".to_string(),
            quality_metrics: None,
            quality_gate_action: Some("continue".to_string()),
            quality_gate_message: None,
            candidate_draft: None,
            candidate_gateway_metadata: None,
        }
    }
}

fn build_generated_history_preview(content: &str) -> String {
    content
        .chars()
        .take(CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH)
        .collect()
}

fn build_generated_history_story_runtime_snapshot_from_contract(
    story_runtime_contract: &Value,
) -> Option<Value> {
    let guidance = story_runtime_contract
        .get("guidance")
        .and_then(Value::as_object);
    let blueprint = story_runtime_contract
        .get("blueprint")
        .and_then(Value::as_object);
    if guidance.is_none() && blueprint.is_none() {
        return None;
    }

    let mut snapshot = serde_json::Map::new();
    if let Some(guidance) = guidance {
        for field_name in [
            "creative_mode",
            "story_focus",
            "plot_stage",
            "story_creation_brief",
            "quality_preset",
            "quality_notes",
        ] {
            if let Some(value) = guidance
                .get(field_name)
                .cloned()
                .filter(|value| !value.is_null())
            {
                snapshot.insert(field_name.to_string(), value);
            }
        }
    }

    if let Some(blueprint) = blueprint {
        snapshot.insert(
            "story_long_term_goal".to_string(),
            blueprint
                .get("long_term_goal")
                .cloned()
                .unwrap_or_else(|| json!("")),
        );
        snapshot.insert(
            "chapter_count".to_string(),
            blueprint
                .get("chapter_count")
                .cloned()
                .unwrap_or(Value::Null),
        );
        snapshot.insert(
            "current_chapter_number".to_string(),
            blueprint
                .get("current_chapter_number")
                .cloned()
                .unwrap_or(Value::Null),
        );
        snapshot.insert(
            "target_word_count".to_string(),
            blueprint
                .get("target_word_count")
                .cloned()
                .unwrap_or(Value::Null),
        );
        snapshot.insert(
            "character_focus".to_string(),
            blueprint
                .get("character_focus_names")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "foreshadow_payoff_plan".to_string(),
            blueprint
                .get("foreshadow_payoff_plan")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "character_state_ledger".to_string(),
            blueprint
                .get("character_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "relationship_state_ledger".to_string(),
            blueprint
                .get("relationship_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "foreshadow_state_ledger".to_string(),
            blueprint
                .get("foreshadow_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "organization_state_ledger".to_string(),
            blueprint
                .get("organization_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "career_state_ledger".to_string(),
            blueprint
                .get("career_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
    }

    (!snapshot.is_empty()).then_some(Value::Object(snapshot))
}

fn generated_history_story_runtime_contract(quality_metrics: Option<&Value>) -> Option<Value> {
    quality_metrics
        .and_then(|metrics| metrics.get("story_runtime_contract"))
        .filter(|payload| payload.is_object())
        .cloned()
}

fn generated_history_story_runtime_snapshot(
    quality_metrics: Option<&Value>,
    story_runtime_contract: Option<&Value>,
) -> Option<Value> {
    quality_metrics
        .and_then(|metrics| metrics.get("quality_runtime_context"))
        .and_then(|payload| payload.as_object().filter(|payload| !payload.is_empty()))
        .map(|payload| Value::Object(payload.clone()))
        .or_else(|| {
            story_runtime_contract
                .and_then(build_generated_history_story_runtime_snapshot_from_contract)
        })
}

fn build_generated_chapter_quality_history_payload(
    content: &str,
    quality_metrics: Option<&Value>,
    candidate_gateway_metadata: Option<&Value>,
    content_applied: bool,
    attempt_state: Option<&str>,
    created_at: chrono::NaiveDateTime,
) -> Value {
    let story_runtime_contract = generated_history_story_runtime_contract(quality_metrics);
    let story_runtime_snapshot =
        generated_history_story_runtime_snapshot(quality_metrics, story_runtime_contract.as_ref());

    let mut payload = serde_json::Map::from_iter([
        (
            "log_type".to_string(),
            json!(CHAPTER_GENERATION_HISTORY_LOG_TYPE),
        ),
        ("content".to_string(), json!(content)),
        (
            "preview".to_string(),
            json!(build_generated_history_preview(content)),
        ),
        (
            "quality_metrics".to_string(),
            quality_metrics.cloned().unwrap_or(Value::Null),
        ),
        (
            "generated_at".to_string(),
            json!(created_at.format("%Y-%m-%dT%H:%M:%S").to_string()),
        ),
        ("content_applied".to_string(), json!(content_applied)),
        (
            "attempt_state".to_string(),
            json!(resolve_generated_history_attempt_state(
                content_applied,
                attempt_state,
            )),
        ),
    ]);

    if let Some(story_runtime_snapshot) = story_runtime_snapshot {
        payload.insert("story_runtime_snapshot".to_string(), story_runtime_snapshot);
    }
    if let Some(story_runtime_contract) = story_runtime_contract {
        payload.insert("story_runtime_contract".to_string(), story_runtime_contract);
    }
    if let Some(candidate_gateway_metadata) = candidate_gateway_metadata {
        payload.insert(
            "candidate_gateway".to_string(),
            candidate_gateway_metadata.clone(),
        );
    }

    Value::Object(payload)
}

#[derive(Debug, Clone, PartialEq)]
struct ChapterGenerationRuntimeContext {
    chapter_model: chapter::Model,
    project_model: project::Model,
    previous_chapter: Option<chapter::Model>,
    previous_chapter_prompt_context: PreviousChapterPromptContext,
}

impl ChapterGenerationRuntimeContext {
    fn build_generated_history_payload(
        result: &GeneratedChapterResult,
        created_at: chrono::NaiveDateTime,
    ) -> Value {
        build_generated_chapter_quality_history_payload(
            &result.content,
            result.quality_metrics.as_ref(),
            result.candidate_gateway_metadata.as_ref(),
            result.content_applied,
            Some(&result.attempt_state),
            created_at,
        )
    }

    fn build_generated_history_model(
        &self,
        prompt: String,
        result: &GeneratedChapterResult,
        created_at: chrono::NaiveDateTime,
    ) -> generation_history::ActiveModel {
        generation_history::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(self.chapter_model.project_id.clone()),
            chapter_id: Set(Some(self.chapter_model.id.clone())),
            prompt: Set(Some(prompt)),
            generated_content: Set(Some(
                Self::build_generated_history_payload(result, created_at).to_string(),
            )),
            model: Set(Some(CHAPTER_GENERATION_HISTORY_MODEL.to_string())),
            tokens_used: Set(None),
            generation_time: Set(None),
            created_at: Set(Some(created_at)),
        }
    }

    fn build_generated_result(
        &self,
        response: AIResponse,
    ) -> Result<GeneratedChapterResult, String> {
        self.build_generated_result_from_content(response.content)
    }

    fn build_generated_result_from_content(
        &self,
        content: String,
    ) -> Result<GeneratedChapterResult, String> {
        let (cleaned_content, _) = sanitize_generated_narrative_text(&content);
        if cleaned_content.trim().is_empty() {
            return Err(
                "chapter generation produced empty narrative after sanitization".to_string(),
            );
        }
        if contains_chapter_workflow_meta_text(&cleaned_content) {
            return Err("chapter generation produced workflow/meta text".to_string());
        }
        let word_count = cleaned_content.chars().count() as i32;

        Ok(GeneratedChapterResult {
            chapter_id: self.chapter_model.id.clone(),
            chapter_number: self.chapter_model.chapter_number,
            title: self.chapter_model.title.clone(),
            content: cleaned_content,
            word_count,
            saved_word_count: word_count,
            chapter_status: "completed".to_string(),
            content_applied: true,
            provisional_draft_saved: false,
            attempt_state: "applied".to_string(),
            quality_metrics: None,
            quality_gate_action: Some("continue".to_string()),
            quality_gate_message: None,
            candidate_draft: None,
            candidate_gateway_metadata: None,
        })
    }

    fn build_generated_result_from_candidate(
        &self,
        candidate: &Value,
    ) -> Result<GeneratedChapterResult, String> {
        let content = single_generation_candidate_gateway_content(candidate)?;
        let mut result = self.build_generated_result_from_content(content)?;
        let quality_metrics = candidate
            .get("quality_metrics")
            .filter(|payload| payload.is_object())
            .cloned();
        let quality_gate_action =
            generated_result_quality_gate_action(candidate, quality_metrics.as_ref());
        let quality_gate_message =
            generated_result_quality_gate_message(candidate, quality_metrics.as_ref());
        let quality_gate_requires_followup =
            !matches!(quality_gate_action.as_deref(), None | Some("continue"));
        let provisional_draft_saved = matches!(quality_gate_action.as_deref(), Some("retry"));

        result.quality_metrics = quality_metrics.clone();
        result.quality_gate_action = quality_gate_action.clone();
        result.quality_gate_message = quality_gate_message;
        result.content_applied = !quality_gate_requires_followup;
        result.provisional_draft_saved = provisional_draft_saved;
        result.attempt_state = if result.content_applied {
            "applied".to_string()
        } else {
            quality_gate_action
                .clone()
                .unwrap_or_else(|| "candidate".to_string())
        };
        result.chapter_status = if result.content_applied {
            "completed".to_string()
        } else if provisional_draft_saved {
            "draft".to_string()
        } else {
            self.chapter_model.status.clone()
        };

        if quality_gate_requires_followup {
            let draft_attempt = build_single_generation_candidate_draft_attempt(
                &self.chapter_model,
                &result,
                self.chapter_model.content.as_deref().unwrap_or_default(),
                self.chapter_model.word_count,
            );
            result.candidate_draft = Some(build_candidate_draft_payload(
                &draft_attempt,
                self.chapter_model.updated_at,
                false,
            ));
        }

        Ok(result)
    }

    async fn persist_generated_result(
        self,
        db: &DatabaseConnection,
        prompt: String,
        mut result: GeneratedChapterResult,
    ) -> Result<GeneratedChapterResult, String> {
        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(|error| error.to_string())?;
        let previous_word_count = self.chapter_model.word_count.max(0);
        let should_persist_content = result.content_applied || result.provisional_draft_saved;

        let history = self.build_generated_history_model(prompt, &result, now);
        if should_persist_content {
            let mut active: chapter::ActiveModel = self.chapter_model.clone().into();
            active.content = Set(Some(result.content.clone()));
            active.word_count = Set(result.word_count);
            active.status = Set(result.chapter_status.clone());
            active.updated_at = Set(Some(now));
            active
                .update(&txn)
                .await
                .map_err(|error| error.to_string())?;
            result.saved_word_count = result.word_count;
        } else {
            result.saved_word_count = previous_word_count;
        }

        if !result.content_applied {
            let draft_attempt = build_single_generation_candidate_draft_attempt(
                &self.chapter_model,
                &result,
                self.chapter_model.content.as_deref().unwrap_or_default(),
                previous_word_count,
            );
            let draft_summary =
                build_candidate_draft_payload(&draft_attempt, self.chapter_model.updated_at, false);
            chapter_draft_attempt::ActiveModel {
                id: Set(draft_attempt.id.clone()),
                project_id: Set(draft_attempt.project_id.clone()),
                chapter_id: Set(draft_attempt.chapter_id.clone()),
                batch_task_id: Set(draft_attempt.batch_task_id.clone()),
                source: Set(draft_attempt.source.clone()),
                attempt_state: Set(draft_attempt.attempt_state.clone()),
                quality_gate_action: Set(draft_attempt.quality_gate_action.clone()),
                quality_gate_decision: Set(draft_attempt.quality_gate_decision.clone()),
                word_count: Set(draft_attempt.word_count),
                summary_preview: Set(draft_attempt.summary_preview.clone()),
                content_preview: Set(draft_attempt.content_preview.clone()),
                quality_metrics: Set(draft_attempt.quality_metrics.clone()),
                repair_payload: Set(draft_attempt.repair_payload.clone()),
                created_at: Set(draft_attempt.created_at),
            }
            .insert(&txn)
            .await
            .map_err(|error| error.to_string())?;
            result.candidate_draft = Some(draft_summary);
        }

        history
            .insert(&txn)
            .await
            .map_err(|error| error.to_string())?;

        txn.commit().await.map_err(|error| error.to_string())?;

        Ok(result)
    }

    fn build_prompt(
        &self,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
    ) -> Result<String, String> {
        let (provider_payload, previous_chapter_prompt_context) = compact_generation_context(
            &self.project_model.outline_mode,
            target_word_count,
            provider_payload,
            self.previous_chapter_prompt_context.clone(),
        );
        build_prompt_with_provider_payload(
            &self.chapter_model,
            &self.project_model,
            previous_chapter_prompt_context,
            self.previous_chapter.is_some(),
            target_word_count,
            provider_payload,
            overrides,
        )
    }

    async fn generate_and_persist(
        self,
        db: &DatabaseConnection,
        user_ai_service: &AIService,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
    ) -> Result<GeneratedChapterResult, String> {
        let prompt = self.build_prompt(target_word_count, provider_payload, overrides)?;
        let response = user_ai_service
            .generate_text(&prompt, None, None)
            .await
            .map_err(|error| error.to_string())?;

        let generated_result = self.build_generated_result(response)?;
        self.persist_generated_result(db, prompt, generated_result)
            .await
    }

    async fn generate_and_persist_with_candidate_route_gateway(
        self,
        db: &DatabaseConnection,
        ai_config: AIConfig,
        target_word_count: i32,
        provider_payload: PromptContextProviderPayload,
        overrides: &ChapterGenerationPromptOverrides,
        gateway_config: ChapterCandidateRouteGatewayConfig,
    ) -> Result<GeneratedChapterResult, String> {
        let prompt = self.build_prompt(target_word_count, provider_payload, overrides)?;
        let mut request = build_single_generation_candidate_executor_request(
            &prompt,
            target_word_count,
            &ai_config,
        );
        let fallback_prompt = prompt.clone();
        let fallback_ai_config = ai_config.clone();

        let output = execute_chapter_candidate_route_gateway(
            &mut request,
            ai_config,
            build_single_generation_candidate_quality_adapter(&self, target_word_count),
            gateway_config,
            move |_request, context| {
                Box::pin(async move {
                    generate_single_generation_direct_fallback_candidate(
                        fallback_ai_config,
                        fallback_prompt,
                        context,
                    )
                    .await
                })
            },
        )
        .await?;

        let mut result = self.build_generated_result_from_candidate(&output.result)?;
        result.candidate_gateway_metadata =
            Some(build_single_generation_candidate_gateway_metadata(&output));
        self.persist_generated_result(db, prompt, result).await
    }
}

pub(crate) fn build_single_generation_candidate_executor_request(
    prompt: &str,
    target_word_count: i32,
    ai_config: &AIConfig,
) -> ChapterCandidateExecutorRequest {
    ChapterCandidateExecutorRequest {
        base_generate_kwargs: Map::from_iter([
            ("prompt".to_string(), json!(prompt)),
            ("temperature".to_string(), json!(ai_config.temperature)),
            ("max_tokens".to_string(), json!(ai_config.max_tokens)),
        ]),
        target_word_count: i64::from(target_word_count),
        source: "chapter".to_string(),
        generation_label: "single_generation_candidate".to_string(),
        max_candidates: SINGLE_GENERATION_CANDIDATE_MAX_CANDIDATES,
        runtime_state: None,
    }
}

fn build_single_generation_candidate_quality_adapter(
    context: &ChapterGenerationRuntimeContext,
    target_word_count: i32,
) -> crate::services::chapter_candidate_quality_adapter_service::ChapterCandidateQualityAdapter<
    impl FnMut(
        crate::services::chapter_candidate_quality_adapter_service::CandidateQualityRuntimeContextBuildInput,
    ) -> Value,
    impl FnMut(
        crate::services::chapter_candidate_quality_adapter_service::CandidateStoryQualityMetricsInput,
    ) -> Value,
    impl FnMut(
        crate::services::chapter_candidate_quality_adapter_service::CandidateQualityGatePlanInput,
    ) -> Value,
>{
    let project_payload = json!({
        "id": context.project_model.id.clone(),
        "title": context.project_model.title.clone(),
        "world_rules": context.project_model.world_rules.clone(),
        "outline_mode": context.project_model.outline_mode.clone(),
    });
    let chapter_payload = json!({
        "id": context.chapter_model.id.clone(),
        "title": context.chapter_model.title.clone(),
        "chapter_number": context.chapter_model.chapter_number,
        "summary": context.chapter_model.summary.clone(),
        "expansion_plan": context.chapter_model.expansion_plan.clone(),
    });
    let chapter_context = json!({
        "chapter_outline": context.chapter_model.expansion_plan
            .as_deref()
            .or(context.chapter_model.summary.as_deref())
            .unwrap_or(""),
        "previous_chapter_continuation_point": context
            .previous_chapter_prompt_context
            .continuation_point
            .clone(),
        "previous_chapter_content": context
            .previous_chapter_prompt_context
            .previous_chapter_content
            .clone(),
    });

    build_chapter_candidate_quality_adapter(
        ChapterCandidateQualityAdapterContext {
            story_packet: Value::Null,
            project: project_payload,
            chapter: chapter_payload,
            chapter_context,
            target_word_count: i64::from(target_word_count),
            generation_intent: json!({"mode": "single_generation_active_route"}),
            retry_count: 0,
            max_retries: 1,
            current_story_repair_payload: None,
            scope: "chapter".to_string(),
            log_prefix: "SingleGeneration".to_string(),
        },
        build_single_generation_quality_runtime_context,
        compute_single_generation_story_quality_metrics,
        resolve_single_generation_quality_gate_plan,
    )
}

async fn generate_single_generation_direct_fallback_candidate(
    ai_config: AIConfig,
    prompt: String,
    context: ChapterCandidateProductionFallbackContext,
) -> Result<Value, String> {
    let response = AIService::new(ai_config)
        .generate_text(&prompt, None, None)
        .await
        .map_err(|error| error.to_string())?;

    Ok(json!({
        "full_content": response.content,
        "generation_path": "direct_generation_fallback",
        "fallback_reason": context.reason,
        "rollback_boundary": context.rollback_boundary,
        "rust_error": context.rust_error,
    }))
}

fn build_single_generation_candidate_gateway_metadata(
    output: &ChapterCandidateProductionAdapterOutput,
) -> Value {
    json!({
        "execution_path": chapter_candidate_production_execution_path_name(output.decision.path),
        "fallback_applied": output.fallback_applied,
        "fallback_reason": output.decision.reason,
        "rollback_boundary": output.decision.rollback_boundary,
        "rust_error": output.rust_error,
    })
}

pub(crate) fn single_generation_candidate_gateway_content(
    candidate: &Value,
) -> Result<String, String> {
    candidate
        .get("full_content")
        .or_else(|| candidate.get("content"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| "candidate route gateway returned empty generated content".to_string())
}

pub(crate) fn build_generated_chapter_history_payload_with_quality_metrics(
    content: &str,
    quality_metrics: Option<&Value>,
    candidate_gateway_metadata: Option<&Value>,
    content_applied: bool,
    attempt_state: Option<&str>,
    created_at: chrono::NaiveDateTime,
) -> Value {
    build_generated_chapter_quality_history_payload(
        content,
        quality_metrics,
        candidate_gateway_metadata,
        content_applied,
        attempt_state,
        created_at,
    )
}

pub(crate) async fn update_latest_generated_chapter_history_quality_metrics(
    db: &DatabaseConnection,
    chapter_id: &str,
    content: &str,
    quality_metrics: &Value,
) -> Result<(), String> {
    let Some(history_model) = generation_history::Entity::find()
        .filter(generation_history::Column::ChapterId.eq(Some(chapter_id.to_string())))
        .filter(
            generation_history::Column::Model
                .eq(Some(CHAPTER_GENERATION_HISTORY_MODEL.to_string())),
        )
        .order_by_desc(generation_history::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(());
    };

    let (content_applied, attempt_state) =
        history_payload_persistence_flags(history_model.generated_content.as_deref());
    let candidate_gateway_metadata =
        history_payload_candidate_gateway_metadata(history_model.generated_content.as_deref());
    let mut active: generation_history::ActiveModel = history_model.into();
    let created_at = active
        .created_at
        .clone()
        .take()
        .flatten()
        .unwrap_or_else(|| Utc::now().naive_utc());
    active.generated_content = Set(Some(
        build_generated_chapter_history_payload_with_quality_metrics(
            content,
            Some(quality_metrics),
            candidate_gateway_metadata.as_ref(),
            content_applied,
            attempt_state.as_deref(),
            created_at,
        )
        .to_string(),
    ));
    active.update(db).await.map_err(|error| error.to_string())?;
    Ok(())
}

fn resolve_generated_history_attempt_state(
    content_applied: bool,
    attempt_state: Option<&str>,
) -> String {
    let trimmed = attempt_state.unwrap_or_default().trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }

    if content_applied {
        "applied".to_string()
    } else {
        "candidate".to_string()
    }
}

fn generated_result_quality_gate_action(
    candidate: &Value,
    quality_metrics: Option<&Value>,
) -> Option<String> {
    let action = candidate
        .get("quality_gate_action")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            candidate
                .get("quality_gate_plan")
                .and_then(|payload| payload.get("action"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        });
    if action.is_some() {
        return action;
    }

    match quality_metrics
        .and_then(|metrics| metrics.get("quality_gate"))
        .and_then(|payload| payload.get("decision"))
        .and_then(Value::as_str)
        .map(str::trim)
    {
        Some("passed") | Some("continue") | Some("allow_save") => Some("continue".to_string()),
        Some("auto_repair") | Some("repair") | Some("retry") => Some("retry".to_string()),
        Some("manual_review") => Some("manual_review".to_string()),
        Some(other) if !other.is_empty() => Some(other.to_string()),
        _ => Some("continue".to_string()),
    }
}

fn generated_result_quality_gate_message(
    candidate: &Value,
    quality_metrics: Option<&Value>,
) -> Option<String> {
    candidate
        .get("quality_gate_message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            quality_metrics
                .and_then(|metrics| metrics.get("quality_gate"))
                .and_then(|payload| {
                    payload
                        .get("summary")
                        .or_else(|| payload.get("label"))
                        .and_then(Value::as_str)
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

fn generated_result_quality_gate_decision(quality_metrics: Option<&Value>) -> Option<String> {
    quality_metrics
        .and_then(|metrics| metrics.get("quality_gate"))
        .and_then(|payload| payload.get("decision"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn build_single_generation_candidate_draft_attempt(
    chapter_model: &chapter::Model,
    result: &GeneratedChapterResult,
    previous_content: &str,
    previous_word_count: i32,
) -> chapter_draft_attempt::Model {
    let mut repair_payload = resolve_active_story_repair_payload_with_quality_fallback(
        None,
        result.quality_metrics.as_ref(),
        result.quality_metrics.as_ref(),
        "chapter",
        "plot_analysis",
        "Plot analysis",
    )
    .and_then(|payload| payload.as_object().cloned())
    .unwrap_or_default();
    repair_payload.insert(
        "previous_content".to_string(),
        json!(previous_content.trim()),
    );
    repair_payload.insert(
        "previous_word_count".to_string(),
        json!(previous_word_count.max(0)),
    );
    repair_payload.insert(
        "candidate_full_content".to_string(),
        json!(result.content.clone()),
    );
    repair_payload.insert("content_complete".to_string(), json!(true));

    chapter_draft_attempt::Model {
        id: Uuid::new_v4().to_string(),
        project_id: chapter_model.project_id.clone(),
        chapter_id: Some(chapter_model.id.clone()),
        batch_task_id: None,
        source: "chapter".to_string(),
        attempt_state: result.attempt_state.clone(),
        quality_gate_action: result.quality_gate_action.clone(),
        quality_gate_decision: generated_result_quality_gate_decision(
            result.quality_metrics.as_ref(),
        ),
        word_count: result.word_count.max(0),
        summary_preview: Some(result.content.chars().take(220).collect::<String>()),
        content_preview: Some(result.content.chars().take(4000).collect::<String>()),
        quality_metrics: result.quality_metrics.clone(),
        repair_payload: Some(Value::Object(repair_payload)),
        created_at: Some(Utc::now().naive_utc()),
    }
}

fn history_payload_persistence_flags(generated_content: Option<&str>) -> (bool, Option<String>) {
    let Some(generated_content) = generated_content else {
        return (true, Some("generated_from_runtime".to_string()));
    };
    let Ok(payload) = serde_json::from_str::<Value>(generated_content) else {
        return (true, Some("generated_from_runtime".to_string()));
    };
    let content_applied = payload
        .get("content_applied")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let attempt_state = payload
        .get("attempt_state")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    (content_applied, attempt_state)
}

fn history_payload_candidate_gateway_metadata(generated_content: Option<&str>) -> Option<Value> {
    let generated_content = generated_content?;
    let payload = serde_json::from_str::<Value>(generated_content).ok()?;
    payload
        .get("candidate_gateway")
        .filter(|metadata| metadata.is_object())
        .cloned()
}

async fn load_generation_context(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
) -> Result<ChapterGenerationRuntimeContext, LoadGenerationContextError> {
    let chapter_model = load_accessible_chapter_for_generation(db, chapter_id, user_id)
        .await
        .map_err(LoadGenerationContextError::Chapter)?;

    let project_model = project::Entity::find_by_id(&chapter_model.project_id)
        .one(db)
        .await
        .map_err(|error| LoadGenerationContextError::Internal(error.to_string()))?
        .ok_or(LoadGenerationContextError::ProjectNotFound)?;

    let previous_chapter = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&chapter_model.project_id))
        .filter(chapter::Column::ChapterNumber.lt(chapter_model.chapter_number))
        .order_by_desc(chapter::Column::ChapterNumber)
        .one(db)
        .await
        .map_err(|error| LoadGenerationContextError::Internal(error.to_string()))?;
    let previous_chapter_prompt_context =
        build_previous_chapter_prompt_context(previous_chapter.as_ref());

    Ok(ChapterGenerationRuntimeContext {
        chapter_model,
        project_model,
        previous_chapter,
        previous_chapter_prompt_context,
    })
}

pub async fn generate_and_persist_chapter_content_with_provider_payload(
    db: &DatabaseConnection,
    user_ai_service: &AIService,
    user_id: &str,
    chapter_id: &str,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
) -> Result<GeneratedChapterResult, String> {
    load_generation_context(db, user_id, chapter_id)
        .await
        .map_err(LoadGenerationContextError::into_runtime_message)?
        .generate_and_persist(
            db,
            user_ai_service,
            target_word_count,
            provider_payload,
            overrides,
        )
        .await
}

pub(crate) async fn generate_and_persist_chapter_content_with_candidate_route_gateway(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
    ai_config: AIConfig,
    gateway_config: ChapterCandidateRouteGatewayConfig,
) -> Result<GeneratedChapterResult, String> {
    load_generation_context(db, user_id, chapter_id)
        .await
        .map_err(LoadGenerationContextError::into_runtime_message)?
        .generate_and_persist_with_candidate_route_gateway(
            db,
            ai_config,
            target_word_count,
            provider_payload,
            overrides,
            gateway_config,
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::{
        build_generated_chapter_history_payload_with_quality_metrics,
        build_previous_chapter_prompt_context, build_single_generation_candidate_executor_request,
        history_payload_candidate_gateway_metadata, single_generation_candidate_gateway_content,
        ChapterGenerationRuntimeContext, GeneratedChapterResult, LoadGenerationContextError,
        CHAPTER_GENERATION_HISTORY_MODEL, CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH,
    };
    use crate::ai::types::AIResponse;
    use crate::models::{chapter, project};
    use crate::services::chapter_generation_access_service::LoadAccessibleChapterForGenerationError;
    use chrono::Utc;
    use serde_json::json;

    fn build_chapter() -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            title: "第一章".to_string(),
            chapter_number: 1,
            content: None,
            summary: None,
            expansion_plan: None,
            status: "pending".to_string(),
            word_count: 0,
            outline_id: None,
            sub_index: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    fn build_project() -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "Project".to_string(),
            description: Some("desc".to_string()),
            theme: None,
            genre: None,
            target_words: 50000,
            current_words: 1200,
            status: "draft".to_string(),
            wizard_status: "idle".to_string(),
            wizard_step: 0,
            outline_mode: "simple".to_string(),
            world_time_period: None,
            world_location: None,
            world_atmosphere: None,
            world_rules: None,
            chapter_count: Some(12),
            narrative_perspective: None,
            character_count: 3,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    fn build_runtime_context() -> ChapterGenerationRuntimeContext {
        ChapterGenerationRuntimeContext {
            chapter_model: build_chapter(),
            project_model: build_project(),
            previous_chapter: None,
            previous_chapter_prompt_context: build_previous_chapter_prompt_context(None),
        }
    }

    #[test]
    fn should_build_generated_chapter_result_from_runtime_context_owner() {
        let response = AIResponse {
            content: "  你好\n世界  ".to_string(),
            tool_calls: None,
            finish_reason: Some("stop".to_string()),
            transport_diagnostics: None,
        };
        let context = build_runtime_context();

        let result = context
            .build_generated_result(response)
            .expect("generated result");

        assert_eq!(result.content, "你好\n世界");
        assert_eq!(result.word_count, 5);
        assert_eq!(result.chapter_id, "chapter-1");
        assert_eq!(result.chapter_number, 1);
    }

    #[test]
    fn should_sanitize_generated_chapter_result_with_rust_narrative_cleaner_owner() {
        let response = AIResponse {
            content: "以下是章节正文：\n\n正常正文第一段。\n\n正常正文第二段。".to_string(),
            tool_calls: None,
            finish_reason: Some("stop".to_string()),
            transport_diagnostics: None,
        };
        let result = build_runtime_context()
            .build_generated_result(response)
            .expect("generated result");

        assert_eq!(result.content, "正常正文第一段。\n\n正常正文第二段。");
        assert_eq!(result.word_count, 18);
    }

    #[test]
    fn should_reject_meta_only_generated_chapter_result_after_sanitization() {
        let response = AIResponse {
            content: "```markdown\n作为AI：我将开始执行\n流程说明".to_string(),
            tool_calls: None,
            finish_reason: Some("stop".to_string()),
            transport_diagnostics: None,
        };
        let error = build_runtime_context()
            .build_generated_result(response)
            .expect_err("meta-only output should be rejected");

        assert_eq!(
            error,
            "chapter generation produced empty narrative after sanitization"
        );
    }

    #[test]
    fn should_keep_generated_chapter_result_transport_fields() {
        let result = GeneratedChapterResult {
            chapter_id: "chapter-1".to_string(),
            chapter_number: 1,
            title: "第一章".to_string(),
            content: "你好\n世界".to_string(),
            word_count: 5,
            ..Default::default()
        };

        assert_eq!(result.chapter_id, "chapter-1");
        assert_eq!(result.chapter_number, 1);
        assert_eq!(result.title, "第一章");
        assert_eq!(result.content, "你好\n世界");
        assert_eq!(result.word_count, 5);
    }

    #[test]
    fn should_build_generated_history_payload_with_quality_metrics_placeholder() {
        let result = GeneratedChapterResult {
            chapter_id: "chapter-1".to_string(),
            chapter_number: 1,
            title: "第一章".to_string(),
            content: "你好\n世界".to_string(),
            word_count: 5,
            ..Default::default()
        };

        let created_at = Utc::now().naive_utc();
        let payload =
            ChapterGenerationRuntimeContext::build_generated_history_payload(&result, created_at);

        assert_eq!(
            payload,
            json!({
                "log_type": "chapter_generation_quality_v1",
                "content": "你好\n世界",
                "preview": "你好\n世界",
                "quality_metrics": null,
                "generated_at": created_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
                "content_applied": true,
                "attempt_state": "generated_from_runtime",
            })
        );
    }

    #[test]
    fn should_keep_chapter_generation_history_model_owner_constant() {
        assert_eq!(CHAPTER_GENERATION_HISTORY_MODEL, "chapter_generation_v1");
    }

    #[test]
    fn should_build_generated_history_payload_with_runtime_quality_metrics() {
        let created_at = Utc::now().naive_utc();
        let payload = build_generated_chapter_history_payload_with_quality_metrics(
            "正文",
            Some(&json!({
                "overall_score": 8.4,
                "repair_guidance": {
                    "summary": "压缩说明段"
                },
                "quality_gate": {
                    "decision": "auto_repair"
                }
            })),
            Some(&json!({
                "execution_path": "rust_candidate_executor",
                "fallback_applied": false,
                "rollback_boundary": "python_candidate_executor_fallback"
            })),
            true,
            Some("generated_from_runtime"),
            created_at,
        );

        assert_eq!(payload["log_type"], "chapter_generation_quality_v1");
        assert_eq!(payload["content"], "正文");
        assert_eq!(payload["preview"], "正文");
        assert_eq!(
            payload["generated_at"],
            created_at.format("%Y-%m-%dT%H:%M:%S").to_string()
        );
        assert_eq!(payload["quality_metrics"]["overall_score"], 8.4);
        assert_eq!(
            payload["quality_metrics"]["repair_guidance"]["summary"],
            "压缩说明段"
        );
        assert_eq!(
            payload["quality_metrics"]["quality_gate"]["decision"],
            "auto_repair"
        );
        assert_eq!(
            payload["candidate_gateway"]["execution_path"],
            "rust_candidate_executor"
        );
        assert_eq!(payload["candidate_gateway"]["fallback_applied"], false);
        assert_eq!(
            history_payload_candidate_gateway_metadata(Some(&payload.to_string()))
                .expect("candidate gateway metadata")["rollback_boundary"],
            "python_candidate_executor_fallback"
        );
        assert_eq!(payload["attempt_state"], "generated_from_runtime");
    }

    #[test]
    fn should_keep_python_compat_runtime_history_metadata_when_contract_exists() {
        let created_at = Utc::now().naive_utc();
        let payload = build_generated_chapter_history_payload_with_quality_metrics(
            "章节正文".repeat(260).as_str(),
            Some(&json!({
                "overall_score": 8.8,
                "quality_runtime_context": {
                    "scope": "chapter",
                    "source": "plot_analysis"
                },
                "story_runtime_contract": {
                    "guidance": {
                        "creative_mode": "hook"
                    },
                    "blueprint": {
                        "current_chapter_number": 6,
                        "target_word_count": 2800
                    }
                }
            })),
            None,
            true,
            Some("generated_from_runtime"),
            created_at,
        );

        assert_eq!(payload["log_type"], "chapter_generation_quality_v1");
        assert_eq!(
            payload["generated_at"],
            created_at.format("%Y-%m-%dT%H:%M:%S").to_string()
        );
        assert_eq!(
            payload["preview"]
                .as_str()
                .expect("preview text")
                .chars()
                .count(),
            CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH
        );
        assert_eq!(
            payload["story_runtime_snapshot"],
            json!({
                "scope": "chapter",
                "source": "plot_analysis"
            })
        );
        assert_eq!(
            payload["story_runtime_contract"]["guidance"]["creative_mode"],
            "hook"
        );
    }

    #[test]
    fn should_derive_story_runtime_snapshot_from_contract_when_metrics_context_missing() {
        let payload = build_generated_chapter_history_payload_with_quality_metrics(
            "正文",
            Some(&json!({
                "overall_score": 7.6,
                "story_runtime_contract": {
                    "guidance": {
                        "story_focus": "advance_plot",
                        "plot_stage": "climax"
                    },
                    "blueprint": {
                        "long_term_goal": "追回失落线索",
                        "chapter_count": 12,
                        "current_chapter_number": 5,
                        "target_word_count": 2600,
                        "character_focus_names": ["沈砚"],
                        "foreshadow_payoff_plan": ["回收暗号"],
                        "character_state_ledger": [],
                        "relationship_state_ledger": [],
                        "foreshadow_state_ledger": [],
                        "organization_state_ledger": [],
                        "career_state_ledger": []
                    }
                }
            })),
            None,
            true,
            Some("generated_from_runtime"),
            Utc::now().naive_utc(),
        );

        assert_eq!(
            payload["story_runtime_snapshot"]["story_focus"],
            "advance_plot"
        );
        assert_eq!(payload["story_runtime_snapshot"]["plot_stage"], "climax");
        assert_eq!(
            payload["story_runtime_snapshot"]["story_long_term_goal"],
            "追回失落线索"
        );
        assert_eq!(payload["story_runtime_snapshot"]["chapter_count"], 12);
        assert_eq!(
            payload["story_runtime_snapshot"]["current_chapter_number"],
            5
        );
        assert_eq!(payload["story_runtime_snapshot"]["target_word_count"], 2600);
        assert_eq!(
            payload["story_runtime_snapshot"]["character_focus"],
            json!(["沈砚"])
        );
        assert_eq!(
            payload["story_runtime_snapshot"]["foreshadow_payoff_plan"],
            json!(["回收暗号"])
        );
    }

    #[test]
    fn should_map_generation_context_access_denied_to_existing_runtime_message() {
        assert_eq!(
            LoadGenerationContextError::Chapter(
                LoadAccessibleChapterForGenerationError::ChapterNotFoundOrAccessDenied,
            )
            .into_runtime_message(),
            "Chapter not found or access denied"
        );
    }

    #[test]
    fn should_map_generation_context_project_missing_to_existing_runtime_message() {
        assert_eq!(
            LoadGenerationContextError::ProjectNotFound.into_runtime_message(),
            "Project not found"
        );
    }

    #[test]
    fn should_keep_generation_context_internal_owner() {
        assert_eq!(
            LoadGenerationContextError::Internal("boom".to_string()).into_runtime_message(),
            "boom"
        );
    }

    #[test]
    fn should_keep_chapter_generation_runtime_context_owner_contract() {
        let context = build_runtime_context();

        assert_eq!(context.chapter_model.id, "chapter-1");
        assert_eq!(context.project_model.id, "project-1");
        assert_eq!(context.previous_chapter, None);
    }

    #[test]
    fn should_build_single_generation_candidate_executor_request_for_active_route_gateway() {
        let mut ai_config = crate::ai::AIConfig::default();
        ai_config.temperature = 0.72;
        ai_config.max_tokens = 4096;

        let request =
            build_single_generation_candidate_executor_request("请生成章节正文", 2400, &ai_config);

        assert_eq!(request.base_generate_kwargs["prompt"], "请生成章节正文");
        assert_eq!(request.base_generate_kwargs["temperature"], 0.72);
        assert_eq!(request.base_generate_kwargs["max_tokens"], 4096);
        assert_eq!(request.target_word_count, 2400);
        assert_eq!(request.source, "chapter");
        assert_eq!(request.generation_label, "single_generation_candidate");
        assert_eq!(request.max_candidates, 1);
        assert!(request.runtime_state.is_none());
    }

    #[test]
    fn should_extract_candidate_gateway_content_from_candidate_or_fallback_payload() {
        let candidate = json!({
            "full_content": "候选章节正文",
            "generation_path": "single_pass"
        });
        let fallback = json!({
            "content": "直接生成正文",
            "generation_path": "direct_generation_fallback"
        });

        assert_eq!(
            single_generation_candidate_gateway_content(&candidate).expect("candidate content"),
            "候选章节正文"
        );
        assert_eq!(
            single_generation_candidate_gateway_content(&fallback).expect("fallback content"),
            "直接生成正文"
        );
    }

    #[test]
    fn should_reject_empty_candidate_gateway_content() {
        let error = single_generation_candidate_gateway_content(&json!({"full_content": " "}))
            .expect_err("empty content should fail");

        assert_eq!(
            error,
            "candidate route gateway returned empty generated content"
        );
    }
}
