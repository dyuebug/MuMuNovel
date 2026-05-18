use serde_json::{json, Value};

pub use crate::services::chapter_narrative_cleaner_service::{
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
