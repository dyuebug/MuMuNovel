use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, Schema, Set,
};
use serde_json::json;

use crate::models::project;
use crate::services::novel_workflow_service::{get_state, NovelWorkflowPhase};

use super::dispatch_owner::{
    dispatch_autopilot_tool, dispatch_autopilot_tool_call, AutopilotToolConfirmation,
    AutopilotToolExecutionContext,
};
use super::schema_owner::{
    autopilot_tool_definitions, AutopilotToolContractError, AutopilotToolName,
    AUTOPILOT_TOOL_CONTRACT_SCHEMA_VERSION,
};

fn confirmed_context<'a>(actor_user_id: &'a str) -> AutopilotToolExecutionContext<'a> {
    AutopilotToolExecutionContext {
        actor_user_id,
        confirmation: AutopilotToolConfirmation::ConfirmedByUser,
        project_scope: None,
    }
}

async fn setup_workflow_db() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("connect workflow sqlite memory db");
    let builder = DbBackend::Sqlite;
    let schema = Schema::new(builder);
    db.execute(builder.build(&schema.create_table_from_entity(project::Entity)))
        .await
        .expect("create projects table");
    db
}

async fn insert_project(db: &DatabaseConnection, id: &str, user_id: &str, status: &str) {
    let created_at = chrono::NaiveDate::from_ymd_opt(2026, 7, 16)
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

fn transition_arguments() -> serde_json::Value {
    json!({
        "project_id": "project-1",
        "expected_phase": "foundation",
        "target_phase": "world_building",
        "reason": "用户确认基础设定完成",
        "related_task_id": "decision-1"
    })
}

#[test]
fn exposes_only_the_static_transition_tool_with_strict_schema() {
    let definitions = autopilot_tool_definitions();
    assert_eq!(definitions.len(), 1);

    let definition = &definitions[0];
    assert_eq!(definition.tool_type, "function");
    assert_eq!(
        definition.function.name,
        AutopilotToolName::TransitionProjectWorkflow.as_str()
    );
    assert_eq!(definition.function.parameters["type"], "object");
    assert_eq!(
        definition.function.parameters["additionalProperties"],
        false
    );
    assert_eq!(
        definition.function.parameters["required"],
        json!(["project_id", "expected_phase", "target_phase"])
    );
    assert_eq!(
        definition.function.parameters["properties"]["expected_phase"]["enum"],
        json!(NovelWorkflowPhase::ALL
            .into_iter()
            .map(NovelWorkflowPhase::as_str)
            .collect::<Vec<_>>())
    );
}

#[tokio::test]
async fn rejects_unknown_tool_and_invalid_arguments_before_workflow_mutation() {
    let db = setup_workflow_db().await;
    insert_project(&db, "project-1", "owner-1", "foundation").await;

    assert_eq!(
        dispatch_autopilot_tool(
            &db,
            confirmed_context("owner-1"),
            "unknown_tool",
            transition_arguments(),
        )
        .await,
        Err(AutopilotToolContractError::UnknownTool)
    );

    for invalid_arguments in [
        json!(["not", "an", "object"]),
        json!({
            "project_id": "project-1",
            "expected_phase": "foundation",
            "target_phase": "world_building",
            "user_id": "attacker"
        }),
        json!({
            "project_id": "   ",
            "expected_phase": "foundation",
            "target_phase": "world_building"
        }),
        json!({
            "project_id": "project-1",
            "expected_phase": "foundation",
            "target_phase": "invented_phase"
        }),
    ] {
        assert_eq!(
            dispatch_autopilot_tool(
                &db,
                confirmed_context("owner-1"),
                "transition_project_workflow",
                invalid_arguments,
            )
            .await,
            Err(AutopilotToolContractError::InvalidArguments)
        );
    }

    assert_eq!(
        dispatch_autopilot_tool_call(
            &db,
            confirmed_context("owner-1"),
            "transition_project_workflow",
            "{not-json",
        )
        .await,
        Err(AutopilotToolContractError::InvalidArguments)
    );

    assert_eq!(
        get_state(&db, "project-1", "owner-1")
            .await
            .expect("load state")
            .phase,
        NovelWorkflowPhase::Foundation
    );
}

#[tokio::test]
async fn requires_confirmation_without_calling_workflow_service() {
    let db = setup_workflow_db().await;
    insert_project(&db, "project-1", "owner-1", "foundation").await;

    assert_eq!(
        dispatch_autopilot_tool(
            &db,
            AutopilotToolExecutionContext {
                actor_user_id: "owner-1",
                confirmation: AutopilotToolConfirmation::Missing,
                project_scope: None,
            },
            "transition_project_workflow",
            transition_arguments(),
        )
        .await,
        Err(AutopilotToolContractError::ConfirmationRequired)
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
async fn confirmed_tool_delegates_to_canonical_workflow_service() {
    let db = setup_workflow_db().await;
    insert_project(&db, "project-1", "owner-1", "foundation").await;

    let result = dispatch_autopilot_tool(
        &db,
        confirmed_context("owner-1"),
        "transition_project_workflow",
        transition_arguments(),
    )
    .await
    .expect("confirmed transition succeeds");

    assert_eq!(
        result.schema_version,
        AUTOPILOT_TOOL_CONTRACT_SCHEMA_VERSION
    );
    assert_eq!(
        result.tool_name,
        AutopilotToolName::TransitionProjectWorkflow
    );
    assert!(result.receipt.changed);
    assert_eq!(
        result.receipt.previous_phase,
        NovelWorkflowPhase::Foundation
    );
    assert_eq!(
        result.receipt.state.phase,
        NovelWorkflowPhase::WorldBuilding
    );
}

#[tokio::test]
async fn preserves_workflow_access_and_cas_errors_without_internal_details() {
    let db = setup_workflow_db().await;
    insert_project(&db, "project-1", "owner-1", "writing").await;

    assert_eq!(
        dispatch_autopilot_tool(
            &db,
            confirmed_context("other-user"),
            "transition_project_workflow",
            json!({
                "project_id": "project-1",
                "expected_phase": "writing",
                "target_phase": "reviewing"
            }),
        )
        .await,
        Err(AutopilotToolContractError::NotFoundOrAccessDenied)
    );

    let error = dispatch_autopilot_tool(
        &db,
        confirmed_context("owner-1"),
        "transition_project_workflow",
        json!({
            "project_id": "project-1",
            "expected_phase": "foundation",
            "target_phase": "world_building",
            "reason": "super-secret-prompt",
            "related_task_id": "https://secret.example"
        }),
    )
    .await
    .expect_err("stale phase is surfaced");

    assert_eq!(
        error,
        AutopilotToolContractError::StaleExpectedPhase {
            expected: NovelWorkflowPhase::Foundation,
            actual: NovelWorkflowPhase::Writing,
        }
    );
    let rendered = format!("{error:?}").to_ascii_lowercase();
    for forbidden in [
        "super-secret-prompt",
        "https://secret.example",
        "api_key",
        "authorization",
    ] {
        assert!(!rendered.contains(forbidden));
    }
}

#[tokio::test]
async fn task_project_scope_must_match_tool_arguments_before_workflow_mutation() {
    let db = setup_workflow_db().await;
    insert_project(&db, "project-1", "owner-1", "foundation").await;

    let context = AutopilotToolExecutionContext {
        actor_user_id: "owner-1",
        confirmation: AutopilotToolConfirmation::ConfirmedByUser,
        project_scope: Some("project-2"),
    };
    assert_eq!(
        dispatch_autopilot_tool(
            &db,
            context,
            "transition_project_workflow",
            transition_arguments(),
        )
        .await,
        Err(AutopilotToolContractError::ProjectScopeMismatch)
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
async fn task_project_scope_allows_matching_canonical_project() {
    let db = setup_workflow_db().await;
    insert_project(&db, "project-1", "owner-1", "foundation").await;

    let result = dispatch_autopilot_tool(
        &db,
        AutopilotToolExecutionContext {
            actor_user_id: "owner-1",
            confirmation: AutopilotToolConfirmation::ConfirmedByUser,
            project_scope: Some("project-1"),
        },
        "transition_project_workflow",
        transition_arguments(),
    )
    .await
    .expect("matching task scope succeeds");

    assert!(result.receipt.changed);
    assert_eq!(
        result.receipt.state.phase,
        NovelWorkflowPhase::WorldBuilding
    );
}
