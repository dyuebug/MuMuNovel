use serde_json::{json, Value};

pub(crate) fn build_available_models_payload(provider: &str, models: Vec<Value>) -> Value {
    let count = models.len();
    json!({
        "provider": provider,
        "models": models,
        "count": count,
    })
}

pub(crate) fn build_available_models_fallback_payload(
    provider: &str,
    fallback_models: Vec<Value>,
    error: &str,
) -> Value {
    let count = fallback_models.len();
    json!({
        "provider": provider,
        "models": fallback_models,
        "count": count,
        "message": format!("Model list fallback applied: {}", error),
        "fallback_applied": true,
    })
}

pub(crate) fn normalize_fetch_models_payload(models: Vec<Value>) -> Vec<Value> {
    models
        .into_iter()
        .filter_map(|item| {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                let trimmed = id.trim();
                if !trimmed.is_empty() {
                    return Some(json!({
                        "id": trimmed,
                        "owned_by": item
                            .get("owned_by")
                            .and_then(Value::as_str)
                            .or_else(|| item.get("description").and_then(Value::as_str))
                    }));
                }
            }

            let value = item
                .get("value")
                .and_then(Value::as_str)
                .or_else(|| item.get("name").and_then(Value::as_str))
                .or_else(|| item.get("label").and_then(Value::as_str))
                .map(str::trim)
                .filter(|text| !text.is_empty())?;

            Some(json!({
                "id": value,
                "owned_by": item
                    .get("owned_by")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("description").and_then(Value::as_str))
                    .or_else(|| item.get("label").and_then(Value::as_str))
            }))
        })
        .collect()
}

pub(crate) fn build_fetch_models_success_payload(models: Vec<Value>) -> Value {
    let model_count = models.len();
    json!({
        "success": true,
        "models": normalize_fetch_models_payload(models),
        "message": format!("Fetched {} models", model_count)
    })
}

pub(crate) fn build_fetch_models_fallback_payload(
    fallback_models: Vec<Value>,
    error: &str,
) -> Value {
    json!({
        "success": true,
        "models": fallback_models,
        "message": format!("Model list fallback applied: {}", error)
    })
}

pub(crate) fn build_fetch_models_failure_payload(error: &str, error_type: &str) -> Value {
    json!({
        "success": false,
        "models": [],
        "message": "Failed to fetch models",
        "error": error,
        "error_type": error_type
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_available_models_fallback_payload, build_available_models_payload,
        build_fetch_models_failure_payload, build_fetch_models_fallback_payload,
        build_fetch_models_success_payload, normalize_fetch_models_payload,
    };

    #[test]
    fn build_available_models_payload_keeps_provider_models_and_count() {
        let payload = build_available_models_payload(
            "openai",
            vec![json!({"value": "gpt-4o", "label": "gpt-4o", "description": "OpenAI-compatible"})],
        );

        assert_eq!(payload["provider"], "openai");
        assert_eq!(payload["count"], 1);
        assert_eq!(payload["models"][0]["value"], "gpt-4o");
    }

    #[test]
    fn build_available_models_fallback_payload_keeps_existing_shell() {
        let payload = build_available_models_fallback_payload(
            "gemini",
            vec![json!({"value": "gemini-2.5-pro"})],
            "timeout",
        );

        assert_eq!(payload["provider"], "gemini");
        assert_eq!(payload["count"], 1);
        assert_eq!(payload["fallback_applied"], true);
        assert_eq!(payload["message"], "Model list fallback applied: timeout");
    }

    #[test]
    fn normalize_fetch_models_payload_accepts_id_and_value_shapes() {
        let payload = normalize_fetch_models_payload(vec![
            json!({"id": "gpt-4o", "owned_by": "openai"}),
            json!({"value": "claude-3-5-sonnet-latest", "description": "Anthropic"}),
        ]);

        assert_eq!(payload.len(), 2);
        assert_eq!(payload[0]["id"], "gpt-4o");
        assert_eq!(payload[0]["owned_by"], "openai");
        assert_eq!(payload[1]["id"], "claude-3-5-sonnet-latest");
        assert_eq!(payload[1]["owned_by"], "Anthropic");
    }

    #[test]
    fn build_fetch_models_payloads_keep_success_and_failure_contracts() {
        let success = build_fetch_models_success_payload(vec![json!({"id": "gpt-4o"})]);
        assert_eq!(success["success"], true);
        assert_eq!(success["models"][0]["id"], "gpt-4o");

        let fallback =
            build_fetch_models_fallback_payload(vec![json!({"id": "gpt-4o-mini"})], "network");
        assert_eq!(fallback["success"], true);
        assert_eq!(fallback["message"], "Model list fallback applied: network");

        let failure = build_fetch_models_failure_payload("failed", "NetworkError");
        assert_eq!(failure["success"], false);
        assert_eq!(failure["models"], json!([]));
        assert_eq!(failure["error_type"], "NetworkError");
    }
}
