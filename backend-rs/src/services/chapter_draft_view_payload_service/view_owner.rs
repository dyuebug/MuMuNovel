use chrono::NaiveDateTime;
use serde_json::{json, Value};

use crate::models::{chapter_draft_attempt, generation_history};
use crate::services::chapter_draft_history_service::parse_reviser_result_from_history;
use crate::services::chapter_draft_source_service::{
    extract_candidate_draft_full_content, format_datetime, is_draft_stale, json_i64,
    python_truthy_json_i64, python_truthy_scalar_text,
};

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

pub fn build_auto_revision_draft_payload(
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

pub fn build_candidate_draft_payload(
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

pub fn build_chapter_draft_analysis_view_fragments(
    histories: &[generation_history::Model],
    candidate_attempt: Option<&chapter_draft_attempt::Model>,
    chapter_updated_at: Option<NaiveDateTime>,
    include_full_text: bool,
) -> ChapterDraftAnalysisViewFragments {
    let auto_revision_draft = histories.iter().find_map(|history| {
        let reviser_result =
            parse_reviser_result_from_history(history.generated_content.as_deref())?;
        Some(build_auto_revision_draft_payload(
            &reviser_result,
            Some(&history.id),
            history.created_at,
            chapter_updated_at,
            include_full_text,
        ))
    });

    let candidate_draft = candidate_attempt.map(|attempt| {
        build_candidate_draft_payload(attempt, chapter_updated_at, include_full_text)
    });

    ChapterDraftAnalysisViewFragments {
        auto_revision_draft,
        candidate_draft,
    }
}

#[allow(dead_code)]
pub(crate) fn build_chapter_draft_view_payload_owner_contract() -> Value {
    json!({
        "owner": "chapter_draft_view_payload_service",
        "scope": "chapter_draft_analysis_view_payload_candidate_and_auto_revision_projection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_draft_view_payload_service.rs",
            "backend-rs/src/services/chapter_draft_view_payload_service/view_owner.rs",
            "backend-rs/src/services/chapter_draft_source_service.rs",
            "backend-rs/src/services/chapter_draft_history_service.rs",
            "backend-rs/src/services/chapter_analysis_runtime_service.rs"
        ],
        "behavior_contract": {
            "auto_revision_payload_owner": "build_auto_revision_draft_payload",
            "candidate_payload_owner": "build_candidate_draft_payload",
            "full_text_gate": "include_full_text",
            "candidate_apply_risk_owner": "candidate_apply_risk_payload",
            "normalization_limits": {
                "failed_metrics": 3,
                "risk_items": 4
            }
        },
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-chapter-draft-owner",
            "chapter_draft_manifest_probe_count": 8,
            "rust_manifest_probe_count": 8,
            "python_fallback_probe_count": 0,
            "analysis_view_fragment_owner": "chapter_draft_view_payload_service",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "chapter generation history source-map deleted; this owner now depends on Rust-only draft view/payload contracts",
            "status": "rust_service_runtime_owner_with_deleted_python_source_map"
        },
        "rollback_boundary": {
            "python_source_map_retained": false,
            "approval_required_before_python_edit": false
        }
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::{json, Value};

    use crate::models::{chapter_draft_attempt, generation_history};
    use crate::services::chapter_draft_history_service::parse_reviser_result_from_history;
    use crate::services::chapter_draft_source_service::{
        extract_candidate_draft_full_content, format_datetime, is_draft_stale,
    };

    use super::{
        auto_revision_draft_view_counts, build_auto_revision_draft_payload,
        build_candidate_draft_payload, build_chapter_draft_analysis_view_fragments,
        build_chapter_draft_view_payload_owner_contract, candidate_apply_risk_payload,
        normalize_candidate_items,
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
    fn should_publish_chapter_draft_view_payload_owner_contract() {
        let contract = build_chapter_draft_view_payload_owner_contract();

        assert_eq!(contract["owner"], "chapter_draft_view_payload_service");
        assert_eq!(
            contract["behavior_contract"]["auto_revision_payload_owner"],
            "build_auto_revision_draft_payload"
        );
        assert_eq!(
            contract["behavior_contract"]["candidate_payload_owner"],
            "build_candidate_draft_payload"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profile"],
            "phase5-chapter-draft-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["chapter_draft_manifest_probe_count"],
            8
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            true
        );
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
                "revised_word_count": false
            }),
            "修订正文",
        );

        assert_eq!(counts.critical_count, 1);
        assert_eq!(counts.major_count, 0);
        assert_eq!(counts.priority_issue_count, 1);
        assert_eq!(counts.applied_critical_count, 0);
        assert_eq!(counts.applied_issue_count, 1);
        assert_eq!(counts.revised_word_count, 0);
    }

    #[test]
    fn should_normalize_candidate_items_from_multiple_shapes() {
        let items = normalize_candidate_items(
            Some(&json!([
                " 线索A ",
                {"label": "线索B"},
                3,
                true,
                "",
                {"ignored": "x"},
                "线索A"
            ])),
            5,
        );

        assert_eq!(items, vec!["线索A", "线索B", "3", "True"]);
    }

    #[test]
    fn should_build_candidate_apply_risk_from_missing_highlights_and_failed_metrics() {
        let payload = candidate_apply_risk_payload(
            &Value::Null,
            &json!({
                "continuity": {"missing_items": ["前因"]},
                "foreshadow": {"missing_items": ["伏笔"]},
            }),
            &json!({
                "failed_metrics": [{"label": "节奏"}]
            }),
            None,
            None,
        );

        assert_eq!(payload["status"], json!("warning"));
        assert_eq!(
            payload["items"],
            json!([
                "连续性待补齐：前因",
                "伏笔/回收待补齐：伏笔",
                "质量门禁关注项：节奏"
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
                    {"label": "节奏"},
                    {"label": "一致性"}
                ]
            }),
            None,
            None,
        );

        assert_eq!(payload["status"], json!("warning"));
        assert_eq!(payload["items"], json!(["质量门禁关注项：节奏；一致性"]));
    }

    #[test]
    fn should_build_candidate_apply_risk_from_gate_status_or_action() {
        let payload = candidate_apply_risk_payload(
            &Value::Null,
            &Value::Null,
            &json!({"status": "warning"}),
            Some("manual_review"),
            None,
        );

        assert_eq!(payload["status"], json!("warning"));
        assert_eq!(
            payload["summary"],
            json!("恢复前请先确认这些一致性 / 质量风险是否可接受。")
        );
        assert_eq!(
            payload["items"],
            json!(["当前候选稿仍建议先做一致性复核，再决定是否直接恢复。"])
        );
    }

    #[test]
    fn should_build_auto_revision_draft_payload_with_preview_defaults() {
        let payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": "修订后的正文",
                "critical_count": 2,
                "major_count": 3
            }),
            Some("history-1"),
            Some(naive_datetime(2026, 5, 17, 12, 30, 45)),
            Some(naive_datetime(2026, 5, 18, 12, 30, 45)),
            false,
        );

        assert_eq!(payload["history_id"], json!("history-1"));
        assert_eq!(payload["critical_count"], json!(2));
        assert_eq!(payload["major_count"], json!(3));
        assert_eq!(payload["priority_issue_count"], json!(5));
        assert_eq!(payload["applied_issue_count"], json!(0));
        assert_eq!(payload["revised_text_preview"], json!("修订后的正文"));
        assert_eq!(payload["has_full_text"], json!(true));
        assert_eq!(payload["is_stale"], json!(true));
        assert_eq!(payload["created_at"], json!("2026-05-17T12:30:45"));
        assert_eq!(payload.get("revised_text"), None);
    }

    #[test]
    fn should_build_auto_revision_draft_payload_with_full_text() {
        let payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": "修订后的正文",
                "revised_text_preview": "预览",
                "change_summary": ["调整"],
                "unresolved_issues": ["问题"],
                "revised_word_count": 1234
            }),
            None,
            None,
            None,
            true,
        );

        assert_eq!(payload["revised_text_preview"], json!("预览"));
        assert_eq!(payload["revised_text"], json!("修订后的正文"));
        assert_eq!(payload["change_summary"], json!(["调整"]));
        assert_eq!(payload["unresolved_issues"], json!(["问题"]));
        assert_eq!(payload["revised_word_count"], json!(1234));
    }

    #[test]
    fn should_build_auto_revision_draft_payload_with_python_compat_preview_values() {
        let payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": "修订后的正文",
                "revised_text_preview": true
            }),
            None,
            None,
            None,
            false,
        );
        let whitespace_only_payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": "修订后的正文",
                "revised_text_preview": "   "
            }),
            None,
            None,
            None,
            false,
        );

        assert_eq!(payload["revised_text_preview"], json!("True"));
        assert_eq!(
            whitespace_only_payload["revised_text_preview"],
            json!("修订后的正文")
        );
    }

    #[test]
    fn should_build_auto_revision_draft_payload_with_python_compat_counts() {
        let numeric_payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": "修订后的正文",
                "priority_issue_count": "8",
                "applied_issue_count": "4"
            }),
            None,
            None,
            None,
            false,
        );
        let bool_preview_payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": "修订后的正文",
                "priority_issue_count": true,
                "applied_issue_count": false
            }),
            None,
            None,
            None,
            false,
        );
        let falsey_payload = build_auto_revision_draft_payload(
            &json!({
                "revised_text": "修订后的正文",
                "priority_issue_count": 0
            }),
            None,
            None,
            None,
            false,
        );

        assert_eq!(numeric_payload["priority_issue_count"], json!(8));
        assert_eq!(numeric_payload["applied_issue_count"], json!(4));
        assert_eq!(bool_preview_payload["priority_issue_count"], json!(1));
        assert_eq!(bool_preview_payload["applied_issue_count"], json!(0));
        assert_eq!(falsey_payload["priority_issue_count"], json!(0));
    }

    #[test]
    fn should_build_candidate_draft_payload_with_full_content_and_quality_fields() {
        let chapter_updated_at = naive_datetime(2026, 5, 18, 12, 30, 45);
        let draft_attempt = chapter_draft_attempt::Model {
            id: "attempt-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            batch_task_id: None,
            source: " quality_repair_candidate ".to_string(),
            attempt_state: " ready ".to_string(),
            quality_gate_action: Some("manual_review".to_string()),
            quality_gate_decision: Some("blocked".to_string()),
            word_count: 0,
            summary_preview: Some(" 摘要 ".to_string()),
            content_preview: Some(" 预览 ".to_string()),
            quality_metrics: Some(json!({
                "quality_gate": {
                    "failed_metrics": [{
                        "key": "pace",
                        "label": "节奏",
                        "value": "0.3",
                        "threshold": 0.8,
                        "gap": true,
                        "focus_area": "紧凑度",
                        "repair_target": "压缩铺垫"
                    }]
                },
                "repair_guidance": {
                    "summary": "建议聚焦冲突推进",
                    "recommended_actions": ["压缩铺垫"],
                    "repair_targets": ["冲突推进"],
                    "preserve_strengths": ["人物情绪"],
                    "focus_areas": ["节奏控制"]
                },
                "candidate_selection": {"score": 0.91},
                "quality_highlights": {
                    "continuity": {"missing_items": ["前因"]},
                    "highlight_points": ["节奏更快"]
                }
            })),
            repair_payload: Some(json!({
                "candidate_full_content": "完整候选正文",
                "summary": "修复摘要",
                "repair_targets": ["冲突推进"],
                "preserve_strengths": ["人物情绪"],
                "apply_risk": {
                    "status": "warning",
                    "summary": "请确认",
                    "items": ["仍需人工复核"]
                }
            })),
            created_at: Some(naive_datetime(2026, 5, 17, 12, 30, 45)),
        };

        let payload = build_candidate_draft_payload(&draft_attempt, Some(chapter_updated_at), true);

        assert_eq!(payload["attempt_id"], json!("attempt-1"));
        assert_eq!(payload["source"], json!("quality_repair_candidate"));
        assert_eq!(payload["attempt_state"], json!("ready"));
        assert_eq!(payload["quality_gate_action"], json!("manual_review"));
        assert_eq!(payload["quality_gate_decision"], json!("blocked"));
        assert_eq!(payload["word_count"], json!(6));
        assert_eq!(payload["summary_preview"], json!("摘要"));
        assert_eq!(payload["content_preview"], json!("预览"));
        assert_eq!(payload["created_at"], json!("2026-05-17T12:30:45"));
        assert_eq!(payload["is_stale"], json!(true));
        assert_eq!(payload["has_full_content"], json!(true));
        assert_eq!(payload["content_complete"], json!(true));
        assert_eq!(payload["can_apply"], json!(true));
        assert_eq!(payload["content"], json!("完整候选正文"));
        assert_eq!(payload["candidate_selection"], json!({"score": 0.91}));
        assert_eq!(
            payload["failed_metrics"],
            json!([{
                "key": "pace",
                "label": "节奏",
                "value": 0.3,
                "threshold": 0.8,
                "gap": 1.0,
                "focus_area": "紧凑度",
                "repair_target": "压缩铺垫"
            }])
        );
        assert_eq!(payload["repair_summary"], json!("修复摘要"));
        assert_eq!(payload["apply_risk"]["summary"], json!("请确认"));
    }

    #[test]
    fn should_build_candidate_draft_payload_with_compat_repair_fields() {
        let draft_attempt = chapter_draft_attempt::Model {
            id: "attempt-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            batch_task_id: None,
            source: "quality_repair_candidate".to_string(),
            attempt_state: "ready".to_string(),
            quality_gate_action: None,
            quality_gate_decision: None,
            word_count: 0,
            summary_preview: None,
            content_preview: None,
            quality_metrics: Some(json!({
                "repair_guidance": {
                    "summary": "指导摘要",
                    "repair_targets": ["目标A"],
                    "preserve_strengths": ["优点A"],
                    "focus_areas": ["关注A"]
                }
            })),
            repair_payload: Some(json!({
                "summary": "修复摘要",
                "repair_targets": ["目标B"],
                "preserve_strengths": ["优点B"],
                "content_complete": true,
                "full_content": "完整正文"
            })),
            created_at: None,
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["repair_targets"], json!(["目标B"]));
        assert_eq!(payload["preserved_strengths"], json!(["优点B"]));
        assert_eq!(payload["preserve_strengths"], json!(["优点B"]));
        assert_eq!(payload["focus_areas"], json!(["关注A"]));
        assert_eq!(payload["repair_summary"], json!("修复摘要"));
    }

    #[test]
    fn should_build_candidate_draft_payload_with_fallback_apply_risk() {
        let draft_attempt = chapter_draft_attempt::Model {
            id: "attempt-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            batch_task_id: None,
            source: "quality_repair_candidate".to_string(),
            attempt_state: "ready".to_string(),
            quality_gate_action: Some("manual_review".to_string()),
            quality_gate_decision: None,
            word_count: 1,
            summary_preview: None,
            content_preview: None,
            quality_metrics: Some(json!({
                "quality_gate": {
                    "status": "warning",
                    "failed_metrics": [{"label": "节奏"}]
                }
            })),
            repair_payload: Some(json!({})),
            created_at: None,
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, false);

        assert_eq!(payload["apply_risk"]["status"], json!("warning"));
        assert_eq!(payload["risk_points"], json!([]));
    }

    #[test]
    fn should_build_candidate_draft_payload_without_full_content_for_preview_only() {
        let draft_attempt = chapter_draft_attempt::Model {
            id: "attempt-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            batch_task_id: None,
            source: "quality_repair_candidate".to_string(),
            attempt_state: "ready".to_string(),
            quality_gate_action: None,
            quality_gate_decision: None,
            word_count: 0,
            summary_preview: Some("摘要".to_string()),
            content_preview: Some("预览片段".to_string()),
            quality_metrics: None,
            repair_payload: Some(json!({})),
            created_at: Some(naive_datetime(2026, 5, 17, 12, 30, 45)),
        };

        let payload = build_candidate_draft_payload(&draft_attempt, None, true);

        assert_eq!(payload["content_preview"], json!("预览片段"));
        assert_eq!(payload["has_full_content"], json!(false));
        assert_eq!(payload["can_apply"], json!(false));
        assert!(payload.get("content").is_none());
    }

    #[test]
    fn should_build_candidate_draft_payload_with_missing_created_at() {
        let draft_attempt = candidate_draft_attempt(
            Some("候选预览"),
            0,
            Some(json!({
                "full_content": "完整候选正文",
                "content_complete": true
            })),
        );

        let payload = build_candidate_draft_payload(
            &draft_attempt,
            Some(naive_datetime(2026, 5, 17, 12, 30, 45)),
            true,
        );

        assert!(payload["created_at"].is_null());
        assert_eq!(payload["is_stale"], json!(false));
        assert_eq!(payload["word_count"], json!(4));
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
            false,
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
        let fragments = build_chapter_draft_analysis_view_fragments(&[], None, None, false);

        assert!(fragments.auto_revision_draft.is_none());
        assert!(fragments.candidate_draft.is_none());
    }

    #[test]
    fn should_parse_reviser_history_payload_for_analysis_fragments() {
        let history = generation_history(
            "history-1",
            Some(
                json!({
                    "log_type": "chapter_text_reviser_v1",
                    "reviser_result": {
                        "revised_text": "修订正文"
                    }
                })
                .to_string(),
            ),
            Some(naive_datetime(2026, 5, 17, 12, 30, 45)),
        );

        let parsed = parse_reviser_result_from_history(history.generated_content.as_deref());

        assert_eq!(parsed, Some(json!({"revised_text": "修订正文"})));
    }

    #[test]
    fn should_build_chapter_draft_analysis_view_fragments_with_full_text_when_requested() {
        let chapter_updated_at = naive_datetime(2026, 5, 18, 9, 0, 0);
        let history = generation_history(
            "history-full",
            Some(
                json!({
                    "log_type": "chapter_text_reviser_v1",
                    "reviser_result": {
                        "revised_text": "修订正文"
                    }
                })
                .to_string(),
            ),
            Some(naive_datetime(2026, 5, 17, 12, 30, 45)),
        );
        let candidate_attempt = candidate_draft_attempt(
            Some("候选正文"),
            4,
            Some(json!({
                "candidate_full_content": "完整候选正文",
                "content_complete": true
            })),
        );

        let fragments = build_chapter_draft_analysis_view_fragments(
            &[history],
            Some(&candidate_attempt),
            Some(chapter_updated_at),
            true,
        );

        let auto_revision = fragments.auto_revision_draft.expect("auto revision draft");
        let candidate = fragments.candidate_draft.expect("candidate draft");

        assert_eq!(auto_revision["revised_text"], json!("修订正文"));
        assert_eq!(candidate["content"], json!("完整候选正文"));
    }

    #[test]
    fn should_keep_candidate_source_helpers_compatible() {
        let draft_attempt = candidate_draft_attempt(
            Some("预览"),
            0,
            Some(json!({
                "candidate_full_content": "完整候选正文",
                "content_complete": true
            })),
        );

        let (content, has_full_content) = extract_candidate_draft_full_content(&draft_attempt);

        assert_eq!(content, "完整候选正文");
        assert!(has_full_content);
        assert_eq!(format_datetime(None), None);
        assert!(!is_draft_stale(None, None));
    }
}
