use serde::Serialize;
use serde_json::{json, Value};

use crate::models::chapter;

fn serialize_value<T: Serialize + ?Sized>(value: &T, fallback: Value) -> Value {
    serde_json::to_value(value).unwrap_or(fallback)
}

pub fn compatible_chapter_list_payload(chapters: &[chapter::Model]) -> Value {
    let items = serialize_value(chapters, json!([]));
    json!({
        "success": true,
        "data": items.clone(),
        "items": items,
        "total": chapters.len()
    })
}

pub fn project_path_chapter_list_payload(chapters: &[chapter::Model]) -> Value {
    let items = serialize_value(chapters, json!([]));
    json!({
        "items": items,
        "total": chapters.len()
    })
}

pub fn compatible_chapter_payload(chapter: chapter::Model) -> Value {
    let chapter_value = serialize_value(&chapter, json!({}));
    match chapter_value {
        Value::Object(mut map) => {
            let data = Value::Object(map.clone());
            map.insert("success".to_string(), json!(true));
            map.insert("data".to_string(), data);
            Value::Object(map)
        }
        _ => json!({
            "success": true,
            "data": chapter
        }),
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use crate::models::chapter;

    use super::{
        compatible_chapter_list_payload, compatible_chapter_payload,
        project_path_chapter_list_payload,
    };

    fn chapter_model(id: &str, number: i32) -> chapter::Model {
        chapter::Model {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            chapter_number: number,
            title: format!("第{}章", number),
            content: Some("正文".to_string()),
            summary: None,
            word_count: 2,
            status: "draft".to_string(),
            outline_id: None,
            sub_index: 0,
            expansion_plan: None,
            created_at: NaiveDateTime::default(),
            updated_at: Some(NaiveDateTime::default()),
        }
    }

    #[test]
    fn should_build_compatible_chapter_list_payload() {
        let chapters = vec![chapter_model("chapter-1", 1), chapter_model("chapter-2", 2)];

        let payload = compatible_chapter_list_payload(&chapters);

        assert_eq!(payload["success"], true);
        assert_eq!(payload["total"], 2);
        assert_eq!(payload["data"][0]["id"], "chapter-1");
        assert_eq!(payload["items"][1]["id"], "chapter-2");
        assert_eq!(payload["data"], payload["items"]);
    }

    #[test]
    fn should_build_project_path_chapter_list_payload() {
        let chapters = vec![chapter_model("chapter-1", 1)];

        let payload = project_path_chapter_list_payload(&chapters);

        assert_eq!(payload["total"], 1);
        assert_eq!(payload["items"][0]["id"], "chapter-1");
        assert!(payload.get("success").is_none());
        assert!(payload.get("data").is_none());
    }

    #[test]
    fn should_build_compatible_chapter_payload() {
        let payload = compatible_chapter_payload(chapter_model("chapter-1", 1));

        assert_eq!(payload["success"], true);
        assert_eq!(payload["id"], "chapter-1");
        assert_eq!(payload["title"], "第1章");
        assert_eq!(payload["data"]["id"], "chapter-1");
        assert_eq!(payload["data"]["title"], "第1章");
        assert!(payload["data"].get("success").is_none());
    }
}
