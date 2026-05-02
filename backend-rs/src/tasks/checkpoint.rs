use chrono::Utc;
use serde_json::Value;

pub fn touch_checkpoint(
    existing: Option<&Value>,
    event: &str,
    progress: Option<i32>,
    message: Option<&str>,
    extra: Option<&Value>,
) -> Value {
    let mut cp = existing.cloned().unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = cp.as_object_mut() {
        obj.insert("event".into(), serde_json::Value::String(event.into()));
        obj.insert("updated_at".into(), serde_json::Value::String(Utc::now().to_rfc3339()));

        if let Some(p) = progress {
            obj.insert("progress".into(), serde_json::Value::Number(p.into()));
        }
        if let Some(m) = message {
            obj.insert("message".into(), serde_json::Value::String(m.into()));
        }
        if let Some(ext) = extra {
            if let Some(ext_obj) = ext.as_object() {
                for (k, v) in ext_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
    }

    cp
}
