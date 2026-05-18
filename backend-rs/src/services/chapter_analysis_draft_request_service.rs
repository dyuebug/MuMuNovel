use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DraftLookupRequest {
    pub history_id: Option<String>,
    pub attempt_id: Option<String>,
}

impl DraftLookupRequest {
    pub fn history_id(&self) -> Option<&str> {
        self.history_id.as_deref()
    }

    pub fn attempt_id(&self) -> Option<&str> {
        self.attempt_id.as_deref()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DraftApplyRequest {
    pub history_id: Option<String>,
    pub attempt_id: Option<String>,
    pub allow_stale: bool,
}

impl DraftApplyRequest {
    pub fn history_id(&self) -> Option<&str> {
        self.history_id.as_deref()
    }

    pub fn attempt_id(&self) -> Option<&str> {
        self.attempt_id.as_deref()
    }
}

pub fn parse_auto_revision_draft_lookup_request(
    query: &HashMap<String, String>,
) -> DraftLookupRequest {
    DraftLookupRequest {
        history_id: query.get("history_id").cloned(),
        attempt_id: None,
    }
}

pub fn parse_auto_revision_draft_apply_request(body: &Value) -> DraftApplyRequest {
    DraftApplyRequest {
        history_id: parse_optional_body_string(body, "history_id"),
        attempt_id: None,
        allow_stale: parse_allow_stale(body),
    }
}

pub fn parse_candidate_draft_lookup_request(
    query: &HashMap<String, String>,
) -> DraftLookupRequest {
    DraftLookupRequest {
        history_id: None,
        attempt_id: query.get("attempt_id").cloned(),
    }
}

pub fn parse_candidate_draft_apply_request(body: &Value) -> DraftApplyRequest {
    DraftApplyRequest {
        history_id: None,
        attempt_id: parse_optional_body_string(body, "attempt_id"),
        allow_stale: parse_allow_stale(body),
    }
}

fn parse_optional_body_string(body: &Value, key: &str) -> Option<String> {
    body.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn parse_allow_stale(body: &Value) -> bool {
    body.get("allow_stale")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
