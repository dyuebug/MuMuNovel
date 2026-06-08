use chrono::NaiveDateTime;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::Value;

use crate::models::chapter_draft_attempt;

pub(crate) fn format_datetime(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string())
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

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::json;

    use crate::models::chapter_draft_attempt;

    use super::{extract_candidate_draft_full_content, format_datetime, is_draft_stale};

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
    fn should_extract_candidate_full_content_from_complete_preview_payload() {
        let draft_attempt = candidate_draft_attempt(
            Some("候选正文"),
            4,
            Some(json!({
                "content_complete": true
            })),
        );

        let (full_content, has_full_content) = extract_candidate_draft_full_content(&draft_attempt);

        assert_eq!(full_content, "候选正文");
        assert!(has_full_content);
    }
}
