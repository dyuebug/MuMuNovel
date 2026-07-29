use super::r8_eval_service::{
    R8EvalSafeSummaryV1, R8EvalVerdict, R8_EVAL_SAFE_SUMMARY_SCHEMA_VERSION,
};
use crate::services::generation_contract_service::GENERATION_CONTRACT_SCHEMA_VERSION;
use crate::services::generation_execution_audit_service::GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION;

pub(crate) fn complete_safe_summary() -> R8EvalSafeSummaryV1 {
    R8EvalSafeSummaryV1 {
        schema_version: R8_EVAL_SAFE_SUMMARY_SCHEMA_VERSION.to_owned(),
        generation_contract_schema_version: GENERATION_CONTRACT_SCHEMA_VERSION.to_owned(),
        generation_intent_kind: "chapter_review".to_owned(),
        generation_execution_audit_schema_version: GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION
            .to_owned(),
        execution_role: "reviewer".to_owned(),
        has_quality_summary: true,
    }
}

pub(crate) fn expected_complete_verdict() -> R8EvalVerdict {
    R8EvalVerdict::Complete
}
