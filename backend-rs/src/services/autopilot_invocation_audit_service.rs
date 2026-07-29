use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::models::autopilot_invocation_audit;
use crate::services::autopilot_tool_contract_service::{
    parse_transition_project_workflow_args, AutopilotToolContractError,
    AutopilotToolExecutionResultV1, AutopilotToolName, AUTOPILOT_TOOL_CONTRACT_SCHEMA_VERSION,
};
use crate::tasks::types::TaskRecord;

pub const AUTOPILOT_INVOCATION_AUDIT_SCHEMA_VERSION: &str = "autopilot-invocation-audit/v1";
pub const AUTOPILOT_INVOCATION_EXECUTION_MODE: &str = "direct_business_tool";
const STATUS_QUEUED: &str = "queued";
const STATUS_RUNNING: &str = "running";
const STATUS_SUCCEEDED: &str = "succeeded";
const STATUS_FAILED: &str = "failed";
const STATUS_CANCELLED: &str = "cancelled";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutopilotInvocationAuditError {
    InvalidPayload,
    ProjectScopeMismatch,
    ConfirmationRequired,
    Persistence,
}

impl AutopilotInvocationAuditError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidPayload => "invalid_autopilot_payload",
            Self::ProjectScopeMismatch => "project_scope_mismatch",
            Self::ConfirmationRequired => "confirmation_required",
            Self::Persistence => "audit_persistence_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutopilotInvocationAuditReadModel {
    pub audit_id: String,
    pub task_id: String,
    pub project_id: String,
    pub actor_user_id: String,
    pub schema_version: String,
    pub tool_name: String,
    pub tool_schema_version: String,
    pub confirmed_by_user: bool,
    pub execution_mode: String,
    pub provider_name: Option<String>,
    pub model_name: Option<String>,
    pub prompt_digest: Option<String>,
    pub input_digest: String,
    pub input_summary: Value,
    pub status: String,
    pub result_summary: Option<Value>,
    pub error_code: Option<String>,
    pub created_at: NaiveDateTime,
    pub started_at: Option<NaiveDateTime>,
    pub completed_at: Option<NaiveDateTime>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct NovelAutopilotTaskPayload {
    pub tool_name: String,
    pub arguments: String,
    pub confirmed_by_user: bool,
}

pub fn parse_novel_autopilot_task_payload(
    payload: Value,
) -> Result<NovelAutopilotTaskPayload, AutopilotInvocationAuditError> {
    serde_json::from_value(payload).map_err(|_| AutopilotInvocationAuditError::InvalidPayload)
}

pub(crate) fn validate_novel_autopilot_task_payload(
    record: &TaskRecord,
    payload: Value,
) -> Result<NovelAutopilotTaskPayload, AutopilotInvocationAuditError> {
    let payload = parse_novel_autopilot_task_payload(payload)?;
    AutopilotToolName::parse(&payload.tool_name).map_err(map_contract_error)?;
    if !payload.confirmed_by_user {
        return Err(AutopilotInvocationAuditError::ConfirmationRequired);
    }

    let arguments: Value = serde_json::from_str(&payload.arguments)
        .map_err(|_| AutopilotInvocationAuditError::InvalidPayload)?;
    let args = parse_transition_project_workflow_args(arguments).map_err(map_contract_error)?;
    if args.project_id != record.project_id {
        return Err(AutopilotInvocationAuditError::ProjectScopeMismatch);
    }

    Ok(payload)
}

pub async fn create_queued_autopilot_invocation_audit(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: &Value,
) -> Result<(), AutopilotInvocationAuditError> {
    let payload = validate_novel_autopilot_task_payload(record, payload.clone())?;
    let tool = AutopilotToolName::parse(&payload.tool_name).map_err(map_contract_error)?;
    let arguments: Value = serde_json::from_str(&payload.arguments)
        .map_err(|_| AutopilotInvocationAuditError::InvalidPayload)?;
    let args = parse_transition_project_workflow_args(arguments).map_err(map_contract_error)?;

    let input_summary = json!({
        "expected_phase": args.expected_phase.as_str(),
        "target_phase": args.target_phase.as_str(),
        "reason_provided": args.reason.is_some(),
        "related_task_id_provided": args.related_task_id.is_some(),
    });
    let now = Utc::now().naive_utc();
    autopilot_invocation_audit::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        task_id: Set(record.task_id.clone()),
        project_id: Set(record.project_id.clone()),
        actor_user_id: Set(record.user_id.clone()),
        schema_version: Set(AUTOPILOT_INVOCATION_AUDIT_SCHEMA_VERSION.to_string()),
        tool_name: Set(tool.as_str().to_string()),
        tool_schema_version: Set(AUTOPILOT_TOOL_CONTRACT_SCHEMA_VERSION.to_string()),
        confirmed_by_user: Set(true),
        execution_mode: Set(AUTOPILOT_INVOCATION_EXECUTION_MODE.to_string()),
        provider_name: Set(None),
        model_name: Set(None),
        prompt_digest: Set(None),
        input_digest: Set(sha256_prefixed_digest(payload_to_digest_source(&payload))),
        input_summary: Set(input_summary.to_string()),
        status: Set(STATUS_QUEUED.to_string()),
        result_summary: Set(None),
        error_code: Set(None),
        created_at: Set(now),
        started_at: Set(None),
        completed_at: Set(None),
    }
    .insert(db)
    .await
    .map_err(|_| AutopilotInvocationAuditError::Persistence)?;

    Ok(())
}

pub async fn mark_autopilot_invocation_running(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<(), AutopilotInvocationAuditError> {
    let now = Utc::now().naive_utc();
    let result = autopilot_invocation_audit::Entity::update_many()
        .col_expr(
            autopilot_invocation_audit::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_RUNNING),
        )
        .col_expr(
            autopilot_invocation_audit::Column::StartedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(autopilot_invocation_audit::Column::TaskId.eq(task_id))
        .filter(autopilot_invocation_audit::Column::Status.eq(STATUS_QUEUED))
        .exec(db)
        .await
        .map_err(|_| AutopilotInvocationAuditError::Persistence)?;
    if result.rows_affected == 1 {
        Ok(())
    } else {
        Err(AutopilotInvocationAuditError::Persistence)
    }
}

pub async fn mark_autopilot_invocation_succeeded<C>(
    db: &C,
    task_id: &str,
    result: &AutopilotToolExecutionResultV1,
) -> Result<(), AutopilotInvocationAuditError>
where
    C: ConnectionTrait,
{
    let now = Utc::now().naive_utc();
    let result_summary = json!({
        "changed": result.receipt.changed,
        "previous_phase": result.receipt.previous_phase.as_str(),
        "current_phase": result.receipt.state.phase.as_str(),
    })
    .to_string();
    let result = autopilot_invocation_audit::Entity::update_many()
        .col_expr(
            autopilot_invocation_audit::Column::Status,
            sea_orm::sea_query::Expr::value(STATUS_SUCCEEDED),
        )
        .col_expr(
            autopilot_invocation_audit::Column::ResultSummary,
            sea_orm::sea_query::Expr::value(result_summary),
        )
        .col_expr(
            autopilot_invocation_audit::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(autopilot_invocation_audit::Column::TaskId.eq(task_id))
        .filter(autopilot_invocation_audit::Column::Status.is_in([STATUS_QUEUED, STATUS_RUNNING]))
        .exec(db)
        .await
        .map_err(|_| AutopilotInvocationAuditError::Persistence)?;
    if result.rows_affected == 1 {
        Ok(())
    } else {
        Err(AutopilotInvocationAuditError::Persistence)
    }
}

pub async fn mark_autopilot_invocation_failed(
    db: &DatabaseConnection,
    task_id: &str,
    error: &AutopilotToolContractError,
) -> Result<(), AutopilotInvocationAuditError> {
    mark_terminal_error(db, task_id, STATUS_FAILED, autopilot_error_code(error)).await
}

pub async fn mark_autopilot_invocation_cancelled(
    db: &DatabaseConnection,
    task_id: &str,
) -> Result<(), AutopilotInvocationAuditError> {
    mark_terminal_error(db, task_id, STATUS_CANCELLED, "cancelled_by_user").await
}

pub async fn list_project_autopilot_invocation_audits(
    db: &DatabaseConnection,
    project_id: &str,
    limit: u64,
) -> Result<Vec<AutopilotInvocationAuditReadModel>, AutopilotInvocationAuditError> {
    let records = autopilot_invocation_audit::Entity::find()
        .filter(autopilot_invocation_audit::Column::ProjectId.eq(project_id))
        .order_by_desc(autopilot_invocation_audit::Column::CreatedAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(|_| AutopilotInvocationAuditError::Persistence)?;
    Ok(records.into_iter().map(to_read_model).collect())
}

async fn mark_terminal_error(
    db: &DatabaseConnection,
    task_id: &str,
    status: &'static str,
    error_code: &'static str,
) -> Result<(), AutopilotInvocationAuditError> {
    let now = Utc::now().naive_utc();
    autopilot_invocation_audit::Entity::update_many()
        .col_expr(
            autopilot_invocation_audit::Column::Status,
            sea_orm::sea_query::Expr::value(status),
        )
        .col_expr(
            autopilot_invocation_audit::Column::ErrorCode,
            sea_orm::sea_query::Expr::value(error_code),
        )
        .col_expr(
            autopilot_invocation_audit::Column::CompletedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(autopilot_invocation_audit::Column::TaskId.eq(task_id))
        .filter(autopilot_invocation_audit::Column::Status.is_in([STATUS_QUEUED, STATUS_RUNNING]))
        .exec(db)
        .await
        .map_err(|_| AutopilotInvocationAuditError::Persistence)?;
    Ok(())
}

fn payload_to_digest_source(payload: &NovelAutopilotTaskPayload) -> String {
    json!({
        "tool_name": payload.tool_name,
        "arguments": payload.arguments,
        "confirmed_by_user": payload.confirmed_by_user,
    })
    .to_string()
}

fn sha256_prefixed_digest(value: String) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn map_contract_error(error: AutopilotToolContractError) -> AutopilotInvocationAuditError {
    match error {
        AutopilotToolContractError::ProjectScopeMismatch => {
            AutopilotInvocationAuditError::ProjectScopeMismatch
        }
        AutopilotToolContractError::ConfirmationRequired => {
            AutopilotInvocationAuditError::ConfirmationRequired
        }
        _ => AutopilotInvocationAuditError::InvalidPayload,
    }
}

fn autopilot_error_code(error: &AutopilotToolContractError) -> &'static str {
    match error {
        AutopilotToolContractError::UnknownTool => "unknown_tool",
        AutopilotToolContractError::InvalidArguments => "invalid_arguments",
        AutopilotToolContractError::ConfirmationRequired => "confirmation_required",
        AutopilotToolContractError::ProjectScopeMismatch => "project_scope_mismatch",
        AutopilotToolContractError::NotFoundOrAccessDenied => "project_not_found_or_access_denied",
        AutopilotToolContractError::StaleExpectedPhase { .. } => "stale_expected_phase",
        AutopilotToolContractError::InvalidTransition { .. } => "invalid_transition",
        AutopilotToolContractError::ReasonRequired { .. } => "reason_required",
        AutopilotToolContractError::Internal => "tool_execution_failed",
    }
}

fn to_read_model(record: autopilot_invocation_audit::Model) -> AutopilotInvocationAuditReadModel {
    AutopilotInvocationAuditReadModel {
        audit_id: record.id,
        task_id: record.task_id,
        project_id: record.project_id,
        actor_user_id: record.actor_user_id,
        schema_version: record.schema_version,
        tool_name: record.tool_name,
        tool_schema_version: record.tool_schema_version,
        confirmed_by_user: record.confirmed_by_user,
        execution_mode: record.execution_mode,
        provider_name: record.provider_name,
        model_name: record.model_name,
        prompt_digest: record.prompt_digest,
        input_digest: record.input_digest,
        input_summary: serde_json::from_str(&record.input_summary).unwrap_or_else(|_| json!({})),
        status: record.status,
        result_summary: record
            .result_summary
            .and_then(|summary| serde_json::from_str(&summary).ok()),
        error_code: record.error_code,
        created_at: record.created_at,
        started_at: record.started_at,
        completed_at: record.completed_at,
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Schema};
    use serde_json::json;

    use super::*;
    use crate::models::{autopilot_invocation_audit, project};
    use crate::tasks::types::TaskRecord;

    async fn setup_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.expect("sqlite");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(project::Entity)))
            .await
            .expect("projects");
        db.execute(
            builder.build(&schema.create_table_from_entity(autopilot_invocation_audit::Entity)),
        )
        .await
        .expect("autopilot audit");
        db
    }

    fn record() -> TaskRecord {
        TaskRecord::new(
            "task-1".to_string(),
            "novel_autopilot".to_string(),
            "owner-1".to_string(),
            "project-1".to_string(),
            "interactive".to_string(),
        )
    }

    #[tokio::test]
    async fn queued_audit_redacts_reason_and_provider_prompt_fields() {
        let db = setup_db().await;
        create_queued_autopilot_invocation_audit(
            &db,
            &record(),
            &json!({
                "tool_name": "transition_project_workflow",
                "arguments": "{\"project_id\":\"project-1\",\"expected_phase\":\"foundation\",\"target_phase\":\"world_building\",\"reason\":\"do-not-store\",\"related_task_id\":\"secret-task\"}",
                "confirmed_by_user": true,
            }),
        )
        .await
        .expect("queued audit");

        let records = list_project_autopilot_invocation_audits(&db, "project-1", 20)
            .await
            .expect("read audit");
        assert_eq!(records.len(), 1);
        let item = &records[0];
        assert_eq!(item.status, STATUS_QUEUED);
        assert_eq!(item.execution_mode, AUTOPILOT_INVOCATION_EXECUTION_MODE);
        assert_eq!(item.provider_name, None);
        assert_eq!(item.model_name, None);
        assert_eq!(item.prompt_digest, None);
        assert!(item.input_digest.starts_with("sha256:"));
        assert_eq!(item.input_summary["reason_provided"], true);
        assert!(!item.input_summary.to_string().contains("do-not-store"));
        assert!(!item.input_summary.to_string().contains("secret-task"));
    }

    #[tokio::test]
    async fn cancellation_marks_active_audits_without_overwriting_failed_terminal_state() {
        let db = setup_db().await;
        let payload = json!({
            "tool_name": "transition_project_workflow",
            "arguments": "{\"project_id\":\"project-1\",\"expected_phase\":\"foundation\",\"target_phase\":\"world_building\"}",
            "confirmed_by_user": true,
        });

        let mut cancelled_record = record();
        cancelled_record.task_id = "task-cancelled".to_string();
        create_queued_autopilot_invocation_audit(&db, &cancelled_record, &payload)
            .await
            .expect("queued cancelled audit");
        mark_autopilot_invocation_running(&db, &cancelled_record.task_id)
            .await
            .expect("running audit");
        mark_autopilot_invocation_cancelled(&db, &cancelled_record.task_id)
            .await
            .expect("cancelled audit");

        let mut failed_record = record();
        failed_record.task_id = "task-failed".to_string();
        create_queued_autopilot_invocation_audit(&db, &failed_record, &payload)
            .await
            .expect("queued failed audit");
        mark_autopilot_invocation_failed(
            &db,
            &failed_record.task_id,
            &AutopilotToolContractError::InvalidArguments,
        )
        .await
        .expect("failed audit");
        mark_autopilot_invocation_cancelled(&db, &failed_record.task_id)
            .await
            .expect("terminal audit update is a no-op");

        let records = list_project_autopilot_invocation_audits(&db, "project-1", 20)
            .await
            .expect("read audits");
        let cancelled = records
            .iter()
            .find(|item| item.task_id == cancelled_record.task_id)
            .expect("cancelled audit");
        assert_eq!(cancelled.status, STATUS_CANCELLED);
        assert_eq!(cancelled.error_code.as_deref(), Some("cancelled_by_user"));
        assert!(cancelled.started_at.is_some());
        assert!(cancelled.completed_at.is_some());

        let failed = records
            .iter()
            .find(|item| item.task_id == failed_record.task_id)
            .expect("failed audit");
        assert_eq!(failed.status, STATUS_FAILED);
        assert_eq!(failed.error_code.as_deref(), Some("invalid_arguments"));
    }

    #[tokio::test]
    async fn project_scope_mismatch_does_not_create_an_audit_record() {
        let db = setup_db().await;
        let result = create_queued_autopilot_invocation_audit(
            &db,
            &record(),
            &json!({
                "tool_name": "transition_project_workflow",
                "arguments": "{\"project_id\":\"project-2\",\"expected_phase\":\"foundation\",\"target_phase\":\"world_building\"}",
                "confirmed_by_user": true,
            }),
        )
        .await;

        assert_eq!(
            result,
            Err(AutopilotInvocationAuditError::ProjectScopeMismatch)
        );
        assert!(
            list_project_autopilot_invocation_audits(&db, "project-1", 20)
                .await
                .expect("read audits")
                .is_empty()
        );
    }

    #[test]
    fn malformed_or_unconfirmed_payload_never_becomes_auditable_invocation() {
        assert_eq!(
            parse_novel_autopilot_task_payload(json!({"tool_name": "transition_project_workflow"})),
            Err(AutopilotInvocationAuditError::InvalidPayload)
        );
    }
}
