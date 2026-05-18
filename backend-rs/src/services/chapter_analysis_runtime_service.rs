use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};
use sea_orm::QuerySelect;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::ai::service::AIService;
use crate::models::{analysis_task, chapter, character, foreshadow, plot_analysis, project};
use crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError;
use crate::services::chapter_service::ChapterService;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;
use crate::services::wizard_service::clean_json_response;

fn json_i32(value: Option<i64>) -> i32 {
    value.unwrap_or_default().clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn json_f64(value: Option<f64>) -> Option<f64> {
    value.filter(|number| number.is_finite())
}

fn normalize_analysis_status(status: &str) -> String {
    match status {
        "pending" | "running" | "completed" | "failed" => status.to_string(),
        _ => "failed".to_string(),
    }
}

fn build_chapter_analysis_report(payload: &Value) -> Option<String> {
    let mut sections = Vec::new();

    if let Some(plot_stage) = payload.get("plot_stage").and_then(Value::as_str) {
        if !plot_stage.trim().is_empty() {
            sections.push(format!("剧情阶段：{}", plot_stage.trim()));
        }
    }

    if let Some(conflict) = payload.get("conflict") {
        let description = conflict
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !description.is_empty() {
            sections.push(format!("冲突分析：{}", description));
        }
    }

    if let Some(scores) = payload.get("scores") {
        let justification = scores
            .get("score_justification")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !justification.is_empty() {
            sections.push(format!("评分说明：{}", justification));
        }
    }

    if let Some(suggestions) = payload.get("suggestions").and_then(Value::as_array) {
        let joined = suggestions
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join("；");
        if !joined.is_empty() {
            sections.push(format!("改进建议：{}", joined));
        }
    }

    if sections.is_empty() {
        None
    } else {
        Some(sections.join("\n"))
    }
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
    if let Some(existing) = analysis_task::Entity::find_by_id(task_id).one(db).await? {
        let mut active: analysis_task::ActiveModel = existing.into();
        active.status = Set("running".to_string());
        active.progress = Set(10);
        active.started_at = Set(Some(Utc::now().naive_utc()));
        active.error_message = Set(None);
        let _ = active.update(db).await?;
    }
    Ok(())
}

async fn mark_analysis_task_failed(
    db: &DatabaseConnection,
    task_id: &str,
    error_message: String,
) -> Result<(), sea_orm::DbErr> {
    if let Some(existing) = analysis_task::Entity::find_by_id(task_id).one(db).await? {
        let mut active: analysis_task::ActiveModel = existing.into();
        active.status = Set("failed".to_string());
        active.progress = Set(0);
        active.error_message = Set(Some(error_message));
        active.completed_at = Set(Some(Utc::now().naive_utc()));
        let _ = active.update(db).await?;
    }
    Ok(())
}

async fn persist_chapter_analysis_result(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
    task_id: &str,
    payload: &Value,
) -> Result<(), String> {
    let now = Utc::now().naive_utc();
    let scores = payload.get("scores").cloned().unwrap_or(Value::Null);
    let conflict = payload.get("conflict").cloned().unwrap_or(Value::Null);
    let emotional_arc = payload.get("emotional_arc").cloned().unwrap_or(Value::Null);

    let analysis = plot_analysis::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(chapter_model.project_id.clone()),
        chapter_id: Set(chapter_model.id.clone()),
        plot_stage: Set(
            payload
                .get("plot_stage")
                .and_then(Value::as_str)
                .map(str::to_string),
        ),
        conflict_level: Set(Some(json_i32(
            conflict.get("level").and_then(Value::as_i64),
        ))),
        conflict_types: Set(conflict.get("types").cloned()),
        emotional_tone: Set(
            emotional_arc
                .get("primary_emotion")
                .and_then(Value::as_str)
                .map(str::to_string),
        ),
        emotional_intensity: Set(json_f64(
            emotional_arc.get("intensity").and_then(Value::as_f64),
        )),
        emotional_curve: Set(
            emotional_arc
                .get("curve")
                .cloned()
                .or_else(|| emotional_arc.get("secondary_emotions").cloned()),
        ),
        hooks: Set(payload.get("hooks").cloned()),
        hooks_count: Set(
            payload
                .get("hooks")
                .and_then(Value::as_array)
                .map(|items| items.len() as i32)
                .unwrap_or(0),
        ),
        hooks_avg_strength: Set(payload.get("hooks").and_then(Value::as_array).and_then(
            |items| {
                let strengths = items
                    .iter()
                    .filter_map(|item| item.get("strength").and_then(Value::as_f64))
                    .collect::<Vec<_>>();
                if strengths.is_empty() {
                    None
                } else {
                    Some(strengths.iter().sum::<f64>() / strengths.len() as f64)
                }
            },
        )),
        foreshadows: Set(payload.get("foreshadows").cloned()),
        foreshadows_planted: Set(
            payload
                .get("foreshadows")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter(|item| item.get("type").and_then(Value::as_str) == Some("planted"))
                        .count() as i32
                })
                .unwrap_or(0),
        ),
        foreshadows_resolved: Set(
            payload
                .get("foreshadows")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter(|item| item.get("type").and_then(Value::as_str) == Some("resolved"))
                        .count() as i32
                })
                .unwrap_or(0),
        ),
        plot_points: Set(payload.get("plot_points").cloned()),
        plot_points_count: Set(
            payload
                .get("plot_points")
                .and_then(Value::as_array)
                .map(|items| items.len() as i32)
                .unwrap_or(0),
        ),
        character_states: Set(payload.get("character_states").cloned()),
        scenes: Set(
            payload
                .get("scenes")
                .cloned()
                .or_else(|| payload.get("serial_rhythm").cloned()),
        ),
        pacing: Set(
            payload
                .get("pacing")
                .and_then(Value::as_str)
                .map(str::to_string),
        ),
        overall_quality_score: Set(json_f64(scores.get("overall").and_then(Value::as_f64))),
        pacing_score: Set(json_f64(scores.get("pacing").and_then(Value::as_f64))),
        engagement_score: Set(json_f64(scores.get("engagement").and_then(Value::as_f64))),
        coherence_score: Set(json_f64(scores.get("coherence").and_then(Value::as_f64))),
        analysis_report: Set(
            build_chapter_analysis_report(payload).or_else(|| Some(payload.to_string())),
        ),
        suggestions: Set(payload.get("suggestions").cloned()),
        word_count: Set(Some(chapter_model.word_count)),
        dialogue_ratio: Set(json_f64(payload.get("dialogue_ratio").and_then(Value::as_f64))),
        description_ratio: Set(json_f64(
            payload.get("description_ratio").and_then(Value::as_f64),
        )),
        created_at: Set(Some(now)),
    };

    analysis.insert(db).await.map_err(|error| error.to_string())?;

    if let Some(existing) = analysis_task::Entity::find_by_id(task_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
    {
        let mut active: analysis_task::ActiveModel = existing.into();
        active.status = Set(normalize_analysis_status("completed"));
        active.progress = Set(100);
        active.completed_at = Set(Some(now));
        active.error_message = Set(None);
        active.update(db).await.map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub async fn create_chapter_analysis_task(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_model: &chapter::Model,
) -> Result<analysis_task::Model, CreateChapterAnalysisTaskError> {
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
    let task = analysis_task::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        chapter_id: Set(chapter_model.id.clone()),
        user_id: Set(user_id.to_string()),
        project_id: Set(project_model.id.clone()),
        status: Set("pending".to_string()),
        progress: Set(0),
        error_message: Set(None),
        created_at: Set(Some(now)),
        started_at: Set(None),
        completed_at: Set(None),
    };

    task.insert(db)
        .await
        .map_err(|error| CreateChapterAnalysisTaskError::Internal(error.to_string()))
}

pub async fn enqueue_chapter_analysis_task(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
) -> Result<Value, CreateChapterAnalysisTaskError> {
    let chapter_model = ChapterService::get(db, chapter_id, user_id)
        .await
        .map_err(CreateChapterAnalysisTaskError::Internal)?
        .ok_or(CreateChapterAnalysisTaskError::ProjectMissing)?;

    let task = create_chapter_analysis_task(db, user_id, &chapter_model).await?;

    Ok(json!({
        "task_id": task.id,
        "chapter_id": chapter_id,
        "status": "pending",
        "message": "章节分析任务已创建",
    }))
}

pub async fn execute_chapter_analysis_background(
    db: DatabaseConnection,
    user_id: String,
    chapter_id: String,
    task_id: String,
) {
    let run = async {
        mark_analysis_task_running(&db, &task_id)
            .await
            .map_err(|error| error.to_string())?;

        let chapter_model = ChapterService::get(&db, &chapter_id, &user_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "章节不存在或内容为空".to_string())?;

        let chapter_content = chapter_model.content.clone().unwrap_or_default();
        if chapter_content.trim().is_empty() {
            return Err("章节不存在或内容为空".to_string());
        }

        let project_model = project::Entity::find_by_id(&chapter_model.project_id)
            .one(&db)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "项目不存在".to_string())?;

        if project_model.user_id != user_id {
            return Err("项目不存在".to_string());
        }

        let prompt = build_chapter_analysis_prompt(&db, &chapter_model, &project_model).await?;
        let ai_config = SettingsService::build_ai_config(&db, &user_id, None, None, None).await?;
        let ai_service = AIService::new(ai_config);
        let response = ai_service
            .generate_text(&prompt, None, None)
            .await
            .map_err(|error| error.to_string())?;

        let cleaned = clean_json_response(&response.content);
        let parsed: Value =
            serde_json::from_str(&cleaned).map_err(|error| format!("JSON解析失败: {}", error))?;

        persist_chapter_analysis_result(&db, &chapter_model, &task_id, &parsed).await
    }
    .await;

    if let Err(error_message) = run {
        let _ = mark_analysis_task_failed(&db, &task_id, error_message).await;
    }
}
