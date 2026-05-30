use serde_json::{json, Value};

pub(crate) fn build_stored_api_key_payload(api_key: Option<&str>) -> Value {
    let trimmed = api_key.unwrap_or_default().trim();
    json!({
        "api_key": trimmed,
        "has_api_key": !trimmed.is_empty(),
    })
}

#[cfg(test)]
mod tests {
    use super::build_stored_api_key_payload;

    #[test]
    fn build_stored_api_key_payload_trims_and_marks_present_key() {
        let payload = build_stored_api_key_payload(Some("  secret-key  "));

        assert_eq!(payload["api_key"], "secret-key");
        assert_eq!(payload["has_api_key"], true);
    }

    #[test]
    fn build_stored_api_key_payload_handles_empty_and_missing_key() {
        let empty = build_stored_api_key_payload(Some("   "));
        assert_eq!(empty["api_key"], "");
        assert_eq!(empty["has_api_key"], false);

        let missing = build_stored_api_key_payload(None);
        assert_eq!(missing["api_key"], "");
        assert_eq!(missing["has_api_key"], false);
    }
}
