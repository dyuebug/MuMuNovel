use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::services::generation_contract_service::{
    GenerationContractHistorySummaryV1, GENERATION_CONTRACT_SCHEMA_VERSION,
};
use crate::services::generation_execution_audit_service::{
    GenerationExecutionAuditV1, GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION,
};

pub(crate) const R8_EVAL_SAFE_SUMMARY_SCHEMA_VERSION: &str = "r8-eval-safe-summary/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct R8EvalSafeSummaryV1 {
    pub(crate) schema_version: String,
    pub(crate) generation_contract_schema_version: String,
    pub(crate) generation_intent_kind: String,
    pub(crate) generation_execution_audit_schema_version: String,
    pub(crate) execution_role: String,
    pub(crate) has_quality_summary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum R8EvalVerdict {
    Complete,
    PartialWithoutQualitySummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum R8EvalError {
    UnsupportedSafeSummarySchema,
    UnsupportedGenerationContractSchema,
    UnsupportedGenerationExecutionAuditSchema,
    InvalidSafeEnumLabel,
}

pub(crate) fn build_r8_eval_safe_summary(
    generation_contract: &GenerationContractHistorySummaryV1,
    generation_execution_audit: &GenerationExecutionAuditV1,
    has_quality_summary: bool,
) -> Result<R8EvalSafeSummaryV1, R8EvalError> {
    if generation_contract.schema_version != GENERATION_CONTRACT_SCHEMA_VERSION {
        return Err(R8EvalError::UnsupportedGenerationContractSchema);
    }
    if generation_execution_audit.schema_version != GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION {
        return Err(R8EvalError::UnsupportedGenerationExecutionAuditSchema);
    }

    Ok(R8EvalSafeSummaryV1 {
        schema_version: R8_EVAL_SAFE_SUMMARY_SCHEMA_VERSION.to_owned(),
        generation_contract_schema_version: generation_contract.schema_version.clone(),
        generation_intent_kind: serialize_safe_enum_label(&generation_contract.intent_kind)?,
        generation_execution_audit_schema_version: generation_execution_audit
            .schema_version
            .clone(),
        execution_role: serialize_safe_enum_label(&generation_execution_audit.role)?,
        has_quality_summary,
    })
}

pub(crate) fn evaluate_r8_safe_summary(
    summary: &R8EvalSafeSummaryV1,
) -> Result<R8EvalVerdict, R8EvalError> {
    if summary.schema_version != R8_EVAL_SAFE_SUMMARY_SCHEMA_VERSION {
        return Err(R8EvalError::UnsupportedSafeSummarySchema);
    }
    if summary.generation_contract_schema_version != GENERATION_CONTRACT_SCHEMA_VERSION {
        return Err(R8EvalError::UnsupportedGenerationContractSchema);
    }
    if summary.generation_execution_audit_schema_version
        != GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION
    {
        return Err(R8EvalError::UnsupportedGenerationExecutionAuditSchema);
    }

    Ok(if summary.has_quality_summary {
        R8EvalVerdict::Complete
    } else {
        R8EvalVerdict::PartialWithoutQualitySummary
    })
}

fn serialize_safe_enum_label<T: Serialize>(value: &T) -> Result<String, R8EvalError> {
    match serde_json::to_value(value).map_err(|_| R8EvalError::InvalidSafeEnumLabel)? {
        Value::String(label) => Ok(label),
        _ => Err(R8EvalError::InvalidSafeEnumLabel),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{build_r8_eval_safe_summary, evaluate_r8_safe_summary, R8EvalError, R8EvalVerdict};
    use crate::services::generation_contract_service::{
        GenerationContractHistorySummaryV1, GenerationIntentKind, GenerationTarget,
        GENERATION_CONTRACT_SCHEMA_VERSION,
    };
    use crate::services::generation_execution_audit_service::{
        GenerationExecutionAuditV1, GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION,
    };
    use crate::services::r8_eval_fixture::{complete_safe_summary, expected_complete_verdict};
    use crate::services::role_model_policy_service::{GenerationRole, ModelSelectionSource};

    fn generation_contract_summary() -> GenerationContractHistorySummaryV1 {
        GenerationContractHistorySummaryV1 {
            schema_version: GENERATION_CONTRACT_SCHEMA_VERSION.to_owned(),
            input_digest: "private-contract-digest".to_owned(),
            target: GenerationTarget::chapter("private-project", "private-chapter"),
            intent_kind: GenerationIntentKind::ChapterReview,
            sources: Vec::new(),
        }
    }

    fn generation_execution_audit() -> GenerationExecutionAuditV1 {
        GenerationExecutionAuditV1 {
            schema_version: GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION.to_owned(),
            role: GenerationRole::Reviewer,
            policy_schema_version: "role-model-policy/v1".to_owned(),
            policy_digest: "private-policy-digest".to_owned(),
            requested_provider: Some("private-provider".to_owned()),
            requested_model: Some("private-model".to_owned()),
            resolved_provider: "private-provider".to_owned(),
            resolved_model: "private-model".to_owned(),
            actual_provider: "private-provider".to_owned(),
            actual_model: "private-model".to_owned(),
            provider_source: ModelSelectionSource::RoleOverride,
            model_source: ModelSelectionSource::RoleOverride,
            fallbacks: Vec::new(),
            endpoint_summary: None,
        }
    }

    #[test]
    fn r8_eval_projects_a_complete_safe_summary_without_private_source_fields() {
        let summary = build_r8_eval_safe_summary(
            &generation_contract_summary(),
            &generation_execution_audit(),
            true,
        )
        .expect("safe summary");

        assert_eq!(summary, complete_safe_summary());
        assert_eq!(
            evaluate_r8_safe_summary(&summary).expect("evaluate safe summary"),
            expected_complete_verdict()
        );

        let value = serde_json::to_value(&summary).expect("serialize safe summary");
        assert_eq!(
            value,
            json!({
                "schema_version": "r8-eval-safe-summary/v1",
                "generation_contract_schema_version": "generation-contract/v1",
                "generation_intent_kind": "chapter_review",
                "generation_execution_audit_schema_version": "generation-execution-audit/v1",
                "execution_role": "reviewer",
                "has_quality_summary": true,
            })
        );
        for forbidden_field in [
            "input_digest",
            "target",
            "project_id",
            "chapter_id",
            "policy_digest",
            "requested_provider",
            "requested_model",
            "resolved_provider",
            "resolved_model",
            "actual_provider",
            "actual_model",
            "fallbacks",
            "endpoint_summary",
        ] {
            assert!(
                value.get(forbidden_field).is_none(),
                "{forbidden_field} leaked"
            );
        }
    }

    #[test]
    fn r8_eval_marks_missing_quality_summary_as_partial_without_reading_raw_history() {
        let summary = build_r8_eval_safe_summary(
            &generation_contract_summary(),
            &generation_execution_audit(),
            false,
        )
        .expect("safe summary");

        assert_eq!(
            evaluate_r8_safe_summary(&summary).expect("evaluate safe summary"),
            R8EvalVerdict::PartialWithoutQualitySummary
        );
    }

    #[test]
    fn r8_eval_rejects_unknown_source_or_summary_schema_with_stable_errors() {
        let mut unknown_contract = generation_contract_summary();
        unknown_contract.schema_version = "generation-contract/v999".to_owned();
        assert_eq!(
            build_r8_eval_safe_summary(&unknown_contract, &generation_execution_audit(), true),
            Err(R8EvalError::UnsupportedGenerationContractSchema)
        );

        let mut unknown_summary = complete_safe_summary();
        unknown_summary.schema_version = "r8-eval-safe-summary/v999".to_owned();
        assert_eq!(
            evaluate_r8_safe_summary(&unknown_summary),
            Err(R8EvalError::UnsupportedSafeSummarySchema)
        );
    }
}
