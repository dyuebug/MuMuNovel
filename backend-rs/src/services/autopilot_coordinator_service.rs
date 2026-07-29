use sea_orm::{DatabaseConnection, TransactionTrait};
use serde_json::{to_value, Value};

use crate::services::autopilot_invocation_audit_service::{
    mark_autopilot_invocation_failed, mark_autopilot_invocation_running,
    mark_autopilot_invocation_succeeded, validate_novel_autopilot_task_payload,
    AutopilotInvocationAuditError,
};
use crate::services::autopilot_tool_contract_service::{
    dispatch_autopilot_tool_call, AutopilotToolConfirmation, AutopilotToolContractError,
    AutopilotToolExecutionContext,
};
use crate::tasks::types::TaskRecord;

const INVALID_AUTOPILOT_TASK_PAYLOAD: &str = "invalid novel autopilot task payload";
const AUTOPILOT_TASK_EXECUTION_FAILED: &str = "autopilot task execution failed";

pub async fn execute_novel_autopilot_task(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: Value,
) -> Result<Value, String> {
    if record.project_id.trim().is_empty() {
        return Err(INVALID_AUTOPILOT_TASK_PAYLOAD.to_string());
    }

    let payload =
        validate_novel_autopilot_task_payload(record, payload).map_err(map_audit_payload_error)?;
    let confirmation = if payload.confirmed_by_user {
        AutopilotToolConfirmation::ConfirmedByUser
    } else {
        AutopilotToolConfirmation::Missing
    };
    let context = AutopilotToolExecutionContext {
        actor_user_id: &record.user_id,
        confirmation,
        project_scope: Some(&record.project_id),
    };

    mark_autopilot_invocation_running(db, &record.task_id)
        .await
        .map_err(|error| {
            tracing::error!(
                event = "autopilot_invocation_audit_running_failed",
                task_id = %record.task_id,
                error_code = error.code(),
                "autopilot invocation audit could not enter running state"
            );
            AUTOPILOT_TASK_EXECUTION_FAILED.to_string()
        })?;

    let transaction = db
        .begin()
        .await
        .map_err(|_| AUTOPILOT_TASK_EXECUTION_FAILED.to_string())?;
    let result = dispatch_autopilot_tool_call(
        &transaction,
        context,
        &payload.tool_name,
        &payload.arguments,
    )
    .await;

    match result {
        Ok(result) => {
            if let Err(error) =
                mark_autopilot_invocation_succeeded(&transaction, &record.task_id, &result).await
            {
                let _ = transaction.rollback().await;
                if let Err(fallback_error) = mark_autopilot_invocation_failed(
                    db,
                    &record.task_id,
                    &AutopilotToolContractError::Internal,
                )
                .await
                {
                    tracing::error!(
                        event = "autopilot_invocation_audit_failure_fallback_failed",
                        task_id = %record.task_id,
                        error_code = fallback_error.code(),
                        "autopilot invocation audit fallback could not enter failed state"
                    );
                }
                tracing::error!(
                    event = "autopilot_invocation_audit_success_failed",
                    task_id = %record.task_id,
                    error_code = error.code(),
                    "autopilot invocation audit could not enter succeeded state"
                );
                return Err(AUTOPILOT_TASK_EXECUTION_FAILED.to_string());
            }
            transaction
                .commit()
                .await
                .map_err(|_| AUTOPILOT_TASK_EXECUTION_FAILED.to_string())?;
            tracing::info!(
                event = "novel_autopilot_task_execution",
                task_id = %record.task_id,
                tool_name = %result.tool_name.as_str(),
                "novel autopilot task execution completed"
            );
            to_value(result).map_err(|_| AUTOPILOT_TASK_EXECUTION_FAILED.to_string())
        }
        Err(error) => {
            let _ = transaction.rollback().await;
            if let Err(audit_error) =
                mark_autopilot_invocation_failed(db, &record.task_id, &error).await
            {
                tracing::error!(
                    event = "autopilot_invocation_audit_failure_update_failed",
                    task_id = %record.task_id,
                    error_code = audit_error.code(),
                    "autopilot invocation audit failure state could not be persisted"
                );
            }
            Err(map_autopilot_task_error(error))
        }
    }
}

fn map_audit_payload_error(error: AutopilotInvocationAuditError) -> String {
    match error {
        AutopilotInvocationAuditError::ConfirmationRequired => {
            "user confirmation is required for this autopilot tool".to_string()
        }
        AutopilotInvocationAuditError::InvalidPayload
        | AutopilotInvocationAuditError::ProjectScopeMismatch => {
            INVALID_AUTOPILOT_TASK_PAYLOAD.to_string()
        }
        AutopilotInvocationAuditError::Persistence => AUTOPILOT_TASK_EXECUTION_FAILED.to_string(),
    }
}

fn map_autopilot_task_error(error: AutopilotToolContractError) -> String {
    match error {
        AutopilotToolContractError::ProjectScopeMismatch => {
            INVALID_AUTOPILOT_TASK_PAYLOAD.to_string()
        }
        AutopilotToolContractError::Internal => AUTOPILOT_TASK_EXECUTION_FAILED.to_string(),
        safe_error => safe_error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, Schema, Set,
        Statement,
    };
    use serde_json::json;

    use super::{execute_novel_autopilot_task, INVALID_AUTOPILOT_TASK_PAYLOAD};
    use crate::models::{autopilot_invocation_audit, project};
    use crate::services::autopilot_invocation_audit_service::{
        create_queued_autopilot_invocation_audit, list_project_autopilot_invocation_audits,
    };
    use crate::services::autopilot_safety_gate_fixture as safety_fixture;
    use crate::services::novel_workflow_service::{get_state, NovelWorkflowPhase};
    use crate::tasks::types::TaskRecord;

    async fn setup_workflow_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect workflow sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(project::Entity)))
            .await
            .expect("create projects table");
        db.execute(
            builder.build(&schema.create_table_from_entity(autopilot_invocation_audit::Entity)),
        )
        .await
        .expect("create autopilot invocation audits table");
        db
    }

    async fn insert_project(db: &DatabaseConnection, id: &str, user_id: &str, status: &str) {
        let created_at = NaiveDate::from_ymd_opt(2026, 7, 16)
            .expect("valid date")
            .and_hms_opt(8, 0, 0)
            .expect("valid time");
        project::ActiveModel {
            id: Set(id.to_string()),
            user_id: Set(user_id.to_string()),
            title: Set(format!("Workflow {id}")),
            target_words: Set(100_000),
            current_words: Set(0),
            status: Set(status.to_string()),
            wizard_status: Set("completed".to_string()),
            wizard_step: Set(0),
            outline_mode: Set("linear".to_string()),
            character_count: Set(0),
            created_at: Set(created_at),
            updated_at: Set(Some(created_at)),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert workflow project");
    }

    fn task_record(project_id: &str, user_id: &str) -> TaskRecord {
        TaskRecord::new(
            "task-1".to_string(),
            "novel_autopilot".to_string(),
            user_id.to_string(),
            project_id.to_string(),
            "interactive".to_string(),
        )
    }

    fn transition_payload(project_id: &str) -> serde_json::Value {
        safety_fixture::confirmed_transition_payload(project_id)
    }

    async fn queue_audit(
        db: &DatabaseConnection,
        record: &TaskRecord,
        payload: &serde_json::Value,
    ) {
        create_queued_autopilot_invocation_audit(db, record, payload)
            .await
            .expect("valid task payload creates queued audit");
    }

    #[tokio::test]
    async fn confirmed_task_uses_task_actor_and_project_scope_for_canonical_transition() {
        let db = setup_workflow_db().await;
        insert_project(&db, "project-1", "owner-1", "foundation").await;

        let record = task_record("project-1", "owner-1");
        let payload = transition_payload("project-1");
        queue_audit(&db, &record, &payload).await;

        let result = execute_novel_autopilot_task(&db, &record, payload)
            .await
            .expect("confirmed task transition succeeds");

        assert_eq!(result["schema_version"], "autopilot-tool-contract/v1");
        assert_eq!(result["tool_name"], "transition_project_workflow");
        assert_eq!(
            get_state(&db, "project-1", "owner-1")
                .await
                .expect("load changed state")
                .phase,
            NovelWorkflowPhase::WorldBuilding
        );
        let audit = list_project_autopilot_invocation_audits(&db, "project-1", 1)
            .await
            .expect("read completed audit")
            .pop()
            .expect("one audit row");
        assert_eq!(audit.status, "succeeded");
        assert_eq!(audit.execution_mode, "direct_business_tool");
        assert_eq!(
            audit
                .result_summary
                .as_ref()
                .and_then(|summary| summary.get("current_phase")),
            Some(&json!("world_building"))
        );
    }

    #[tokio::test]
    async fn task_payload_cannot_cross_project_scope_or_override_confirmation() {
        let db = setup_workflow_db().await;
        insert_project(&db, "project-1", "owner-1", "foundation").await;

        let scope_error = execute_novel_autopilot_task(
            &db,
            &task_record("project-1", "owner-1"),
            transition_payload("project-2"),
        )
        .await
        .expect_err("different payload project must fail");
        assert_eq!(scope_error, INVALID_AUTOPILOT_TASK_PAYLOAD);

        let missing_confirmation = execute_novel_autopilot_task(
            &db,
            &task_record("project-1", "owner-1"),
            json!({
                "tool_name": "transition_project_workflow",
                "arguments": r#"{"project_id":"project-1","expected_phase":"foundation","target_phase":"world_building"}"#,
                "confirmed_by_user": false
            }),
        )
        .await
        .expect_err("unconfirmed write must fail");
        assert_eq!(
            missing_confirmation,
            "user confirmation is required for this autopilot tool"
        );
        assert_eq!(
            get_state(&db, "project-1", "owner-1")
                .await
                .expect("load unchanged state")
                .phase,
            NovelWorkflowPhase::Foundation
        );
    }

    #[tokio::test]
    async fn stale_transition_records_a_redacted_failed_audit_without_updating_workflow() {
        let db = setup_workflow_db().await;
        insert_project(&db, "project-1", "owner-1", "world_building").await;
        let record = task_record("project-1", "owner-1");
        let payload = transition_payload("project-1");
        queue_audit(&db, &record, &payload).await;

        let error = execute_novel_autopilot_task(&db, &record, payload)
            .await
            .expect_err("stale expected phase must fail");
        assert_eq!(
            error,
            "stale expected workflow phase: expected foundation, actual world_building"
        );

        let audit = list_project_autopilot_invocation_audits(&db, "project-1", 1)
            .await
            .expect("read failed audit")
            .pop()
            .expect("one audit row");
        assert_eq!(audit.status, "failed");
        assert_eq!(audit.error_code.as_deref(), Some("stale_expected_phase"));
        assert_eq!(
            get_state(&db, "project-1", "owner-1")
                .await
                .expect("workflow stays unchanged")
                .phase,
            NovelWorkflowPhase::WorldBuilding
        );
    }

    #[tokio::test]
    async fn strict_payload_rejects_unknown_fields_and_unknown_tool_without_leaking_arguments() {
        let db = setup_workflow_db().await;
        insert_project(&db, "project-1", "owner-1", "foundation").await;

        let invalid_payload = execute_novel_autopilot_task(
            &db,
            &task_record("project-1", "owner-1"),
            json!({
                "tool_name": "transition_project_workflow",
                "arguments": "{}",
                "confirmed_by_user": true,
                "user_id": "attacker"
            }),
        )
        .await
        .expect_err("unknown task payload field must fail");
        assert_eq!(invalid_payload, INVALID_AUTOPILOT_TASK_PAYLOAD);

        let unknown_tool = execute_novel_autopilot_task(
            &db,
            &task_record("project-1", "owner-1"),
            json!({
                "tool_name": "drop_project_table",
                "arguments": r#"{"secret":"do-not-leak"}"#,
                "confirmed_by_user": true
            }),
        )
        .await
        .expect_err("unknown tool must fail");
        assert_eq!(unknown_tool, INVALID_AUTOPILOT_TASK_PAYLOAD);
        assert!(!unknown_tool.contains("do-not-leak"));
    }
    #[tokio::test]
    async fn succeeded_audit_projection_failure_rolls_back_workflow_and_records_safe_fallback() {
        let db = setup_workflow_db().await;
        insert_project(
            &db,
            safety_fixture::PROJECT_ID,
            safety_fixture::OWNER_ID,
            "foundation",
        )
        .await;
        let mut record = task_record(safety_fixture::PROJECT_ID, safety_fixture::OWNER_ID);
        record.task_id = safety_fixture::TASK_ID.to_string();
        let payload = safety_fixture::confirmed_transition_payload(safety_fixture::PROJECT_ID);
        queue_audit(&db, &record, &payload).await;

        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            format!(
                "CREATE TRIGGER g2_delete_audit_before_success \
                 AFTER UPDATE OF status ON projects \
                 BEGIN DELETE FROM autopilot_invocation_audits WHERE task_id = '{}'; END",
                safety_fixture::TASK_ID
            ),
        ))
        .await
        .expect("install test-only audit failure trigger");

        let error = execute_novel_autopilot_task(&db, &record, payload)
            .await
            .expect_err("succeeded audit write failure must surface a safe task error");
        assert_eq!(error, "autopilot task execution failed");
        assert_eq!(
            get_state(&db, safety_fixture::PROJECT_ID, safety_fixture::OWNER_ID)
                .await
                .expect("workflow state remains readable")
                .phase,
            NovelWorkflowPhase::Foundation
        );
        let audits = list_project_autopilot_invocation_audits(&db, safety_fixture::PROJECT_ID, 10)
            .await
            .expect("fallback audit remains readable");
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].status, "failed");
        assert_eq!(
            audits[0].error_code.as_deref(),
            Some(safety_fixture::TERMINAL_AUDIT_FAILURE_CODE)
        );
    }
}
