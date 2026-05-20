use serde_json::{json, Value};

use crate::models::chapter;

pub fn single_task_chapter_payload(chapter_model: &chapter::Model) -> Value {
    json!([{
        "id": chapter_model.id,
        "chapter_number": chapter_model.chapter_number,
        "title": chapter_model.title,
    }])
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use crate::models::chapter;

    use super::single_task_chapter_payload;

    fn chapter_model() -> chapter::Model {
        chapter::Model {
            id: "chapter-1".to_string(),
            project_id: "project-1".to_string(),
            chapter_number: 7,
            title: "第七章".to_string(),
            content: Some("正文不应进入批量任务章节快照".to_string()),
            summary: Some("摘要不应进入批量任务章节快照".to_string()),
            word_count: 12,
            status: "draft".to_string(),
            outline_id: Some("outline-1".to_string()),
            sub_index: 0,
            expansion_plan: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    #[test]
    fn should_build_single_task_chapter_payload() {
        let payload = single_task_chapter_payload(&chapter_model());

        assert_eq!(payload[0]["id"], "chapter-1");
        assert_eq!(payload[0]["chapter_number"], 7);
        assert_eq!(payload[0]["title"], "第七章");
        assert!(payload[0].get("content").is_none());
        assert!(payload[0].get("summary").is_none());
        assert!(payload[0].get("word_count").is_none());
        assert!(payload[0].get("status").is_none());
    }
}
