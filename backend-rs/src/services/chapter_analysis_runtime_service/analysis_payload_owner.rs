use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use tracing::warn;

use crate::models::{chapter, story_memory};
use crate::services::chapter_generation_runtime_service::GeneratedChapterResult;
use crate::services::foreshadow_service::build_sync_foreshadow_from_analysis_request_from_route_payload;
use crate::services::foreshadow_service::ForeshadowService;
use crate::services::foreshadow_service::SyncForeshadowFromAnalysisRouteRequest;
use crate::services::story_memory_vector_index_service::{
    delete_story_memory_vector_records_by_chapter, upsert_story_memory_vector_record,
};

pub(crate) fn json_i32(value: Option<i64>) -> i32 {
    value
        .unwrap_or_default()
        .clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

pub(crate) fn json_f64(value: Option<f64>) -> Option<f64> {
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

pub(crate) fn find_text_position(full_text: &str, keyword: &str) -> (i32, i32) {
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
pub(crate) struct AnalysisMemoryDraft {
    pub(crate) memory_type: String,
    pub(crate) title: Option<String>,
    pub(crate) content: String,
    pub(crate) metadata: Value,
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

pub(crate) fn extract_analysis_memories(
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

pub(crate) fn build_analysis_runtime_chapter_model(
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

pub(crate) async fn replace_analysis_memories_after_persist(
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

pub(crate) fn build_chapter_analysis_report(payload: &Value) -> Option<String> {
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

pub(crate) fn build_analysis_foreshadow_sync_route_request(
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

pub(crate) fn build_chapter_analysis_quality_metrics_payload(payload: &Value) -> Option<Value> {
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

pub(crate) async fn sync_analysis_foreshadows_after_persist(
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

pub(crate) fn build_chapter_analysis_payload_owner_contract() -> Value {
    json!({
        "owner": "chapter_analysis_runtime_service::analysis_payload_owner",
        "scope": "analysis_payload_memory_quality_metrics_override_and_foreshadow_sync_projection",
        "python_source_map": [
            "backend/app/services/chapter_analysis_response_service.py",
            "backend/app/services/manual_chapter_analysis_execution_service.py",
            "backend/app/services/memory_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_analysis_runtime_service.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service/analysis_payload_owner.rs",
            "backend-rs/src/services/chapter_analysis_service.rs",
            "backend-rs/src/services/chapter_quality_metrics_query_service.rs"
        ],
        "behavior_contract": {
            "payload_entrypoints": [
                "build_generated_chapter_analysis_overrides",
                "build_analysis_runtime_chapter_model",
                "build_chapter_analysis_report",
                "build_chapter_analysis_quality_metrics_payload",
                "build_analysis_foreshadow_sync_route_request"
            ],
            "memory_entrypoints": [
                "extract_analysis_memories",
                "replace_analysis_memories_after_persist",
                "find_text_position"
            ],
            "foreshadow_sync_entrypoints": [
                "sync_analysis_foreshadows_after_persist",
                "build_sync_foreshadow_from_analysis_request_from_route_payload",
                "ForeshadowService::sync_from_analysis"
            ],
            "quality_metrics_fields": [
                "overall_score",
                "pacing_score",
                "engagement_score",
                "coherence_score",
                "repair_guidance",
                "quality_gate",
                "quality_runtime_context"
            ],
            "memory_types": [
                "chapter_summary",
                "hook",
                "foreshadow",
                "plot_point",
                "character_event"
            ]
        },
        "active_consumers": [
            "chapter_analysis_runtime_service::persist_chapter_analysis_result",
            "chapter_analysis_runtime_service::analyze_chapter_now_with_overrides",
            "chapter_analysis_runtime_service::analyze_generated_chapter_follow_up"
        ],
        "rollback_boundary": {
            "python_source_map_retained": true,
            "approval_required_before_python_edit": true
        }
    })
}
