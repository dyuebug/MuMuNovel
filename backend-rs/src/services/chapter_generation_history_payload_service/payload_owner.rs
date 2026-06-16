use serde_json::{json, Value};

use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner;
use crate::services::chapter_single_generation_result_lifecycle_service::resolve_generated_history_attempt_state;

pub(crate) const CHAPTER_GENERATION_HISTORY_LOG_TYPE: &str = "chapter_generation_quality_v1";
pub(crate) const CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH: usize = 500;

pub(crate) fn build_chapter_generation_history_payload_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_history_payload_service",
        "scope": "generated_history_payload_projection_runtime_snapshot_contract_and_quality_metrics_normalization",
        "python_source_map": [
            "backend/app/services/chapter_generation/stream/candidate_service.py",
            "backend/app/services/chapter_generation/stream/finalize_service.py",
            "backend/app/services/story_repair_payload_service.py",
            "backend/app/models/generation_history.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_generation_history_payload_service.rs",
            "backend-rs/src/services/chapter_generation_history_payload_service/payload_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service.rs",
            "backend-rs/src/services/chapter_quality_metrics_query_service.rs",
            "backend-rs/src/services/chapter_single_generation_result_lifecycle_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_generated_chapter_history_payload_with_quality_metrics",
                "generated_history_story_runtime_contract",
                "generated_history_story_runtime_snapshot",
                "generated_history_runtime_snapshot_from_payload",
                "normalize_generated_history_quality_metrics"
            ],
            "history_payload_fields": [
                "log_type",
                "content",
                "preview",
                "quality_metrics",
                "generated_at",
                "content_applied",
                "attempt_state",
                "story_runtime_snapshot",
                "story_runtime_contract",
                "candidate_gateway"
            ],
            "preview_limit": CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH,
            "normalization_scope": "batch"
        },
        "active_consumers": [
            "chapter_generation_runtime_service",
            "chapter_quality_metrics_query_service",
            "chapter_single_generation_result_lifecycle_service"
        ],
        "validation_boundary": [
            "cargo test chapter_generation_runtime_service",
            "cargo test chapter_quality_metrics_query_service",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_generation_history_payload_and_query_shells_as_source_map_until_explicit_freeze_delete_round",
            "rollback_files": [
                "backend/app/services/chapter_generation/stream/candidate_service.py",
                "backend/app/services/chapter_generation/stream/finalize_service.py",
                "backend/app/models/generation_history.py"
            ]
        }
    })
}

fn build_generated_history_preview(content: &str) -> String {
    content
        .chars()
        .take(CHAPTER_GENERATION_HISTORY_PREVIEW_LENGTH)
        .collect()
}

pub(crate) fn build_generated_history_story_runtime_snapshot_from_contract(
    story_runtime_contract: &Value,
) -> Option<Value> {
    let guidance = story_runtime_contract
        .get("guidance")
        .and_then(Value::as_object);
    let blueprint = story_runtime_contract
        .get("blueprint")
        .and_then(Value::as_object);
    if guidance.is_none() && blueprint.is_none() {
        return None;
    }

    let mut snapshot = serde_json::Map::new();
    if let Some(guidance) = guidance {
        for field_name in [
            "creative_mode",
            "story_focus",
            "plot_stage",
            "story_creation_brief",
            "quality_preset",
            "quality_notes",
        ] {
            if let Some(value) = guidance
                .get(field_name)
                .cloned()
                .filter(|value| !value.is_null())
            {
                snapshot.insert(field_name.to_string(), value);
            }
        }
    }

    if let Some(blueprint) = blueprint {
        snapshot.insert(
            "story_long_term_goal".to_string(),
            blueprint
                .get("long_term_goal")
                .cloned()
                .unwrap_or_else(|| json!("")),
        );
        snapshot.insert(
            "chapter_count".to_string(),
            blueprint
                .get("chapter_count")
                .cloned()
                .unwrap_or(Value::Null),
        );
        snapshot.insert(
            "current_chapter_number".to_string(),
            blueprint
                .get("current_chapter_number")
                .cloned()
                .unwrap_or(Value::Null),
        );
        snapshot.insert(
            "target_word_count".to_string(),
            blueprint
                .get("target_word_count")
                .cloned()
                .unwrap_or(Value::Null),
        );
        snapshot.insert(
            "character_focus".to_string(),
            blueprint
                .get("character_focus_names")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "foreshadow_payoff_plan".to_string(),
            blueprint
                .get("foreshadow_payoff_plan")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "character_state_ledger".to_string(),
            blueprint
                .get("character_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "relationship_state_ledger".to_string(),
            blueprint
                .get("relationship_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "foreshadow_state_ledger".to_string(),
            blueprint
                .get("foreshadow_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "organization_state_ledger".to_string(),
            blueprint
                .get("organization_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
        snapshot.insert(
            "career_state_ledger".to_string(),
            blueprint
                .get("career_state_ledger")
                .cloned()
                .filter(Value::is_array)
                .unwrap_or_else(|| json!([])),
        );
    }

    (!snapshot.is_empty()).then_some(Value::Object(snapshot))
}

pub(crate) fn generated_history_story_runtime_contract(
    quality_metrics: Option<&Value>,
) -> Option<Value> {
    quality_metrics
        .and_then(|metrics| metrics.get("story_runtime_contract"))
        .filter(|payload| payload.is_object())
        .cloned()
}

pub(crate) fn generated_history_story_runtime_snapshot(
    quality_metrics: Option<&Value>,
    story_runtime_contract: Option<&Value>,
) -> Option<Value> {
    quality_metrics
        .and_then(|metrics| metrics.get("quality_runtime_context"))
        .and_then(|payload| payload.as_object().filter(|payload| !payload.is_empty()))
        .map(|payload| Value::Object(payload.clone()))
        .or_else(|| {
            story_runtime_contract
                .and_then(build_generated_history_story_runtime_snapshot_from_contract)
        })
}

pub(crate) fn generated_history_runtime_snapshot_from_payload(payload: &Value) -> Option<Value> {
    payload
        .get("story_runtime_snapshot")
        .and_then(|value| value.as_object().filter(|value| !value.is_empty()))
        .map(|value| Value::Object(value.clone()))
        .or_else(|| {
            payload
                .get("story_runtime_contract")
                .filter(|value| value.is_object())
                .and_then(build_generated_history_story_runtime_snapshot_from_contract)
        })
}

pub(crate) fn normalize_generated_history_quality_metrics(payload: &Value) -> Option<Value> {
    let mut metrics = payload.get("quality_metrics")?.as_object()?.clone();

    if metrics
        .get("story_runtime_contract")
        .is_none_or(|value| !value.is_object())
    {
        if let Some(story_runtime_contract) = payload
            .get("story_runtime_contract")
            .filter(|value| value.is_object())
            .cloned()
        {
            metrics.insert("story_runtime_contract".to_string(), story_runtime_contract);
        }
    }

    if let Some(runtime_snapshot) = generated_history_runtime_snapshot_from_payload(payload)
        .and_then(|value| value.as_object().cloned())
    {
        let merged_runtime_context = match metrics
            .get("quality_runtime_context")
            .and_then(Value::as_object)
            .filter(|value| !value.is_empty())
        {
            Some(existing_runtime_context) => {
                let mut merged_runtime_context = runtime_snapshot;
                for (key, value) in existing_runtime_context {
                    merged_runtime_context.insert(key.clone(), value.clone());
                }
                Value::Object(merged_runtime_context)
            }
            None => Value::Object(runtime_snapshot),
        };
        metrics.insert(
            "quality_runtime_context".to_string(),
            merged_runtime_context,
        );
    }

    story_repair_quality_context_owner::normalize_quality_metrics_history_item(
        &Value::Object(metrics),
        "batch",
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedHistoryPayloadView {
    pub(crate) quality_metrics: Option<Value>,
    pub(crate) story_runtime_contract: Option<Value>,
    pub(crate) story_runtime_snapshot: Option<Value>,
    pub(crate) candidate_gateway_metadata: Option<Value>,
}

pub(crate) fn generated_history_payload_view(
    quality_metrics: Option<&Value>,
    candidate_gateway_metadata: Option<&Value>,
) -> GeneratedHistoryPayloadView {
    let quality_metrics = quality_metrics.cloned();
    let story_runtime_contract = generated_history_story_runtime_contract(quality_metrics.as_ref());
    let story_runtime_snapshot = generated_history_story_runtime_snapshot(
        quality_metrics.as_ref(),
        story_runtime_contract.as_ref(),
    );

    GeneratedHistoryPayloadView {
        quality_metrics,
        story_runtime_contract,
        story_runtime_snapshot,
        candidate_gateway_metadata: candidate_gateway_metadata.cloned(),
    }
}

fn build_generated_chapter_quality_history_payload(
    content: &str,
    quality_metrics: Option<&Value>,
    candidate_gateway_metadata: Option<&Value>,
    content_applied: bool,
    attempt_state: Option<&str>,
    created_at: chrono::NaiveDateTime,
) -> Value {
    let history_view = generated_history_payload_view(quality_metrics, candidate_gateway_metadata);

    let mut payload = serde_json::Map::from_iter([
        (
            "log_type".to_string(),
            json!(CHAPTER_GENERATION_HISTORY_LOG_TYPE),
        ),
        ("content".to_string(), json!(content)),
        (
            "preview".to_string(),
            json!(build_generated_history_preview(content)),
        ),
        (
            "quality_metrics".to_string(),
            history_view.quality_metrics.clone().unwrap_or(Value::Null),
        ),
        (
            "generated_at".to_string(),
            json!(created_at.format("%Y-%m-%dT%H:%M:%S").to_string()),
        ),
        ("content_applied".to_string(), json!(content_applied)),
        (
            "attempt_state".to_string(),
            json!(resolve_generated_history_attempt_state(
                content_applied,
                attempt_state,
            )),
        ),
    ]);

    if let Some(story_runtime_snapshot) = history_view.story_runtime_snapshot {
        payload.insert("story_runtime_snapshot".to_string(), story_runtime_snapshot);
    }
    if let Some(story_runtime_contract) = history_view.story_runtime_contract {
        payload.insert("story_runtime_contract".to_string(), story_runtime_contract);
    }
    if let Some(candidate_gateway_metadata) = history_view.candidate_gateway_metadata {
        payload.insert("candidate_gateway".to_string(), candidate_gateway_metadata);
    }

    Value::Object(payload)
}

pub(crate) fn build_generated_chapter_history_payload_with_quality_metrics(
    content: &str,
    quality_metrics: Option<&Value>,
    candidate_gateway_metadata: Option<&Value>,
    content_applied: bool,
    attempt_state: Option<&str>,
    created_at: chrono::NaiveDateTime,
) -> Value {
    build_generated_chapter_quality_history_payload(
        content,
        quality_metrics,
        candidate_gateway_metadata,
        content_applied,
        attempt_state,
        created_at,
    )
}
