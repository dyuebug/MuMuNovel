use serde_json::{json, Value};

use crate::services::chapter_narrative_cleaner_service::{
    contains_chapter_workflow_meta_text, sanitize_generated_narrative_text,
};

pub enum FinalizePartialRegenerationError {
    EmptyContent,
    WorkflowMetaText,
}

pub struct FinalizedPartialRegenerationResult {
    pub cleaned_text: String,
    pub payload: Value,
}

pub struct FinalizedChapterRegenerationResult {
    pub cleaned_text: String,
    pub payload: Value,
}

pub fn normalize_partial_regeneration_output(text: &str) -> String {
    let mut cleaned = text.replace("\r\n", "\n").trim().to_string();
    let prefixes = [
        "重写后：",
        "重写后:",
        "改写后：",
        "改写后:",
        "以下是重写后的内容：",
        "以下是重写后的内容:",
        "重写内容：",
        "重写内容:",
    ];
    for prefix in prefixes {
        if cleaned.starts_with(prefix) {
            cleaned = cleaned[prefix.len()..].trim().to_string();
            break;
        }
    }

    if (cleaned.starts_with('"') && cleaned.ends_with('"'))
        || (cleaned.starts_with('\'') && cleaned.ends_with('\''))
    {
        let mut chars = cleaned.chars();
        let _ = chars.next();
        let _ = chars.next_back();
        cleaned = chars.collect::<String>().trim().to_string();
    }
    if (cleaned.starts_with('「') && cleaned.ends_with('」'))
        || (cleaned.starts_with('『') && cleaned.ends_with('』'))
    {
        let mut chars = cleaned.chars();
        let _ = chars.next();
        let _ = chars.next_back();
        cleaned = chars.collect::<String>().trim().to_string();
    }

    cleaned.trim().to_string()
}

pub fn finalize_partial_regeneration_result(
    generated_text: &str,
    original_word_count: usize,
    start_position: usize,
    end_position: usize,
) -> Result<FinalizedPartialRegenerationResult, FinalizePartialRegenerationError> {
    let normalized = normalize_partial_regeneration_output(generated_text);
    let (cleaned_text, _) = sanitize_generated_narrative_text(&normalized);
    if cleaned_text.trim().is_empty() {
        return Err(FinalizePartialRegenerationError::EmptyContent);
    }
    if contains_chapter_workflow_meta_text(&cleaned_text) {
        return Err(FinalizePartialRegenerationError::WorkflowMetaText);
    }

    let payload = json!({
        "new_text": cleaned_text,
        "word_count": cleaned_text.chars().count(),
        "original_word_count": original_word_count,
        "start_position": start_position,
        "end_position": end_position,
    });

    Ok(FinalizedPartialRegenerationResult {
        cleaned_text,
        payload,
    })
}

pub fn finalize_chapter_regeneration_result(
    generated_text: &str,
    chapter_id: &str,
) -> Result<FinalizedChapterRegenerationResult, FinalizePartialRegenerationError> {
    let (cleaned_text, _) = sanitize_generated_narrative_text(generated_text);
    if cleaned_text.trim().is_empty() {
        return Err(FinalizePartialRegenerationError::EmptyContent);
    }
    if contains_chapter_workflow_meta_text(&cleaned_text) {
        return Err(FinalizePartialRegenerationError::WorkflowMetaText);
    }

    let payload = json!({
        "content": cleaned_text,
        "word_count": cleaned_text.chars().count(),
        "generation_task_id": chapter_id,
        "analysis_task_id": Value::Null,
    });

    Ok(FinalizedChapterRegenerationResult {
        cleaned_text,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        finalize_chapter_regeneration_result, finalize_partial_regeneration_result,
        normalize_partial_regeneration_output, FinalizePartialRegenerationError,
    };

    #[test]
    fn should_normalize_partial_regeneration_output_prefixes_and_quotes() {
        assert_eq!(
            normalize_partial_regeneration_output("\r\n重写后： \"新的正文\" \r\n"),
            "新的正文"
        );
        assert_eq!(
            normalize_partial_regeneration_output("以下是重写后的内容：『新的正文』"),
            "新的正文"
        );
        assert_eq!(
            normalize_partial_regeneration_output("改写后:'新的正文'"),
            "新的正文"
        );
    }

    #[test]
    fn should_finalize_partial_regeneration_result_payload() {
        let result = finalize_partial_regeneration_result("重写后：新的正文", 12, 3, 8);
        let result = match result {
            Ok(result) => result,
            Err(_) => panic!("partial regeneration result should be valid"),
        };

        assert_eq!(result.cleaned_text, "新的正文");
        assert_eq!(result.payload["new_text"], "新的正文");
        assert_eq!(result.payload["word_count"], 4);
        assert_eq!(result.payload["original_word_count"], 12);
        assert_eq!(result.payload["start_position"], 3);
        assert_eq!(result.payload["end_position"], 8);
    }

    #[test]
    fn should_finalize_chapter_regeneration_result_payload() {
        let result = finalize_chapter_regeneration_result("新的章节正文", "chapter-1");
        let result = match result {
            Ok(result) => result,
            Err(_) => panic!("chapter regeneration result should be valid"),
        };

        assert_eq!(result.cleaned_text, "新的章节正文");
        assert_eq!(result.payload["content"], "新的章节正文");
        assert_eq!(result.payload["word_count"], 6);
        assert_eq!(result.payload["generation_task_id"], "chapter-1");
        assert!(result.payload["analysis_task_id"].is_null());
    }

    #[test]
    fn should_reject_meta_only_regeneration_result_as_empty() {
        let result = finalize_partial_regeneration_result(
            "```markdown\n作为AI：我将开始执行\n流程说明",
            12,
            3,
            8,
        );
        let error = match result {
            Ok(_) => panic!("meta-only partial regeneration result should be rejected"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            FinalizePartialRegenerationError::EmptyContent
        ));
    }

    #[test]
    fn should_reject_meta_only_chapter_regeneration_result_as_empty() {
        let result = finalize_chapter_regeneration_result(
            "```markdown\n作为AI：我将开始执行\n流程说明",
            "chapter-1",
        );
        let error = match result {
            Ok(_) => panic!("meta-only chapter regeneration result should be rejected"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            FinalizePartialRegenerationError::EmptyContent
        ));
    }

    #[test]
    fn should_preserve_partial_and_chapter_payload_shape_differences() {
        let partial = match finalize_partial_regeneration_result("重写后：新的片段", 20, 5, 11)
        {
            Ok(result) => result,
            Err(_) => panic!("partial regeneration result should be valid"),
        };
        let chapter = match finalize_chapter_regeneration_result("新的章节正文", "chapter-2")
        {
            Ok(result) => result,
            Err(_) => panic!("chapter regeneration result should be valid"),
        };

        assert_eq!(partial.payload["new_text"], "新的片段");
        assert!(partial.payload.get("content").is_none());
        assert_eq!(partial.payload["original_word_count"], 20);
        assert_eq!(partial.payload["start_position"], 5);
        assert_eq!(partial.payload["end_position"], 11);

        assert_eq!(chapter.payload["content"], "新的章节正文");
        assert!(chapter.payload.get("new_text").is_none());
        assert_eq!(chapter.payload["generation_task_id"], "chapter-2");
        assert!(chapter.payload["analysis_task_id"].is_null());
    }
}
