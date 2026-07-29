use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ai::execution_trace::{
    AIExecutionFallbackSummaryV1, AIExecutionTraceV1, EndpointExecutionSummaryV1,
    AI_EXECUTION_TRACE_SCHEMA_VERSION,
};
use crate::services::role_model_policy_service::{
    GenerationRole, ModelSelectionSource, ResolvedRoleModelPolicyV1,
    ROLE_MODEL_POLICY_SCHEMA_VERSION,
};

pub const GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION: &str = "generation-execution-audit/v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationExecutionAuditError {
    InvalidHistoryPayload,
    UnsupportedAuditSchema(String),
    UnsupportedExecutionTraceSchema(String),
    UnsupportedPolicySchema(String),
    Serialization(String),
}

impl fmt::Display for GenerationExecutionAuditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHistoryPayload => {
                formatter.write_str("generation execution audit history payload must be an object")
            }
            Self::UnsupportedAuditSchema(schema) => {
                write!(
                    formatter,
                    "unsupported generation execution audit schema: {schema}"
                )
            }
            Self::UnsupportedExecutionTraceSchema(schema) => {
                write!(formatter, "unsupported AI execution trace schema: {schema}")
            }
            Self::UnsupportedPolicySchema(schema) => {
                write!(formatter, "unsupported role model policy schema: {schema}")
            }
            Self::Serialization(error) => {
                write!(
                    formatter,
                    "generation execution audit serialization failed: {error}"
                )
            }
        }
    }
}

impl std::error::Error for GenerationExecutionAuditError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationExecutionAuditV1 {
    pub schema_version: String,
    pub role: GenerationRole,
    pub policy_schema_version: String,
    pub policy_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model: Option<String>,
    pub resolved_provider: String,
    pub resolved_model: String,
    pub actual_provider: String,
    pub actual_model: String,
    pub provider_source: ModelSelectionSource,
    pub model_source: ModelSelectionSource,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<AIExecutionFallbackSummaryV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_summary: Option<EndpointExecutionSummaryV1>,
}

pub fn build_generation_execution_audit(
    resolved_policy: &ResolvedRoleModelPolicyV1,
    execution: &AIExecutionTraceV1,
) -> Result<GenerationExecutionAuditV1, GenerationExecutionAuditError> {
    if resolved_policy.policy_schema_version != ROLE_MODEL_POLICY_SCHEMA_VERSION {
        return Err(GenerationExecutionAuditError::UnsupportedPolicySchema(
            resolved_policy.policy_schema_version.clone(),
        ));
    }
    if execution.schema_version != AI_EXECUTION_TRACE_SCHEMA_VERSION {
        return Err(
            GenerationExecutionAuditError::UnsupportedExecutionTraceSchema(
                execution.schema_version.clone(),
            ),
        );
    }

    Ok(GenerationExecutionAuditV1 {
        schema_version: GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION.to_string(),
        role: resolved_policy.role,
        policy_schema_version: resolved_policy.policy_schema_version.clone(),
        policy_digest: resolved_policy.policy_digest.clone(),
        requested_provider: resolved_policy.requested_provider.clone(),
        requested_model: resolved_policy.requested_model.clone(),
        resolved_provider: resolved_policy.resolved_provider.clone(),
        resolved_model: resolved_policy.resolved_model.clone(),
        actual_provider: execution.actual_provider.clone(),
        actual_model: execution.actual_model.clone(),
        provider_source: resolved_policy.provider_source,
        model_source: resolved_policy.model_source,
        fallbacks: execution.fallbacks.clone(),
        endpoint_summary: execution.endpoint_summary.clone(),
    })
}
