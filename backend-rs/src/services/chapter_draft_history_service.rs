use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::Value;

use crate::models::generation_history;
use crate::services::chapter_draft_source_service::format_datetime;

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

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime};
    use serde_json::json;

    use crate::models::generation_history;

    use super::{
        parse_checker_result_from_history, parse_reviser_result_from_history,
        ChapterAnalysisCheckerFragments,
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
}
