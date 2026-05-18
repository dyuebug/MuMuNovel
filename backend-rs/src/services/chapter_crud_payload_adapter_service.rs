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
