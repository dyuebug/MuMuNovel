use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set, TransactionTrait};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{chapter, generation_history};
use crate::services::chapter_analysis_service::{AutoRevisionDraftError, CandidateDraftError};
use crate::services::chapter_draft_query_service::{
    extract_candidate_draft_full_content, format_datetime, is_draft_stale,
    load_candidate_draft_attempt, load_latest_reviser_history,
};
use crate::services::chapter_narrative_cleaner_service::{
    contains_chapter_workflow_meta_text, sanitize_generated_narrative_text,
};

pub async fn apply_candidate_draft_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
    attempt_id: Option<&str>,
    allow_stale: bool,
) -> Result<Value, CandidateDraftError> {
    let draft_attempt = load_candidate_draft_attempt(db, &chapter.id, attempt_id)
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    let draft_attempt = draft_attempt.ok_or(CandidateDraftError::NotFound)?;

    let (candidate_content_raw, has_full_content) =
        extract_candidate_draft_full_content(&draft_attempt);
    if !has_full_content || candidate_content_raw.trim().is_empty() {
        return Err(CandidateDraftError::PreviewOnly);
    }

    let (candidate_content, _) = sanitize_generated_narrative_text(&candidate_content_raw);
    if candidate_content.trim().is_empty() {
        return Err(CandidateDraftError::EmptyContent);
    }
    if contains_chapter_workflow_meta_text(&candidate_content) {
        return Err(CandidateDraftError::WorkflowMetaText);
    }

    let stale = is_draft_stale(chapter.updated_at, draft_attempt.created_at);
    if stale && !allow_stale {
        return Err(CandidateDraftError::Stale);
    }

    let generated_content = json!({
        "content": candidate_content,
        "quality_metrics": draft_attempt.quality_metrics.clone().unwrap_or(Value::Null),
        "content_applied": true,
        "attempt_state": "applied_from_candidate",
    });

    let now = Utc::now().naive_utc();
    let old_word_count = chapter.word_count.max(0);
    let new_word_count = candidate_content.chars().count() as i32;
    let txn = db
        .begin()
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    let mut chapter_active: chapter::ActiveModel = chapter.clone().into();
    chapter_active.content = Set(Some(candidate_content));
    chapter_active.word_count = Set(new_word_count);
    chapter_active.updated_at = Set(Some(now));
    chapter_active
        .update(&txn)
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    let history = generation_history::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(chapter.project_id.clone()),
        chapter_id: Set(Some(chapter.id.clone())),
        prompt: Set(Some(format!(
            "apply candidate draft: chapter {} {}",
            chapter.chapter_number, chapter.title
        ))),
        generated_content: Set(Some(generated_content.to_string())),
        model: Set(Some("chapter_candidate_apply_v1".to_string())),
        tokens_used: Set(None),
        generation_time: Set(None),
        created_at: Set(Some(now)),
    };
    history
        .insert(&txn)
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    txn.commit()
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    Ok(json!({
        "success": true,
        "chapter_id": chapter.id,
        "word_count": new_word_count,
        "old_word_count": old_word_count,
        "draft_attempt_id": draft_attempt.id,
        "draft_created_at": format_datetime(draft_attempt.created_at),
        "stale_applied": stale,
        "message": "候选草稿已恢复到章节正文",
    }))
}

pub async fn apply_auto_revision_draft_payload(
    db: &DatabaseConnection,
    chapter: &chapter::Model,
    history_id: Option<&str>,
    allow_stale: bool,
) -> Result<Value, AutoRevisionDraftError> {
    let reviser_loaded = load_latest_reviser_history(db, &chapter.id, history_id)
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;
    let (reviser_history, reviser_result) =
        reviser_loaded.ok_or(AutoRevisionDraftError::NotFound)?;

    let revised_text_raw = reviser_result
        .get("revised_text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (revised_text, _) = sanitize_generated_narrative_text(revised_text_raw);
    if revised_text.trim().is_empty() {
        return Err(AutoRevisionDraftError::EmptyContent);
    }
    if contains_chapter_workflow_meta_text(&revised_text) {
        return Err(AutoRevisionDraftError::WorkflowMetaText);
    }

    let stale = is_draft_stale(chapter.updated_at, reviser_history.created_at);
    if stale && !allow_stale {
        return Err(AutoRevisionDraftError::Stale);
    }

    let critical_count = reviser_result
        .get("critical_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let major_count = reviser_result
        .get("major_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let priority_issue_count = reviser_result
        .get("priority_issue_count")
        .and_then(Value::as_i64)
        .unwrap_or(critical_count + major_count);
    let applied_critical_count = reviser_result
        .get("applied_critical_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let applied_issue_count = reviser_result
        .get("applied_issue_count")
        .and_then(Value::as_i64)
        .or(Some(applied_critical_count))
        .unwrap_or(0);
    let history_payload = json!({
        "log_type": "chapter_text_reviser_apply_v1",
        "source_history_id": reviser_history.id,
        "source_created_at": format_datetime(reviser_history.created_at),
        "critical_count": critical_count,
        "major_count": major_count,
        "priority_issue_count": priority_issue_count,
        "applied_critical_count": applied_critical_count,
        "applied_issue_count": applied_issue_count,
        "old_word_count": chapter.word_count,
        "new_word_count": revised_text.chars().count(),
        "stale_applied": stale,
        "allow_stale": allow_stale,
        "applied_at": format_datetime(Some(Utc::now().naive_utc())),
    });

    let now = Utc::now().naive_utc();
    let old_word_count = chapter.word_count.max(0);
    let new_word_count = revised_text.chars().count() as i32;
    let txn = db
        .begin()
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    let mut chapter_active: chapter::ActiveModel = chapter.clone().into();
    chapter_active.content = Set(Some(revised_text));
    chapter_active.word_count = Set(new_word_count);
    chapter_active.updated_at = Set(Some(now));
    chapter_active
        .update(&txn)
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    let history = generation_history::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        project_id: Set(chapter.project_id.clone()),
        chapter_id: Set(Some(chapter.id.clone())),
        prompt: Set(Some(format!(
            "自动修订应用: 第{}章 {}",
            chapter.chapter_number, chapter.title
        ))),
        generated_content: Set(Some(history_payload.to_string())),
        model: Set(Some("chapter_text_reviser_apply_v1".to_string())),
        tokens_used: Set(None),
        generation_time: Set(None),
        created_at: Set(Some(now)),
    };
    history
        .insert(&txn)
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    txn.commit()
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    Ok(json!({
        "success": true,
        "chapter_id": chapter.id,
        "word_count": new_word_count,
        "old_word_count": old_word_count,
        "draft_history_id": reviser_history.id,
        "draft_created_at": format_datetime(reviser_history.created_at),
        "stale_applied": stale,
        "message": "自动修订草稿已应用到章节正文",
    }))
}
