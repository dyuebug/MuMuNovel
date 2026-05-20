use chrono::NaiveDateTime;
use serde_json::Value;

use crate::models::generation_history;

pub struct ChapterAnalysisCheckerFragments {
    pub checker_result: Option<Value>,
    pub checker_created_at: Option<String>,
}

fn format_datetime(value: Option<NaiveDateTime>) -> Option<String> {
    value.map(|datetime| datetime.format("%Y-%m-%dT%H:%M:%S").to_string())
}

fn parse_checker_result(history: &generation_history::Model) -> Option<Value> {
    history.generated_content.as_ref().and_then(|content| {
        serde_json::from_str::<Value>(content)
            .ok()
            .and_then(|payload| {
                if payload.get("log_type").and_then(Value::as_str)
                    == Some("chapter_text_checker_v1")
                {
                    payload.get("checker_result").cloned()
                } else {
                    None
                }
            })
    })
}

pub fn build_chapter_analysis_checker_fragments(
    histories: &[generation_history::Model],
) -> ChapterAnalysisCheckerFragments {
    let checker_result = histories.iter().find_map(parse_checker_result);
    let checker_created_at = histories.iter().find_map(|history| {
        parse_checker_result(history)?;
        format_datetime(history.created_at)
    });

    ChapterAnalysisCheckerFragments {
        checker_result,
        checker_created_at,
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use serde_json::json;

    use crate::models::generation_history;

    use super::build_chapter_analysis_checker_fragments;

    fn history(id: &str, generated_content: Option<String>) -> generation_history::Model {
        generation_history::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_id: Some("chapter-1".to_string()),
            prompt: None,
            generated_content,
            model: None,
            tokens_used: None,
            generation_time: None,
            created_at: Some(
                NaiveDateTime::parse_from_str("2026-05-17T12:30:45", "%Y-%m-%dT%H:%M:%S")
                    .expect("test datetime should parse"),
            ),
        }
    }

    #[test]
    fn should_build_checker_fragments_from_first_matching_history() {
        let histories = vec![
            history(
                "unrelated",
                Some(json!({"log_type": "other", "checker_result": {"score": 1}}).to_string()),
            ),
            history(
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
            ),
        ];

        let fragments = build_chapter_analysis_checker_fragments(&histories);

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
            history("invalid-json", Some("{not-json".to_string())),
            history(
                "missing-result",
                Some(json!({"log_type": "chapter_text_checker_v1"}).to_string()),
            ),
        ];

        let fragments = build_chapter_analysis_checker_fragments(&histories);

        assert_eq!(fragments.checker_result, None);
        assert_eq!(fragments.checker_created_at, None);
    }

    #[test]
    fn should_skip_checker_created_at_when_matching_history_has_no_created_at() {
        let mut item = history(
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
        );
        item.created_at = None;

        let fragments = build_chapter_analysis_checker_fragments(&[item]);

        assert_eq!(fragments.checker_result, Some(json!({"score": 88})));
        assert_eq!(fragments.checker_created_at, None);
    }
}
