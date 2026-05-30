use serde::Serialize;
use serde_json::{json, to_value, Value};

pub fn build_prompt_template_list_payload<T: Serialize>(
    templates: T,
    total: usize,
    categories: Vec<String>,
) -> Value {
    json!({
        "templates": to_value(templates).unwrap_or_else(|_| json!([])),
        "total": total,
        "categories": categories,
    })
}

pub fn build_prompt_template_system_defaults_payload<T: Serialize>(
    templates: T,
    total: usize,
) -> Value {
    json!({
        "templates": to_value(templates).unwrap_or_else(|_| json!([])),
        "total": total,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_prompt_template_list_payload, build_prompt_template_system_defaults_payload,
    };

    #[test]
    fn build_prompt_template_list_payload_keeps_templates_total_and_categories() {
        let payload = build_prompt_template_list_payload(
            vec![json!({"template_key": "alpha"})],
            1,
            vec!["分类A".to_string(), "分类B".to_string()],
        );

        assert_eq!(payload["total"], 1);
        assert_eq!(payload["templates"][0]["template_key"], "alpha");
        assert_eq!(payload["categories"][0], "分类A");
        assert_eq!(payload["categories"][1], "分类B");
    }

    #[test]
    fn build_prompt_template_system_defaults_payload_keeps_templates_and_total() {
        let payload = build_prompt_template_system_defaults_payload(
            vec![
                json!({"template_key": "alpha"}),
                json!({"template_key": "beta"}),
            ],
            2,
        );

        assert_eq!(payload["total"], 2);
        assert_eq!(payload["templates"][0]["template_key"], "alpha");
        assert_eq!(payload["templates"][1]["template_key"], "beta");
    }
}
