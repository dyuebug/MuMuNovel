use chrono::NaiveDateTime;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use crate::models::{chapter, chapter_draft_attempt, generation_history};
use crate::services::chapter_analysis_service::{AutoRevisionDraftError, CandidateDraftError};

pub struct ChapterDraftAnalysisViewFragments {
    pub auto_revision_draft: Option<Value>,
    pub candidate_draft: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AutoRevisionDraftViewCounts {
    critical_count: i32,
    major_count: i32,
    priority_issue_count: i32,
    applied_critical_count: i32,
    applied_issue_count: i32,
    revised_word_count: i32,
}

pub(crate) fn format_datetime(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string())
}

fn parse_reviser_result_from_history(generated_content: Option<&str>) -> Option<Value> {
    let generated_content = generated_content?;
    let payload: Value = serde_json::from_str(generated_content).ok()?;
    if payload.get("log_type").and_then(Value::as_str) != Some("chapter_text_reviser_v1") {
        return None;
    }
    let reviser_result = payload.get("reviser_result")?;
    reviser_result.is_object().then(|| reviser_result.clone())
}

pub(crate) fn is_draft_stale(
    chapter_updated_at: Option<NaiveDateTime>,
    draft_created_at: Option<NaiveDateTime>,
) -> bool {
    matches!(
        (chapter_updated_at, draft_created_at),
        (Some(chapter_updated_at), Some(draft_created_at)) if chapter_updated_at > draft_created_at
    )
}

pub(crate) fn json_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_bool().map(i64::from))
            .or_else(|| {
                value
                    .as_str()
                    .and_then(|text| text.trim().parse::<i64>().ok())
            })
    })
}

pub(crate) fn python_truthy_json_i64(value: Option<&Value>) -> Option<i64> {
    json_i64(value).filter(|value| *value != 0)
}

fn json_f64(value: Option<&Value>) -> Option<f64> {
    value.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_bool().map(|value| if value { 1.0 } else { 0.0 }))
            .or_else(|| {
                value
                    .as_str()
                    .and_then(|text| text.trim().parse::<f64>().ok())
            })
    })
}

pub(crate) fn python_truthy_scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Bool(true) => Some("True".to_string()),
        Value::Bool(false) => None,
        Value::Number(value) => {
            if value.as_i64() == Some(0) || value.as_u64() == Some(0) || value.as_f64() == Some(0.0)
            {
                None
            } else {
                Some(value.to_string())
            }
        }
        _ => None,
    }
}

fn python_truthy_json(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Null) | None => false,
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => {
            value.as_i64() != Some(0) && value.as_u64() != Some(0) && value.as_f64() != Some(0.0)
        }
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(map)) => !map.is_empty(),
    }
}

fn auto_revision_draft_view_counts(
    reviser_result: &Value,
    revised_text: &str,
) -> AutoRevisionDraftViewCounts {
    let critical_count = json_i64(reviser_result.get("critical_count")).unwrap_or(0) as i32;
    let major_count = json_i64(reviser_result.get("major_count")).unwrap_or(0) as i32;
    let priority_issue_count = python_truthy_json_i64(reviser_result.get("priority_issue_count"))
        .unwrap_or((critical_count + major_count) as i64) as i32;
    let applied_issue_count = python_truthy_json_i64(reviser_result.get("applied_issue_count"))
        .or_else(|| python_truthy_json_i64(reviser_result.get("applied_critical_count")))
        .unwrap_or(0) as i32;
    let applied_critical_count = json_i64(reviser_result.get("applied_critical_count"))
        .unwrap_or(applied_issue_count as i64) as i32;
    let revised_word_count = json_i64(reviser_result.get("revised_word_count"))
        .unwrap_or(revised_text.chars().count() as i64) as i32;

    AutoRevisionDraftViewCounts {
        critical_count,
        major_count,
        priority_issue_count,
        applied_critical_count,
        applied_issue_count,
        revised_word_count,
    }
}

fn normalize_candidate_items(value: Option<&Value>, limit: usize) -> Vec<String> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let push_item = |items: &mut Vec<String>,
                     seen: &mut std::collections::HashSet<String>,
                     raw: &str,
                     limit: usize| {
        let trimmed = raw.trim();
        if trimmed.is_empty() || seen.contains(trimmed) || items.len() >= limit {
            return;
        }
        seen.insert(trimmed.to_string());
        items.push(trimmed.to_string());
    };

    match value {
        Some(Value::String(text)) => {
            push_item(&mut items, &mut seen, text, limit);
        }
        Some(Value::Array(values)) => {
            for item in values {
                match item {
                    Value::String(_) | Value::Bool(_) | Value::Number(_) => {
                        if let Some(text) = python_truthy_scalar_text(item) {
                            push_item(&mut items, &mut seen, &text, limit);
                        }
                    }
                    Value::Object(map) => {
                        if let Some(text) = map
                            .get("label")
                            .and_then(python_truthy_scalar_text)
                            .or_else(|| map.get("name").and_then(python_truthy_scalar_text))
                            .or_else(|| map.get("value").and_then(python_truthy_scalar_text))
                            .or_else(|| map.get("item").and_then(python_truthy_scalar_text))
                        {
                            push_item(&mut items, &mut seen, &text, limit);
                        }
                    }
                    _ => {}
                }
                if items.len() >= limit {
                    break;
                }
            }
        }
        Some(Value::Object(map)) => {
            for key in ["label", "name", "value", "item", "summary"] {
                if let Some(text) = map.get(key).and_then(python_truthy_scalar_text) {
                    push_item(&mut items, &mut seen, &text, limit);
                    break;
                }
            }
        }
        Some(Value::Bool(_)) | Some(Value::Number(_)) => {
            if let Some(text) = value.and_then(python_truthy_scalar_text) {
                push_item(&mut items, &mut seen, &text, limit);
            }
        }
        _ => {}
    }

    items
}

fn normalize_candidate_items_with_fallback(
    primary: Option<&Value>,
    fallback: Option<&Value>,
    limit: usize,
) -> Vec<String> {
    let primary_items = normalize_candidate_items(primary, limit);
    if primary_items.is_empty() {
        normalize_candidate_items(fallback, limit)
    } else {
        primary_items
    }
}

fn trimmed_python_truthy_scalar_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(python_truthy_scalar_text)
        .map(|text| text.trim().to_string())
}

fn python_truthy_scalar_or_null(value: Option<&Value>) -> Value {
    trimmed_python_truthy_scalar_text(value)
        .filter(|text| !text.is_empty())
        .map(|text| json!(text))
        .unwrap_or(Value::Null)
}

fn candidate_failed_metric_labels(quality_gate: &Value, limit: usize) -> Vec<String> {
    let labels = quality_gate
        .get("failed_metrics")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_object()
                        .and_then(|payload| payload.get("label"))
                        .cloned()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    normalize_candidate_items(Some(&Value::Array(labels)), limit)
}

fn candidate_apply_risk_payload(
    apply_risk: &Value,
    quality_highlights: &Value,
    quality_gate: &Value,
    quality_gate_action: Option<&str>,
    quality_gate_decision: Option<&str>,
) -> Value {
    if apply_risk
        .as_object()
        .is_some_and(candidate_apply_risk_has_meaningful_content)
    {
        return apply_risk.clone();
    }

    let mut items = Vec::new();
    let empty_map = serde_json::Map::new();
    let highlights = quality_highlights.as_object().unwrap_or(&empty_map);
    let gate = quality_gate.as_object().unwrap_or(&empty_map);

    let continuity_missing = normalize_candidate_items(
        highlights
            .get("continuity")
            .and_then(Value::as_object)
            .and_then(|payload| payload.get("missing_items")),
        3,
    );
    if !continuity_missing.is_empty() {
        items.push(format!("连续性待补齐：{}", continuity_missing.join("；")));
    }

    let foreshadow_missing = normalize_candidate_items(
        highlights
            .get("foreshadow")
            .and_then(Value::as_object)
            .and_then(|payload| payload.get("missing_items")),
        3,
    );
    if !foreshadow_missing.is_empty() {
        items.push(format!(
            "伏笔/回收待补齐：{}",
            foreshadow_missing.join("；")
        ));
    }

    let failed_metric_labels = candidate_failed_metric_labels(quality_gate, 3);
    if !failed_metric_labels.is_empty() {
        items.push(format!(
            "质量门禁关注项：{}",
            failed_metric_labels.join("；")
        ));
    }

    let quality_gate_status = gate
        .get("status")
        .and_then(python_truthy_scalar_text)
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let normalized_action = quality_gate_action
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    let normalized_decision = quality_gate_decision
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .or_else(|| gate.get("decision").and_then(python_truthy_scalar_text))
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    if items.is_empty()
        && (matches!(quality_gate_status.as_str(), "warning" | "blocked")
            || matches!(normalized_action.as_str(), "manual_review" | "auto_repair")
            || matches!(
                normalized_decision.as_str(),
                "manual_review" | "auto_repair"
            ))
    {
        items.push("当前候选稿仍建议先做一致性复核，再决定是否直接恢复。".to_string());
    }

    if items.is_empty() {
        return Value::Null;
    }

    json!({
        "status": "warning",
        "summary": "恢复前请先确认这些一致性 / 质量风险是否可接受。",
        "items": items.into_iter().take(4).collect::<Vec<_>>(),
    })
}

fn candidate_apply_risk_has_meaningful_content(payload: &serde_json::Map<String, Value>) -> bool {
    trimmed_python_truthy_scalar_text(payload.get("summary")).is_some()
        || trimmed_python_truthy_scalar_text(payload.get("status")).is_some()
        || payload
            .get("items")
            .map(|value| !normalize_candidate_items(Some(value), 4).is_empty())
            .unwrap_or(false)
}

fn candidate_quality_highlights_has_meaningful_content(
    payload: &serde_json::Map<String, Value>,
) -> bool {
    ["continuity", "foreshadow"].into_iter().any(|key| {
        payload
            .get(key)
            .and_then(Value::as_object)
            .is_some_and(|facet| !facet.is_empty())
    })
}

pub(crate) fn extract_candidate_draft_full_content(
    draft_attempt: &chapter_draft_attempt::Model,
) -> (String, bool) {
    let repair_payload = draft_attempt
        .repair_payload
        .as_ref()
        .and_then(Value::as_object);
    if let Some(full_content) = repair_payload
        .and_then(|payload| payload.get("candidate_full_content"))
        .and_then(python_truthy_scalar_text)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
    {
        return (full_content, true);
    }

    let preview_content = draft_attempt
        .content_preview
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if preview_content.is_empty() {
        return (String::new(), false);
    }

    if repair_payload
        .and_then(|payload| payload.get("content_complete"))
        .is_some_and(|value| python_truthy_json(Some(value)))
    {
        return (preview_content, true);
    }

    let word_count = draft_attempt.word_count.max(0) as usize;
    if word_count > 0 && preview_content.chars().count() == word_count {
        return (preview_content, true);
    }

    (String::new(), false)
}

fn build_auto_revision_draft_payload(
    reviser_result: &Value,
    history_id: Option<&str>,
    created_at: Option<NaiveDateTime>,
    chapter_updated_at: Option<NaiveDateTime>,
    include_full_text: bool,
) -> Value {
    let revised_text = reviser_result
        .get("revised_text")
        .and_then(python_truthy_scalar_text)
        .unwrap_or_default()
        .to_string();
    let mut revised_text_preview = reviser_result
        .get("revised_text_preview")
        .and_then(python_truthy_scalar_text)
        .unwrap_or_default()
        .trim()
        .to_string();
    if revised_text_preview.is_empty() && !revised_text.is_empty() {
        revised_text_preview = revised_text.chars().take(500).collect();
    }

    let counts = auto_revision_draft_view_counts(reviser_result, &revised_text);

    let mut payload = serde_json::Map::new();
    payload.insert("history_id".to_string(), json!(history_id));
    payload.insert("critical_count".to_string(), json!(counts.critical_count));
    payload.insert("major_count".to_string(), json!(counts.major_count));
    payload.insert(
        "priority_issue_count".to_string(),
        json!(counts.priority_issue_count),
    );
    payload.insert(
        "applied_critical_count".to_string(),
        json!(counts.applied_critical_count),
    );
    payload.insert(
        "applied_issue_count".to_string(),
        json!(counts.applied_issue_count),
    );
    payload.insert(
        "change_summary".to_string(),
        reviser_result
            .get("change_summary")
            .cloned()
            .unwrap_or(Value::Null),
    );
    payload.insert(
        "revised_word_count".to_string(),
        reviser_result
            .get("revised_word_count")
            .cloned()
            .unwrap_or_else(|| json!(counts.revised_word_count)),
    );
    payload.insert(
        "unresolved_issues".to_string(),
        reviser_result
            .get("unresolved_issues")
            .cloned()
            .unwrap_or_else(|| json!([])),
    );
    payload.insert(
        "revised_text_preview".to_string(),
        json!(revised_text_preview),
    );
    payload.insert("has_full_text".to_string(), json!(!revised_text.is_empty()));
    payload.insert(
        "is_stale".to_string(),
        json!(is_draft_stale(chapter_updated_at, created_at)),
    );
    payload.insert("created_at".to_string(), json!(format_datetime(created_at)));
    if include_full_text {
        payload.insert("revised_text".to_string(), json!(revised_text));
    }
    Value::Object(payload)
}

fn build_candidate_draft_payload(
    draft_attempt: &chapter_draft_attempt::Model,
    chapter_updated_at: Option<NaiveDateTime>,
    include_full_text: bool,
) -> Value {
    let quality_metrics = draft_attempt
        .quality_metrics
        .clone()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let repair_payload = draft_attempt
        .repair_payload
        .clone()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let quality_gate = quality_metrics
        .get("quality_gate")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let repair_guidance = quality_metrics
        .get("repair_guidance")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let candidate_selection = quality_metrics
        .get("candidate_selection")
        .cloned()
        .filter(Value::is_object)
        .unwrap_or(Value::Null);
    let raw_quality_highlights = quality_metrics
        .get("quality_highlights")
        .cloned()
        .or_else(|| repair_payload.get("quality_highlights").cloned())
        .filter(Value::is_object)
        .unwrap_or(Value::Null);
    let quality_highlights = if raw_quality_highlights
        .as_object()
        .is_some_and(candidate_quality_highlights_has_meaningful_content)
    {
        raw_quality_highlights.clone()
    } else {
        Value::Null
    };
    let raw_apply_risk = quality_metrics
        .get("apply_risk")
        .cloned()
        .or_else(|| repair_payload.get("apply_risk").cloned())
        .filter(Value::is_object)
        .unwrap_or(Value::Null);
    let apply_risk = candidate_apply_risk_payload(
        &raw_apply_risk,
        &quality_highlights,
        &quality_gate,
        draft_attempt.quality_gate_action.as_deref(),
        draft_attempt.quality_gate_decision.as_deref(),
    );

    let (full_content, has_full_content) = extract_candidate_draft_full_content(draft_attempt);
    let mut preview_text = draft_attempt
        .content_preview
        .as_deref()
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .or_else(|| draft_attempt.summary_preview.clone())
        .unwrap_or_default()
        .trim()
        .to_string();
    if preview_text.is_empty() && !full_content.is_empty() {
        preview_text = full_content.chars().take(500).collect();
    }

    let failed_metrics = quality_gate
        .get("failed_metrics")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let object = item.as_object()?;
                    let key =
                        trimmed_python_truthy_scalar_text(object.get("key")).unwrap_or_default();
                    let label = trimmed_python_truthy_scalar_text(object.get("label"))
                        .or_else(|| trimmed_python_truthy_scalar_text(object.get("key")))
                        .unwrap_or_default();
                    Some(json!({
                        "key": key,
                        "label": label,
                        "value": json_f64(object.get("value")).unwrap_or(0.0),
                        "threshold": json_f64(object.get("threshold")).unwrap_or(0.0),
                        "gap": json_f64(object.get("gap")).unwrap_or(0.0),
                        "focus_area": python_truthy_scalar_or_null(object.get("focus_area")),
                        "repair_target": python_truthy_scalar_or_null(object.get("repair_target")),
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let highlight_points = normalize_candidate_items(
        raw_quality_highlights
            .as_object()
            .and_then(|payload| payload.get("highlight_points")),
        6,
    );
    let risk_points = normalize_candidate_items(
        raw_apply_risk
            .as_object()
            .and_then(|payload| payload.get("risk_points")),
        6,
    );
    let recommended_actions = normalize_candidate_items(
        repair_guidance
            .as_object()
            .and_then(|payload| payload.get("recommended_actions")),
        8,
    );
    let repair_targets = normalize_candidate_items_with_fallback(
        repair_payload
            .as_object()
            .and_then(|payload| payload.get("repair_targets")),
        repair_guidance
            .as_object()
            .and_then(|payload| payload.get("repair_targets")),
        4,
    );
    let preserved_strengths = normalize_candidate_items(
        repair_payload
            .as_object()
            .and_then(|payload| payload.get("preserve_strengths")),
        6,
    );
    let preserve_strengths = normalize_candidate_items_with_fallback(
        repair_payload
            .as_object()
            .and_then(|payload| payload.get("preserve_strengths")),
        repair_guidance
            .as_object()
            .and_then(|payload| payload.get("preserve_strengths")),
        4,
    );
    let focus_areas = normalize_candidate_items_with_fallback(
        repair_guidance
            .as_object()
            .and_then(|payload| payload.get("focus_areas")),
        quality_gate
            .as_object()
            .and_then(|payload| payload.get("focus_areas")),
        4,
    );
    let repair_summary = trimmed_python_truthy_scalar_text(repair_payload.get("summary"))
        .or_else(|| trimmed_python_truthy_scalar_text(repair_guidance.get("summary")))
        .filter(|text| !text.is_empty())
        .map(|text| json!(text))
        .unwrap_or(Value::Null);
    let display_word_count = if draft_attempt.word_count != 0 {
        draft_attempt.word_count
    } else {
        full_content.chars().count() as i32
    };

    let mut payload = serde_json::Map::new();
    payload.insert("attempt_id".to_string(), json!(draft_attempt.id));
    payload.insert("source".to_string(), json!(draft_attempt.source.trim()));
    payload.insert(
        "attempt_state".to_string(),
        json!(draft_attempt.attempt_state.trim()),
    );
    payload.insert(
        "quality_gate_action".to_string(),
        json!(draft_attempt.quality_gate_action),
    );
    payload.insert(
        "quality_gate_decision".to_string(),
        json!(draft_attempt.quality_gate_decision),
    );
    payload.insert("word_count".to_string(), json!(display_word_count));
    payload.insert(
        "summary_preview".to_string(),
        json!(draft_attempt
            .summary_preview
            .as_deref()
            .unwrap_or_default()
            .trim()),
    );
    payload.insert("content_preview".to_string(), json!(preview_text));
    payload.insert(
        "created_at".to_string(),
        json!(format_datetime(draft_attempt.created_at)),
    );
    payload.insert(
        "is_stale".to_string(),
        json!(is_draft_stale(chapter_updated_at, draft_attempt.created_at)),
    );
    payload.insert("has_full_content".to_string(), json!(has_full_content));
    payload.insert("content_complete".to_string(), json!(has_full_content));
    payload.insert("can_apply".to_string(), json!(has_full_content));
    payload.insert("quality_metrics".to_string(), quality_metrics);
    payload.insert("repair_payload".to_string(), repair_payload);
    payload.insert("quality_gate".to_string(), quality_gate);
    payload.insert("repair_guidance".to_string(), repair_guidance);
    payload.insert("candidate_selection".to_string(), candidate_selection);
    payload.insert("quality_highlights".to_string(), quality_highlights);
    payload.insert("apply_risk".to_string(), apply_risk);
    payload.insert("failed_metrics".to_string(), json!(failed_metrics));
    payload.insert("highlight_points".to_string(), json!(highlight_points));
    payload.insert("risk_points".to_string(), json!(risk_points));
    payload.insert(
        "recommended_actions".to_string(),
        json!(recommended_actions),
    );
    payload.insert("repair_targets".to_string(), json!(repair_targets));
    payload.insert(
        "preserved_strengths".to_string(),
        json!(preserved_strengths),
    );
    payload.insert("preserve_strengths".to_string(), json!(preserve_strengths));
    payload.insert("focus_areas".to_string(), json!(focus_areas));
    payload.insert("repair_summary".to_string(), repair_summary);
    if include_full_text && has_full_content {
        payload.insert("content".to_string(), json!(full_content));
    }
    Value::Object(payload)
}

pub(crate) async fn load_candidate_draft_attempt(
    db: &DatabaseConnection,
    chapter_id: &str,
    attempt_id: Option<&str>,
) -> Result<Option<chapter_draft_attempt::Model>, sea_orm::DbErr> {
    let mut query = chapter_draft_attempt::Entity::find()
        .filter(chapter_draft_attempt::Column::ChapterId.eq(Some(chapter_id.to_string())));

    if let Some(attempt_id) = attempt_id.filter(|value| !value.trim().is_empty()) {
        query = query.filter(chapter_draft_attempt::Column::Id.eq(attempt_id.to_string()));
    } else {
        query = query
            .order_by_desc(chapter_draft_attempt::Column::CreatedAt)
            .limit(1);
    }

    query.one(db).await
}

pub(crate) async fn load_latest_reviser_history(
    db: &DatabaseConnection,
    chapter_id: &str,
    history_id: Option<&str>,
) -> Result<Option<(generation_history::Model, Value)>, sea_orm::DbErr> {
    if let Some(history_id) = history_id.filter(|value| !value.trim().is_empty()) {
        let history = generation_history::Entity::find_by_id(history_id)
            .filter(generation_history::Column::ChapterId.eq(Some(chapter_id.to_string())))
            .one(db)
            .await?;
        return Ok(history.and_then(|model| {
            parse_reviser_result_from_history(model.generated_content.as_deref())
                .map(|reviser_result| (model, reviser_result))
        }));
    }

    let histories = generation_history::Entity::find()
        .filter(generation_history::Column::ChapterId.eq(Some(chapter_id.to_string())))
        .order_by_desc(generation_history::Column::CreatedAt)
        .limit(60)
        .all(db)
        .await?;

    Ok(histories.into_iter().find_map(|history| {
        parse_reviser_result_from_history(history.generated_content.as_deref())
            .map(|reviser_result| (history, reviser_result))
    }))
}

pub async fn load_candidate_draft_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
    attempt_id: Option<&str>,
) -> Result<Value, CandidateDraftError> {
    let draft_attempt = load_candidate_draft_attempt(db, &chapter.id, attempt_id)
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    let draft_attempt = draft_attempt.ok_or(CandidateDraftError::NotFound)?;
    Ok(json!({
        "chapter_id": chapter.id,
        "candidate_draft": build_candidate_draft_payload(&draft_attempt, chapter.updated_at, true),
    }))
}

pub async fn load_auto_revision_draft_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
    history_id: Option<&str>,
) -> Result<Value, AutoRevisionDraftError> {
    let reviser_loaded = load_latest_reviser_history(db, &chapter.id, history_id)
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    let (reviser_history, reviser_result) =
        reviser_loaded.ok_or(AutoRevisionDraftError::NotFound)?;

    Ok(json!({
        "chapter_id": chapter.id,
        "auto_revision_draft": build_auto_revision_draft_payload(
            &reviser_result,
            Some(&reviser_history.id),
            reviser_history.created_at,
            chapter.updated_at,
            true,
        ),
    }))
}

pub fn build_chapter_draft_analysis_view_fragments(
    histories: &[generation_history::Model],
    candidate_attempt: Option<&chapter_draft_attempt::Model>,
    chapter_updated_at: Option<NaiveDateTime>,
) -> ChapterDraftAnalysisViewFragments {
    let auto_revision_draft = histories.iter().find_map(|history| {
        let reviser_result =
            parse_reviser_result_from_history(history.generated_content.as_deref())?;
        Some(build_auto_revision_draft_payload(
            &reviser_result,
            Some(&history.id),
            history.created_at,
            chapter_updated_at,
            false,
        ))
    });

    let candidate_draft = candidate_attempt
        .map(|attempt| build_candidate_draft_payload(attempt, chapter_updated_at, false));

    ChapterDraftAnalysisViewFragments {
        auto_revision_draft,
        candidate_draft,
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::{json, Value};

    use crate::models::{chapter_draft_attempt, generation_history};

    use super::{
        auto_revision_draft_view_counts, build_auto_revision_draft_payload,
        build_candidate_draft_payload, build_chapter_draft_analysis_view_fragments,
        candidate_apply_risk_payload, extract_candidate_draft_full_content, format_datetime,
        is_draft_stale, normalize_candidate_items, parse_reviser_result_from_history,
    };

    fn naive_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .expect("valid date")
            .and_hms_opt(hour, minute, second)
            .expect("valid time")
    }

    fn candidate_draft_attempt(
        content_preview: Option<&str>,
        word_count: i32,
        repair_payload: Option<serde_json::Value>,
    ) -> chapter_draft_attempt::Model {
        chapter_draft_attempt::Model {
            id: "attempt-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            batch_task_id: None,
            source: "quality_repair_candidate".to_string(),
            attempt_state: "ready".to_string(),
            quality_gate_action: None,
            quality_gate_decision: None,
            word_count,
            summary_preview: None,
            content_preview: content_preview.map(str::to_string),
            quality_metrics: None,
            repair_payload,
            created_at: None,
        }
    }

    fn generation_history(
        id: &str,
        generated_content: Option<String>,
        created_at: Option<chrono::NaiveDateTime>,
    ) -> generation_history::Model {
        generation_history::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            prompt: None,
            generated_content,
            model: None,
            tokens_used: None,
            generation_time: None,
            created_at,
        }
    }

    #[test]
    fn should_build_auto_revision_draft_view_counts_with_defaults() {
        let counts = auto_revision_draft_view_counts(
            &json!({
                "critical_count": 2,
                "major_count": 3,
                "applied_critical_count": 1
            }),
            "修订正文",
        );

        assert_eq!(counts.critical_count, 2);
        assert_eq!(counts.major_count, 3);
        assert_eq!(counts.priority_issue_count, 5);
        assert_eq!(counts.applied_critical_count, 1);
        assert_eq!(counts.applied_issue_count, 1);
        assert_eq!(counts.revised_word_count, 4);
    }

    #[test]
    fn should_preserve_explicit_auto_revision_draft_view_counts() {
        let counts = auto_revision_draft_view_counts(
            &json!({
                "critical_count": 2,
                "major_count": 3,
                "priority_issue_count": 9,
                "applied_critical_count": 1,
                "applied_issue_count": 4,
                "revised_word_count": 1200
            }),
            "修订正文",
        );

        assert_eq!(counts.critical_count, 2);
        assert_eq!(counts.major_count, 3);
        assert_eq!(counts.priority_issue_count, 9);
        assert_eq!(counts.applied_critical_count, 1);
        assert_eq!(counts.applied_issue_count, 4);
        assert_eq!(counts.revised_word_count, 1200);
    }

    #[test]
    fn should_preserve_explicit_auto_revision_revised_word_count_payload_value() {
        let string_payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": "修订正文",
                "revised_word_count": "1200"
            }),
            None,
            None,
            None,
            false,
        );
        let bool_payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": "修订正文",
                "revised_word_count": true
            }),
            None,
            None,
            None,
            false,
        );
        let fallback_payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": " 修订正文 "
            }),
            None,
            None,
            None,
            false,
        );

        assert_eq!(string_payload["revised_word_count"], json!("1200"));
        assert_eq!(bool_payload["revised_word_count"], json!(true));
        assert_eq!(fallback_payload["revised_word_count"], json!(6));
    }

    #[test]
    fn should_parse_auto_revision_draft_view_counts_from_numeric_strings() {
        let counts = auto_revision_draft_view_counts(
            &json!({
                "critical_count": "2",
                "major_count": "3",
                "priority_issue_count": "9",
                "applied_critical_count": "1",
                "applied_issue_count": "4",
                "revised_word_count": "1200"
            }),
            "修订正文",
        );

        assert_eq!(counts.critical_count, 2);
        assert_eq!(counts.major_count, 3);
        assert_eq!(counts.priority_issue_count, 9);
        assert_eq!(counts.applied_critical_count, 1);
        assert_eq!(counts.applied_issue_count, 4);
        assert_eq!(counts.revised_word_count, 1200);
    }

    #[test]
    fn should_parse_auto_revision_draft_view_counts_from_bool_values_for_python_compat() {
        let counts = auto_revision_draft_view_counts(
            &json!({
                "critical_count": true,
                "major_count": false,
                "priority_issue_count": true,
                "applied_critical_count": false,
                "applied_issue_count": true,
                "revised_word_count": true
            }),
            "修订正文",
        );

        assert_eq!(counts.critical_count, 1);
        assert_eq!(counts.major_count, 0);
        assert_eq!(counts.priority_issue_count, 1);
        assert_eq!(counts.applied_critical_count, 0);
        assert_eq!(counts.applied_issue_count, 1);
        assert_eq!(counts.revised_word_count, 1);
    }

    #[test]
    fn should_fallback_auto_revision_draft_view_counts_from_falsey_values_like_python() {
        let counts = auto_revision_draft_view_counts(
            &json!({
                "critical_count": 2,
                "major_count": 3,
                "priority_issue_count": false,
                "applied_critical_count": 4,
                "applied_issue_count": 0
            }),
            "修订正文",
        );

        assert_eq!(counts.priority_issue_count, 5);
        assert_eq!(counts.applied_critical_count, 4);
        assert_eq!(counts.applied_issue_count, 4);
    }

    #[test]
    fn should_extract_candidate_full_content_from_repair_payload_first() {
        let draft_attempt = candidate_draft_attempt(
            Some(" preview-only "),
            12,
            Some(json!({
                "candidate_full_content": " full candidate content "
            })),
        );

        let (full_content, has_full_content) = extract_candidate_draft_full_content(&draft_attempt);

        assert!(has_full_content);
        assert_eq!(full_content, "full candidate content");
    }

    #[test]
    fn should_coerce_candidate_full_content_scalars_like_python() {
        let numeric_attempt = candidate_draft_attempt(
            Some(" preview-only "),
            12,
            Some(json!({
                "candidate_full_content": 42
            })),
        );
        let falsey_attempt = candidate_draft_attempt(
            Some(" preview-only "),
            99,
            Some(json!({
                "candidate_full_content": false
            })),
        );

        let (numeric_content, numeric_has_full_content) =
            extract_candidate_draft_full_content(&numeric_attempt);
        let (falsey_content, falsey_has_full_content) =
            extract_candidate_draft_full_content(&falsey_attempt);

        assert!(numeric_has_full_content);
        assert_eq!(numeric_content, "42");
        assert!(!falsey_has_full_content);
        assert_eq!(falsey_content, "");
    }

    #[test]
    fn should_treat_complete_candidate_preview_as_full_content() {
        let draft_attempt = candidate_draft_attempt(
            Some(" preview content "),
            0,
            Some(json!({
                "content_complete": true
            })),
        );

        let (full_content, has_full_content) = extract_candidate_draft_full_content(&draft_attempt);

        assert!(has_full_content);
        assert_eq!(full_content, "preview content");
    }

    #[test]
    fn should_treat_truthy_candidate_content_complete_like_python() {
        let string_attempt = candidate_draft_attempt(
            Some(" preview content "),
            0,
            Some(json!({
                "content_complete": "false"
            })),
        );
        let numeric_attempt = candidate_draft_attempt(
            Some(" preview content "),
            0,
            Some(json!({
                "content_complete": 1
            })),
        );
        let falsey_attempt = candidate_draft_attempt(
            Some(" preview content "),
            0,
            Some(json!({
                "content_complete": ""
            })),
        );

        let (string_content, string_has_full_content) =
            extract_candidate_draft_full_content(&string_attempt);
        let (numeric_content, numeric_has_full_content) =
            extract_candidate_draft_full_content(&numeric_attempt);
        let (falsey_content, falsey_has_full_content) =
            extract_candidate_draft_full_content(&falsey_attempt);

        assert!(string_has_full_content);
        assert_eq!(string_content, "preview content");
        assert!(numeric_has_full_content);
        assert_eq!(numeric_content, "preview content");
        assert!(!falsey_has_full_content);
        assert_eq!(falsey_content, "");
    }

    #[test]
    fn should_treat_candidate_preview_matching_word_count_as_full_content() {
        let draft_attempt = candidate_draft_attempt(Some("章节正文"), 4, None);

        let (full_content, has_full_content) = extract_candidate_draft_full_content(&draft_attempt);

        assert!(has_full_content);
        assert_eq!(full_content, "章节正文");
    }

    #[test]
    fn should_reject_candidate_preview_only_content() {
        let draft_attempt = candidate_draft_attempt(Some(" preview-only "), 99, None);

        let (full_content, has_full_content) = extract_candidate_draft_full_content(&draft_attempt);

        assert!(!has_full_content);
        assert_eq!(full_content, "");
    }

    #[test]
    fn should_normalize_candidate_items_from_strings_with_trim_dedupe_and_limit() {
        let value = json!([
            " first ",
            "",
            "first",
            {"label": " second "},
            {"name": "third"},
            {"value": "fourth"}
        ]);

        let items = normalize_candidate_items(Some(&value), 3);

        assert_eq!(items, vec!["first", "second", "third"]);
    }

    #[test]
    fn should_normalize_candidate_items_from_object_by_supported_field_order() {
        let value = json!({
            "summary": "summary item",
            "item": "item value",
            "value": 42,
            "name": "name item",
            "label": "label item"
        });

        let items = normalize_candidate_items(Some(&value), 6);

        assert_eq!(items, vec!["label item"]);
    }

    #[test]
    fn should_normalize_candidate_items_from_scalar_values_for_python_compat() {
        let scalar_number = json!(42);
        let scalar_zero = json!(0);
        let scalar_bool = json!(true);
        let scalar_false = json!(false);
        let array_value = json!([
            42,
            true,
            false,
            0,
            42,
            {"value": 7},
            {"label": false},
            {"summary": "ignored in array object"}
        ]);
        let object_value = json!({
            "value": 7
        });

        assert_eq!(
            normalize_candidate_items(Some(&scalar_number), 6),
            vec!["42"]
        );
        assert!(normalize_candidate_items(Some(&scalar_zero), 6).is_empty());
        assert_eq!(
            normalize_candidate_items(Some(&scalar_bool), 6),
            vec!["True"]
        );
        assert!(normalize_candidate_items(Some(&scalar_false), 6).is_empty());
        assert_eq!(
            normalize_candidate_items(Some(&array_value), 6),
            vec!["42", "True", "7"]
        );
        assert_eq!(normalize_candidate_items(Some(&object_value), 6), vec!["7"]);
    }

    #[test]
    fn should_ignore_empty_and_unsupported_candidate_items() {
        let array_value = json!([
            {"summary": "not read from array object"},
            {"label": " "},
            null,
            " kept "
        ]);

        let array_items = normalize_candidate_items(Some(&array_value), 6);
        let empty_items = normalize_candidate_items(None, 6);

        assert_eq!(array_items, vec!["kept"]);
        assert!(empty_items.is_empty());
    }

    #[test]
    fn should_preserve_explicit_candidate_apply_risk_payload() {
        let explicit_risk = json!({
            "status": "custom",
            "summary": "explicit",
            "items": ["explicit risk"]
        });

        let payload =
            candidate_apply_risk_payload(&explicit_risk, &Value::Null, &Value::Null, None, None);

        assert_eq!(payload, explicit_risk);
    }

    #[test]
    fn should_ignore_empty_explicit_candidate_apply_risk_payload_like_python() {
        let payload = candidate_apply_risk_payload(
            &json!({}),
            &Value::Null,
            &json!({"status": "ok"}),
            None,
            None,
        );

        assert_eq!(payload, Value::Null);
    }

    #[test]
    fn should_ignore_legacy_risk_points_only_apply_risk_for_ui_payload() {
        let payload = candidate_apply_risk_payload(
            &json!({
                "risk_points": ["legacy risk"]
            }),
            &Value::Null,
            &json!({"status": "ok"}),
            None,
            None,
        );

        assert_eq!(payload, Value::Null);
    }

    #[test]
    fn should_build_candidate_apply_risk_from_missing_highlights_and_failed_metrics() {
        let payload = candidate_apply_risk_payload(
            &Value::Null,
            &json!({
                "continuity": {
                    "missing_items": [" 角色状态 ", "关系变化"]
                },
                "foreshadow": {
                    "missing_items": ["伏笔兑现"]
                }
            }),
            &json!({
                "failed_metrics": [{
                    "label": "逻辑链"
                }]
            }),
            None,
            None,
        );

        assert_eq!(payload["status"], json!("warning"));
        assert_eq!(
            payload["summary"],
            json!("恢复前请先确认这些一致性 / 质量风险是否可接受。")
        );
        assert_eq!(
            payload["items"],
            json!([
                "连续性待补齐：角色状态；关系变化",
                "伏笔/回收待补齐：伏笔兑现",
                "质量门禁关注项：逻辑链"
            ])
        );
    }

    #[test]
    fn should_build_candidate_apply_risk_from_failed_metric_labels_only() {
        let payload = candidate_apply_risk_payload(
            &Value::Null,
            &Value::Null,
            &json!({
                "failed_metrics": [
                    {
                        "key": "logic",
                        "value": "should not appear"
                    },
                    {
                        "label": 42
                    },
                    {
                        "label": false
                    },
                    {
                        "label": true
                    }
                ]
            }),
            None,
            None,
        );

        assert_eq!(payload["items"], json!(["质量门禁关注项：42；True"]));
    }

    #[test]
    fn should_build_candidate_apply_risk_from_gate_status_or_action() {
        let from_status = candidate_apply_risk_payload(
            &Value::Null,
            &Value::Null,
            &json!({"status": " blocked "}),
            None,
            None,
        );
        let from_action = candidate_apply_risk_payload(
            &Value::Null,
            &Value::Null,
            &Value::Null,
            Some(" auto_repair "),
            None,
        );
        let without_risk = candidate_apply_risk_payload(
            &Value::Null,
            &Value::Null,
            &json!({"status": "ok"}),
            None,
            None,
        );

        assert_eq!(
            from_status["items"],
            json!(["当前候选稿仍建议先做一致性复核，再决定是否直接恢复。"])
        );
        assert_eq!(
            from_action["items"],
            json!(["当前候选稿仍建议先做一致性复核，再决定是否直接恢复。"])
        );
        assert_eq!(without_risk, Value::Null);
    }

    #[test]
    fn should_parse_candidate_apply_risk_gate_status_and_decision_scalars_like_python() {
        let fallback_decision = candidate_apply_risk_payload(
            &Value::Null,
            &Value::Null,
            &json!({"decision": "manual_review"}),
            None,
            Some(""),
        );
        let scalar_decision = candidate_apply_risk_payload(
            &Value::Null,
            &Value::Null,
            &json!({"decision": true}),
            None,
            None,
        );

        assert_eq!(
            fallback_decision["items"],
            json!(["当前候选稿仍建议先做一致性复核，再决定是否直接恢复。"])
        );
        assert_eq!(scalar_decision, Value::Null);
    }

    #[test]
    fn should_format_optional_datetime_without_timezone_suffix() {
        let formatted = format_datetime(Some(naive_datetime(2026, 5, 19, 8, 7, 6)));

        assert_eq!(formatted.as_deref(), Some("2026-05-19T08:07:06"));
        assert_eq!(format_datetime(None), None);
    }

    #[test]
    fn should_detect_draft_staleness_only_when_chapter_is_newer() {
        let draft_created_at = naive_datetime(2026, 5, 19, 8, 0, 0);

        assert!(is_draft_stale(
            Some(naive_datetime(2026, 5, 19, 8, 0, 1)),
            Some(draft_created_at),
        ));
        assert!(!is_draft_stale(
            Some(draft_created_at),
            Some(draft_created_at),
        ));
        assert!(!is_draft_stale(None, Some(draft_created_at)));
        assert!(!is_draft_stale(Some(draft_created_at), None));
    }

    #[test]
    fn should_parse_reviser_result_from_matching_history_payload() {
        let generated_content = json!({
            "log_type": "chapter_text_reviser_v1",
            "reviser_result": {
                "revised_text": "修订正文",
                "critical_count": 1
            }
        })
        .to_string();

        let parsed = parse_reviser_result_from_history(Some(&generated_content));

        assert_eq!(
            parsed.and_then(|value| value.get("revised_text").cloned()),
            Some(json!("修订正文"))
        );
    }

    #[test]
    fn should_reject_invalid_reviser_history_payloads() {
        assert!(parse_reviser_result_from_history(None).is_none());
        assert!(parse_reviser_result_from_history(Some("not-json")).is_none());
        assert!(parse_reviser_result_from_history(Some(
            r#"{"log_type":"other","reviser_result":{"revised_text":"ignored"}}"#
        ))
        .is_none());
        assert!(parse_reviser_result_from_history(Some(
            r#"{"log_type":"chapter_text_reviser_v1","reviser_result":"not-object"}"#
        ))
        .is_none());
        assert!(parse_reviser_result_from_history(Some(
            r#"{"log_type":"chapter_text_reviser_v1"}"#
        ))
        .is_none());
    }

    #[test]
    fn should_build_auto_revision_draft_payload_with_preview_defaults() {
        let created_at = naive_datetime(2026, 5, 19, 8, 0, 0);
        let chapter_updated_at = naive_datetime(2026, 5, 19, 8, 0, 1);
        let payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": " 修订正文 ",
                "critical_count": 1,
                "major_count": 2
            }),
            Some("history-1"),
            Some(created_at),
            Some(chapter_updated_at),
            false,
        );

        assert_eq!(payload["history_id"], json!("history-1"));
        assert_eq!(payload["revised_text_preview"], json!(" 修订正文 "));
        assert_eq!(payload["has_full_text"], json!(true));
        assert_eq!(payload["is_stale"], json!(true));
        assert_eq!(payload["created_at"], json!("2026-05-19T08:00:00"));
        assert_eq!(payload["unresolved_issues"], json!([]));
        assert_eq!(payload.get("revised_text"), None);
    }

    #[test]
    fn should_include_auto_revision_full_text_when_requested() {
        let payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": "完整修订正文",
                "revised_text_preview": "预览",
                "unresolved_issues": ["仍需确认"]
            }),
            None,
            None,
            None,
            true,
        );

        assert_eq!(payload["history_id"], Value::Null);
        assert_eq!(payload["revised_text_preview"], json!("预览"));
        assert_eq!(payload["revised_text"], json!("完整修订正文"));
        assert_eq!(payload["has_full_text"], json!(true));
        assert_eq!(payload["is_stale"], json!(false));
        assert_eq!(payload["created_at"], Value::Null);
        assert_eq!(payload["unresolved_issues"], json!(["仍需确认"]));
    }

    #[test]
    fn should_preserve_auto_revision_revised_text_whitespace_for_python_compat() {
        let payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": " 修订正文 "
            }),
            None,
            None,
            None,
            true,
        );
        let whitespace_only_payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": "   "
            }),
            None,
            None,
            None,
            false,
        );

        assert_eq!(payload["revised_text"], json!(" 修订正文 "));
        assert_eq!(payload["revised_text_preview"], json!(" 修订正文 "));
        assert_eq!(payload["has_full_text"], json!(true));
        assert_eq!(
            whitespace_only_payload["revised_text_preview"],
            json!("   ")
        );
        assert_eq!(whitespace_only_payload["has_full_text"], json!(true));
    }

    #[test]
    fn should_coerce_auto_revision_revised_text_scalars_like_python() {
        let numeric_payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": 42
            }),
            None,
            None,
            None,
            true,
        );
        let bool_preview_payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": "fallback",
                "revised_text_preview": true
            }),
            None,
            None,
            None,
            false,
        );
        let falsey_payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": false
            }),
            None,
            None,
            None,
            true,
        );

        assert_eq!(numeric_payload["revised_text"], json!("42"));
        assert_eq!(numeric_payload["revised_text_preview"], json!("42"));
        assert_eq!(numeric_payload["has_full_text"], json!(true));
        assert_eq!(numeric_payload["revised_word_count"], json!(2));
        assert_eq!(bool_preview_payload["revised_text_preview"], json!("True"));
        assert_eq!(falsey_payload["revised_text"], json!(""));
        assert_eq!(falsey_payload["revised_text_preview"], json!(""));
        assert_eq!(falsey_payload["has_full_text"], json!(false));
    }

    #[test]
    fn should_build_candidate_draft_payload_with_full_content_and_quality_fields() {
        let created_at = naive_datetime(2026, 5, 19, 8, 0, 0);
        let chapter_updated_at = naive_datetime(2026, 5, 19, 8, 0, 1);
        let draft_attempt = chapter_draft_attempt::Model {
            created_at: Some(created_at),
            summary_preview: Some("摘要".to_string()),
            content_preview: Some(" 候选正文 ".to_string()),
            word_count: 4,
            quality_metrics: Some(json!({
                "quality_gate": {
                    "failed_metrics": [{
                        "key": "logic",
                        "value": 0.6,
                        "threshold": 0.8,
                        "gap": 0.2,
                        "focus_area": "logic_flow",
                        "repair_target": "补足因果链"
                    }]
                },
                "repair_guidance": {
                    "recommended_actions": [" 修复动作 "],
                    "summary": "guidance summary"
                },
                "quality_highlights": {
                    "highlight_points": [" 亮点 "]
                },
                "apply_risk": {
                    "risk_points": [" 风险 "]
                }
            })),
            repair_payload: Some(json!({
                "candidate_full_content": " 完整候选正文 ",
                "preserve_strengths": [" 保留优点 "],
                "summary": "repair summary"
            })),
            ..candidate_draft_attempt(None, 0, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, Some(chapter_updated_at), true);

        assert_eq!(payload["attempt_id"], json!("attempt-1"));
        assert_eq!(payload["content_preview"], json!("候选正文"));
        assert_eq!(payload["content"], json!("完整候选正文"));
        assert_eq!(payload["has_full_content"], json!(true));
        assert_eq!(payload["can_apply"], json!(true));
        assert_eq!(payload["is_stale"], json!(true));
        assert_eq!(payload["created_at"], json!("2026-05-19T08:00:00"));
        assert_eq!(payload["highlight_points"], json!(["亮点"]));
        assert_eq!(payload["risk_points"], json!(["风险"]));
        assert_eq!(payload["recommended_actions"], json!(["修复动作"]));
        assert_eq!(payload["preserved_strengths"], json!(["保留优点"]));
        assert_eq!(payload["repair_summary"], json!("repair summary"));
        assert_eq!(payload["failed_metrics"][0]["key"], json!("logic"));
        assert_eq!(payload["failed_metrics"][0]["value"], json!(0.6));
        assert_eq!(payload["failed_metrics"][0]["threshold"], json!(0.8));
        assert_eq!(payload["failed_metrics"][0]["gap"], json!(0.2));
        assert_eq!(
            payload["failed_metrics"][0]["focus_area"],
            json!("logic_flow")
        );
        assert_eq!(
            payload["failed_metrics"][0]["repair_target"],
            json!("补足因果链")
        );
    }

    #[test]
    fn should_default_candidate_failed_metric_numbers_for_python_compat() {
        let draft_attempt = chapter_draft_attempt::Model {
            quality_metrics: Some(json!({
                "quality_gate": {
                    "failed_metrics": [{
                        "key": "logic"
                    }]
                }
            })),
            ..candidate_draft_attempt(Some("候选正文"), 4, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["failed_metrics"][0]["key"], json!("logic"));
        assert_eq!(payload["failed_metrics"][0]["label"], json!("logic"));
        assert_eq!(payload["failed_metrics"][0]["value"], json!(0.0));
        assert_eq!(payload["failed_metrics"][0]["threshold"], json!(0.0));
        assert_eq!(payload["failed_metrics"][0]["gap"], json!(0.0));
        assert_eq!(payload["failed_metrics"][0]["focus_area"], Value::Null);
        assert_eq!(payload["failed_metrics"][0]["repair_target"], Value::Null);
    }

    #[test]
    fn should_trim_candidate_failed_metric_strings_for_python_compat() {
        let draft_attempt = chapter_draft_attempt::Model {
            quality_metrics: Some(json!({
                "quality_gate": {
                    "failed_metrics": [{
                        "key": " logic ",
                        "label": " 逻辑链 ",
                        "value": "0.6",
                        "threshold": true,
                        "gap": false,
                        "focus_area": "   ",
                        "repair_target": " 补足因果 "
                    }]
                },
                "repair_guidance": {
                    "summary": "   "
                }
            })),
            repair_payload: Some(json!({
                "summary": " "
            })),
            ..candidate_draft_attempt(Some("候选正文"), 4, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["failed_metrics"][0]["key"], json!("logic"));
        assert_eq!(payload["failed_metrics"][0]["label"], json!("逻辑链"));
        assert_eq!(payload["failed_metrics"][0]["value"], json!(0.6));
        assert_eq!(payload["failed_metrics"][0]["threshold"], json!(1.0));
        assert_eq!(payload["failed_metrics"][0]["gap"], json!(0.0));
        assert_eq!(payload["failed_metrics"][0]["focus_area"], Value::Null);
        assert_eq!(
            payload["failed_metrics"][0]["repair_target"],
            json!("补足因果")
        );
        assert_eq!(payload["repair_summary"], Value::Null);
    }

    #[test]
    fn should_coerce_candidate_failed_metric_scalar_strings_like_python() {
        let draft_attempt = chapter_draft_attempt::Model {
            quality_metrics: Some(json!({
                "quality_gate": {
                    "failed_metrics": [
                        {
                            "key": 42,
                            "label": true,
                            "focus_area": false,
                            "repair_target": 0
                        },
                        {
                            "key": 7,
                            "label": false,
                            "focus_area": 3,
                            "repair_target": true
                        }
                    ]
                }
            })),
            ..candidate_draft_attempt(Some("候选正文"), 4, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["failed_metrics"][0]["key"], json!("42"));
        assert_eq!(payload["failed_metrics"][0]["label"], json!("True"));
        assert_eq!(payload["failed_metrics"][0]["focus_area"], Value::Null);
        assert_eq!(payload["failed_metrics"][0]["repair_target"], Value::Null);
        assert_eq!(payload["failed_metrics"][1]["key"], json!("7"));
        assert_eq!(payload["failed_metrics"][1]["label"], json!("7"));
        assert_eq!(payload["failed_metrics"][1]["focus_area"], json!("3"));
        assert_eq!(payload["failed_metrics"][1]["repair_target"], json!("True"));
    }

    #[test]
    fn should_fallback_candidate_repair_summary_with_python_truthiness() {
        let draft_attempt = chapter_draft_attempt::Model {
            quality_metrics: Some(json!({
                "repair_guidance": {
                    "summary": 12
                }
            })),
            repair_payload: Some(json!({
                "summary": false
            })),
            ..candidate_draft_attempt(Some("候选正文"), 4, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["repair_summary"], json!("12"));
    }

    #[test]
    fn should_build_candidate_draft_payload_with_compat_repair_fields() {
        let draft_attempt = chapter_draft_attempt::Model {
            quality_metrics: Some(json!({
                "quality_gate": {
                    "focus_areas": ["fallback focus"]
                },
                "repair_guidance": {
                    "repair_targets": [" guidance target "],
                    "preserve_strengths": [" guidance strength "],
                    "focus_areas": [" guidance focus "]
                }
            })),
            repair_payload: Some(json!({
                "repair_targets": [" repair target "],
                "preserve_strengths": [" repair strength "]
            })),
            ..candidate_draft_attempt(Some("候选正文"), 4, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["repair_targets"], json!(["repair target"]));
        assert_eq!(payload["preserve_strengths"], json!(["repair strength"]));
        assert_eq!(payload["preserved_strengths"], json!(["repair strength"]));
        assert_eq!(payload["focus_areas"], json!(["guidance focus"]));
    }

    #[test]
    fn should_build_candidate_draft_payload_with_fallback_apply_risk() {
        let draft_attempt = chapter_draft_attempt::Model {
            quality_metrics: Some(json!({
                "quality_gate": {
                    "failed_metrics": [{
                        "label": "逻辑链"
                    }]
                },
                "quality_highlights": {
                    "continuity": {
                        "missing_items": ["角色状态"]
                    }
                }
            })),
            repair_payload: Some(json!({})),
            ..candidate_draft_attempt(Some("候选正文"), 4, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["apply_risk"]["status"], json!("warning"));
        assert_eq!(
            payload["apply_risk"]["items"],
            json!(["连续性待补齐：角色状态", "质量门禁关注项：逻辑链"])
        );
    }

    #[test]
    fn should_ignore_empty_explicit_candidate_quality_highlights_like_python() {
        let draft_attempt = chapter_draft_attempt::Model {
            quality_metrics: Some(json!({
                "quality_highlights": {}
            })),
            ..candidate_draft_attempt(Some("候选正文"), 4, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["quality_highlights"], Value::Null);
        assert_eq!(payload["highlight_points"], json!([]));
    }

    #[test]
    fn should_preserve_legacy_highlight_points_without_exposing_empty_ui_quality_highlights() {
        let draft_attempt = chapter_draft_attempt::Model {
            quality_metrics: Some(json!({
                "quality_highlights": {
                    "highlight_points": ["legacy highlight"]
                }
            })),
            ..candidate_draft_attempt(Some("候选正文"), 4, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["quality_highlights"], Value::Null);
        assert_eq!(payload["highlight_points"], json!(["legacy highlight"]));
    }

    #[test]
    fn should_fallback_candidate_draft_compat_fields_to_guidance_and_quality_gate() {
        let draft_attempt = chapter_draft_attempt::Model {
            quality_metrics: Some(json!({
                "quality_gate": {
                    "focus_areas": [" gate focus "]
                },
                "repair_guidance": {
                    "repair_targets": [" guidance target "],
                    "preserve_strengths": [" guidance strength "]
                }
            })),
            repair_payload: Some(json!({})),
            ..candidate_draft_attempt(Some("候选正文"), 4, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["repair_targets"], json!(["guidance target"]));
        assert_eq!(payload["preserve_strengths"], json!(["guidance strength"]));
        assert_eq!(payload["focus_areas"], json!(["gate focus"]));
    }

    #[test]
    fn should_fallback_candidate_compat_item_fields_after_empty_normalization() {
        let draft_attempt = chapter_draft_attempt::Model {
            quality_metrics: Some(json!({
                "quality_gate": {
                    "focus_areas": [" gate focus "]
                },
                "repair_guidance": {
                    "repair_targets": [" guidance target "],
                    "preserve_strengths": [" guidance strength "],
                    "focus_areas": []
                }
            })),
            repair_payload: Some(json!({
                "repair_targets": false,
                "preserve_strengths": 0
            })),
            ..candidate_draft_attempt(Some("候选正文"), 4, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["repair_targets"], json!(["guidance target"]));
        assert_eq!(payload["preserve_strengths"], json!(["guidance strength"]));
        assert_eq!(payload["focus_areas"], json!(["gate focus"]));
    }

    #[test]
    fn should_build_candidate_draft_payload_without_full_content_for_preview_only() {
        let draft_attempt = candidate_draft_attempt(
            Some(" preview only "),
            100,
            Some(json!({
                "quality_highlights": {
                    "highlight_points": ["fallback highlight"]
                },
                "apply_risk": {
                    "risk_points": ["fallback risk"]
                }
            })),
        );

        let payload = build_candidate_draft_payload(&draft_attempt, None, true);

        assert_eq!(payload["content_preview"], json!("preview only"));
        assert_eq!(payload["has_full_content"], json!(false));
        assert_eq!(payload["content_complete"], json!(false));
        assert_eq!(payload["can_apply"], json!(false));
        assert_eq!(payload.get("content"), None);
        assert_eq!(payload["quality_metrics"], json!({}));
        assert_eq!(payload["highlight_points"], json!(["fallback highlight"]));
        assert_eq!(payload["risk_points"], json!(["fallback risk"]));
        assert_eq!(payload["recommended_actions"], json!([]));
        assert_eq!(payload["preserved_strengths"], json!([]));
        assert_eq!(payload["repair_targets"], json!([]));
        assert_eq!(payload["preserve_strengths"], json!([]));
        assert_eq!(payload["focus_areas"], json!([]));
        assert_eq!(payload["repair_summary"], Value::Null);
    }

    #[test]
    fn should_preserve_legacy_risk_points_without_exposing_empty_ui_apply_risk() {
        let draft_attempt = candidate_draft_attempt(
            Some(" preview only "),
            100,
            Some(json!({
                "apply_risk": {
                    "risk_points": ["legacy risk"]
                }
            })),
        );

        let payload = build_candidate_draft_payload(&draft_attempt, None, true);

        assert_eq!(payload["apply_risk"], Value::Null);
        assert_eq!(payload["risk_points"], json!(["legacy risk"]));
    }

    #[test]
    fn should_build_candidate_draft_payload_with_missing_created_at() {
        let draft_attempt = candidate_draft_attempt(
            Some("候选正文"),
            4,
            Some(json!({
                "content_complete": true
            })),
        );

        let payload = build_candidate_draft_payload(
            &draft_attempt,
            Some(naive_datetime(2026, 5, 19, 8, 0, 1)),
            true,
        );

        assert_eq!(payload["created_at"], Value::Null);
        assert_eq!(payload["is_stale"], json!(false));
        assert_eq!(payload["has_full_content"], json!(true));
        assert_eq!(payload["content"], json!("候选正文"));
    }

    #[test]
    fn should_use_candidate_summary_preview_when_content_preview_is_missing() {
        let draft_attempt = chapter_draft_attempt::Model {
            summary_preview: Some(" 摘要预览 ".to_string()),
            content_preview: None,
            ..candidate_draft_attempt(None, 0, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, true);

        assert_eq!(payload["summary_preview"], json!("摘要预览"));
        assert_eq!(payload["content_preview"], json!("摘要预览"));
        assert_eq!(payload["has_full_content"], json!(false));
        assert_eq!(payload["can_apply"], json!(false));
        assert_eq!(payload.get("content"), None);
    }

    #[test]
    fn should_fallback_candidate_content_preview_when_empty_like_python() {
        let draft_attempt = chapter_draft_attempt::Model {
            summary_preview: Some(" 摘要预览 ".to_string()),
            content_preview: Some("".to_string()),
            ..candidate_draft_attempt(None, 0, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, true);

        assert_eq!(payload["summary_preview"], json!("摘要预览"));
        assert_eq!(payload["content_preview"], json!("摘要预览"));
        assert_eq!(payload["has_full_content"], json!(false));
        assert_eq!(payload["can_apply"], json!(false));
        assert_eq!(payload.get("content"), None);
    }

    #[test]
    fn should_trim_candidate_source_state_and_fallback_word_count() {
        let draft_attempt = chapter_draft_attempt::Model {
            source: " batch ".to_string(),
            attempt_state: " ready ".to_string(),
            word_count: 0,
            repair_payload: Some(json!({
                "candidate_full_content": "完整候选正文"
            })),
            ..candidate_draft_attempt(None, 0, None)
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, true);

        assert_eq!(payload["source"], json!("batch"));
        assert_eq!(payload["attempt_state"], json!("ready"));
        assert_eq!(payload["word_count"], json!(6));
        assert_eq!(payload["content"], json!("完整候选正文"));
    }

    #[test]
    fn should_exclude_candidate_full_text_from_analysis_view_payload() {
        let draft_attempt = candidate_draft_attempt(
            None,
            0,
            Some(json!({
                "candidate_full_content": " 完整候选正文 "
            })),
        );

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["content_preview"], json!("完整候选正文"));
        assert_eq!(payload["has_full_content"], json!(true));
        assert_eq!(payload["can_apply"], json!(true));
        assert_eq!(payload.get("content"), None);
    }

    #[test]
    fn should_build_chapter_draft_analysis_view_fragments_from_latest_valid_inputs() {
        let history_created_at = naive_datetime(2026, 5, 19, 8, 0, 0);
        let chapter_updated_at = naive_datetime(2026, 5, 19, 8, 0, 1);
        let invalid_history = generation_history(
            "history-invalid",
            Some(json!({"log_type": "other"}).to_string()),
            Some(history_created_at),
        );
        let valid_history = generation_history(
            "history-valid",
            Some(
                json!({
                    "log_type": "chapter_text_reviser_v1",
                    "reviser_result": {
                        "revised_text": "修订正文"
                    }
                })
                .to_string(),
            ),
            Some(history_created_at),
        );
        let candidate_attempt = candidate_draft_attempt(
            Some("候选正文"),
            4,
            Some(json!({
                "content_complete": true
            })),
        );

        let fragments = build_chapter_draft_analysis_view_fragments(
            &[invalid_history, valid_history],
            Some(&candidate_attempt),
            Some(chapter_updated_at),
        );

        let auto_revision = fragments.auto_revision_draft.expect("auto revision draft");
        let candidate = fragments.candidate_draft.expect("candidate draft");
        assert_eq!(auto_revision["history_id"], json!("history-valid"));
        assert_eq!(auto_revision["revised_text_preview"], json!("修订正文"));
        assert_eq!(auto_revision.get("revised_text"), None);
        assert_eq!(auto_revision["is_stale"], json!(true));
        assert_eq!(candidate["attempt_id"], json!("attempt-1"));
        assert_eq!(candidate["content_preview"], json!("候选正文"));
        assert_eq!(candidate["has_full_content"], json!(true));
        assert_eq!(candidate.get("content"), None);
    }

    #[test]
    fn should_build_empty_chapter_draft_analysis_view_fragments_without_inputs() {
        let fragments = build_chapter_draft_analysis_view_fragments(&[], None, None);

        assert!(fragments.auto_revision_draft.is_none());
        assert!(fragments.candidate_draft.is_none());
    }
}
