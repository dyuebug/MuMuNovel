use std::collections::HashMap;

use serde_json::{json, Value};

pub fn build_prompt_template_preview_success_payload(
    rendered: String,
    parameters: &HashMap<String, String>,
) -> Value {
    json!({
        "success": true,
        "rendered_content": rendered,
        "parameters_used": parameters.keys().collect::<Vec<_>>(),
    })
}

pub fn build_prompt_template_preview_error_payload(error: &str) -> Value {
    json!({
        "success": false,
        "error": format!("渲染失败: {error}"),
        "rendered_content": Value::Null,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{
        build_prompt_template_preview_error_payload, build_prompt_template_preview_success_payload,
    };

    #[test]
    fn build_prompt_template_preview_success_payload_keeps_rendered_content_and_parameters() {
        let mut parameters = HashMap::new();
        parameters.insert("tone".to_string(), "warm".to_string());

        let payload = build_prompt_template_preview_success_payload(
            "rendered result".to_string(),
            &parameters,
        );

        assert_eq!(payload["success"], true);
        assert_eq!(payload["rendered_content"], "rendered result");
        let parameters_used = payload["parameters_used"]
            .as_array()
            .expect("parameters_used should be an array");
        assert_eq!(parameters_used.len(), 1);
        assert_eq!(parameters_used[0], "tone");
    }

    #[test]
    fn build_prompt_template_preview_error_payload_keeps_compat_error_shape() {
        let payload = build_prompt_template_preview_error_payload("missing variable");

        assert_eq!(payload["success"], false);
        assert_eq!(payload["error"], "渲染失败: missing variable");
        assert!(payload["rendered_content"].is_null());
    }
}
