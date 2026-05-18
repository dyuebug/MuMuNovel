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
