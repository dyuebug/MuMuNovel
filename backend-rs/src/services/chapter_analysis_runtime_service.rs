use chrono::Utc;
use sea_orm::QuerySelect;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::{json, Value};
use tracing::warn;
use uuid::Uuid;

use crate::ai::service::AIService;
use crate::models::{
    analysis_task, chapter, character, foreshadow, plot_analysis, project, story_memory,
};
use crate::services::chapter_access_service::{
    load_accessible_chapter, LoadAccessibleChapterError,
};
use crate::services::chapter_analysis_character_state_service::{
    sync_character_states_from_analysis, sync_organization_states_from_analysis,
};
use crate::services::chapter_analysis_query_service::analysis_task_status_payload;
use crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError;
use crate::services::chapter_analysis_task_state_service::{
    apply_analysis_task_state_by_id, build_analysis_task_active_model, AnalysisTaskStage,
};
use crate::services::chapter_generation_runtime_service::update_latest_generated_chapter_history_quality_metrics;
use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
use crate::services::chapter_service::ChapterService;
use crate::services::foreshadow_request_service::build_sync_foreshadow_from_analysis_request_from_route_payload;
use crate::services::foreshadow_request_service::SyncForeshadowFromAnalysisRouteRequest;
use crate::services::foreshadow_service::ForeshadowService;
use crate::services::prompt_template_service::PromptTemplateService;
use crate::services::settings_service::SettingsService;
use crate::services::story_memory_vector_index_service::{
    delete_story_memory_vector_records_by_chapter, upsert_story_memory_vector_record,
};
use crate::services::wizard_service::clean_json_response;

fn json_i32(value: Option<i64>) -> i32 {
    value
        .unwrap_or_default()
        .clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn json_f64(value: Option<f64>) -> Option<f64> {
    value.filter(|number| number.is_finite())
}

fn normalized_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

fn json_bool_as_i32(value: Option<&Value>) -> i32 {
    match value {
        Some(Value::Bool(flag)) => i32::from(*flag),
        Some(Value::Number(number)) => i32::from(number.as_i64().unwrap_or_default() != 0),
        Some(Value::String(text)) => {
            let normalized = text.trim().to_ascii_lowercase();
            i32::from(matches!(normalized.as_str(), "1" | "true" | "yes"))
        }
        _ => 0,
    }
}

fn filtered_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn sanitize_search_text(text: &str) -> String {
    text.chars()
        .filter(|ch| {
            !matches!(
                ch,
                '，' | '。'
                    | '！'
                    | '？'
                    | '、'
                    | '；'
                    | '：'
                    | '"'
                    | '\''
                    | '（'
                    | '）'
                    | '《'
                    | '》'
                    | '【'
                    | '】'
            )
        })
        .collect()
}

fn byte_offset_to_char_index(text: &str, byte_offset: usize) -> i32 {
    text[..byte_offset].chars().count() as i32
}

fn find_text_position(full_text: &str, keyword: &str) -> (i32, i32) {
    let trimmed_keyword = keyword.trim();
    if full_text.trim().is_empty() || trimmed_keyword.is_empty() {
        return (-1, 0);
    }

    if let Some(position) = full_text.find(trimmed_keyword) {
        return (
            byte_offset_to_char_index(full_text, position),
            trimmed_keyword.chars().count() as i32,
        );
    }

    let sanitized_keyword = sanitize_search_text(trimmed_keyword);
    let sanitized_text = sanitize_search_text(full_text);
    if !sanitized_keyword.is_empty() {
        if let Some(position) = sanitized_text.find(&sanitized_keyword) {
            return (
                byte_offset_to_char_index(&sanitized_text, position),
                sanitized_keyword.chars().count() as i32,
            );
        }
    }

    let keyword_chars = trimmed_keyword.chars().count();
    if keyword_chars > 10 {
        let partial = trimmed_keyword.chars().take(15).collect::<String>();
        if let Some(position) = full_text.find(&partial) {
            return (
                byte_offset_to_char_index(full_text, position),
                partial.chars().count() as i32,
            );
        }
    }

    (-1, 0)
}

#[derive(Debug, Clone)]
struct AnalysisMemoryDraft {
    memory_type: String,
    title: Option<String>,
    content: String,
    metadata: Value,
}

fn push_analysis_memory(
    drafts: &mut Vec<AnalysisMemoryDraft>,
    memory_type: &str,
    title: Option<String>,
    content: String,
    metadata: Value,
) {
    let trimmed_content = content.trim();
    if trimmed_content.is_empty() {
        return;
    }

    drafts.push(AnalysisMemoryDraft {
        memory_type: memory_type.to_string(),
        title,
        content: trimmed_content.to_string(),
        metadata,
    });
}

fn extract_analysis_memories(
    chapter_model: &chapter::Model,
    payload: &Value,
) -> Vec<AnalysisMemoryDraft> {
    let mut drafts = Vec::new();
    let chapter_id = chapter_model.id.clone();
    let chapter_number = chapter_model.chapter_number;
    let chapter_title = chapter_model.title.trim();
    let chapter_content = chapter_model.content.clone().unwrap_or_default();

    let chapter_summary = normalized_string(payload.get("summary"))
        .or_else(|| {
            payload
                .get("plot_points")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .take(3)
                        .filter_map(|item| normalized_string(item.get("content")))
                        .collect::<Vec<_>>()
                        .join("；")
                })
                .filter(|item| !item.is_empty())
        })
        .or_else(|| {
            if chapter_content.trim().is_empty() {
                None
            } else {
                let summary = chapter_content.chars().take(300).collect::<String>();
                let needs_suffix = chapter_content.chars().count() > 300;
                Some(if needs_suffix {
                    format!("{}...", summary)
                } else {
                    summary
                })
            }
        });

    if let Some(summary) = chapter_summary {
        push_analysis_memory(
            &mut drafts,
            "chapter_summary",
            Some(format!("第{}章《{}》摘要", chapter_number, chapter_title)),
            summary.clone(),
            json!({
                "chapter_id": chapter_id,
                "chapter_number": chapter_number,
                "importance_score": 0.6,
                "tags": ["摘要", "章节概览", chapter_title],
                "is_foreshadow": 0,
                "text_position": 0,
                "text_length": summary.chars().count() as i32,
            }),
        );
    }

    if let Some(hooks) = payload.get("hooks").and_then(Value::as_array) {
        for hook in hooks {
            let strength = hook.get("strength").and_then(Value::as_f64).unwrap_or(0.0);
            if strength < 6.0 {
                continue;
            }
            let keyword = normalized_string(hook.get("keyword")).unwrap_or_default();
            let (position, length) = find_text_position(&chapter_content, &keyword);
            let hook_type =
                normalized_string(hook.get("type")).unwrap_or_else(|| "未知".to_string());
            let hook_position = normalized_string(hook.get("position")).unwrap_or_default();
            push_analysis_memory(
                &mut drafts,
                "hook",
                Some(format!("{} - {}", hook_type, hook_position)),
                format!(
                    "[{}钩子] {}",
                    hook_type,
                    normalized_string(hook.get("content")).unwrap_or_default()
                ),
                json!({
                    "chapter_id": chapter_id,
                    "chapter_number": chapter_number,
                    "importance_score": (strength / 10.0).min(1.0),
                    "tags": [hook_type, hook_position],
                    "is_foreshadow": 0,
                    "keyword": keyword,
                    "text_position": position,
                    "text_length": length,
                    "strength": strength,
                    "position_desc": hook_position,
                }),
            );
        }
    }

    if let Some(foreshadows) = payload.get("foreshadows").and_then(Value::as_array) {
        for foreshadow in foreshadows {
            let foreshadow_type =
                normalized_string(foreshadow.get("type")).unwrap_or_else(|| "planted".to_string());
            let keyword = normalized_string(foreshadow.get("keyword")).unwrap_or_default();
            let (position, length) = find_text_position(&chapter_content, &keyword);
            let strength = foreshadow
                .get("strength")
                .and_then(Value::as_f64)
                .unwrap_or(5.0);
            push_analysis_memory(
                &mut drafts,
                "foreshadow",
                Some(if foreshadow_type == "planted" {
                    "埋下伏笔".to_string()
                } else {
                    "回收伏笔".to_string()
                }),
                normalized_string(foreshadow.get("content")).unwrap_or_default(),
                json!({
                    "chapter_id": chapter_id,
                    "chapter_number": chapter_number,
                    "importance_score": (strength / 10.0).min(1.0),
                    "tags": ["伏笔", foreshadow_type],
                    "is_foreshadow": if foreshadow_type == "planted" { 1 } else { 2 },
                    "reference_chapter": foreshadow.get("reference_chapter").cloned().unwrap_or(Value::Null),
                    "keyword": keyword,
                    "text_position": position,
                    "text_length": length,
                    "foreshadow_type": foreshadow_type,
                    "strength": strength,
                    "related_characters": filtered_string_array(foreshadow.get("related_characters")),
                }),
            );
        }
    }

    if let Some(plot_points) = payload.get("plot_points").and_then(Value::as_array) {
        for plot_point in plot_points {
            let importance = plot_point
                .get("importance")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            if importance < 0.6 {
                continue;
            }

            let keyword = normalized_string(plot_point.get("keyword")).unwrap_or_default();
            let (position, length) = find_text_position(&chapter_content, &keyword);
            let plot_type =
                normalized_string(plot_point.get("type")).unwrap_or_else(|| "未知".to_string());
            let content = normalized_string(plot_point.get("content")).unwrap_or_default();
            let impact = normalized_string(plot_point.get("impact")).unwrap_or_default();
            push_analysis_memory(
                &mut drafts,
                "plot_point",
                Some(format!("情节点 - {}", plot_type)),
                format!("{}。影响: {}", content, impact),
                json!({
                    "chapter_id": chapter_id,
                    "chapter_number": chapter_number,
                    "importance_score": importance,
                    "tags": ["情节点", plot_type],
                    "is_foreshadow": 0,
                    "keyword": keyword,
                    "text_position": position,
                    "text_length": length,
                }),
            );
        }
    }

    if let Some(character_states) = payload.get("character_states").and_then(Value::as_array) {
        for character_state in character_states {
            let character_name = normalized_string(character_state.get("character_name"))
                .unwrap_or_else(|| "未知角色".to_string());
            push_analysis_memory(
                &mut drafts,
                "character_event",
                Some(format!("{}的变化", character_name)),
                format!(
                    "{}的状态变化: {} → {}。{}",
                    character_name,
                    normalized_string(character_state.get("state_before")).unwrap_or_default(),
                    normalized_string(character_state.get("state_after")).unwrap_or_default(),
                    normalized_string(character_state.get("psychological_change"))
                        .unwrap_or_default()
                ),
                json!({
                    "chapter_id": chapter_id,
                    "chapter_number": chapter_number,
                    "importance_score": 0.7,
                    "tags": ["角色", character_name, "状态变化"],
                    "related_characters": [character_name],
                    "is_foreshadow": 0,
                }),
            );
        }
    }

    if let Some(conflict) = payload.get("conflict") {
        let level = conflict.get("level").and_then(Value::as_f64).unwrap_or(0.0);
        if level >= 7.0 {
            let parties = filtered_string_array(conflict.get("parties"));
            let types = filtered_string_array(conflict.get("types"));
            let description = normalized_string(conflict.get("description")).unwrap_or_default();
            let content = if parties.is_empty() {
                format!("重要冲突: {}", description)
            } else {
                format!(
                    "重要冲突: {}。冲突各方: {}",
                    description,
                    parties.join(", ")
                )
            };
            let mut tags = vec!["冲突".to_string()];
            tags.extend(types.clone());
            push_analysis_memory(
                &mut drafts,
                "plot_point",
                Some(format!("冲突 - 强度{}", level as i32)),
                content,
                json!({
                    "chapter_id": chapter_id,
                    "chapter_number": chapter_number,
                    "importance_score": (level / 10.0).min(1.0),
                    "tags": tags,
                    "is_foreshadow": 0,
                    "related_characters": parties,
                }),
            );
        }
    }

    drafts
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChapterAnalysisRuntimeOverrides {
    chapter_content_override: Option<String>,
    chapter_word_count_override: Option<i32>,
}

impl ChapterAnalysisRuntimeOverrides {
    pub fn new(
        chapter_content_override: Option<String>,
        chapter_word_count_override: Option<i32>,
    ) -> Self {
        Self {
            chapter_content_override: chapter_content_override
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            chapter_word_count_override,
        }
    }
}

pub(crate) fn build_generated_chapter_analysis_overrides(
    generated: &GeneratedChapterResult,
) -> ChapterAnalysisRuntimeOverrides {
    ChapterAnalysisRuntimeOverrides::new(
        Some(generated.content.clone()),
        Some(generated.word_count),
    )
}

fn build_analysis_runtime_chapter_model(
    chapter_model: &chapter::Model,
    overrides: &ChapterAnalysisRuntimeOverrides,
) -> chapter::Model {
    let mut effective = chapter_model.clone();
    if let Some(content_override) = overrides.chapter_content_override.as_ref() {
        effective.content = Some(content_override.clone());
    }

    effective.word_count = overrides
        .chapter_word_count_override
        .unwrap_or_else(|| {
            if chapter_model.word_count > 0 {
                chapter_model.word_count
            } else {
                effective
                    .content
                    .as_deref()
                    .unwrap_or_default()
                    .chars()
                    .count() as i32
            }
        })
        .max(0);

    effective
}

async fn replace_analysis_memories_after_persist(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_model: &chapter::Model,
    payload: &Value,
) -> Result<usize, String> {
    let drafts = extract_analysis_memories(chapter_model, payload);
    let saved_count = drafts.len();

    story_memory::Entity::delete_many()
        .filter(story_memory::Column::ProjectId.eq(&chapter_model.project_id))
        .filter(story_memory::Column::ChapterId.eq(&chapter_model.id))
        .exec(db)
        .await
        .map_err(|error| error.to_string())?;
    delete_story_memory_vector_records_by_chapter(&chapter_model.project_id, &chapter_model.id)
        .await?;

    let now = Utc::now().naive_utc();
    for (index, draft) in drafts.iter().enumerate() {
        let memory_id = format!("{}_{}_{}", chapter_model.id, draft.memory_type, index);
        let saved = story_memory::ActiveModel {
            id: Set(memory_id.clone()),
            project_id: Set(chapter_model.project_id.clone()),
            chapter_id: Set(Some(chapter_model.id.clone())),
            memory_type: Set(draft.memory_type.clone()),
            title: Set(draft.title.clone()),
            content: Set(draft.content.clone()),
            full_context: Set(None),
            related_characters: Set(draft
                .metadata
                .get("related_characters")
                .cloned()
                .filter(|value| value.is_array())),
            related_locations: Set(draft
                .metadata
                .get("related_locations")
                .cloned()
                .filter(|value| value.is_array())),
            tags: Set(draft
                .metadata
                .get("tags")
                .cloned()
                .filter(|value| value.is_array())),
            importance_score: Set(json_f64(
                draft
                    .metadata
                    .get("importance_score")
                    .and_then(Value::as_f64),
            )),
            story_timeline: Set(chapter_model.chapter_number),
            chapter_position: Set(json_i32(
                draft.metadata.get("text_position").and_then(Value::as_i64),
            )),
            text_length: Set(json_i32(
                draft.metadata.get("text_length").and_then(Value::as_i64),
            )),
            is_foreshadow: Set(draft
                .metadata
                .get("is_foreshadow")
                .and_then(Value::as_i64)
                .map(|value| value as i32)
                .unwrap_or_else(|| json_bool_as_i32(draft.metadata.get("is_foreshadow")))),
            foreshadow_resolved_at: Set(None),
            foreshadow_strength: Set(json_f64(
                draft.metadata.get("strength").and_then(Value::as_f64),
            )),
            vector_id: Set(Some(memory_id.clone())),
            embedding_model: Set(None),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
        }
        .insert(db)
        .await
        .map_err(|error| error.to_string())?;

        upsert_story_memory_vector_record(
            db,
            user_id,
            &saved,
            &draft.content,
            draft.metadata.clone(),
        )
        .await?;
    }

    Ok(saved_count)
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

fn build_analysis_foreshadow_sync_route_request(
    chapter_model: &chapter::Model,
    payload: &Value,
) -> Option<SyncForeshadowFromAnalysisRouteRequest> {
    let foreshadows = payload.get("foreshadows")?.as_array()?;
    if foreshadows.is_empty() {
        return None;
    }

    Some(SyncForeshadowFromAnalysisRouteRequest::new(json!({
        "chapter_id": chapter_model.id,
        "chapter_number": chapter_model.chapter_number,
        "analysis_foreshadows": foreshadows,
    })))
}

fn build_chapter_analysis_quality_metrics_payload(payload: &Value) -> Option<Value> {
    let scores = payload.get("scores")?;
    let overall_score = scores.get("overall").and_then(Value::as_f64)?;
    let pacing_score = scores.get("pacing").and_then(Value::as_f64);
    let engagement_score = scores.get("engagement").and_then(Value::as_f64);
    let coherence_score = scores.get("coherence").and_then(Value::as_f64);
    let score_justification = scores
        .get("score_justification")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let suggestion_items = payload
        .get("suggestions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .take(4)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let metric_pairs = [
        ("pacing", "节奏", pacing_score),
        ("engagement", "吸引力", engagement_score),
        ("coherence", "连贯性", coherence_score),
    ];
    let weakest_metric = metric_pairs
        .into_iter()
        .filter_map(|(key, label, value)| value.map(|score| (key, label, score)))
        .min_by(|left, right| left.2.total_cmp(&right.2));
    let weakest_metric_key = weakest_metric.map(|item| item.0.to_string());
    let weakest_metric_label = weakest_metric.map(|item| item.1.to_string());
    let weakest_metric_value = weakest_metric.map(|item| item.2);

    let mut focus_areas = Vec::new();
    if pacing_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("节奏".to_string());
    }
    if engagement_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("吸引力".to_string());
    }
    if coherence_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("连贯性".to_string());
    }

    let mut preserve_strengths = Vec::new();
    if pacing_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("节奏稳定".to_string());
    }
    if engagement_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("追读牵引".to_string());
    }
    if coherence_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("逻辑连贯".to_string());
    }
    if preserve_strengths.is_empty() && payload.get("hooks").and_then(Value::as_array).is_some() {
        preserve_strengths.push("钩子密度".to_string());
    }

    let repair_summary = suggestion_items
        .first()
        .cloned()
        .or(score_justification.clone())
        .or_else(|| build_chapter_analysis_report(payload))
        .unwrap_or_else(|| "当前章节质量分析已完成。".to_string());

    let (quality_gate_status, quality_gate_decision, quality_gate_label) = if overall_score < 6.0 {
        ("failed", "manual_review", "需要人工复核")
    } else if overall_score < 8.0 {
        ("warning", "auto_repair", "建议继续修复")
    } else {
        ("passed", "passed", "当前章节通过")
    };

    let failed_metrics = weakest_metric_label
        .as_ref()
        .map(|label| vec![json!({"label": label})])
        .unwrap_or_default();

    Some(json!({
        "overall_score": overall_score,
        "pacing_score": pacing_score,
        "engagement_score": engagement_score,
        "coherence_score": coherence_score,
        "repair_guidance": {
            "summary": repair_summary,
            "repair_targets": suggestion_items,
            "preserve_strengths": preserve_strengths,
            "focus_areas": focus_areas,
            "weakest_metric_key": weakest_metric_key,
            "weakest_metric_label": weakest_metric_label,
            "weakest_metric_value": weakest_metric_value,
        },
        "quality_gate": {
            "status": quality_gate_status,
            "decision": quality_gate_decision,
            "label": quality_gate_label,
            "summary": repair_summary,
            "failed_metrics": failed_metrics,
        },
        "quality_runtime_context": {
            "scope": "chapter",
            "source": "plot_analysis",
            "score_justification": score_justification,
        }
    }))
}

async fn sync_analysis_foreshadows_after_persist(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
    payload: &Value,
) -> Option<Value> {
    let Some(route_request) = build_analysis_foreshadow_sync_route_request(chapter_model, payload)
    else {
        return None;
    };

    let request = build_sync_foreshadow_from_analysis_request_from_route_payload(route_request);
    match ForeshadowService::sync_from_analysis(db, &chapter_model.project_id, &request).await {
        Ok(stats) => Some(stats),
        Err(error) => {
            warn!(
                chapter_id = %chapter_model.id,
                project_id = %chapter_model.project_id,
                error = %error,
                "chapter analysis foreshadow sync failed after plot analysis persist"
            );
            None
        }
    }
}

async fn sync_analysis_character_states_after_persist(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
    payload: &Value,
) {
    let Some(character_states) = payload.get("character_states").and_then(Value::as_array) else {
        return;
    };
    if character_states.is_empty() {
        return;
    }

    if let Err(error) = sync_character_states_from_analysis(
        db,
        &chapter_model.project_id,
        chapter_model.chapter_number,
        character_states,
    )
    .await
    {
        warn!(
            chapter_id = %chapter_model.id,
            project_id = %chapter_model.project_id,
            error = %error,
            "chapter analysis character state sync failed after plot analysis persist"
        );
    }
}

async fn sync_analysis_organization_states_after_persist(
    db: &DatabaseConnection,
    chapter_model: &chapter::Model,
    payload: &Value,
) {
    let Some(organization_states) = payload.get("organization_states").and_then(Value::as_array)
    else {
        return;
    };
    if organization_states.is_empty() {
        return;
    }

    if let Err(error) = sync_organization_states_from_analysis(
        db,
        &chapter_model.project_id,
        chapter_model.chapter_number,
        organization_states,
    )
    .await
    {
        warn!(
            chapter_id = %chapter_model.id,
            project_id = %chapter_model.project_id,
            error = %error,
            "chapter analysis organization state sync failed after plot analysis persist"
        );
    }
}

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
    fn from_create_state(create_state: ChapterAnalysisTaskCreateState) -> Self {
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

fn build_chapter_analysis_task_create_response_payload(
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

async fn persist_chapter_analysis_result(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_model: &chapter::Model,
    task_id: &str,
    payload: &Value,
) -> Result<Value, String> {
    let now = Utc::now().naive_utc();
    let scores = payload.get("scores").cloned().unwrap_or(Value::Null);
    let conflict = payload.get("conflict").cloned().unwrap_or(Value::Null);
    let emotional_arc = payload.get("emotional_arc").cloned().unwrap_or(Value::Null);
    let quality_metrics_payload = build_chapter_analysis_quality_metrics_payload(payload);

    let analysis = plot_analysis::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(chapter_model.project_id.clone()),
        chapter_id: Set(chapter_model.id.clone()),
        plot_stage: Set(payload
            .get("plot_stage")
            .and_then(Value::as_str)
            .map(str::to_string)),
        conflict_level: Set(Some(json_i32(
            conflict.get("level").and_then(Value::as_i64),
        ))),
        conflict_types: Set(conflict.get("types").cloned()),
        emotional_tone: Set(emotional_arc
            .get("primary_emotion")
            .and_then(Value::as_str)
            .map(str::to_string)),
        emotional_intensity: Set(json_f64(
            emotional_arc.get("intensity").and_then(Value::as_f64),
        )),
        emotional_curve: Set(emotional_arc
            .get("curve")
            .cloned()
            .or_else(|| emotional_arc.get("secondary_emotions").cloned())),
        hooks: Set(payload.get("hooks").cloned()),
        hooks_count: Set(payload
            .get("hooks")
            .and_then(Value::as_array)
            .map(|items| items.len() as i32)
            .unwrap_or(0)),
        hooks_avg_strength: Set(payload
            .get("hooks")
            .and_then(Value::as_array)
            .and_then(|items| {
                let strengths = items
                    .iter()
                    .filter_map(|item| item.get("strength").and_then(Value::as_f64))
                    .collect::<Vec<_>>();
                if strengths.is_empty() {
                    None
                } else {
                    Some(strengths.iter().sum::<f64>() / strengths.len() as f64)
                }
            })),
        foreshadows: Set(payload.get("foreshadows").cloned()),
        foreshadows_planted: Set(payload
            .get("foreshadows")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|item| item.get("type").and_then(Value::as_str) == Some("planted"))
                    .count() as i32
            })
            .unwrap_or(0)),
        foreshadows_resolved: Set(payload
            .get("foreshadows")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|item| item.get("type").and_then(Value::as_str) == Some("resolved"))
                    .count() as i32
            })
            .unwrap_or(0)),
        plot_points: Set(payload.get("plot_points").cloned()),
        plot_points_count: Set(payload
            .get("plot_points")
            .and_then(Value::as_array)
            .map(|items| items.len() as i32)
            .unwrap_or(0)),
        character_states: Set(payload.get("character_states").cloned()),
        scenes: Set(payload
            .get("scenes")
            .cloned()
            .or_else(|| payload.get("serial_rhythm").cloned())),
        pacing: Set(payload
            .get("pacing")
            .and_then(Value::as_str)
            .map(str::to_string)),
        overall_quality_score: Set(json_f64(scores.get("overall").and_then(Value::as_f64))),
        pacing_score: Set(json_f64(scores.get("pacing").and_then(Value::as_f64))),
        engagement_score: Set(json_f64(scores.get("engagement").and_then(Value::as_f64))),
        coherence_score: Set(json_f64(scores.get("coherence").and_then(Value::as_f64))),
        analysis_report: Set(
            build_chapter_analysis_report(payload).or_else(|| Some(payload.to_string()))
        ),
        suggestions: Set(payload.get("suggestions").cloned()),
        word_count: Set(Some(chapter_model.word_count)),
        dialogue_ratio: Set(json_f64(
            payload.get("dialogue_ratio").and_then(Value::as_f64),
        )),
        description_ratio: Set(json_f64(
            payload.get("description_ratio").and_then(Value::as_f64),
        )),
        created_at: Set(Some(now)),
    };

    let saved_analysis = analysis
        .insert(db)
        .await
        .map_err(|error| error.to_string())?;

    let memories_count =
        replace_analysis_memories_after_persist(db, user_id, chapter_model, payload).await?;
    let foreshadow_stats = sync_analysis_foreshadows_after_persist(db, chapter_model, payload)
        .await
        .unwrap_or_else(|| {
            json!({
                "planted_count": 0,
                "resolved_count": 0,
                "created_count": 0,
            })
        });
    sync_analysis_character_states_after_persist(db, chapter_model, payload).await;
    sync_analysis_organization_states_after_persist(db, chapter_model, payload).await;
    if let Some(quality_metrics_payload) = quality_metrics_payload.as_ref() {
        let chapter_content = chapter_model.content.clone().unwrap_or_default();
        let _ = update_latest_generated_chapter_history_quality_metrics(
            db,
            &chapter_model.id,
            &chapter_content,
            quality_metrics_payload,
        )
        .await;
    }

    if !task_id.trim().is_empty() {
        let _ =
            apply_analysis_task_state_by_id(db, task_id, AnalysisTaskStage::Completed, None, now)
                .await
                .map_err(|error| error.to_string())?;
    }

    Ok(json!({
        "analysis": saved_analysis,
        "quality_metrics": quality_metrics_payload,
        "memories_count": memories_count,
        "foreshadow_stats": foreshadow_stats,
    }))
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

pub async fn enqueue_chapter_analysis_task(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
) -> Result<Value, CreateChapterAnalysisTaskError> {
    let chapter_model = ChapterService::get(db, chapter_id, user_id)
        .await
        .map_err(CreateChapterAnalysisTaskError::Internal)?
        .ok_or(CreateChapterAnalysisTaskError::ProjectMissing)?;

    let create_state = create_chapter_analysis_task(db, user_id, &chapter_model).await?;
    let payload = load_created_analysis_task_payload(db, &create_state)
        .await
        .map_err(CreateChapterAnalysisTaskError::Internal)?;

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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::services::chapter_access_service::LoadAccessibleChapterError;
    use crate::services::chapter_analysis_service::CreateChapterAnalysisTaskError;

    use super::{
        build_analysis_foreshadow_sync_route_request, build_analysis_runtime_chapter_model,
        build_chapter_analysis_quality_metrics_payload, build_chapter_analysis_report,
        build_chapter_analysis_task_create_response_payload,
        build_generated_chapter_analysis_overrides, extract_analysis_memories, find_text_position,
        json_f64, json_i32, ChapterAnalysisRuntimeOverrides, ChapterAnalysisTaskCreateState,
        PrepareChapterAnalysisTriggerError, PreparedChapterAnalysisTriggerExecution,
    };
    use crate::models::chapter;
    use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;

    #[test]
    fn should_clamp_json_i32_values() {
        assert_eq!(json_i32(None), 0);
        assert_eq!(json_i32(Some(42)), 42);
        assert_eq!(json_i32(Some(i64::from(i32::MAX) + 1)), i32::MAX);
        assert_eq!(json_i32(Some(i64::from(i32::MIN) - 1)), i32::MIN);
    }

    #[test]
    fn should_filter_non_finite_json_f64_values() {
        assert_eq!(json_f64(Some(0.75)), Some(0.75));
        assert_eq!(json_f64(Some(f64::NAN)), None);
        assert_eq!(json_f64(Some(f64::INFINITY)), None);
        assert_eq!(json_f64(None), None);
    }

    #[test]
    fn should_build_chapter_analysis_report_from_payload_sections() {
        let payload = json!({
            "plot_stage": " 高潮 ",
            "conflict": {
                "description": " 正面对抗 "
            },
            "scores": {
                "score_justification": " 节奏稳定 "
            },
            "suggestions": [" 强化铺垫 ", "", 7, "压缩说明"]
        });

        let report = build_chapter_analysis_report(&payload);

        assert_eq!(
            report,
            Some("剧情阶段：高潮\n冲突分析：正面对抗\n评分说明：节奏稳定\n改进建议：强化铺垫；压缩说明".to_string())
        );
    }

    #[test]
    fn should_skip_empty_chapter_analysis_report_sections() {
        let payload = json!({
            "plot_stage": "  ",
            "conflict": {
                "description": ""
            },
            "scores": {},
            "suggestions": [" ", 1]
        });

        assert_eq!(build_chapter_analysis_report(&payload), None);
    }

    #[test]
    fn should_build_chapter_analysis_task_create_payload() {
        let payload =
            ChapterAnalysisTaskCreateState::new("task-123".to_string(), "chapter-456".to_string())
                .compatibility_payload();

        assert_eq!(
            payload,
            json!({
                "task_id": "task-123",
                "chapter_id": "chapter-456",
                "status": "pending",
                "message": "章节分析任务已创建",
            })
        );
    }

    #[test]
    fn should_keep_chapter_analysis_trigger_create_state_contract_minimal() {
        let create_state =
            ChapterAnalysisTaskCreateState::new("task-1".to_string(), "chapter-1".to_string());

        assert_eq!(create_state.task_id, "task-1");
        assert_eq!(create_state.chapter_id, "chapter-1");
        assert_eq!(create_state.compatibility_payload()["task_id"], "task-1");
        assert_eq!(create_state.task_id(), "task-1");
    }

    #[test]
    fn should_build_chapter_analysis_task_create_response_payload_from_status_owner() {
        let create_state =
            ChapterAnalysisTaskCreateState::new("task-10".to_string(), "chapter-20".to_string());
        let payload = build_chapter_analysis_task_create_response_payload(
            json!({
                "has_task": true,
                "task_id": "task-10",
                "chapter_id": "chapter-20",
                "status": "pending",
                "progress": 0,
                "error_message": null,
                "error_code": null,
                "auto_recovered": false,
                "created_at": "2026-06-02T12:00:00",
                "started_at": null,
                "completed_at": null,
            }),
            &create_state,
        );

        assert_eq!(payload["has_task"], true);
        assert_eq!(payload["task_id"], "task-10");
        assert_eq!(payload["chapter_id"], "chapter-20");
        assert_eq!(payload["status"], "pending");
        assert_eq!(payload["progress"], 0);
        assert_eq!(payload["auto_recovered"], false);
        assert_eq!(payload["message"], "章节分析任务已创建");
    }

    #[test]
    fn should_keep_prepared_chapter_analysis_trigger_execution_task_identity() {
        let prepared = PreparedChapterAnalysisTriggerExecution::from_create_state(
            ChapterAnalysisTaskCreateState::new("task-2".to_string(), "chapter-2".to_string()),
        );

        assert_eq!(prepared.task_id(), "task-2");
    }

    #[test]
    fn should_keep_trigger_write_workflow_chapter_error_shape() {
        let error = PrepareChapterAnalysisTriggerError::Chapter(
            LoadAccessibleChapterError::NotFoundOrAccessDenied,
        );

        assert!(matches!(
            error,
            PrepareChapterAnalysisTriggerError::Chapter(
                LoadAccessibleChapterError::NotFoundOrAccessDenied
            )
        ));
    }

    #[test]
    fn should_keep_trigger_write_workflow_create_error_shape() {
        let error = PrepareChapterAnalysisTriggerError::Create(
            CreateChapterAnalysisTaskError::ChapterEmpty,
        );

        assert!(matches!(
            error,
            PrepareChapterAnalysisTriggerError::Create(
                CreateChapterAnalysisTaskError::ChapterEmpty
            )
        ));
    }

    #[test]
    fn should_build_analysis_foreshadow_sync_route_request_from_payload() {
        let chapter_model = chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "第12章".to_string(),
            content: Some("测试内容".to_string()),
            summary: None,
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: None,
        };
        let payload = json!({
            "foreshadows": [
                {
                    "type": "planted",
                    "content": "主角第一次注意到钥匙上的旧纹章"
                }
            ]
        });

        let request = build_analysis_foreshadow_sync_route_request(&chapter_model, &payload)
            .expect("should build request");

        assert_eq!(
            *request.body(),
            json!({
                "chapter_id": "chapter-1",
                "chapter_number": 12,
                "analysis_foreshadows": [
                    {
                        "type": "planted",
                        "content": "主角第一次注意到钥匙上的旧纹章"
                    }
                ]
            })
        );
    }

    #[test]
    fn should_skip_analysis_foreshadow_sync_route_request_when_empty() {
        let chapter_model = chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "第12章".to_string(),
            content: Some("测试内容".to_string()),
            summary: None,
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: None,
        };

        assert!(build_analysis_foreshadow_sync_route_request(
            &chapter_model,
            &json!({ "foreshadows": [] })
        )
        .is_none());
        assert!(build_analysis_foreshadow_sync_route_request(&chapter_model, &json!({})).is_none());
    }

    #[test]
    fn should_find_text_position_with_exact_match() {
        assert_eq!(
            find_text_position("主角看见旧纹章钥匙。", "旧纹章钥匙"),
            (4, 5)
        );
    }

    #[test]
    fn should_extract_analysis_memories_from_payload() {
        let chapter_model = chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "风雪夜".to_string(),
            content: Some("主角看见旧纹章钥匙，心中一震。双方随即爆发正面冲突。".to_string()),
            summary: None,
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: None,
        };
        let payload = json!({
            "summary": "主角发现关键线索并卷入冲突",
            "hooks": [
                {
                    "type": "悬念",
                    "content": "钥匙上的纹章来历不明",
                    "position": "开篇",
                    "keyword": "旧纹章钥匙",
                    "strength": 8
                }
            ],
            "foreshadows": [
                {
                    "type": "planted",
                    "content": "钥匙暗示王室秘闻",
                    "keyword": "旧纹章钥匙",
                    "strength": 7,
                    "related_characters": ["主角"]
                }
            ],
            "plot_points": [
                {
                    "type": "turning_point",
                    "content": "主角决定追查钥匙来源",
                    "impact": "推动主线升级",
                    "keyword": "旧纹章钥匙",
                    "importance": 0.8
                }
            ],
            "character_states": [
                {
                    "character_name": "主角",
                    "state_before": "迟疑",
                    "state_after": "坚定",
                    "psychological_change": "决定主动调查"
                }
            ],
            "conflict": {
                "level": 8,
                "description": "双方围绕钥匙归属激烈争执",
                "parties": ["主角", "黑衣人"],
                "types": ["外部冲突"]
            }
        });

        let memories = extract_analysis_memories(&chapter_model, &payload);
        let memory_types = memories
            .iter()
            .map(|item| item.memory_type.as_str())
            .collect::<Vec<_>>();

        assert!(memory_types.contains(&"chapter_summary"));
        assert!(memory_types.contains(&"hook"));
        assert!(memory_types.contains(&"foreshadow"));
        assert!(memory_types.contains(&"plot_point"));
        assert!(memory_types.contains(&"character_event"));

        let summary = memories
            .iter()
            .find(|item| item.memory_type == "chapter_summary")
            .expect("chapter summary memory");
        assert_eq!(summary.title.as_deref(), Some("第12章《风雪夜》摘要"));

        let foreshadow = memories
            .iter()
            .find(|item| item.memory_type == "foreshadow")
            .expect("foreshadow memory");
        assert_eq!(
            foreshadow
                .metadata
                .get("is_foreshadow")
                .and_then(serde_json::Value::as_i64),
            Some(1)
        );
    }

    #[test]
    fn should_build_analysis_runtime_chapter_model_with_overrides() {
        let chapter_model = chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "风雪夜".to_string(),
            content: Some("旧正文".to_string()),
            summary: None,
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: None,
        };

        let effective = build_analysis_runtime_chapter_model(
            &chapter_model,
            &ChapterAnalysisRuntimeOverrides::new(Some(" 新正文 ".to_string()), Some(4321)),
        );

        assert_eq!(effective.content.as_deref(), Some("新正文"));
        assert_eq!(effective.word_count, 4321);
        assert_eq!(effective.id, chapter_model.id);
        assert_eq!(effective.project_id, chapter_model.project_id);
    }

    #[test]
    fn should_build_generated_chapter_follow_up_analysis_overrides() {
        let overrides = build_generated_chapter_analysis_overrides(&GeneratedChapterResult {
            chapter_id: "chapter-1".to_string(),
            chapter_number: 12,
            title: "风雪夜".to_string(),
            content: " 新生成正文 ".to_string(),
            word_count: 4321,
            ..Default::default()
        });

        let chapter_model = chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 12,
            title: "风雪夜".to_string(),
            content: Some("旧正文".to_string()),
            summary: None,
            word_count: 1200,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: chrono::NaiveDateTime::default(),
            updated_at: None,
        };
        let effective = build_analysis_runtime_chapter_model(&chapter_model, &overrides);

        assert_eq!(effective.content.as_deref(), Some("新生成正文"));
        assert_eq!(effective.word_count, 4321);
    }

    #[test]
    fn should_build_chapter_analysis_quality_metrics_payload_from_analysis_scores() {
        let payload = json!({
            "scores": {
                "overall": 7.6,
                "pacing": 7.1,
                "engagement": 8.4,
                "coherence": 7.8,
                "score_justification": "中段说明略多，但悬念还在。"
            },
            "hooks": [{"type": "悬念"}],
            "suggestions": ["压缩说明段", "提前冲突触发"]
        });

        let metrics =
            build_chapter_analysis_quality_metrics_payload(&payload).expect("metrics payload");

        assert_eq!(metrics["overall_score"], 7.6);
        assert_eq!(
            metrics["repair_guidance"]["repair_targets"],
            json!(["压缩说明段", "提前冲突触发"])
        );
        assert_eq!(metrics["quality_gate"]["decision"], "auto_repair");
        assert_eq!(
            metrics["quality_runtime_context"]["source"],
            "plot_analysis"
        );
    }
}
