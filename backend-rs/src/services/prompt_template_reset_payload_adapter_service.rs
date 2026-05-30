use serde_json::{json, Value};

pub fn build_prompt_template_sync_to_default_payload(
    template_key: &str,
    deleted: bool,
    status: Value,
) -> Value {
    let (action, message) = if deleted {
        ("reset_to_system_default", "已同步到系统默认模板")
    } else {
        ("already_system_default", "当前已是系统默认模板")
    };

    json!({
        "template_key": template_key,
        "action": action,
        "message": message,
        "status": status,
    })
}

pub fn build_prompt_template_reset_payload(template_key: &str, deleted: bool) -> Value {
    json!({
        "message": if deleted { "已重置为系统默认" } else { "已是系统默认状态" },
        "template_key": template_key,
    })
}

pub fn build_prompt_template_delete_payload(template_key: &str) -> Value {
    json!({
        "message": "模板已删除",
        "template_key": template_key,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_prompt_template_delete_payload, build_prompt_template_reset_payload,
        build_prompt_template_sync_to_default_payload,
    };

    #[test]
    fn build_prompt_template_sync_to_default_payload_keeps_deleted_contract() {
        let payload = build_prompt_template_sync_to_default_payload(
            "chapter_generate",
            true,
            json!({"sync_state": "system_default"}),
        );

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["action"], "reset_to_system_default");
        assert_eq!(payload["message"], "已同步到系统默认模板");
        assert_eq!(payload["status"]["sync_state"], "system_default");
    }

    #[test]
    fn build_prompt_template_sync_to_default_payload_keeps_existing_default_contract() {
        let payload = build_prompt_template_sync_to_default_payload(
            "chapter_generate",
            false,
            json!({"sync_state": "system_default"}),
        );

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["action"], "already_system_default");
        assert_eq!(payload["message"], "当前已是系统默认模板");
        assert_eq!(payload["status"]["sync_state"], "system_default");
    }

    #[test]
    fn build_prompt_template_reset_payload_keeps_reset_message_variants() {
        let deleted_payload = build_prompt_template_reset_payload("chapter_generate", true);
        let unchanged_payload = build_prompt_template_reset_payload("chapter_generate", false);

        assert_eq!(deleted_payload["template_key"], "chapter_generate");
        assert_eq!(deleted_payload["message"], "已重置为系统默认");
        assert_eq!(unchanged_payload["template_key"], "chapter_generate");
        assert_eq!(unchanged_payload["message"], "已是系统默认状态");
    }

    #[test]
    fn build_prompt_template_delete_payload_keeps_delete_success_shape() {
        let payload = build_prompt_template_delete_payload("chapter_generate");

        assert_eq!(payload["template_key"], "chapter_generate");
        assert_eq!(payload["message"], "模板已删除");
    }
}
