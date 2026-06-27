use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde_json::{json, Value};

use crate::models::{chapter, generation_history};
use crate::services::chapter_draft_source_service::{
    format_datetime, json_i64, python_truthy_json_i64,
};

pub(crate) const CANDIDATE_DRAFT_APPLY_MODEL: &str = "chapter_candidate_apply_v1";
pub(crate) const AUTO_REVISION_DRAFT_APPLY_MODEL: &str = "chapter_text_reviser_apply_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AutoRevisionApplyIssueCounts {
    critical_count: i64,
    major_count: i64,
    priority_issue_count: i64,
    applied_critical_count: i64,
    applied_issue_count: i64,
}

pub struct ChapterAnalysisCheckerFragments {
    pub checker_result: Option<Value>,
    pub checker_created_at: Option<String>,
}

impl ChapterAnalysisCheckerFragments {
    pub fn from_histories(histories: &[generation_history::Model]) -> Self {
        let checker_result = histories.iter().find_map(|history| {
            parse_checker_result_from_history(history.generated_content.as_deref())
        });
        let checker_created_at = histories.iter().find_map(|history| {
            parse_checker_result_from_history(history.generated_content.as_deref())?;
            format_datetime(history.created_at)
        });

        Self {
            checker_result,
            checker_created_at,
        }
    }
}

pub(crate) fn parse_reviser_result_from_history(generated_content: Option<&str>) -> Option<Value> {
    let generated_content = generated_content?;
    let payload: Value = serde_json::from_str(generated_content).ok()?;
    if payload.get("log_type").and_then(Value::as_str) != Some("chapter_text_reviser_v1") {
        return None;
    }
    let reviser_result = payload.get("reviser_result")?;
    reviser_result.is_object().then(|| reviser_result.clone())
}

pub(crate) fn parse_checker_result_from_history(generated_content: Option<&str>) -> Option<Value> {
    let generated_content = generated_content?;
    let payload: Value = serde_json::from_str(generated_content).ok()?;
    if payload.get("log_type").and_then(Value::as_str) != Some("chapter_text_checker_v1") {
        return None;
    }
    let checker_result = payload.get("checker_result")?;
    checker_result.is_object().then(|| checker_result.clone())
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

pub(crate) async fn load_recent_generation_histories(
    db: &DatabaseConnection,
    chapter_id: &str,
    limit: u64,
) -> Result<Vec<generation_history::Model>, sea_orm::DbErr> {
    generation_history::Entity::find()
        .filter(generation_history::Column::ChapterId.eq(Some(chapter_id.to_string())))
        .order_by_desc(generation_history::Column::CreatedAt)
        .limit(limit)
        .all(db)
        .await
}

pub(crate) fn candidate_draft_generated_content_payload(
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

pub(crate) fn auto_revision_apply_history_payload(
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

pub(crate) fn candidate_draft_apply_history_model(
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

pub(crate) fn auto_revision_draft_apply_history_model(
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

#[allow(dead_code)]
pub(crate) fn build_chapter_draft_history_owner_contract() -> Value {
    json!({
        "owner": "chapter_draft_history_service",
        "scope": "chapter_draft_apply_history_reviser_checker_fragments_and_generation_history_models",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_draft_history_service.rs",
            "backend-rs/src/services/chapter_draft_source_service.rs",
            "backend-rs/src/services/chapter_draft_view_payload_service.rs"
        ],
        "behavior_contract": {
            "candidate_apply_model": CANDIDATE_DRAFT_APPLY_MODEL,
            "auto_revision_apply_model": AUTO_REVISION_DRAFT_APPLY_MODEL,
            "checker_result_log_type": "chapter_text_checker_v1",
            "reviser_result_log_type": "chapter_text_reviser_v1",
            "history_created_at_format_owner": "chapter_draft_source_service::format_datetime"
        },
        "service_runtime_closeout_status": {
            "owner_profile": "phase5-chapter-draft-owner",
            "chapter_draft_manifest_probe_count": 8,
            "rust_manifest_probe_count": 8,
            "python_fallback_probe_count": 0,
            "history_payload_owner": "chapter_draft_history_service",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": true,
            "remaining_cutover_gate": "chapter generation history source-map deleted; this owner now depends on Rust-only draft/history contracts",
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
    use chrono::{NaiveDate, NaiveDateTime};
    use serde_json::{json, Value};

    use crate::models::{chapter, generation_history};

    use super::{
        auto_revision_apply_history_payload, auto_revision_apply_issue_counts,
        auto_revision_draft_apply_history_model, auto_revision_draft_apply_history_prompt,
        build_chapter_draft_history_owner_contract, candidate_draft_apply_history_model,
        candidate_draft_apply_history_prompt, candidate_draft_generated_content_payload,
        parse_checker_result_from_history, parse_reviser_result_from_history,
        ChapterAnalysisCheckerFragments, AUTO_REVISION_DRAFT_APPLY_MODEL,
        CANDIDATE_DRAFT_APPLY_MODEL,
    };

    fn naive_datetime(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .expect("valid date")
            .and_hms_opt(hour, minute, second)
            .expect("valid time")
    }

    fn generation_history(
        id: &str,
        generated_content: Option<String>,
        created_at: Option<NaiveDateTime>,
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
            created_at: naive_datetime(2026, 5, 17, 8, 30, 15),
            updated_at: Some(naive_datetime(2026, 5, 18, 8, 30, 15)),
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
            created_at: Some(naive_datetime(2026, 5, 18, 8, 30, 15)),
        }
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
    fn should_publish_chapter_draft_history_owner_contract() {
        let contract = build_chapter_draft_history_owner_contract();

        assert_eq!(contract["owner"], "chapter_draft_history_service");
        assert_eq!(
            contract["behavior_contract"]["candidate_apply_model"],
            CANDIDATE_DRAFT_APPLY_MODEL
        );
        assert_eq!(
            contract["behavior_contract"]["auto_revision_apply_model"],
            AUTO_REVISION_DRAFT_APPLY_MODEL
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
    fn should_parse_checker_result_from_matching_history_payload() {
        let generated_content = json!({
            "log_type": "chapter_text_checker_v1",
            "checker_result": {
                "score": 91,
                "status": "passed"
            }
        })
        .to_string();

        let parsed = parse_checker_result_from_history(Some(&generated_content));

        assert_eq!(
            parsed.and_then(|value| value.get("status").cloned()),
            Some(json!("passed"))
        );
    }

    #[test]
    fn should_build_checker_fragments_from_first_matching_history() {
        let histories = vec![
            generation_history(
                "unrelated",
                Some(json!({"log_type": "other", "checker_result": {"score": 1}}).to_string()),
                Some(naive_datetime(2026, 5, 17, 12, 30, 45)),
            ),
            generation_history(
                "checker",
                Some(
                    json!({
                        "log_type": "chapter_text_checker_v1",
                        "checker_result": {
                            "score": 91,
                            "status": "passed"
                        }
                    })
                    .to_string(),
                ),
                Some(naive_datetime(2026, 5, 17, 12, 30, 45)),
            ),
        ];

        let fragments = ChapterAnalysisCheckerFragments::from_histories(&histories);

        assert_eq!(
            fragments.checker_result,
            Some(json!({"score": 91, "status": "passed"}))
        );
        assert_eq!(
            fragments.checker_created_at,
            Some("2026-05-17T12:30:45".to_string())
        );
    }

    #[test]
    fn should_ignore_invalid_or_non_checker_histories() {
        let histories = vec![
            generation_history("invalid-json", Some("{not-json".to_string()), None),
            generation_history(
                "missing-result",
                Some(json!({"log_type": "chapter_text_checker_v1"}).to_string()),
                None,
            ),
        ];

        let fragments = ChapterAnalysisCheckerFragments::from_histories(&histories);

        assert_eq!(fragments.checker_result, None);
        assert_eq!(fragments.checker_created_at, None);
    }

    #[test]
    fn should_skip_checker_created_at_when_matching_history_has_no_created_at() {
        let histories = vec![generation_history(
            "checker",
            Some(
                json!({
                    "log_type": "chapter_text_checker_v1",
                    "checker_result": {
                        "score": 88
                    }
                })
                .to_string(),
            ),
            None,
        )];

        let fragments = ChapterAnalysisCheckerFragments::from_histories(&histories);

        assert_eq!(fragments.checker_result, Some(json!({"score": 88})));
        assert_eq!(fragments.checker_created_at, None);
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
            Some(naive_datetime(2026, 5, 19, 8, 30, 15)),
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
            naive_datetime(2026, 5, 19, 8, 30, 15),
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
        assert_eq!(
            history.created_at.unwrap(),
            Some(naive_datetime(2026, 5, 19, 8, 30, 15))
        );
    }

    #[test]
    fn should_build_auto_revision_draft_apply_history_model() {
        let history = auto_revision_draft_apply_history_model(
            "history-new".to_string(),
            &chapter_model(),
            json!({"log_type": AUTO_REVISION_DRAFT_APPLY_MODEL}),
            naive_datetime(2026, 5, 19, 8, 30, 15),
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
        assert_eq!(
            history.created_at.unwrap(),
            Some(naive_datetime(2026, 5, 19, 8, 30, 15))
        );
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
}
