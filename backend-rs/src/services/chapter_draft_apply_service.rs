use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set, TransactionTrait};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{chapter, chapter_draft_attempt, generation_history};
use crate::services::chapter_analysis_service::{AutoRevisionDraftError, CandidateDraftError};
use crate::services::chapter_draft_query_service::{
    extract_candidate_draft_full_content, format_datetime, is_draft_stale, json_i64,
    load_candidate_draft_attempt, load_latest_reviser_history, python_truthy_json_i64,
    python_truthy_scalar_text,
};
use crate::services::chapter_narrative_cleaner_service::{
    contains_chapter_workflow_meta_text, sanitize_generated_narrative_text,
};

const CANDIDATE_DRAFT_APPLY_MODEL: &str = "chapter_candidate_apply_v1";
const AUTO_REVISION_DRAFT_APPLY_MODEL: &str = "chapter_text_reviser_apply_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ApplyDraftWordCounts {
    old_word_count: i32,
    new_word_count: i32,
    new_word_count_usize: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AutoRevisionApplyIssueCounts {
    critical_count: i64,
    major_count: i64,
    priority_issue_count: i64,
    applied_critical_count: i64,
    applied_issue_count: i64,
}

fn sanitize_apply_draft_text<E>(
    raw_text: &str,
    empty_error: E,
    workflow_meta_error: E,
) -> Result<String, E> {
    let (cleaned_text, _) = sanitize_generated_narrative_text(raw_text);
    if cleaned_text.trim().is_empty() {
        return Err(empty_error);
    }
    if contains_chapter_workflow_meta_text(&cleaned_text) {
        return Err(workflow_meta_error);
    }
    Ok(cleaned_text)
}

fn prepare_candidate_draft_apply_text(
    draft_attempt: &chapter_draft_attempt::Model,
) -> Result<String, CandidateDraftError> {
    let (candidate_content_raw, has_full_content) =
        extract_candidate_draft_full_content(draft_attempt);
    if !has_full_content || candidate_content_raw.trim().is_empty() {
        return Err(CandidateDraftError::PreviewOnly);
    }

    sanitize_apply_draft_text(
        &candidate_content_raw,
        CandidateDraftError::EmptyContent,
        CandidateDraftError::WorkflowMetaText,
    )
}

fn prepare_auto_revision_draft_apply_text(
    reviser_result: &Value,
) -> Result<String, AutoRevisionDraftError> {
    let revised_text_raw = reviser_result
        .get("revised_text")
        .and_then(python_truthy_scalar_text)
        .unwrap_or_default();

    sanitize_apply_draft_text(
        &revised_text_raw,
        AutoRevisionDraftError::EmptyContent,
        AutoRevisionDraftError::WorkflowMetaText,
    )
}

fn validate_apply_draft_staleness<E>(
    chapter_updated_at: Option<chrono::NaiveDateTime>,
    draft_created_at: Option<chrono::NaiveDateTime>,
    allow_stale: bool,
    stale_error: E,
) -> Result<bool, E> {
    let stale = is_draft_stale(chapter_updated_at, draft_created_at);
    if stale && !allow_stale {
        return Err(stale_error);
    }
    Ok(stale)
}

fn apply_draft_word_counts(previous_word_count: i32, new_text: &str) -> ApplyDraftWordCounts {
    let new_word_count_usize = new_text.chars().count();
    ApplyDraftWordCounts {
        old_word_count: previous_word_count.max(0),
        new_word_count: new_word_count_usize as i32,
        new_word_count_usize,
    }
}

fn draft_apply_chapter_update_model(
    chapter: &chapter::Model,
    content: String,
    new_word_count: i32,
    updated_at: chrono::NaiveDateTime,
) -> chapter::ActiveModel {
    let mut chapter_active: chapter::ActiveModel = chapter.clone().into();
    chapter_active.content = Set(Some(content));
    chapter_active.word_count = Set(new_word_count);
    chapter_active.updated_at = Set(Some(updated_at));
    chapter_active
}

fn candidate_draft_generated_content_payload(
    candidate_content: &str,
    quality_metrics: Option<Value>,
) -> Value {
    json!({
        "content": candidate_content,
        "quality_metrics": quality_metrics.unwrap_or(Value::Null),
        "content_applied": true,
        "attempt_state": "applied_from_candidate",
    })
}

fn candidate_draft_apply_response_payload(
    chapter_id: &str,
    new_word_count: i32,
    old_word_count: i32,
    draft_attempt_id: &str,
    draft_created_at: Option<chrono::NaiveDateTime>,
    stale: bool,
) -> Value {
    json!({
        "success": true,
        "chapter_id": chapter_id,
        "word_count": new_word_count,
        "old_word_count": old_word_count,
        "draft_attempt_id": draft_attempt_id,
        "draft_created_at": format_datetime(draft_created_at),
        "stale_applied": stale,
        "message": "候选草稿已恢复到章节正文",
    })
}

fn auto_revision_apply_issue_counts(reviser_result: &Value) -> AutoRevisionApplyIssueCounts {
    let critical_count = json_i64(reviser_result.get("critical_count")).unwrap_or(0);
    let major_count = json_i64(reviser_result.get("major_count")).unwrap_or(0);
    let priority_issue_count = python_truthy_json_i64(reviser_result.get("priority_issue_count"))
        .unwrap_or(critical_count + major_count);
    let applied_critical_count =
        json_i64(reviser_result.get("applied_critical_count")).unwrap_or(0);
    let applied_issue_count = python_truthy_json_i64(reviser_result.get("applied_issue_count"))
        .or_else(|| python_truthy_json_i64(reviser_result.get("applied_critical_count")))
        .unwrap_or(0);

    AutoRevisionApplyIssueCounts {
        critical_count,
        major_count,
        priority_issue_count,
        applied_critical_count,
        applied_issue_count,
    }
}

fn auto_revision_apply_history_payload(
    reviser_history: &generation_history::Model,
    reviser_result: &Value,
    old_word_count: i32,
    new_word_count: usize,
    stale: bool,
    allow_stale: bool,
    applied_at: Option<chrono::NaiveDateTime>,
) -> Value {
    let issue_counts = auto_revision_apply_issue_counts(reviser_result);

    json!({
        "log_type": AUTO_REVISION_DRAFT_APPLY_MODEL,
        "source_history_id": reviser_history.id,
        "source_created_at": format_datetime(reviser_history.created_at),
        "critical_count": issue_counts.critical_count,
        "major_count": issue_counts.major_count,
        "priority_issue_count": issue_counts.priority_issue_count,
        "applied_critical_count": issue_counts.applied_critical_count,
        "applied_issue_count": issue_counts.applied_issue_count,
        "old_word_count": old_word_count,
        "new_word_count": new_word_count,
        "stale_applied": stale,
        "allow_stale": allow_stale,
        "applied_at": format_datetime(applied_at),
    })
}

fn candidate_draft_apply_history_prompt(chapter_number: i32, chapter_title: &str) -> String {
    format!(
        "apply candidate draft: chapter {} {}",
        chapter_number, chapter_title
    )
}

fn auto_revision_draft_apply_history_prompt(chapter_number: i32, chapter_title: &str) -> String {
    format!("自动修订应用: 第{}章 {}", chapter_number, chapter_title)
}

fn candidate_draft_apply_history_model(
    history_id: String,
    chapter: &chapter::Model,
    generated_content: Value,
    created_at: chrono::NaiveDateTime,
) -> generation_history::ActiveModel {
    generation_history::ActiveModel {
        id: Set(history_id),
        project_id: Set(chapter.project_id.clone()),
        chapter_id: Set(Some(chapter.id.clone())),
        prompt: Set(Some(candidate_draft_apply_history_prompt(
            chapter.chapter_number,
            &chapter.title,
        ))),
        generated_content: Set(Some(generated_content.to_string())),
        model: Set(Some(CANDIDATE_DRAFT_APPLY_MODEL.to_string())),
        tokens_used: Set(None),
        generation_time: Set(None),
        created_at: Set(Some(created_at)),
    }
}

fn auto_revision_draft_apply_history_model(
    history_id: String,
    chapter: &chapter::Model,
    history_payload: Value,
    created_at: chrono::NaiveDateTime,
) -> generation_history::ActiveModel {
    generation_history::ActiveModel {
        id: Set(history_id),
        project_id: Set(chapter.project_id.clone()),
        chapter_id: Set(Some(chapter.id.clone())),
        prompt: Set(Some(auto_revision_draft_apply_history_prompt(
            chapter.chapter_number,
            &chapter.title,
        ))),
        generated_content: Set(Some(history_payload.to_string())),
        model: Set(Some(AUTO_REVISION_DRAFT_APPLY_MODEL.to_string())),
        tokens_used: Set(None),
        generation_time: Set(None),
        created_at: Set(Some(created_at)),
    }
}

fn auto_revision_draft_apply_response_payload(
    chapter_id: &str,
    new_word_count: i32,
    old_word_count: i32,
    draft_history_id: &str,
    draft_created_at: Option<chrono::NaiveDateTime>,
    stale: bool,
) -> Value {
    json!({
        "success": true,
        "chapter_id": chapter_id,
        "word_count": new_word_count,
        "old_word_count": old_word_count,
        "draft_history_id": draft_history_id,
        "draft_created_at": format_datetime(draft_created_at),
        "stale_applied": stale,
        "message": "自动修订草稿已应用到章节正文",
    })
}

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

    let candidate_content = prepare_candidate_draft_apply_text(&draft_attempt)?;

    let stale = validate_apply_draft_staleness(
        chapter.updated_at,
        draft_attempt.created_at,
        allow_stale,
        CandidateDraftError::Stale,
    )?;

    let generated_content = candidate_draft_generated_content_payload(
        &candidate_content,
        draft_attempt.quality_metrics.clone(),
    );

    let now = Utc::now().naive_utc();
    let word_counts = apply_draft_word_counts(chapter.word_count, &candidate_content);
    let txn = db
        .begin()
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    let chapter_active = draft_apply_chapter_update_model(
        chapter,
        candidate_content,
        word_counts.new_word_count,
        now,
    );
    chapter_active
        .update(&txn)
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    let history = candidate_draft_apply_history_model(
        Uuid::new_v4().to_string(),
        chapter,
        generated_content,
        now,
    );
    history
        .insert(&txn)
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    txn.commit()
        .await
        .map_err(|error| CandidateDraftError::Internal(error.to_string()))?;

    Ok(candidate_draft_apply_response_payload(
        &chapter.id,
        word_counts.new_word_count,
        word_counts.old_word_count,
        &draft_attempt.id,
        draft_attempt.created_at,
        stale,
    ))
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

    let revised_text = prepare_auto_revision_draft_apply_text(&reviser_result)?;

    let stale = validate_apply_draft_staleness(
        chapter.updated_at,
        reviser_history.created_at,
        allow_stale,
        AutoRevisionDraftError::Stale,
    )?;

    let word_counts = apply_draft_word_counts(chapter.word_count, &revised_text);
    let history_payload = auto_revision_apply_history_payload(
        &reviser_history,
        &reviser_result,
        chapter.word_count,
        word_counts.new_word_count_usize,
        stale,
        allow_stale,
        Some(Utc::now().naive_utc()),
    );
    let now = Utc::now().naive_utc();
    let txn = db
        .begin()
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    let chapter_active =
        draft_apply_chapter_update_model(chapter, revised_text, word_counts.new_word_count, now);
    chapter_active
        .update(&txn)
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    let history = auto_revision_draft_apply_history_model(
        Uuid::new_v4().to_string(),
        chapter,
        history_payload,
        now,
    );
    history
        .insert(&txn)
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    txn.commit()
        .await
        .map_err(|error| AutoRevisionDraftError::Internal(error.to_string()))?;

    Ok(auto_revision_draft_apply_response_payload(
        &chapter.id,
        word_counts.new_word_count,
        word_counts.old_word_count,
        &reviser_history.id,
        reviser_history.created_at,
        stale,
    ))
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::{json, Value};

    use crate::models::{chapter, chapter_draft_attempt, generation_history};
    use crate::services::chapter_analysis_service::{AutoRevisionDraftError, CandidateDraftError};

    use super::{
        apply_draft_word_counts, auto_revision_apply_history_payload,
        auto_revision_apply_issue_counts, auto_revision_draft_apply_history_model,
        auto_revision_draft_apply_history_prompt, auto_revision_draft_apply_response_payload,
        candidate_draft_apply_history_model, candidate_draft_apply_history_prompt,
        candidate_draft_apply_response_payload, candidate_draft_generated_content_payload,
        draft_apply_chapter_update_model, prepare_auto_revision_draft_apply_text,
        prepare_candidate_draft_apply_text, sanitize_apply_draft_text,
        validate_apply_draft_staleness, AUTO_REVISION_DRAFT_APPLY_MODEL,
        CANDIDATE_DRAFT_APPLY_MODEL,
    };

    fn fixed_time(day: u32) -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 5, day)
            .unwrap()
            .and_hms_opt(8, 30, 15)
            .unwrap()
    }

    fn chapter_model() -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
            summary: Some("summary".to_string()),
            content: Some("旧正文".to_string()),
            word_count: 900,
            status: "completed".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: fixed_time(17),
            updated_at: Some(fixed_time(18)),
        }
    }

    fn draft_attempt(
        content_preview: Option<&str>,
        word_count: i32,
    ) -> chapter_draft_attempt::Model {
        chapter_draft_attempt::Model {
            id: "attempt-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            batch_task_id: None,
            source: "quality_gate_candidate".to_string(),
            attempt_state: "candidate_ready".to_string(),
            quality_gate_action: None,
            quality_gate_decision: None,
            word_count,
            summary_preview: None,
            content_preview: content_preview.map(str::to_string),
            quality_metrics: None,
            repair_payload: None,
            created_at: Some(fixed_time(18)),
        }
    }

    fn reviser_history() -> generation_history::Model {
        generation_history::Model {
            id: "history-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            prompt: Some("prompt".to_string()),
            generated_content: Some("{}".to_string()),
            model: Some("chapter_text_reviser_v1".to_string()),
            tokens_used: Some(10),
            generation_time: Some(1.5),
            created_at: Some(fixed_time(18)),
        }
    }

    #[test]
    fn should_build_candidate_draft_generated_content_payload() {
        let payload =
            candidate_draft_generated_content_payload("候选正文", Some(json!({"overall": 0.91})));

        assert_eq!(payload["content"], "候选正文");
        assert_eq!(payload["quality_metrics"]["overall"], 0.91);
        assert_eq!(payload["content_applied"], true);
        assert_eq!(payload["attempt_state"], "applied_from_candidate");

        let without_metrics = candidate_draft_generated_content_payload("候选正文", None);
        assert_eq!(without_metrics["quality_metrics"], Value::Null);
    }

    #[test]
    fn should_sanitize_candidate_draft_apply_text() {
        let result = sanitize_apply_draft_text(
            "  候选正文  ",
            CandidateDraftError::EmptyContent,
            CandidateDraftError::WorkflowMetaText,
        );

        match result {
            Ok(cleaned) => assert_eq!(cleaned, "候选正文"),
            Err(_) => panic!("expected sanitized candidate draft text"),
        }
    }

    #[test]
    fn should_reject_empty_candidate_draft_apply_text() {
        let error = sanitize_apply_draft_text(
            "   ",
            CandidateDraftError::EmptyContent,
            CandidateDraftError::WorkflowMetaText,
        )
        .unwrap_err();

        assert!(matches!(error, CandidateDraftError::EmptyContent));
    }

    #[test]
    fn should_prepare_candidate_draft_apply_text_from_complete_preview() {
        let result = prepare_candidate_draft_apply_text(&draft_attempt(Some("候选正文"), 4));

        match result {
            Ok(text) => assert_eq!(text, "候选正文"),
            Err(_) => panic!("complete candidate preview should be applicable"),
        }
    }

    #[test]
    fn should_prepare_candidate_draft_apply_text_from_truthy_scalar_full_content() {
        let draft_attempt = chapter_draft_attempt::Model {
            repair_payload: Some(json!({
                "candidate_full_content": 42
            })),
            ..draft_attempt(Some("预览"), 999)
        };

        let result = prepare_candidate_draft_apply_text(&draft_attempt);

        match result {
            Ok(text) => assert_eq!(text, "42"),
            Err(_) => panic!("numeric candidate full content should be applicable"),
        }
    }

    #[test]
    fn should_reject_candidate_draft_apply_preview_only_text() {
        let error = prepare_candidate_draft_apply_text(&draft_attempt(Some("预览"), 999))
            .expect_err("incomplete candidate preview should be rejected");

        assert!(matches!(error, CandidateDraftError::PreviewOnly));
    }

    #[test]
    fn should_strip_meta_prefix_from_auto_revision_apply_text() {
        let result = sanitize_apply_draft_text(
            "以下是章节正文：\n正文",
            AutoRevisionDraftError::EmptyContent,
            AutoRevisionDraftError::WorkflowMetaText,
        );

        match result {
            Ok(cleaned) => assert_eq!(cleaned, "正文"),
            Err(_) => panic!("expected meta prefix to be stripped before apply"),
        }
    }

    #[test]
    fn should_reject_auto_revision_apply_missing_revised_text() {
        let error = prepare_auto_revision_draft_apply_text(&json!({}))
            .expect_err("missing revised_text should be rejected");

        assert!(matches!(error, AutoRevisionDraftError::EmptyContent));
    }

    #[test]
    fn should_allow_fresh_candidate_draft_apply() {
        let stale = match validate_apply_draft_staleness(
            Some(fixed_time(17)),
            Some(fixed_time(18)),
            false,
            CandidateDraftError::Stale,
        ) {
            Ok(stale) => stale,
            Err(_) => panic!("fresh draft should be allowed"),
        };

        assert_eq!(stale, false);
    }

    #[test]
    fn should_reject_stale_candidate_draft_apply_when_not_allowed() {
        let error = validate_apply_draft_staleness(
            Some(fixed_time(19)),
            Some(fixed_time(18)),
            false,
            CandidateDraftError::Stale,
        )
        .unwrap_err();

        assert!(matches!(error, CandidateDraftError::Stale));
    }

    #[test]
    fn should_allow_stale_auto_revision_draft_apply_when_requested() {
        let stale = match validate_apply_draft_staleness(
            Some(fixed_time(19)),
            Some(fixed_time(18)),
            true,
            AutoRevisionDraftError::Stale,
        ) {
            Ok(stale) => stale,
            Err(_) => panic!("allowed stale auto revision draft should pass"),
        };

        assert_eq!(stale, true);
    }

    #[test]
    fn should_calculate_apply_draft_word_counts() {
        let counts = apply_draft_word_counts(900, "新正文");

        assert_eq!(counts.old_word_count, 900);
        assert_eq!(counts.new_word_count, 3);
        assert_eq!(counts.new_word_count_usize, 3);
    }

    #[test]
    fn should_clamp_negative_old_apply_draft_word_count() {
        let counts = apply_draft_word_counts(-5, "正文");

        assert_eq!(counts.old_word_count, 0);
        assert_eq!(counts.new_word_count, 2);
        assert_eq!(counts.new_word_count_usize, 2);
    }

    #[test]
    fn should_build_draft_apply_chapter_update_model() {
        let active = draft_apply_chapter_update_model(
            &chapter_model(),
            "新正文".to_string(),
            3,
            fixed_time(19),
        );

        assert_eq!(active.id.unwrap(), "chapter-1");
        assert_eq!(active.project_id.unwrap(), "project-1");
        assert_eq!(active.content.unwrap(), Some("新正文".to_string()));
        assert_eq!(active.word_count.unwrap(), 3);
        assert_eq!(active.updated_at.unwrap(), Some(fixed_time(19)));
        assert_eq!(active.title.unwrap(), "第七章");
        assert_eq!(active.status.unwrap(), "completed");
    }

    #[test]
    fn should_build_candidate_draft_apply_response_payload() {
        let payload = candidate_draft_apply_response_payload(
            "chapter-1",
            1200,
            900,
            "attempt-1",
            Some(fixed_time(17)),
            true,
        );

        assert_eq!(payload["success"], true);
        assert_eq!(payload["chapter_id"], "chapter-1");
        assert_eq!(payload["word_count"], 1200);
        assert_eq!(payload["old_word_count"], 900);
        assert_eq!(payload["draft_attempt_id"], "attempt-1");
        assert_eq!(payload["draft_created_at"], "2026-05-17T08:30:15");
        assert_eq!(payload["stale_applied"], true);
        assert_eq!(payload["message"], "候选草稿已恢复到章节正文");
    }

    #[test]
    fn should_build_candidate_draft_apply_response_payload_with_missing_created_at() {
        let payload = candidate_draft_apply_response_payload(
            "chapter-1",
            1200,
            900,
            "attempt-1",
            None,
            false,
        );

        assert_eq!(payload["draft_created_at"], Value::Null);
        assert_eq!(payload["stale_applied"], false);
        assert_eq!(payload["draft_attempt_id"], "attempt-1");
    }

    #[test]
    fn should_build_auto_revision_apply_history_payload() {
        let payload = auto_revision_apply_history_payload(
            &reviser_history(),
            &json!({
                "critical_count": 2,
                "major_count": 3,
                "applied_critical_count": 1
            }),
            900,
            1100,
            true,
            true,
            Some(fixed_time(19)),
        );

        assert_eq!(payload["log_type"], AUTO_REVISION_DRAFT_APPLY_MODEL);
        assert_eq!(payload["source_history_id"], "history-1");
        assert_eq!(payload["source_created_at"], "2026-05-18T08:30:15");
        assert_eq!(payload["critical_count"], 2);
        assert_eq!(payload["major_count"], 3);
        assert_eq!(payload["priority_issue_count"], 5);
        assert_eq!(payload["applied_critical_count"], 1);
        assert_eq!(payload["applied_issue_count"], 1);
        assert_eq!(payload["old_word_count"], 900);
        assert_eq!(payload["new_word_count"], 1100);
        assert_eq!(payload["stale_applied"], true);
        assert_eq!(payload["allow_stale"], true);
        assert_eq!(payload["applied_at"], "2026-05-19T08:30:15");
    }

    #[test]
    fn should_build_auto_revision_apply_history_payload_with_missing_timestamps() {
        let mut history = reviser_history();
        history.created_at = None;

        let payload = auto_revision_apply_history_payload(
            &history,
            &json!({}),
            900,
            1100,
            false,
            false,
            None,
        );

        assert_eq!(payload["source_created_at"], Value::Null);
        assert_eq!(payload["applied_at"], Value::Null);
        assert_eq!(payload["critical_count"], 0);
        assert_eq!(payload["major_count"], 0);
        assert_eq!(payload["priority_issue_count"], 0);
        assert_eq!(payload["applied_issue_count"], 0);
    }

    #[test]
    fn should_build_auto_revision_apply_issue_counts_with_defaults() {
        let counts = auto_revision_apply_issue_counts(&json!({
            "critical_count": 2,
            "major_count": 3,
            "applied_critical_count": 1
        }));

        assert_eq!(counts.critical_count, 2);
        assert_eq!(counts.major_count, 3);
        assert_eq!(counts.priority_issue_count, 5);
        assert_eq!(counts.applied_critical_count, 1);
        assert_eq!(counts.applied_issue_count, 1);
    }

    #[test]
    fn should_preserve_explicit_auto_revision_apply_issue_counts() {
        let counts = auto_revision_apply_issue_counts(&json!({
            "critical_count": 2,
            "major_count": 3,
            "priority_issue_count": 9,
            "applied_critical_count": 1,
            "applied_issue_count": 4
        }));

        assert_eq!(counts.critical_count, 2);
        assert_eq!(counts.major_count, 3);
        assert_eq!(counts.priority_issue_count, 9);
        assert_eq!(counts.applied_critical_count, 1);
        assert_eq!(counts.applied_issue_count, 4);
    }

    #[test]
    fn should_parse_auto_revision_apply_issue_counts_from_numeric_strings() {
        let counts = auto_revision_apply_issue_counts(&json!({
            "critical_count": "2",
            "major_count": "3",
            "priority_issue_count": "9",
            "applied_critical_count": "1",
            "applied_issue_count": "4"
        }));

        assert_eq!(counts.critical_count, 2);
        assert_eq!(counts.major_count, 3);
        assert_eq!(counts.priority_issue_count, 9);
        assert_eq!(counts.applied_critical_count, 1);
        assert_eq!(counts.applied_issue_count, 4);
    }

    #[test]
    fn should_parse_auto_revision_apply_issue_counts_from_bool_values_for_python_compat() {
        let counts = auto_revision_apply_issue_counts(&json!({
            "critical_count": true,
            "major_count": false,
            "priority_issue_count": true,
            "applied_critical_count": false,
            "applied_issue_count": true
        }));

        assert_eq!(counts.critical_count, 1);
        assert_eq!(counts.major_count, 0);
        assert_eq!(counts.priority_issue_count, 1);
        assert_eq!(counts.applied_critical_count, 0);
        assert_eq!(counts.applied_issue_count, 1);
    }

    #[test]
    fn should_fallback_auto_revision_apply_issue_counts_from_falsey_values_like_python() {
        let counts = auto_revision_apply_issue_counts(&json!({
            "critical_count": 2,
            "major_count": 3,
            "priority_issue_count": false,
            "applied_critical_count": 4,
            "applied_issue_count": 0
        }));

        assert_eq!(counts.priority_issue_count, 5);
        assert_eq!(counts.applied_critical_count, 4);
        assert_eq!(counts.applied_issue_count, 4);
    }

    #[test]
    fn should_build_candidate_draft_apply_history_fields() {
        assert_eq!(CANDIDATE_DRAFT_APPLY_MODEL, "chapter_candidate_apply_v1");
        assert_eq!(
            candidate_draft_apply_history_prompt(7, "第七章"),
            "apply candidate draft: chapter 7 第七章"
        );
    }

    #[test]
    fn should_build_auto_revision_draft_apply_history_fields() {
        assert_eq!(
            AUTO_REVISION_DRAFT_APPLY_MODEL,
            "chapter_text_reviser_apply_v1"
        );
        assert_eq!(
            auto_revision_draft_apply_history_prompt(7, "第七章"),
            "自动修订应用: 第7章 第七章"
        );
    }

    #[test]
    fn should_build_candidate_draft_apply_history_model() {
        let history = candidate_draft_apply_history_model(
            "history-new".to_string(),
            &chapter_model(),
            json!({"content": "候选正文"}),
            fixed_time(19),
        );

        assert_eq!(history.id.unwrap(), "history-new");
        assert_eq!(history.project_id.unwrap(), "project-1");
        assert_eq!(history.chapter_id.unwrap(), Some("chapter-1".to_string()));
        assert_eq!(
            history.prompt.unwrap(),
            Some("apply candidate draft: chapter 7 第七章".to_string())
        );
        assert_eq!(
            history.generated_content.unwrap(),
            Some(json!({"content": "候选正文"}).to_string())
        );
        assert_eq!(
            history.model.unwrap(),
            Some(CANDIDATE_DRAFT_APPLY_MODEL.to_string())
        );
        assert_eq!(history.tokens_used.unwrap(), None);
        assert_eq!(history.generation_time.unwrap(), None);
        assert_eq!(history.created_at.unwrap(), Some(fixed_time(19)));
    }

    #[test]
    fn should_build_auto_revision_draft_apply_history_model() {
        let history = auto_revision_draft_apply_history_model(
            "history-new".to_string(),
            &chapter_model(),
            json!({"log_type": AUTO_REVISION_DRAFT_APPLY_MODEL}),
            fixed_time(19),
        );

        assert_eq!(history.id.unwrap(), "history-new");
        assert_eq!(history.project_id.unwrap(), "project-1");
        assert_eq!(history.chapter_id.unwrap(), Some("chapter-1".to_string()));
        assert_eq!(
            history.prompt.unwrap(),
            Some("自动修订应用: 第7章 第七章".to_string())
        );
        assert_eq!(
            history.generated_content.unwrap(),
            Some(json!({"log_type": AUTO_REVISION_DRAFT_APPLY_MODEL}).to_string())
        );
        assert_eq!(
            history.model.unwrap(),
            Some(AUTO_REVISION_DRAFT_APPLY_MODEL.to_string())
        );
        assert_eq!(history.tokens_used.unwrap(), None);
        assert_eq!(history.generation_time.unwrap(), None);
        assert_eq!(history.created_at.unwrap(), Some(fixed_time(19)));
    }

    #[test]
    fn should_preserve_explicit_auto_revision_issue_counts() {
        let payload = auto_revision_apply_history_payload(
            &reviser_history(),
            &json!({
                "critical_count": 2,
                "major_count": 3,
                "priority_issue_count": 9,
                "applied_critical_count": 1,
                "applied_issue_count": 4
            }),
            900,
            1100,
            false,
            false,
            None,
        );

        assert_eq!(payload["priority_issue_count"], 9);
        assert_eq!(payload["applied_issue_count"], 4);
        assert_eq!(payload["stale_applied"], false);
        assert_eq!(payload["allow_stale"], false);
        assert_eq!(payload["applied_at"], Value::Null);
    }

    #[test]
    fn should_build_auto_revision_draft_apply_response_payload() {
        let payload = auto_revision_draft_apply_response_payload(
            "chapter-1",
            1300,
            900,
            "history-1",
            Some(fixed_time(18)),
            false,
        );

        assert_eq!(payload["success"], true);
        assert_eq!(payload["chapter_id"], "chapter-1");
        assert_eq!(payload["word_count"], 1300);
        assert_eq!(payload["old_word_count"], 900);
        assert_eq!(payload["draft_history_id"], "history-1");
        assert_eq!(payload["draft_created_at"], "2026-05-18T08:30:15");
        assert_eq!(payload["stale_applied"], false);
        assert_eq!(payload["message"], "自动修订草稿已应用到章节正文");
    }

    #[test]
    fn should_build_auto_revision_draft_apply_response_payload_with_missing_created_at() {
        let payload = auto_revision_draft_apply_response_payload(
            "chapter-1",
            1300,
            900,
            "history-1",
            None,
            true,
        );

        assert_eq!(payload["draft_created_at"], Value::Null);
        assert_eq!(payload["stale_applied"], true);
        assert_eq!(payload["draft_history_id"], "history-1");
    }

    #[test]
    fn should_prepare_auto_revision_draft_apply_text_from_revised_text() {
        let result = prepare_auto_revision_draft_apply_text(&json!({
            "revised_text": "  自动修订正文  "
        }));

        match result {
            Ok(text) => assert_eq!(text, "自动修订正文"),
            Err(_) => panic!("auto revision revised_text should be applicable"),
        }
    }

    #[test]
    fn should_prepare_auto_revision_draft_apply_text_from_truthy_scalar_revised_text() {
        let numeric_result = prepare_auto_revision_draft_apply_text(&json!({
            "revised_text": 42
        }));
        let bool_result = prepare_auto_revision_draft_apply_text(&json!({
            "revised_text": true
        }));
        let false_result = prepare_auto_revision_draft_apply_text(&json!({
            "revised_text": false
        }));

        match numeric_result {
            Ok(text) => assert_eq!(text, "42"),
            Err(_) => panic!("numeric auto revision revised_text should be applicable"),
        }
        match bool_result {
            Ok(text) => assert_eq!(text, "True"),
            Err(_) => panic!("truthy bool auto revision revised_text should be applicable"),
        }
        assert!(matches!(
            false_result,
            Err(AutoRevisionDraftError::EmptyContent)
        ));
    }
}
