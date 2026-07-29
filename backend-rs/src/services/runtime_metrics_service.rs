use serde::Serialize;
use serde_json::Value;

use crate::services::autopilot_invocation_audit_service::AutopilotInvocationAuditReadModel;
use crate::services::novel_workflow_service::NovelWorkflowStateView;
use crate::tasks::types::{TaskRecord, TaskStatus};

pub const RUNTIME_METRICS_SCHEMA_VERSION: &str = "runtime-metrics/v1";
pub const RUNTIME_METRICS_TASK_OBSERVED_LIMIT: usize = 100;
pub const RUNTIME_METRICS_QUALITY_OBSERVED_LIMIT: i64 = 12;
pub const RUNTIME_METRICS_AUTOPILOT_AUDIT_OBSERVED_LIMIT: u64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMetricsDataState {
    Available,
    Empty,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RuntimeMetricsResponseV1 {
    pub schema_version: &'static str,
    pub read_model: &'static str,
    pub workflow: RuntimeMetricsWorkflowSummaryV1,
    pub tasks: RuntimeMetricsTaskSummaryV1,
    pub quality: RuntimeMetricsQualitySummaryV1,
    pub autopilot_audits: RuntimeMetricsAutopilotAuditSummaryV1,
}

impl RuntimeMetricsResponseV1 {
    pub fn new(
        workflow: RuntimeMetricsWorkflowSummaryV1,
        tasks: RuntimeMetricsTaskSummaryV1,
        quality: RuntimeMetricsQualitySummaryV1,
        autopilot_audits: RuntimeMetricsAutopilotAuditSummaryV1,
    ) -> Self {
        Self {
            schema_version: RUNTIME_METRICS_SCHEMA_VERSION,
            read_model: "derived_readonly",
            workflow,
            tasks,
            quality,
            autopilot_audits,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeMetricsWorkflowSummaryV1 {
    pub state: RuntimeMetricsDataState,
    pub schema_version: Option<u32>,
    pub phase: Option<String>,
    pub updated_at: Option<String>,
}

impl RuntimeMetricsWorkflowSummaryV1 {
    pub fn available(workflow: &NovelWorkflowStateView) -> Self {
        Self {
            state: RuntimeMetricsDataState::Available,
            schema_version: Some(workflow.schema_version),
            phase: Some(workflow.phase.to_string()),
            updated_at: Some(workflow.updated_at.and_utc().to_rfc3339()),
        }
    }

    pub fn unavailable() -> Self {
        Self {
            state: RuntimeMetricsDataState::Unavailable,
            schema_version: None,
            phase: None,
            updated_at: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeMetricsTaskSummaryV1 {
    pub state: RuntimeMetricsDataState,
    pub observed_limit: usize,
    pub observed_count: usize,
    pub pending_count: usize,
    pub running_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub cancelled_count: usize,
}

impl RuntimeMetricsTaskSummaryV1 {
    pub fn from_records(records: &[TaskRecord], observed_limit: usize) -> Self {
        let mut summary = Self {
            state: if records.is_empty() {
                RuntimeMetricsDataState::Empty
            } else {
                RuntimeMetricsDataState::Available
            },
            observed_limit,
            observed_count: records.len(),
            pending_count: 0,
            running_count: 0,
            completed_count: 0,
            failed_count: 0,
            cancelled_count: 0,
        };

        for record in records {
            match record.status {
                TaskStatus::Pending => summary.pending_count += 1,
                TaskStatus::Running => summary.running_count += 1,
                TaskStatus::Completed => summary.completed_count += 1,
                TaskStatus::Failed => summary.failed_count += 1,
                TaskStatus::Cancelled => summary.cancelled_count += 1,
            }
        }

        summary
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RuntimeMetricsQualitySummaryV1 {
    pub state: RuntimeMetricsDataState,
    pub observed_limit: i64,
    pub total_chapters: Option<u64>,
    pub analyzed_chapters: Option<u64>,
    pub latest_overall_score: Option<f64>,
    pub overall_score_delta: Option<f64>,
    pub overall_score_trend: Option<RuntimeMetricsQualityTrendV1>,
    pub last_generated_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMetricsQualityTrendV1 {
    Rising,
    Stable,
    Falling,
}

impl RuntimeMetricsQualitySummaryV1 {
    pub fn from_quality_trend_payload(payload: &Value, observed_limit: i64) -> Self {
        let total_chapters = payload.get("total_chapters").and_then(Value::as_u64);
        let analyzed_chapters = payload.get("analyzed_chapters").and_then(Value::as_u64);
        let has_metrics = payload
            .get("has_metrics")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let summary = payload
            .get("quality_metrics_summary")
            .and_then(Value::as_object);

        Self {
            state: if has_metrics {
                RuntimeMetricsDataState::Available
            } else {
                RuntimeMetricsDataState::Empty
            },
            observed_limit,
            total_chapters,
            analyzed_chapters,
            latest_overall_score: summary
                .and_then(|value| value.get("overall_score"))
                .and_then(Value::as_f64),
            overall_score_delta: summary
                .and_then(|value| value.get("overall_score_delta"))
                .and_then(Value::as_f64),
            overall_score_trend: summary
                .and_then(|value| value.get("overall_score_trend"))
                .and_then(Value::as_str)
                .and_then(parse_quality_trend),
            last_generated_at: summary
                .and_then(|value| value.get("last_generated_at"))
                .and_then(Value::as_str)
                .map(str::to_owned),
        }
    }

    pub fn unavailable(observed_limit: i64) -> Self {
        Self {
            state: RuntimeMetricsDataState::Unavailable,
            observed_limit,
            total_chapters: None,
            analyzed_chapters: None,
            latest_overall_score: None,
            overall_score_delta: None,
            overall_score_trend: None,
            last_generated_at: None,
        }
    }
}

fn parse_quality_trend(value: &str) -> Option<RuntimeMetricsQualityTrendV1> {
    match value {
        "rising" => Some(RuntimeMetricsQualityTrendV1::Rising),
        "stable" => Some(RuntimeMetricsQualityTrendV1::Stable),
        "falling" => Some(RuntimeMetricsQualityTrendV1::Falling),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeMetricsAutopilotAuditSummaryV1 {
    pub state: RuntimeMetricsDataState,
    pub observed_limit: u64,
    pub observed_count: usize,
    pub queued_count: usize,
    pub running_count: usize,
    pub succeeded_count: usize,
    pub failed_count: usize,
    pub cancelled_count: usize,
}

impl RuntimeMetricsAutopilotAuditSummaryV1 {
    pub fn from_records(
        records: &[AutopilotInvocationAuditReadModel],
        observed_limit: u64,
    ) -> Self {
        let mut summary = Self {
            state: if records.is_empty() {
                RuntimeMetricsDataState::Empty
            } else {
                RuntimeMetricsDataState::Available
            },
            observed_limit,
            observed_count: records.len(),
            queued_count: 0,
            running_count: 0,
            succeeded_count: 0,
            failed_count: 0,
            cancelled_count: 0,
        };

        for record in records {
            match record.status.as_str() {
                "queued" => summary.queued_count += 1,
                "running" => summary.running_count += 1,
                "succeeded" => summary.succeeded_count += 1,
                "failed" => summary.failed_count += 1,
                "cancelled" => summary.cancelled_count += 1,
                _ => {}
            }
        }

        summary
    }

    pub fn unavailable(observed_limit: u64) -> Self {
        Self {
            state: RuntimeMetricsDataState::Unavailable,
            observed_limit,
            observed_count: 0,
            queued_count: 0,
            running_count: 0,
            succeeded_count: 0,
            failed_count: 0,
            cancelled_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::json;

    use super::*;
    use crate::services::novel_workflow_service::{NovelWorkflowPhase, NovelWorkflowStateView};

    fn task_record(task_id: &str, status: TaskStatus) -> TaskRecord {
        let mut record = TaskRecord::new(
            task_id.to_owned(),
            "chapter_generate".to_owned(),
            "owner-1".to_owned(),
            "project-1".to_owned(),
            "interactive".to_owned(),
        );
        record.status = status;
        record.result = Some(json!({"private_result": "do-not-expose"}));
        record.error = Some("private task failure".to_owned());
        record
    }

    fn workflow_state() -> NovelWorkflowStateView {
        NovelWorkflowStateView::new(
            "project-1".to_owned(),
            NovelWorkflowPhase::Writing,
            NaiveDate::from_ymd_opt(2026, 7, 16)
                .expect("valid date")
                .and_hms_opt(10, 30, 0)
                .expect("valid time"),
        )
    }

    fn audit_record(status: &str) -> AutopilotInvocationAuditReadModel {
        AutopilotInvocationAuditReadModel {
            audit_id: "audit-private".to_owned(),
            task_id: "task-private".to_owned(),
            project_id: "project-1".to_owned(),
            actor_user_id: "owner-1".to_owned(),
            schema_version: "autopilot-invocation-audit/v1".to_owned(),
            tool_name: "transition_project_workflow".to_owned(),
            tool_schema_version: "autopilot-tool/v1".to_owned(),
            confirmed_by_user: true,
            execution_mode: "direct_business_tool".to_owned(),
            provider_name: Some("private-provider".to_owned()),
            model_name: Some("private-model".to_owned()),
            prompt_digest: Some("sha256:private".to_owned()),
            input_digest: "sha256:private-input".to_owned(),
            input_summary: json!({"private_input": "do-not-expose"}),
            status: status.to_owned(),
            result_summary: Some(json!({"private_result": "do-not-expose"})),
            error_code: Some("private_error".to_owned()),
            created_at: NaiveDate::from_ymd_opt(2026, 7, 16)
                .expect("valid date")
                .and_hms_opt(10, 0, 0)
                .expect("valid time"),
            started_at: None,
            completed_at: None,
        }
    }

    #[test]
    fn r8_runtime_metrics_aggregate_only_safe_status_counts() {
        let tasks = vec![
            task_record("task-a", TaskStatus::Pending),
            task_record("task-b", TaskStatus::Running),
            task_record("task-c", TaskStatus::Completed),
            task_record("task-d", TaskStatus::Failed),
            task_record("task-e", TaskStatus::Cancelled),
        ];
        let audits = vec![
            audit_record("queued"),
            audit_record("running"),
            audit_record("succeeded"),
            audit_record("failed"),
            audit_record("cancelled"),
        ];
        let quality = RuntimeMetricsQualitySummaryV1::from_quality_trend_payload(
            &json!({
                "project_id": "project-private",
                "has_metrics": true,
                "total_chapters": 3,
                "analyzed_chapters": 2,
                "items": [{"chapter_id": "chapter-private", "title": "private title"}],
                "quality_metrics_summary": {
                    "overall_score": 81.5,
                    "overall_score_delta": 3.5,
                    "overall_score_trend": "rising",
                    "last_generated_at": "2026-07-16T10:00:00",
                    "recent_history": [{"private": "do-not-expose"}]
                }
            }),
            RUNTIME_METRICS_QUALITY_OBSERVED_LIMIT,
        );
        let response = RuntimeMetricsResponseV1::new(
            RuntimeMetricsWorkflowSummaryV1::available(&workflow_state()),
            RuntimeMetricsTaskSummaryV1::from_records(&tasks, RUNTIME_METRICS_TASK_OBSERVED_LIMIT),
            quality,
            RuntimeMetricsAutopilotAuditSummaryV1::from_records(
                &audits,
                RUNTIME_METRICS_AUTOPILOT_AUDIT_OBSERVED_LIMIT,
            ),
        );
        let value = serde_json::to_value(response).expect("serialize metrics");

        assert_eq!(value["read_model"], "derived_readonly");
        assert_eq!(value["workflow"]["phase"], "writing");
        assert_eq!(value["tasks"]["pending_count"], 1);
        assert_eq!(value["tasks"]["failed_count"], 1);
        assert_eq!(value["quality"]["latest_overall_score"], 81.5);
        assert_eq!(value["quality"]["overall_score_trend"], "rising");
        assert_eq!(value["autopilot_audits"]["succeeded_count"], 1);
        assert!(!value.to_string().contains("private"));
        assert!(!value.to_string().contains("task_id"));
        assert!(!value.to_string().contains("audit_id"));
        assert!(!value.to_string().contains("project_id"));
        assert!(!value.to_string().contains("provider"));
        assert!(!value.to_string().contains("digest"));
    }

    #[test]
    fn r8_runtime_metrics_empty_and_unknown_quality_data_fail_closed() {
        let quality = RuntimeMetricsQualitySummaryV1::from_quality_trend_payload(
            &json!({
                "has_metrics": true,
                "total_chapters": "invalid",
                "analyzed_chapters": null,
                "quality_metrics_summary": {
                    "overall_score": "invalid",
                    "overall_score_delta": true,
                    "overall_score_trend": "unknown",
                    "last_generated_at": 42
                }
            }),
            RUNTIME_METRICS_QUALITY_OBSERVED_LIMIT,
        );

        assert_eq!(quality.state, RuntimeMetricsDataState::Available);
        assert_eq!(quality.total_chapters, None);
        assert_eq!(quality.analyzed_chapters, None);
        assert_eq!(quality.latest_overall_score, None);
        assert_eq!(quality.overall_score_delta, None);
        assert_eq!(quality.overall_score_trend, None);
        assert_eq!(quality.last_generated_at, None);
        assert_eq!(
            RuntimeMetricsTaskSummaryV1::from_records(&[], RUNTIME_METRICS_TASK_OBSERVED_LIMIT)
                .state,
            RuntimeMetricsDataState::Empty
        );
        assert_eq!(
            RuntimeMetricsAutopilotAuditSummaryV1::from_records(
                &[],
                RUNTIME_METRICS_AUTOPILOT_AUDIT_OBSERVED_LIMIT,
            )
            .state,
            RuntimeMetricsDataState::Empty
        );
        assert_eq!(
            RuntimeMetricsQualitySummaryV1::unavailable(RUNTIME_METRICS_QUALITY_OBSERVED_LIMIT)
                .state,
            RuntimeMetricsDataState::Unavailable
        );
    }
}
