use chrono::NaiveDateTime;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde_json::{json, Value};

use crate::models::{chapter, chapter_draft_attempt, generation_history};
use crate::services::chapter_analysis_service::{AutoRevisionDraftError, CandidateDraftError};

pub struct ChapterDraftAnalysisViewFragments {
    pub auto_revision_draft: Option<Value>,
    pub candidate_draft: Option<Value>,
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
                    Value::String(text) => push_item(&mut items, &mut seen, text, limit),
                    Value::Object(map) => {
                        if let Some(text) = map
                            .get("label")
                            .and_then(Value::as_str)
                            .or_else(|| map.get("name").and_then(Value::as_str))
                            .or_else(|| map.get("value").and_then(Value::as_str))
                            .or_else(|| map.get("item").and_then(Value::as_str))
                        {
                            push_item(&mut items, &mut seen, text, limit);
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
                if let Some(text) = map.get(key).and_then(Value::as_str) {
                    push_item(&mut items, &mut seen, text, limit);
                    break;
                }
            }
        }
        _ => {}
    }

    items
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
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return (full_content.to_string(), true);
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
        .and_then(Value::as_bool)
        .unwrap_or(false)
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
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let mut revised_text_preview = reviser_result
        .get("revised_text_preview")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if revised_text_preview.is_empty() && !revised_text.is_empty() {
        revised_text_preview = revised_text.chars().take(500).collect();
    }

    let critical_count = reviser_result
        .get("critical_count")
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32;
    let major_count = reviser_result
        .get("major_count")
        .and_then(Value::as_i64)
        .unwrap_or(0) as i32;
    let priority_issue_count = reviser_result
        .get("priority_issue_count")
        .and_then(Value::as_i64)
        .unwrap_or((critical_count + major_count) as i64) as i32;
    let applied_issue_count = reviser_result
        .get("applied_issue_count")
        .and_then(Value::as_i64)
        .or_else(|| {
            reviser_result
                .get("applied_critical_count")
                .and_then(Value::as_i64)
        })
        .unwrap_or(0) as i32;
    let applied_critical_count = reviser_result
        .get("applied_critical_count")
        .and_then(Value::as_i64)
        .unwrap_or(applied_issue_count as i64) as i32;
    let revised_word_count = reviser_result
        .get("revised_word_count")
        .and_then(Value::as_i64)
        .unwrap_or(revised_text.chars().count() as i64) as i32;

    let mut payload = serde_json::Map::new();
    payload.insert("history_id".to_string(), json!(history_id));
    payload.insert("critical_count".to_string(), json!(critical_count));
    payload.insert("major_count".to_string(), json!(major_count));
    payload.insert("priority_issue_count".to_string(), json!(priority_issue_count));
    payload.insert(
        "applied_critical_count".to_string(),
        json!(applied_critical_count),
    );
    payload.insert("applied_issue_count".to_string(), json!(applied_issue_count));
    payload.insert(
        "change_summary".to_string(),
        reviser_result
            .get("change_summary")
            .cloned()
            .unwrap_or(Value::Null),
    );
    payload.insert("revised_word_count".to_string(), json!(revised_word_count));
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
    let quality_highlights = quality_metrics
        .get("quality_highlights")
        .cloned()
        .or_else(|| repair_payload.get("quality_highlights").cloned())
        .filter(Value::is_object)
        .unwrap_or(Value::Null);
    let apply_risk = quality_metrics
        .get("apply_risk")
        .cloned()
        .or_else(|| repair_payload.get("apply_risk").cloned())
        .filter(Value::is_object)
        .unwrap_or(Value::Null);

    let (full_content, has_full_content) = extract_candidate_draft_full_content(draft_attempt);
    let mut preview_text = draft_attempt
        .content_preview
        .clone()
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
                    Some(json!({
                        "key": object.get("key").and_then(Value::as_str).unwrap_or_default(),
                        "label": object.get("label").and_then(Value::as_str).or_else(|| object.get("key").and_then(Value::as_str)).unwrap_or_default(),
                        "value": object.get("value").and_then(Value::as_f64),
                        "threshold": object.get("threshold").and_then(Value::as_f64),
                        "gap": object.get("gap").and_then(Value::as_f64),
                        "focus_area": object.get("focus_area").and_then(Value::as_str),
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let highlight_points = normalize_candidate_items(
        quality_highlights
            .as_object()
            .and_then(|payload| payload.get("highlight_points")),
        6,
    );
    let risk_points = normalize_candidate_items(
        apply_risk.as_object().and_then(|payload| payload.get("risk_points")),
        6,
    );
    let recommended_actions = normalize_candidate_items(
        repair_guidance
            .as_object()
            .and_then(|payload| payload.get("recommended_actions")),
        8,
    );
    let preserved_strengths = normalize_candidate_items(
        repair_payload
            .as_object()
            .and_then(|payload| payload.get("preserve_strengths")),
        6,
    );
    let repair_summary = json!(
        repair_payload
            .get("summary")
            .and_then(Value::as_str)
            .or_else(|| repair_guidance.get("summary").and_then(Value::as_str))
    );

    let mut payload = serde_json::Map::new();
    payload.insert("attempt_id".to_string(), json!(draft_attempt.id));
    payload.insert("source".to_string(), json!(draft_attempt.source));
    payload.insert("attempt_state".to_string(), json!(draft_attempt.attempt_state));
    payload.insert(
        "quality_gate_action".to_string(),
        json!(draft_attempt.quality_gate_action),
    );
    payload.insert(
        "quality_gate_decision".to_string(),
        json!(draft_attempt.quality_gate_decision),
    );
    payload.insert("word_count".to_string(), json!(draft_attempt.word_count));
    payload.insert(
        "summary_preview".to_string(),
        json!(draft_attempt.summary_preview.clone().unwrap_or_default()),
    );
    payload.insert("content_preview".to_string(), json!(preview_text));
    payload.insert("created_at".to_string(), json!(format_datetime(draft_attempt.created_at)));
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
    payload.insert(
        "preserved_strengths".to_string(),
        json!(preserved_strengths),
    );
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
    let mut query =
        chapter_draft_attempt::Entity::find().filter(chapter_draft_attempt::Column::ChapterId.eq(
            Some(chapter_id.to_string()),
        ));

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
