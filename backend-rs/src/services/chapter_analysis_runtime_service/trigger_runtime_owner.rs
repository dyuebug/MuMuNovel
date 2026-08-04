use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait};
use serde_json::{json, Value};

use crate::ai::service::AIService;
use crate::models::{analysis_task, chapter, project};
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
use crate::services::chapter_analysis_runtime_service::analysis_payload_owner::{
    build_analysis_runtime_chapter_model, build_generated_chapter_analysis_overrides,
    ChapterAnalysisRuntimeOverrides,
};
use crate::services::chapter_analysis_runtime_service::persistence_owner::persist_chapter_analysis_result;
use crate::services::chapter_analysis_runtime_service::query_owner::analysis_task_status_payload;
use crate::services::chapter_analysis_service::{
    apply_analysis_task_state_by_id, build_analysis_task_active_model, AnalysisTaskStage,
    CreateChapterAnalysisTaskError,
};
use crate::services::chapter_generation_contract_prepare_service::{
    build_chapter_review_contract, prepare_chapter_analysis_story_packet,
    project_story_packet_to_analysis_prompt_context,
};
use crate::services::chapter_generation_execution_contract_service::prepare_role_aware_generation_execution_config_with_provider_payload;
use crate::services::chapter_generation_prompt_service::build_placeholder_prompt_context_provider_payload;
use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
use crate::services::chapter_service::ChapterService;
use crate::services::controlled_generation_guidance_service::append_controlled_generation_guidance;
use crate::services::cooperative_cancellation_service::CooperativeCancellationToken;
use crate::services::generation_contract_service::GenerationIntentKind;
use crate::services::generation_execution_audit_service::{
    build_generation_execution_audit, GenerationExecutionAuditV1,
    GENERATION_EXECUTION_AUDIT_HISTORY_FIELD,
};
use crate::services::novel_autopilot::failure_diagnostic::NovelAutopilotProviderFailureHint;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::wizard_service::clean_json_response;

#[derive(Debug)]
pub enum PrepareChapterAnalysisTriggerError {
    Chapter(LoadAccessibleChapterError),
    Create(CreateChapterAnalysisTaskError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChapterAnalysisTaskCreateState {
    pub(crate) task_id: String,
    pub(crate) chapter_id: String,
}

impl ChapterAnalysisTaskCreateState {
    pub(crate) fn new(task_id: String, chapter_id: String) -> Self {
        Self {
            task_id,
            chapter_id,
        }
    }

    pub(crate) fn task_id(&self) -> &str {
        &self.task_id
    }

    pub(crate) fn compatibility_payload(&self) -> Value {
        json!({
            "task_id": self.task_id,
            "chapter_id": self.chapter_id,
            "status": "pending",
            "message": "章节分析任务已创建",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedChapterAnalysisTriggerExecution {
    create_state: ChapterAnalysisTaskCreateState,
}

impl PreparedChapterAnalysisTriggerExecution {
    fn new(create_state: ChapterAnalysisTaskCreateState) -> Self {
        Self { create_state }
    }

    pub(crate) fn task_id(&self) -> &str {
        self.create_state.task_id()
    }

    pub(crate) async fn execute(
        self,
        db: &DatabaseConnection,
        user_id: &str,
    ) -> Result<Value, String> {
        execute_prepared_chapter_analysis_trigger(db, user_id, &self.create_state).await
    }

    #[cfg(test)]
    pub(crate) fn from_create_state(create_state: ChapterAnalysisTaskCreateState) -> Self {
        Self::new(create_state)
    }
}

async fn load_created_analysis_task_payload(
    db: &DatabaseConnection,
    create_state: &ChapterAnalysisTaskCreateState,
) -> Result<Value, String> {
    let task = analysis_task::Entity::find_by_id(create_state.task_id())
        .one(db)
        .await
        .map_err(|error| error.to_string())?;

    analysis_task_status_payload(db, &create_state.chapter_id, task)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) fn build_chapter_analysis_task_create_response_payload(
    status_payload: Value,
    create_state: &ChapterAnalysisTaskCreateState,
) -> Value {
    let mut payload = match status_payload {
        Value::Object(payload) => payload,
        _ => serde_json::Map::new(),
    };

    if let Value::Object(summary_fields) = create_state.compatibility_payload() {
        payload.extend(summary_fields);
    }

    Value::Object(payload)
}

async fn build_chapter_analysis_prompt(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
    project_model: &project::Model,
) -> Result<String, String> {
    let template = PromptTemplateService::system_template_info("PLOT_ANALYSIS")
        .ok_or_else(|| "找不到章节分析模板 PLOT_ANALYSIS".to_string())?;
    let packet = prepare_chapter_analysis_story_packet(db, project_model, chapter_model).await?;
    let review_contract = build_chapter_review_contract(packet)?;
    let prompt_context =
        project_story_packet_to_analysis_prompt_context(&review_contract.story_packet)?;

    PromptTemplateService::format_prompt(&template.content, &prompt_context.into_prompt_params())
}

#[derive(Debug)]
struct ExecutedChapterReview {
    content: String,
    audit: GenerationExecutionAuditV1,
    provider_hint: NovelAutopilotProviderFailureHint,
}

#[derive(Debug)]
pub(crate) enum ChapterAnalysisAutopilotRuntimeError {
    Cancelled,
    Context(&'static str),
    Configuration {
        message: String,
        provider_hint: Option<NovelAutopilotProviderFailureHint>,
    },
    Provider {
        message: String,
        provider_hint: NovelAutopilotProviderFailureHint,
    },
    ResponseInvalid {
        provider_hint: NovelAutopilotProviderFailureHint,
    },
}

impl ChapterAnalysisAutopilotRuntimeError {
    fn into_legacy_message(self) -> String {
        match self {
            Self::Cancelled => "chapter analysis was cancelled".to_string(),
            Self::Context(source) => format!("chapter analysis context invalid: {source}"),
            Self::Configuration { message, .. } | Self::Provider { message, .. } => message,
            Self::ResponseInvalid { .. } => "chapter analysis result must be an object".to_string(),
        }
    }
}

fn finalize_chapter_review_prompt(prompt: String, additional_guidance: Option<&str>) -> String {
    append_controlled_generation_guidance(prompt, additional_guidance)
}

async fn execute_chapter_review_prompt(
    db: &DatabaseConnection,
    user_id: &str,
    prompt: String,
    additional_guidance: Option<&str>,
) -> Result<ExecutedChapterReview, ChapterAnalysisAutopilotRuntimeError> {
    let prepared = prepare_role_aware_generation_execution_config_with_provider_payload(
        db,
        user_id,
        GenerationIntentKind::ChapterReview,
        None,
        build_placeholder_prompt_context_provider_payload(),
    )
    .await
    .map_err(
        |message| ChapterAnalysisAutopilotRuntimeError::Configuration {
            message,
            provider_hint: None,
        },
    )?;
    let provider_hint = NovelAutopilotProviderFailureHint::from_ai_config(&prepared.ai_config);
    let role_policy_context = prepared.role_policy_context.ok_or_else(|| {
        ChapterAnalysisAutopilotRuntimeError::Configuration {
            message: "chapter review role policy context is missing".to_string(),
            provider_hint: Some(provider_hint.clone()),
        }
    })?;
    let prompt = finalize_chapter_review_prompt(prompt, additional_guidance);
    let tracked = AIService::new(prepared.ai_config)
        .generate_text_tracked(
            &prompt,
            None,
            None,
            role_policy_context.allow_model_fallback,
        )
        .await
        .map_err(|error| ChapterAnalysisAutopilotRuntimeError::Provider {
            message: error.error.message,
            provider_hint: NovelAutopilotProviderFailureHint {
                http_status: error.error.status_code,
                ..provider_hint.clone()
            },
        })?;
    let audit =
        build_generation_execution_audit(&role_policy_context.resolved_policy, &tracked.execution)
            .map_err(|_| ChapterAnalysisAutopilotRuntimeError::Context("execution_audit"))?;

    Ok(ExecutedChapterReview {
        content: tracked.response.content,
        audit,
        provider_hint,
    })
}

fn attach_chapter_review_execution_audit(
    analysis_result: &mut Value,
    audit: &GenerationExecutionAuditV1,
) -> Result<(), String> {
    let payload = analysis_result
        .as_object_mut()
        .ok_or_else(|| "chapter analysis result must be an object".to_string())?;
    let audit_value = serde_json::to_value(audit).map_err(|error| error.to_string())?;
    payload.insert(
        GENERATION_EXECUTION_AUDIT_HISTORY_FIELD.to_string(),
        audit_value,
    );
    Ok(())
}

pub(crate) async fn generate_chapter_analysis_payload_for_autopilot_typed(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    additional_guidance: Option<&str>,
    cancellation_token: Option<&CooperativeCancellationToken>,
) -> Result<
    (chapter::Model, Value, NovelAutopilotProviderFailureHint),
    ChapterAnalysisAutopilotRuntimeError,
> {
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        return Err(ChapterAnalysisAutopilotRuntimeError::Cancelled);
    }
    let chapter_model = ChapterService::get(db, chapter_id, user_id)
        .await
        .map_err(|_| ChapterAnalysisAutopilotRuntimeError::Context("chapter_load"))?
        .ok_or(ChapterAnalysisAutopilotRuntimeError::Context(
            "chapter_not_found",
        ))?;
    if chapter_model
        .content
        .as_deref()
        .is_none_or(|content| content.trim().is_empty())
    {
        return Err(ChapterAnalysisAutopilotRuntimeError::Context(
            "chapter_content_empty",
        ));
    }
    let project_model = project::Entity::find_by_id(&chapter_model.project_id)
        .one(db)
        .await
        .map_err(|_| ChapterAnalysisAutopilotRuntimeError::Context("project_load"))?
        .ok_or(ChapterAnalysisAutopilotRuntimeError::Context(
            "project_not_found",
        ))?;
    if project_model.user_id != user_id {
        return Err(ChapterAnalysisAutopilotRuntimeError::Context(
            "project_not_found",
        ));
    }

    let prompt = build_chapter_analysis_prompt(db, &chapter_model, &project_model)
        .await
        .map_err(|_| ChapterAnalysisAutopilotRuntimeError::Context("prompt_context"))?;
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        return Err(ChapterAnalysisAutopilotRuntimeError::Cancelled);
    }
    let executed = execute_chapter_review_prompt(db, user_id, prompt, additional_guidance).await?;
    if cancellation_token.is_some_and(CooperativeCancellationToken::is_cancelled) {
        return Err(ChapterAnalysisAutopilotRuntimeError::Cancelled);
    }
    let cleaned = clean_json_response(&executed.content);
    let parsed: Value = serde_json::from_str(&cleaned).map_err(|_| {
        ChapterAnalysisAutopilotRuntimeError::ResponseInvalid {
            provider_hint: executed.provider_hint.clone(),
        }
    })?;
    if !parsed.is_object() {
        return Err(ChapterAnalysisAutopilotRuntimeError::ResponseInvalid {
            provider_hint: executed.provider_hint,
        });
    }
    Ok((chapter_model, parsed, executed.provider_hint))
}

async fn execute_and_persist_chapter_review(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_model: &chapter::Model,
    project_model: &project::Model,
    task_id: &str,
) -> Result<Value, String> {
    let prompt = build_chapter_analysis_prompt(db, chapter_model, project_model).await?;
    let executed = execute_chapter_review_prompt(db, user_id, prompt, None)
        .await
        .map_err(ChapterAnalysisAutopilotRuntimeError::into_legacy_message)?;
    let cleaned = clean_json_response(&executed.content);
    let parsed: Value =
        serde_json::from_str(&cleaned).map_err(|error| format!("JSON解析失败: {}", error))?;
    let mut persisted =
        persist_chapter_analysis_result(db, user_id, chapter_model, task_id, &parsed).await?;
    attach_chapter_review_execution_audit(&mut persisted, &executed.audit)?;
    Ok(persisted)
}

async fn mark_analysis_task_running(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<(), sea_orm::DbErr> {
    let _ = apply_analysis_task_state_by_id(
        db,
        task_id,
        AnalysisTaskStage::Running,
        None,
        Utc::now().naive_utc(),
    )
    .await?;
    Ok(())
}

async fn mark_analysis_task_failed(
    db: &DatabaseConnection,
    task_id: &str,
    error_message: String,
) -> Result<(), sea_orm::DbErr> {
    let _ = apply_analysis_task_state_by_id(
        db,
        task_id,
        AnalysisTaskStage::Failed,
        Some(error_message),
        Utc::now().naive_utc(),
    )
    .await?;
    Ok(())
}

async fn create_chapter_analysis_task(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_model: &chapter::Model,
) -> Result<ChapterAnalysisTaskCreateState, CreateChapterAnalysisTaskError> {
    let chapter_content = chapter_model.content.clone().unwrap_or_default();
    if chapter_content.trim().is_empty() {
        return Err(CreateChapterAnalysisTaskError::ChapterEmpty);
    }

    let project_model = project::Entity::find_by_id(&chapter_model.project_id)
        .one(db)
        .await
        .map_err(|error| CreateChapterAnalysisTaskError::Internal(error.to_string()))?
        .ok_or(CreateChapterAnalysisTaskError::ProjectMissing)?;

    if project_model.user_id != user_id {
        return Err(CreateChapterAnalysisTaskError::ProjectMissing);
    }

    let now = Utc::now().naive_utc();
    let task = build_analysis_task_active_model(&chapter_model.id, user_id, &project_model.id, now);

    let task = task
        .insert(db)
        .await
        .map_err(|error| CreateChapterAnalysisTaskError::Internal(error.to_string()))?;

    Ok(ChapterAnalysisTaskCreateState::new(
        task.id,
        chapter_model.id.clone(),
    ))
}

pub(crate) async fn prepare_chapter_analysis_trigger(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<ChapterAnalysisTaskCreateState, PrepareChapterAnalysisTriggerError> {
    let chapter = load_accessible_chapter(db, chapter_id, user_id)
        .await
        .map_err(PrepareChapterAnalysisTriggerError::Chapter)?;

    create_chapter_analysis_task(db, user_id, &chapter)
        .await
        .map_err(PrepareChapterAnalysisTriggerError::Create)
}

pub(crate) async fn prepare_chapter_analysis_execution(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<PreparedChapterAnalysisTriggerExecution, PrepareChapterAnalysisTriggerError> {
    let create_state = prepare_chapter_analysis_trigger(db, chapter_id, user_id).await?;

    Ok(PreparedChapterAnalysisTriggerExecution::new(create_state))
}

pub(crate) fn dispatch_prepared_chapter_analysis_trigger(
    db: DatabaseConnection,
    user_id: String,
    create_state: ChapterAnalysisTaskCreateState,
) {
    tokio::spawn(async move {
        execute_chapter_analysis_background(db, user_id, create_state).await;
    });
}

pub async fn trigger_chapter_analysis_write_workflow(
    db: &DatabaseConnection,
    chapter_id: &str,
    user_id: &str,
) -> Result<Value, PrepareChapterAnalysisTriggerError> {
    let create_state = prepare_chapter_analysis_trigger(db, chapter_id, user_id).await?;
    let payload = load_created_analysis_task_payload(db, &create_state)
        .await
        .map_err(|error| {
            PrepareChapterAnalysisTriggerError::Create(CreateChapterAnalysisTaskError::Internal(
                error,
            ))
        })?;

    dispatch_prepared_chapter_analysis_trigger(
        db.clone(),
        user_id.to_string(),
        create_state.clone(),
    );

    Ok(build_chapter_analysis_task_create_response_payload(
        payload,
        &create_state,
    ))
}

pub async fn analyze_chapter_now(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
) -> Result<Value, CreateChapterAnalysisTaskError> {
    analyze_chapter_now_with_overrides(
        db,
        user_id,
        chapter_id,
        ChapterAnalysisRuntimeOverrides::default(),
    )
    .await
}

pub async fn analyze_chapter_now_with_overrides(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
    overrides: ChapterAnalysisRuntimeOverrides,
) -> Result<Value, CreateChapterAnalysisTaskError> {
    let chapter_model = ChapterService::get(db, chapter_id, user_id)
        .await
        .map_err(CreateChapterAnalysisTaskError::Internal)?
        .ok_or(CreateChapterAnalysisTaskError::ProjectMissing)?;
    let effective_chapter_model = build_analysis_runtime_chapter_model(&chapter_model, &overrides);

    let chapter_content = effective_chapter_model.content.clone().unwrap_or_default();
    if chapter_content.trim().is_empty() {
        return Err(CreateChapterAnalysisTaskError::ChapterEmpty);
    }

    let project_model = project::Entity::find_by_id(&effective_chapter_model.project_id)
        .one(db)
        .await
        .map_err(|error| CreateChapterAnalysisTaskError::Internal(error.to_string()))?
        .ok_or(CreateChapterAnalysisTaskError::ProjectMissing)?;
    if project_model.user_id != user_id {
        return Err(CreateChapterAnalysisTaskError::ProjectMissing);
    }

    let persisted = execute_and_persist_chapter_review(
        db,
        user_id,
        &effective_chapter_model,
        &project_model,
        "",
    )
    .await
    .map_err(CreateChapterAnalysisTaskError::Internal)?;

    Ok(json!({
        "success": true,
        "message": format!(
            "分析完成,提取了{}条记忆",
            persisted["memories_count"].as_u64().unwrap_or(0)
        ),
        "analysis": persisted["analysis"].clone(),
        "memories_count": persisted["memories_count"].clone(),
        "foreshadow_stats": persisted["foreshadow_stats"].clone(),
        "generation_execution_audit": persisted[GENERATION_EXECUTION_AUDIT_HISTORY_FIELD].clone(),
    }))
}

pub(crate) async fn analyze_generated_chapter_follow_up(
    db: &DatabaseConnection,
    user_id: &str,
    generated: &GeneratedChapterResult,
) -> Result<Value, CreateChapterAnalysisTaskError> {
    analyze_chapter_now_with_overrides(
        db,
        user_id,
        &generated.chapter_id,
        build_generated_chapter_analysis_overrides(generated),
    )
    .await
}

async fn perform_prepared_chapter_analysis_trigger(
    db: &DatabaseConnection,
    user_id: &str,
    create_state: &ChapterAnalysisTaskCreateState,
) -> Result<Value, String> {
    let task_id = &create_state.task_id;
    let chapter_id = &create_state.chapter_id;
    mark_analysis_task_running(db, task_id)
        .await
        .map_err(|error| error.to_string())?;

    let chapter_model = ChapterService::get(db, chapter_id, user_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "章节不存在或内容为空".to_string())?;

    let chapter_content = chapter_model.content.clone().unwrap_or_default();
    if chapter_content.trim().is_empty() {
        return Err("章节不存在或内容为空".to_string());
    }

    let project_model = project::Entity::find_by_id(&chapter_model.project_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "项目不存在".to_string())?;

    if project_model.user_id != user_id {
        return Err("项目不存在".to_string());
    }

    execute_and_persist_chapter_review(db, user_id, &chapter_model, &project_model, task_id).await
}

pub(crate) async fn execute_prepared_chapter_analysis_trigger(
    db: &DatabaseConnection,
    user_id: &str,
    create_state: &ChapterAnalysisTaskCreateState,
) -> Result<Value, String> {
    let run = perform_prepared_chapter_analysis_trigger(db, user_id, create_state).await;

    if let Err(error_message) = &run {
        let _ = mark_analysis_task_failed(db, &create_state.task_id, error_message.clone()).await;
    }

    run
}

async fn execute_chapter_analysis_background(
    db: DatabaseConnection,
    user_id: String,
    create_state: ChapterAnalysisTaskCreateState,
) {
    let _ = execute_prepared_chapter_analysis_trigger(&db, &user_id, &create_state).await;
}

pub(crate) fn build_chapter_analysis_trigger_runtime_owner_contract() -> Value {
    json!({
        "owner": "chapter_analysis_runtime_service::trigger_runtime_owner",
        "scope": "analysis_task_create_prompt_build_ai_trigger_and_background_execution",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_analysis_runtime_service.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service/trigger_runtime_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service/analysis_payload_owner.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service/persistence_owner.rs",
            "backend-rs/src/services/chapter_analysis_service.rs"
        ],
        "behavior_contract": {
            "task_create_state_owner": "ChapterAnalysisTaskCreateState",
            "task_prepare_owner": "prepare_chapter_analysis_trigger",
            "task_execution_prepare_owner": "prepare_chapter_analysis_execution",
            "task_dispatch_owner": "dispatch_prepared_chapter_analysis_trigger",
            "background_runtime_owner": "execute_prepared_chapter_analysis_trigger",
            "follow_up_analysis_owner": "analyze_generated_chapter_follow_up",
            "direct_analysis_owner": "analyze_chapter_now_with_overrides",
            "prompt_owner": "build_chapter_analysis_prompt",
            "review_execution_owner": "execute_and_persist_chapter_review",
            "review_generation_intent": "chapter_review",
            "role_aware_config_owner": "prepare_role_aware_generation_execution_config_with_provider_payload",
            "tracked_ai_owner": "AIService::generate_text_tracked",
            "execution_audit_owner": "build_generation_execution_audit",
            "audit_result_boundary": "additive_generation_execution_audit_without_generation_history_write",
            "failed_task_recovery_owner": "mark_analysis_task_failed"
        },
        "validation_boundary": [
            "cargo test chapter_analysis_runtime_service",
            "cargo check"
        ],
        "rollback_boundary": {
            "python_source_map_retained": false,
            "same_round_python_edit_required": false
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        attach_chapter_review_execution_audit,
        build_chapter_analysis_trigger_runtime_owner_contract, finalize_chapter_review_prompt,
    };
    use crate::services::generation_execution_audit_service::{
        GenerationExecutionAuditV1, GENERATION_EXECUTION_AUDIT_HISTORY_FIELD,
        GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION,
    };
    use crate::services::role_model_policy_service::{
        GenerationRole, ModelSelectionSource, ROLE_MODEL_POLICY_SCHEMA_VERSION,
    };

    fn reviewer_audit() -> GenerationExecutionAuditV1 {
        GenerationExecutionAuditV1 {
            schema_version: GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION.to_string(),
            role: GenerationRole::Reviewer,
            policy_schema_version: ROLE_MODEL_POLICY_SCHEMA_VERSION.to_string(),
            policy_digest: "review-policy-digest".to_string(),
            requested_provider: None,
            requested_model: None,
            resolved_provider: "openai".to_string(),
            resolved_model: "review-model".to_string(),
            actual_provider: "openai".to_string(),
            actual_model: "review-model".to_string(),
            provider_source: ModelSelectionSource::GlobalSettings,
            model_source: ModelSelectionSource::GlobalSettings,
            fallbacks: Vec::new(),
            endpoint_summary: None,
        }
    }

    #[test]
    fn should_append_guidance_only_to_autopilot_review_prompt() {
        let base_prompt = "Return strict chapter review JSON.".to_string();

        assert_eq!(
            finalize_chapter_review_prompt(base_prompt.clone(), None),
            base_prompt
        );

        let guided = finalize_chapter_review_prompt(
            base_prompt.clone(),
            Some("加强人物动机，但不能改变 JSON 输出结构"),
        );
        assert!(guided.starts_with(&base_prompt));
        assert!(guided.contains("<autopilot_additional_guidance>"));
        assert!(guided.contains("加强人物动机，但不能改变 JSON 输出结构"));
    }

    #[test]
    fn should_attach_reviewer_audit_to_additive_analysis_result() {
        let mut result = json!({
            "analysis": {"plot_stage": "development"},
            "memories_count": 2,
            "generated_content": {
                "generation_contract": {
                    "input_digest": "writer-input-digest"
                },
                "generation_execution_audit": {
                    "role": "writer",
                    "actual_model": "writer-model"
                }
            }
        });

        attach_chapter_review_execution_audit(&mut result, &reviewer_audit())
            .expect("attach reviewer audit");

        let audit = &result[GENERATION_EXECUTION_AUDIT_HISTORY_FIELD];
        assert_eq!(audit["role"], "reviewer");
        assert_eq!(audit["actual_model"], "review-model");
        assert!(audit.get("prompt").is_none());
        assert!(audit.get("content").is_none());
        let serialized_audit = serde_json::to_string(audit).expect("serialize reviewer audit");
        assert!(!serialized_audit.to_ascii_lowercase().contains("api_key"));
        assert!(!serialized_audit
            .to_ascii_lowercase()
            .contains("authorization"));
        assert!(!serialized_audit.contains("https://"));
        assert_eq!(result["memories_count"], 2);
        assert_eq!(
            result["generated_content"]["generation_contract"]["input_digest"],
            "writer-input-digest"
        );
        assert_eq!(
            result["generated_content"]["generation_execution_audit"]["role"],
            "writer"
        );
        assert_eq!(
            result["generated_content"]["generation_execution_audit"]["actual_model"],
            "writer-model"
        );
    }

    #[test]
    fn should_reject_non_object_analysis_result_for_audit_attachment() {
        let mut result = json!([]);

        let error = attach_chapter_review_execution_audit(&mut result, &reviewer_audit())
            .expect_err("reject non-object result");

        assert_eq!(error, "chapter analysis result must be an object");
    }

    #[test]
    fn should_publish_reviewer_tracked_execution_contract_without_history_write() {
        let contract = build_chapter_analysis_trigger_runtime_owner_contract();
        let behavior = &contract["behavior_contract"];

        assert_eq!(behavior["review_generation_intent"], "chapter_review");
        assert_eq!(
            behavior["role_aware_config_owner"],
            "prepare_role_aware_generation_execution_config_with_provider_payload"
        );
        assert_eq!(
            behavior["tracked_ai_owner"],
            "AIService::generate_text_tracked"
        );
        assert_eq!(
            behavior["execution_audit_owner"],
            "build_generation_execution_audit"
        );
        assert_eq!(
            behavior["audit_result_boundary"],
            "additive_generation_execution_audit_without_generation_history_write"
        );
    }
}
