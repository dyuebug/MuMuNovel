use serde_json::{Map, Value};

const MAX_FAILED_METRICS: usize = 8;
const MAX_GUIDANCE_ITEMS: usize = 8;
const MAX_TEXT_CHARS: usize = 160;

pub(crate) fn build_safe_quality_diagnostic(
    quality_metrics: Option<&Value>,
    quality_gate_action: Option<&str>,
    result_digest: Option<&str>,
) -> Value {
    let mut diagnostic = Map::new();
    if let Some(metrics) = quality_metrics.and_then(Value::as_object) {
        copy_number(metrics, "overall_score", &mut diagnostic);
        if let Some(gate) = metrics.get("quality_gate").and_then(Value::as_object) {
            copy_bounded_string(gate, "decision", "quality_decision", &mut diagnostic);
            let failed_metrics = gate
                .get("failed_metrics")
                .and_then(Value::as_array)
                .map(|items| project_failed_metrics(items))
                .unwrap_or_default();
            if !failed_metrics.is_empty() {
                diagnostic.insert("failed_metrics".to_string(), Value::Array(failed_metrics));
            }
        }
        if let Some(guidance) = metrics.get("repair_guidance").and_then(Value::as_object) {
            copy_bounded_string_list(guidance, "repair_targets", &mut diagnostic);
            copy_bounded_string_list(guidance, "focus_areas", &mut diagnostic);
        }
    }
    if let Some(action) = bounded_text(quality_gate_action.unwrap_or_default()) {
        diagnostic.insert("quality_gate_action".to_string(), Value::String(action));
    }
    if let Some(digest) = bounded_text(result_digest.unwrap_or_default()) {
        diagnostic.insert("result_digest".to_string(), Value::String(digest));
    }
    Value::Object(diagnostic)
}

fn project_failed_metrics(items: &[Value]) -> Vec<Value> {
    items
        .iter()
        .filter_map(Value::as_object)
        .take(MAX_FAILED_METRICS)
        .filter_map(|metric| {
            let mut projected = Map::new();
            for key in ["key", "label"] {
                copy_bounded_string(metric, key, key, &mut projected);
            }
            for key in ["value", "threshold", "gap"] {
                copy_number(metric, key, &mut projected);
            }
            (!projected.is_empty()).then_some(Value::Object(projected))
        })
        .collect()
}

fn copy_number(source: &Map<String, Value>, key: &str, target: &mut Map<String, Value>) {
    if let Some(value) = source
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
    {
        target.insert(key.to_string(), Value::from(value));
    }
}

fn copy_bounded_string(
    source: &Map<String, Value>,
    source_key: &str,
    target_key: &str,
    target: &mut Map<String, Value>,
) {
    if let Some(value) = source
        .get(source_key)
        .and_then(Value::as_str)
        .and_then(bounded_text)
    {
        target.insert(target_key.to_string(), Value::String(value));
    }
}

fn copy_bounded_string_list(
    source: &Map<String, Value>,
    key: &str,
    target: &mut Map<String, Value>,
) {
    let items = source
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(bounded_text)
        .take(MAX_GUIDANCE_ITEMS)
        .map(Value::String)
        .collect::<Vec<_>>();
    if !items.is_empty() {
        target.insert(key.to_string(), Value::Array(items));
    }
}

fn bounded_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.chars().take(MAX_TEXT_CHARS).collect())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::build_safe_quality_diagnostic;

    #[test]
    fn safe_quality_diagnostic_uses_allowlist_and_bounds_nested_content() {
        let diagnostic = build_safe_quality_diagnostic(
            Some(&json!({
                "overall_score": 66.1,
                "quality_runtime_context": {
                    "story_packet": "完整 Prompt",
                    "previous_chapter_content": "上一章正文"
                },
                "quality_gate": {
                    "decision": "auto_repair",
                    "failed_metrics": [{
                        "key": "pacing",
                        "label": "节奏",
                        "value": 0.4,
                        "threshold": 0.7,
                        "gap": 0.3,
                        "raw_response": "上游响应"
                    }]
                },
                "repair_guidance": {
                    "repair_targets": ["压缩说明段"],
                    "focus_areas": ["pacing"],
                    "reasoning": "模型推理"
                }
            })),
            Some("auto_repair"),
            Some("sha256:abc"),
        );
        let serialized = serde_json::to_string(&diagnostic).expect("serialize");

        assert_eq!(diagnostic["overall_score"], json!(66.1));
        assert_eq!(diagnostic["quality_decision"], json!("auto_repair"));
        assert_eq!(diagnostic["failed_metrics"][0]["key"], json!("pacing"));
        assert_eq!(diagnostic["repair_targets"], json!(["压缩说明段"]));
        for forbidden in ["完整 Prompt", "上一章正文", "上游响应", "模型推理"] {
            assert!(
                !serialized.contains(forbidden),
                "leaked {forbidden}: {serialized}"
            );
        }
    }
}
