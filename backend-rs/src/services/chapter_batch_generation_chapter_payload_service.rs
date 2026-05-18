use serde_json::{json, Value};

use crate::models::chapter;

pub fn single_task_chapter_payload(chapter_model: &chapter::Model) -> Value {
    json!([{
        "id": chapter_model.id,
        "chapter_number": chapter_model.chapter_number,
        "title": chapter_model.title,
    }])
}
