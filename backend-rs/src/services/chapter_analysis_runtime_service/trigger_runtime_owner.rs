use chrono::Utc;
use sea_orm::QuerySelect;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};
use serde_json::{json, Value};

use crate::ai::service::AIService;
use crate::models::{analysis_task, chapter, character, foreshadow, project};
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
use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
use crate::services::chapter_service::ChapterService;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;
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

    let unresolved_foreshadows = foreshadow::Entity::find()
        .filter(foreshadow::Column::ProjectId.eq(&project_model.id))
        .filter(foreshadow::Column::Status.ne("resolved"))
        .order_by_desc(foreshadow::Column::CreatedAt)
        .limit(50)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let existing_foreshadows = if unresolved_foreshadows.is_empty() {
        "[]".to_string()
    } else {
        unresolved_foreshadows
            .iter()
            .map(|item| {
                format!(
                    "- [ID: {}] 标题：{}；埋入章节：{}；内容：{}",
                    item.id,
                    item.title,
                    item.plant_chapter_number
                        .map(|number| number.to_string())
                        .unwrap_or_else(|| "未知".to_string()),
                    item.content.replace('\n', " ")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(&project_model.id))
        .order_by_asc(character::Column::Name)
        .all(db)
        .await
        .map_err(|error| error.to_string())?;

    let characters_info = if characters.is_empty() {
        "[]".to_string()
    } else {
        characters
            .iter()
            .map(|item| {
                format!(
                    "- {}（身份：{}；状态：{}）",
                    item.name,
                    item.role_type
                        .clone()
                        .unwrap_or_else(|| "未设定".to_string()),
                    item.status
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mut params = std::collections::HashMap::new();
    params.insert(
        "chapter_number".to_string(),
        chapter_model.chapter_number.to_string(),
    );
    params.insert("title".to_string(), chapter_model.title.clone());
    params.insert(
        "word_count".to_string(),
        chapter_model.word_count.max(0).to_string(),
    );
    params.insert(
        "content".to_string(),
        chapter_model.content.clone().unwrap_or_default(),
    );
    params.insert("existing_foreshadows".to_string(), existing_foreshadows);
    params.insert("characters_info".to_string(), characters_info);

    PromptTemplateService::format_prompt(&template.content, &params)
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

    let prompt = build_chapter_analysis_prompt(db, &effective_chapter_model, &project_model)
        .await
        .map_err(CreateChapterAnalysisTaskError::Internal)?;
    let ai_config = SettingsService::build_ai_config(db, user_id, None, None, None)
        .await
        .map_err(CreateChapterAnalysisTaskError::Internal)?;
    let ai_service = AIService::new(ai_config);
    let response = ai_service
        .generate_text(&prompt, None, None)
        .await
        .map_err(|error| CreateChapterAnalysisTaskError::Internal(error.to_string()))?;

    let cleaned = clean_json_response(&response.content);
    let parsed: Value = serde_json::from_str(&cleaned).map_err(|error| {
        CreateChapterAnalysisTaskError::Internal(format!("JSON解析失败: {}", error))
    })?;
    let persisted =
        persist_chapter_analysis_result(db, user_id, &effective_chapter_model, "", &parsed)
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

    let prompt = build_chapter_analysis_prompt(db, &chapter_model, &project_model).await?;
    let ai_config = SettingsService::build_ai_config(db, user_id, None, None, None).await?;
    let ai_service = AIService::new(ai_config);
    let response = ai_service
        .generate_text(&prompt, None, None)
        .await
        .map_err(|error| error.to_string())?;

    let cleaned = clean_json_response(&response.content);
    let parsed: Value =
        serde_json::from_str(&cleaned).map_err(|error| format!("JSON解析失败: {}", error))?;

    persist_chapter_analysis_result(db, user_id, &chapter_model, task_id, &parsed).await
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
