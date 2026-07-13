use chrono::{DateTime, Utc};
use serde_json::Value;

/// 使用当前时间刷新 checkpoint 的兼容入口；需要共享事实时间的 owner 应调用 `touch_checkpoint_at`。
#[allow(dead_code)]
pub fn touch_checkpoint(
    existing: Option<&Value>,
    event: &str,
    progress: Option<i32>,
    message: Option<&str>,
    extra: Option<&Value>,
) -> Value {
    touch_checkpoint_at(existing, event, progress, message, extra, Utc::now())
}

pub fn touch_checkpoint_at(
    existing: Option<&Value>,
    event: &str,
    progress: Option<i32>,
    message: Option<&str>,
    extra: Option<&Value>,
    updated_at: DateTime<Utc>,
) -> Value {
    let mut cp = existing.cloned().unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = cp.as_object_mut() {
        obj.insert("event".into(), serde_json::Value::String(event.into()));
        obj.insert(
            "updated_at".into(),
            serde_json::Value::String(updated_at.to_rfc3339()),
        );

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
