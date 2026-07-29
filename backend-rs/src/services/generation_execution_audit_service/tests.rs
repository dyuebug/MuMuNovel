use serde_json::{json, Value};

use crate::ai::execution_trace::{
    AIExecutionFallbackKind, AIExecutionFallbackSummaryV1, AIExecutionOutcome, AIExecutionTraceV1,
    EndpointExecutionSummaryV1, AI_EXECUTION_TRACE_SCHEMA_VERSION,
};
use crate::services::generation_contract_service::GENERATION_CONTRACT_SCHEMA_VERSION;
use crate::services::role_model_policy_service::{
    GenerationRole, ModelSelectionSource, ResolvedRoleModelPolicyV1,
    ROLE_MODEL_POLICY_SCHEMA_VERSION,
};

use super::{
    build_generation_execution_audit, merge_generation_execution_audit,
    read_generation_execution_audit, GenerationExecutionAuditError,
    GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION,
};

fn resolved_policy() -> ResolvedRoleModelPolicyV1 {
    ResolvedRoleModelPolicyV1 {
        role: GenerationRole::Writer,
        policy_schema_version: ROLE_MODEL_POLICY_SCHEMA_VERSION.to_string(),
        policy_digest: "policy-digest".to_string(),
        requested_provider: Some("openai".to_string()),
        requested_model: Some("requested-model".to_string()),
        resolved_provider: "openai".to_string(),
        resolved_model: "resolved-model".to_string(),
        provider_source: ModelSelectionSource::GlobalSettings,
        model_source: ModelSelectionSource::RoleOverride,
    }
}

fn execution_trace() -> AIExecutionTraceV1 {
    AIExecutionTraceV1 {
        schema_version: AI_EXECUTION_TRACE_SCHEMA_VERSION.to_string(),
        requested_provider: "openai".to_string(),
        requested_model: "resolved-model".to_string(),
        actual_provider: "openai".to_string(),
        actual_model: "fallback-model".to_string(),
        outcome: AIExecutionOutcome::Succeeded,
        fallbacks: vec![AIExecutionFallbackSummaryV1 {
            kind: AIExecutionFallbackKind::ModelFallback,
            reason: "model_not_found".to_string(),
        }],
        endpoint_summary: Some(EndpointExecutionSummaryV1 {
            endpoint_role: "backup".to_string(),
            endpoint_index: 1,
            total_attempts: 2,
            failover_count: 1,
            backup_endpoint_used: true,
        }),
    }
}

#[test]
fn builds_allowlisted_generation_execution_audit() {
    let audit = build_generation_execution_audit(&resolved_policy(), &execution_trace())
        .expect("build audit");

    assert_eq!(
        audit.schema_version,
        GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION
    );
    assert_eq!(audit.role, GenerationRole::Writer);
    assert_eq!(audit.resolved_model, "resolved-model");
    assert_eq!(audit.actual_model, "fallback-model");
    assert_eq!(audit.fallbacks.len(), 1);
    assert_eq!(
        audit
            .endpoint_summary
            .as_ref()
            .expect("endpoint")
            .endpoint_role,
        "backup"
    );

    let serialized = serde_json::to_string(&audit).expect("serialize audit");
    for secret in [
        "api_key",
        "authorization",
        "prompt",
        "content",
        "base_url",
        "effective_base_url",
        "https://secret.example",
    ] {
        assert!(!serialized.to_ascii_lowercase().contains(secret));
    }
}

#[test]
fn missing_history_key_is_backward_compatible() {
    assert_eq!(
        read_generation_execution_audit(&json!({"legacy": true})).expect("read legacy"),
        None
    );
}

#[test]
fn unknown_history_schema_returns_typed_error() {
    let error = read_generation_execution_audit(&json!({
        "generation_execution_audit": {"schema_version": "generation-execution-audit/v999"}
    }))
    .expect_err("reject unknown schema");

    assert_eq!(
        error,
        GenerationExecutionAuditError::UnsupportedAuditSchema(
            "generation-execution-audit/v999".to_string()
        )
    );
}

#[test]
fn audit_coexists_with_generation_contract_summary() {
    let mut history = json!({
        "generation_contract": {
            "schema_version": GENERATION_CONTRACT_SCHEMA_VERSION,
            "intent_kind": "chapter_generate",
            "input_digest": "digest",
            "story_packet_schema_version": "story-packet/v1",
            "generation_intent_schema_version": "generation-intent/v1"
        }
    });
    let original_contract = history["generation_contract"].clone();
    let audit = build_generation_execution_audit(&resolved_policy(), &execution_trace())
        .expect("build audit");
    merge_generation_execution_audit(&mut history, &audit).expect("merge audit");

    assert_eq!(
        read_generation_execution_audit(&history)
            .expect("read audit")
            .expect("audit"),
        audit
    );
    assert_eq!(history["generation_contract"], original_contract);
}

#[test]
fn null_history_payload_is_promoted_but_non_object_is_rejected() {
    let audit = build_generation_execution_audit(&resolved_policy(), &execution_trace())
        .expect("build audit");
    let mut empty = Value::Null;
    merge_generation_execution_audit(&mut empty, &audit).expect("merge into null");
    assert!(empty.get("generation_execution_audit").is_some());

    let mut invalid = json!([]);
    assert_eq!(
        merge_generation_execution_audit(&mut invalid, &audit).expect_err("reject array"),
        GenerationExecutionAuditError::InvalidHistoryPayload
    );
}
