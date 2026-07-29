use chrono::NaiveDateTime;
use serde::Serialize;
use serde_json::Value;

use crate::models::generation_history;
use crate::services::generation_contract_service::read_generation_contract_history_summary;
use crate::services::generation_execution_audit_service::read_generation_execution_audit;

pub(crate) const CREATIVE_ARCHIVE_GENERATION_RECORD_SCHEMA_VERSION: &str =
    "creative-archive-generation-record/v1";
pub(crate) const CREATIVE_ARCHIVE_QUALITY_SUMMARY_SCHEMA_VERSION: &str =
    "creative-archive-quality-summary/v1";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct CreativeArchiveGenerationRecordV1 {
    pub(crate) schema_version: &'static str,
    pub(crate) generated_at: Option<String>,
    pub(crate) feedback_link: CreativeArchiveFeedbackLinkV1,
    pub(crate) generation_contract: Option<CreativeArchiveGenerationContractSummaryV1>,
    pub(crate) execution_audit: Option<CreativeArchiveExecutionAuditSummaryV1>,
    pub(crate) quality_summary: Option<CreativeArchiveQualitySummaryV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CreativeArchiveFeedbackLinkV1 {
    pub(crate) schema_version: &'static str,
    pub(crate) chapter_number: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CreativeArchiveGenerationContractSummaryV1 {
    pub(crate) schema_version: String,
    pub(crate) intent_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct CreativeArchiveExecutionAuditSummaryV1 {
    pub(crate) schema_version: String,
    pub(crate) execution_role: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct CreativeArchiveQualitySummaryV1 {
    pub(crate) schema_version: &'static str,
    pub(crate) overall_score: Option<f64>,
    pub(crate) quality_gate_decision: Option<CreativeArchiveQualityGateDecision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CreativeArchiveQualityGateDecision {
    Pass,
    AutoRepair,
    ManualReview,
}

pub(crate) fn build_creative_archive_generation_record(
    history: &generation_history::Model,
    chapter_number: Option<i32>,
) -> CreativeArchiveGenerationRecordV1 {
    let history_payload = history
        .generated_content
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok());

    CreativeArchiveGenerationRecordV1 {
        schema_version: CREATIVE_ARCHIVE_GENERATION_RECORD_SCHEMA_VERSION,
        generated_at: history.created_at.map(format_archive_datetime),
        feedback_link: CreativeArchiveFeedbackLinkV1 {
            schema_version: "creative-archive-feedback-link/v1",
            chapter_number,
        },
        generation_contract: history_payload
            .as_ref()
            .and_then(build_generation_contract_summary),
        execution_audit: history_payload
            .as_ref()
            .and_then(build_execution_audit_summary),
        quality_summary: history_payload.as_ref().and_then(build_quality_summary),
    }
}

fn build_generation_contract_summary(
    history_payload: &Value,
) -> Option<CreativeArchiveGenerationContractSummaryV1> {
    let summary = read_generation_contract_history_summary(history_payload)
        .ok()
        .flatten()?;
    let intent_kind = serialize_safe_enum_label(&summary.intent_kind)?;

    Some(CreativeArchiveGenerationContractSummaryV1 {
        schema_version: summary.schema_version,
        intent_kind,
    })
}

fn build_execution_audit_summary(
    history_payload: &Value,
) -> Option<CreativeArchiveExecutionAuditSummaryV1> {
    let audit = read_generation_execution_audit(history_payload)
        .ok()
        .flatten()?;
    let execution_role = serialize_safe_enum_label(&audit.role)?;

    Some(CreativeArchiveExecutionAuditSummaryV1 {
        schema_version: audit.schema_version,
        execution_role,
    })
}

fn build_quality_summary(history_payload: &Value) -> Option<CreativeArchiveQualitySummaryV1> {
    let quality_metrics = history_payload.get("quality_metrics")?.as_object()?;
    let overall_score = quality_metrics.get("overall_score").and_then(Value::as_f64);
    let quality_gate_decision = quality_metrics
        .get("quality_gate")
        .and_then(Value::as_object)
        .and_then(|quality_gate| quality_gate.get("decision"))
        .and_then(Value::as_str)
        .and_then(CreativeArchiveQualityGateDecision::from_safe_label);

    if overall_score.is_none() && quality_gate_decision.is_none() {
        return None;
    }

    Some(CreativeArchiveQualitySummaryV1 {
        schema_version: CREATIVE_ARCHIVE_QUALITY_SUMMARY_SCHEMA_VERSION,
        overall_score,
        quality_gate_decision,
    })
}

impl CreativeArchiveQualityGateDecision {
    fn from_safe_label(value: &str) -> Option<Self> {
        match value {
            "pass" => Some(Self::Pass),
            "auto_repair" => Some(Self::AutoRepair),
            "manual_review" => Some(Self::ManualReview),
            _ => None,
        }
    }
}

fn serialize_safe_enum_label<T: Serialize>(value: &T) -> Option<String> {
    serde_json::to_value(value)
        .ok()?
        .as_str()
        .map(ToOwned::to_owned)
}

fn format_archive_datetime(value: NaiveDateTime) -> String {
    value.format("%Y-%m-%dT%H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::{json, Value};

    use super::{
        build_creative_archive_generation_record, CreativeArchiveQualityGateDecision,
        CREATIVE_ARCHIVE_GENERATION_RECORD_SCHEMA_VERSION,
        CREATIVE_ARCHIVE_QUALITY_SUMMARY_SCHEMA_VERSION,
    };
    use crate::models::generation_history;
    use crate::services::generation_contract_service::{
        GenerationContractHistorySummaryV1, GenerationIntentKind, GenerationTarget,
        GENERATION_CONTRACT_SCHEMA_VERSION,
    };
    use crate::services::generation_execution_audit_service::{
        GenerationExecutionAuditV1, GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION,
    };
    use crate::services::role_model_policy_service::{GenerationRole, ModelSelectionSource};

    fn history_with_payload(payload: Value) -> generation_history::Model {
        generation_history::Model {
            id: "private-history-id".to_owned(),
            project_id: "private-project-id".to_owned(),
            chapter_id: Some("private-chapter-id".to_owned()),
            prompt: Some("private prompt".to_owned()),
            generated_content: Some(payload.to_string()),
            model: Some("private-model".to_owned()),
            tokens_used: Some(42),
            generation_time: Some(0.5),
            created_at: Some(
                NaiveDate::from_ymd_opt(2026, 7, 16)
                    .expect("valid test date")
                    .and_hms_opt(12, 34, 56)
                    .expect("valid test time"),
            ),
        }
    }

    fn generation_contract_summary() -> GenerationContractHistorySummaryV1 {
        GenerationContractHistorySummaryV1 {
            schema_version: GENERATION_CONTRACT_SCHEMA_VERSION.to_owned(),
            input_digest: "private-input-digest".to_owned(),
            target: GenerationTarget::chapter("private-project-id", "private-chapter-id"),
            intent_kind: GenerationIntentKind::ChapterReview,
            sources: Vec::new(),
        }
    }

    fn execution_audit() -> GenerationExecutionAuditV1 {
        GenerationExecutionAuditV1 {
            schema_version: GENERATION_EXECUTION_AUDIT_SCHEMA_VERSION.to_owned(),
            role: GenerationRole::Reviewer,
            policy_schema_version: "role-model-policy/v1".to_owned(),
            policy_digest: "private-policy-digest".to_owned(),
            requested_provider: Some("private-requested-provider".to_owned()),
            requested_model: Some("private-requested-model".to_owned()),
            resolved_provider: "private-resolved-provider".to_owned(),
            resolved_model: "private-resolved-model".to_owned(),
            actual_provider: "private-actual-provider".to_owned(),
            actual_model: "private-actual-model".to_owned(),
            provider_source: ModelSelectionSource::RoleOverride,
            model_source: ModelSelectionSource::RoleOverride,
            fallbacks: Vec::new(),
            endpoint_summary: None,
        }
    }

    #[test]
    fn r8_creative_archive_projects_only_safe_history_summaries() {
        let history = history_with_payload(json!({
            "content": "private generated content",
            "preview": "private preview",
            "story_packet": generation_contract_summary(),
            "generation_execution_audit": execution_audit(),
            "quality_metrics": {
                "overall_score": 8.5,
                "quality_gate": {
                    "decision": "manual_review",
                    "summary": "private repair instruction"
                }
            }
        }));

        let record = build_creative_archive_generation_record(&history, Some(7));
        let value = serde_json::to_value(&record).expect("serialize archive record");

        assert_eq!(
            record.schema_version,
            CREATIVE_ARCHIVE_GENERATION_RECORD_SCHEMA_VERSION
        );
        assert_eq!(record.feedback_link.chapter_number, Some(7));
        assert_eq!(record.generated_at.as_deref(), Some("2026-07-16T12:34:56"));
        assert_eq!(
            record
                .generation_contract
                .as_ref()
                .map(|summary| summary.intent_kind.as_str()),
            Some("chapter_review")
        );
        assert_eq!(
            record
                .execution_audit
                .as_ref()
                .map(|summary| summary.execution_role.as_str()),
            Some("reviewer")
        );
        assert_eq!(
            record
                .quality_summary
                .as_ref()
                .map(|summary| summary.schema_version),
            Some(CREATIVE_ARCHIVE_QUALITY_SUMMARY_SCHEMA_VERSION)
        );
        assert_eq!(
            record
                .quality_summary
                .as_ref()
                .and_then(|summary| summary.quality_gate_decision),
            Some(CreativeArchiveQualityGateDecision::ManualReview)
        );

        for forbidden_field in [
            "private-history-id",
            "private-project-id",
            "private-chapter-id",
            "private prompt",
            "private generated content",
            "private preview",
            "private-input-digest",
            "private-policy-digest",
            "private-requested-provider",
            "private-requested-model",
            "private-resolved-provider",
            "private-resolved-model",
            "private-actual-provider",
            "private-actual-model",
            "private repair instruction",
        ] {
            assert!(
                !value.to_string().contains(forbidden_field),
                "{forbidden_field} leaked into archive record"
            );
        }
    }

    #[test]
    fn r8_creative_archive_fails_closed_for_legacy_or_unknown_history_shapes() {
        let history = history_with_payload(json!({
            "story_packet": {"schema_version": "unknown-contract/v9"},
            "generation_execution_audit": {"schema_version": "unknown-audit/v9"},
            "quality_metrics": {
                "overall_score": "not-a-number",
                "quality_gate": {"decision": "unknown"}
            }
        }));

        let record = build_creative_archive_generation_record(&history, None);

        assert!(record.generation_contract.is_none());
        assert!(record.execution_audit.is_none());
        assert!(record.quality_summary.is_none());
        assert_eq!(record.feedback_link.chapter_number, None);
    }

    #[test]
    fn r8_creative_archive_treats_non_json_legacy_content_as_missing_safe_summaries() {
        let mut history = history_with_payload(json!({}));
        history.generated_content = Some("legacy raw generated content".to_owned());

        let record = build_creative_archive_generation_record(&history, Some(1));

        assert!(record.generation_contract.is_none());
        assert!(record.execution_audit.is_none());
        assert!(record.quality_summary.is_none());
    }
}
