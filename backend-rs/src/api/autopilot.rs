use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::{de::Error as _, Deserialize, Deserializer};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::api::background_tasks::{
    create_task_for_authenticated_user, AuthenticatedTaskCreateError,
};
use crate::services::auth::Claims;
use crate::services::autopilot_invocation_audit_service::{
    list_project_autopilot_invocation_audits, AutopilotInvocationAuditError,
    AutopilotInvocationAuditReadModel,
};
use crate::services::autopilot_tool_contract_service::{
    parse_transition_project_workflow_args, AutopilotToolName,
};
use crate::services::book_import_service::BookImportService;
use crate::services::novel_workflow_service::NovelWorkflowPhase;
use crate::services::project_service::{ProjectAccessQueryError, ProjectService};
use crate::tasks::registry::TaskRegistry;
use crate::tasks::stream::TaskStreamHub;
use crate::tasks::types::TaskCreateRequest;

const AUTOPILOT_ACTIONS_ROUTE: &str = "/projects/{project_id}/autopilot/actions";
const AUTOPILOT_INVOCATION_AUDITS_ROUTE: &str = "/projects/{project_id}/autopilot/invocations";
const AUTOPILOT_INVOCATION_AUDIT_LIST_LIMIT: u64 = 50;
const NOVEL_AUTOPILOT_TASK_TYPE: &str = "novel_autopilot";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NovelAutopilotActionRequest {
    tool_name: String,
    arguments: TransitionProjectWorkflowActionRequest,
    confirmed_by_user: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionProjectWorkflowActionRequest {
    #[serde(deserialize_with = "deserialize_canonical_workflow_phase")]
    expected_phase: NovelWorkflowPhase,
    #[serde(deserialize_with = "deserialize_canonical_workflow_phase")]
    target_phase: NovelWorkflowPhase,
    reason: Option<String>,
    related_task_id: Option<String>,
}

fn deserialize_canonical_workflow_phase<'de, D>(
    deserializer: D,
) -> Result<NovelWorkflowPhase, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    let phase = value
        .parse::<NovelWorkflowPhase>()
        .map_err(D::Error::custom)?;
    if value != phase.as_str() {
        return Err(D::Error::custom(
            "workflow phase must use a canonical public value",
        ));
    }
    Ok(phase)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BuildNovelAutopilotTaskPayloadError {
    ConfirmationRequired,
    UnsupportedTool,
    Serialization,
}

fn build_novel_autopilot_task_payload(
    project_id: &str,
    request: NovelAutopilotActionRequest,
) -> Result<Value, BuildNovelAutopilotTaskPayloadError> {
    if !request.confirmed_by_user {
        return Err(BuildNovelAutopilotTaskPayloadError::ConfirmationRequired);
    }

    let tool_name = AutopilotToolName::parse(&request.tool_name)
        .map_err(|_| BuildNovelAutopilotTaskPayloadError::UnsupportedTool)?;
    if tool_name != AutopilotToolName::TransitionProjectWorkflow {
        return Err(BuildNovelAutopilotTaskPayloadError::UnsupportedTool);
    }

    let arguments = json!({
        "project_id": project_id,
        "expected_phase": request.arguments.expected_phase,
        "target_phase": request.arguments.target_phase,
        "reason": request.arguments.reason,
        "related_task_id": request.arguments.related_task_id,
    });
    parse_transition_project_workflow_args(arguments.clone())
        .map_err(|_| BuildNovelAutopilotTaskPayloadError::Serialization)?;
    let arguments = serde_json::to_string(&arguments)
        .map_err(|_| BuildNovelAutopilotTaskPayloadError::Serialization)?;

    Ok(json!({
        "tool_name": tool_name.as_str(),
        "arguments": arguments,
        "confirmed_by_user": true,
    }))
}

fn map_build_novel_autopilot_task_payload_error(
    error: BuildNovelAutopilotTaskPayloadError,
) -> (StatusCode, Json<Value>) {
    match error {
        BuildNovelAutopilotTaskPayloadError::ConfirmationRequired => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "detail": "User confirmation is required for this autopilot action",
                "code": "confirmation_required",
            })),
        ),
        BuildNovelAutopilotTaskPayloadError::UnsupportedTool => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "detail": "Unsupported autopilot action",
                "code": "unsupported_autopilot_action",
            })),
        ),
        BuildNovelAutopilotTaskPayloadError::Serialization => {
            tracing::error!("autopilot control request could not be serialized into tool payload");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "detail": "Unable to create autopilot task",
                    "code": "autopilot_task_creation_failed",
                })),
            )
        }
    }
}

fn map_project_access_error(error: ProjectAccessQueryError) -> (StatusCode, Json<Value>) {
    match error {
        ProjectAccessQueryError::NotFoundOrAccessDenied => (
            StatusCode::NOT_FOUND,
            Json(json!({"detail": "Project not found"})),
        ),
        ProjectAccessQueryError::Internal(detail) => {
            tracing::error!(error = %detail, "autopilot control project access check failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "detail": "Unable to create autopilot task",
                    "code": "autopilot_task_creation_failed",
                })),
            )
        }
    }
}

fn map_autopilot_invocation_audit_error(
    error: AutopilotInvocationAuditError,
) -> (StatusCode, Json<Value>) {
    tracing::error!(
        event = "autopilot_invocation_audit_read_failed",
        error_code = error.code(),
        "autopilot invocation audit query failed"
    );
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "detail": "Unable to read autopilot invocation history",
            "code": "autopilot_invocation_audit_unavailable",
        })),
    )
}

fn map_task_create_error(error: AuthenticatedTaskCreateError) -> (StatusCode, Json<Value>) {
    match error {
        AuthenticatedTaskCreateError::ProjectRequired => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "detail": "project_id is required for this task type",
                "code": "project_id_required",
            })),
        ),
        AuthenticatedTaskCreateError::AutopilotAuditUnavailable => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "detail": "Unable to create autopilot task",
                "code": "autopilot_task_creation_failed",
            })),
        ),
    }
}

async fn list_project_autopilot_invocations(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    ProjectService::ensure_owned_access(&db, &project_id, &claims.sub)
        .await
        .map_err(map_project_access_error)?;

    let items = list_project_autopilot_invocation_audits(
        &db,
        &project_id,
        AUTOPILOT_INVOCATION_AUDIT_LIST_LIMIT,
    )
    .await
    .map_err(map_autopilot_invocation_audit_error)?;

    let items = items
        .into_iter()
        .map(project_autopilot_invocation_history_item)
        .collect::<Vec<_>>();
    Ok(Json(json!({"items": items})))
}

/// Projects the internal audit record into the explicit, UI-safe history contract.
///
/// The audit service retains actor, project and digest fields for durable operational
/// records; the project workflow history endpoint must not expose those internals.
fn project_autopilot_invocation_history_item(record: AutopilotInvocationAuditReadModel) -> Value {
    json!({
        "audit_id": record.audit_id,
        "tool_name": record.tool_name,
        "tool_schema_version": record.tool_schema_version,
        "confirmed_by_user": record.confirmed_by_user,
        "execution_mode": record.execution_mode,
        "input_summary": record.input_summary,
        "status": record.status,
        "result_summary": record.result_summary,
        "error_code": record.error_code,
        "created_at": record.created_at,
        "started_at": record.started_at,
        "completed_at": record.completed_at,
    })
}

async fn create_autopilot_action(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Extension(registry): Extension<TaskRegistry>,
    Extension(stream_hub): Extension<TaskStreamHub>,
    Extension(book_import_service): Extension<Arc<BookImportService>>,
    Path(project_id): Path<String>,
    Json(request): Json<NovelAutopilotActionRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let payload = build_novel_autopilot_task_payload(&project_id, request)
        .map_err(map_build_novel_autopilot_task_payload_error)?;

    ProjectService::ensure_owned_access(&db, &project_id, &claims.sub)
        .await
        .map_err(map_project_access_error)?;

    let response = create_task_for_authenticated_user(
        db,
        registry,
        stream_hub,
        book_import_service,
        &claims.sub,
        TaskCreateRequest {
            task_type: NOVEL_AUTOPILOT_TASK_TYPE.to_string(),
            project_id,
            payload: Some(payload),
            stage_code: None,
            execution_mode: "interactive".to_string(),
            workflow_scope: None,
            checkpoint: None,
        },
    )
    .await
    .map_err(map_task_create_error)?;

    Ok((response.status, Json(response.payload)))
}

pub fn routes() -> Router {
    Router::new()
        .route(AUTOPILOT_ACTIONS_ROUTE, post(create_autopilot_action))
        .route(
            AUTOPILOT_INVOCATION_AUDITS_ROUTE,
            get(list_project_autopilot_invocations),
        )
}

#[cfg(test)]
mod tests {
    use axum::{
        extract::{Extension, Path},
        http::StatusCode,
        Json,
    };
    use chrono::NaiveDate;
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, Schema, Set,
    };
    use serde_json::json;
    use std::sync::Arc;

    use super::{
        build_novel_autopilot_task_payload, create_autopilot_action,
        list_project_autopilot_invocations, BuildNovelAutopilotTaskPayloadError,
        NovelAutopilotActionRequest,
    };
    use crate::models::{autopilot_invocation_audit, project};
    use crate::services::auth::Claims;
    use crate::services::book_import_service::BookImportService;
    use crate::services::novel_workflow_service::{get_state, NovelWorkflowPhase};
    use crate::tasks::registry::TaskRegistry;
    use crate::tasks::stream::TaskStreamHub;
    use crate::tasks::types::TaskStatus;

    async fn setup_project_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect in-memory SQLite database");
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

    async fn setup_project_db_without_autopilot_audit() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect in-memory SQLite database");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        db.execute(builder.build(&schema.create_table_from_entity(project::Entity)))
            .await
            .expect("create projects table without audit table");
        db
    }

    async fn insert_project(db: &DatabaseConnection, id: &str, user_id: &str) {
        let created_at = NaiveDate::from_ymd_opt(2026, 7, 16)
            .expect("valid test date")
            .and_hms_opt(8, 0, 0)
            .expect("valid test time");
        project::ActiveModel {
            id: Set(id.to_string()),
            user_id: Set(user_id.to_string()),
            title: Set(format!("Project {id}")),
            target_words: Set(100_000),
            current_words: Set(0),
            status: Set("foundation".to_string()),
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
        .expect("insert test project");
    }

    fn claims(user_id: &str) -> Claims {
        Claims {
            sub: user_id.to_string(),
            username: user_id.to_string(),
            is_admin: false,
            exp: 0,
            iat: 0,
        }
    }

    fn valid_request() -> NovelAutopilotActionRequest {
        serde_json::from_value(json!({
            "tool_name": "transition_project_workflow",
            "arguments": {
                "expected_phase": "foundation",
                "target_phase": "world_building",
                "reason": "User confirmed this transition",
                "related_task_id": null
            },
            "confirmed_by_user": true
        }))
        .expect("valid request should deserialize")
    }

    #[test]
    fn request_contract_rejects_injected_scope_actor_unknown_tool_and_invalid_phase() {
        for request in [
            json!({
                "tool_name": "transition_project_workflow",
                "arguments": {"expected_phase": "foundation", "target_phase": "world_building"},
                "confirmed_by_user": true,
                "project_id": "attacker-project"
            }),
            json!({
                "tool_name": "transition_project_workflow",
                "arguments": {"expected_phase": "foundation", "target_phase": "world_building", "user_id": "attacker"},
                "confirmed_by_user": true
            }),
            json!({
                "tool_name": "transition_project_workflow",
                "arguments": {"expected_phase": "mystery", "target_phase": "world_building"},
                "confirmed_by_user": true
            }),
            json!({
                "tool_name": "transition_project_workflow",
                "arguments": {"expected_phase": "planning", "target_phase": "world_building"},
                "confirmed_by_user": true
            }),
        ] {
            assert!(
                serde_json::from_value::<NovelAutopilotActionRequest>(request).is_err(),
                "strict request contract must reject injected or invalid fields"
            );
        }

        let unknown_tool = serde_json::from_value(json!({
            "tool_name": "delete_project",
            "arguments": {"expected_phase": "foundation", "target_phase": "world_building"},
            "confirmed_by_user": true
        }))
        .expect("unknown tool is parsed then rejected by allowlist");
        assert_eq!(
            build_novel_autopilot_task_payload("project-1", unknown_tool),
            Err(BuildNovelAutopilotTaskPayloadError::UnsupportedTool)
        );
    }

    #[test]
    fn payload_builder_requires_explicit_confirmation_and_injects_only_route_scope() {
        let missing_confirmation: NovelAutopilotActionRequest = serde_json::from_value(json!({
            "tool_name": "transition_project_workflow",
            "arguments": {"expected_phase": "foundation", "target_phase": "world_building"},
            "confirmed_by_user": false
        }))
        .expect("false confirmation remains a valid DTO value");
        assert_eq!(
            build_novel_autopilot_task_payload("project-1", missing_confirmation),
            Err(BuildNovelAutopilotTaskPayloadError::ConfirmationRequired)
        );

        let payload = build_novel_autopilot_task_payload("route-project", valid_request())
            .expect("confirmed request should build internal payload");
        let arguments = payload["arguments"]
            .as_str()
            .expect("arguments are serialized for the Coordinator");
        let arguments: serde_json::Value =
            serde_json::from_str(arguments).expect("serialized arguments are JSON");
        assert_eq!(arguments["project_id"], "route-project");
        assert!(payload["confirmed_by_user"] == true);
    }

    #[tokio::test]
    async fn confirmed_project_owner_creates_scoped_generic_autopilot_task() {
        let db = setup_project_db().await;
        insert_project(&db, "project-1", "owner-1").await;
        let registry = TaskRegistry::new();
        let stream_hub = TaskStreamHub::new();

        let (status, Json(response)) = create_autopilot_action(
            Extension(claims("owner-1")),
            Extension(db.clone()),
            Extension(registry.clone()),
            Extension(stream_hub),
            Extension(Arc::new(BookImportService::new())),
            Path("project-1".to_string()),
            Json(valid_request()),
        )
        .await
        .expect("owner may create an autopilot task");

        assert_eq!(status, StatusCode::CREATED);
        let task_id = response["task_id"]
            .as_str()
            .expect("generic task response includes task_id");
        let record = registry.get(task_id).await.expect("task is registered");
        assert_eq!(record.task_type, "novel_autopilot");
        assert_eq!(record.user_id, "owner-1");
        assert_eq!(record.project_id, "project-1");

        let completed = tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                let current = registry
                    .get(task_id)
                    .await
                    .expect("task remains registered");
                if matches!(current.status, TaskStatus::Completed | TaskStatus::Failed) {
                    return current;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("autopilot task should complete promptly");
        assert_eq!(completed.status, TaskStatus::Completed);
        assert_eq!(
            completed
                .result
                .as_ref()
                .and_then(|result| result.get("schema_version"))
                .and_then(serde_json::Value::as_str),
            Some("autopilot-tool-contract/v1")
        );
        assert_eq!(
            get_state(&db, "project-1", "owner-1")
                .await
                .expect("workflow state is readable")
                .phase,
            NovelWorkflowPhase::WorldBuilding
        );

        let Json(owner_history) = list_project_autopilot_invocations(
            Extension(claims("owner-1")),
            Extension(db.clone()),
            Path("project-1".to_string()),
        )
        .await
        .expect("project owner may read invocation history");
        let item = &owner_history["items"][0];
        assert_eq!(item["status"], "succeeded");
        assert_eq!(item["execution_mode"], "direct_business_tool");
        assert_eq!(item["input_summary"]["reason_provided"], true);
        assert_eq!(item["input_summary"]["related_task_id_provided"], false);
        assert_eq!(item["result_summary"]["current_phase"], "world_building");
        assert!(item["input_summary"].get("reason").is_none());
        assert!(item["input_summary"].get("related_task_id").is_none());
        assert!(item["result_summary"].get("reason").is_none());
        for internal_field in [
            "task_id",
            "project_id",
            "actor_user_id",
            "schema_version",
            "provider_name",
            "model_name",
            "prompt_digest",
            "input_digest",
        ] {
            assert!(
                item.get(internal_field).is_none(),
                "history API must not expose internal audit field: {internal_field}"
            );
        }

        let denied = list_project_autopilot_invocations(
            Extension(claims("attacker-1")),
            Extension(db),
            Path("project-1".to_string()),
        )
        .await
        .expect_err("non-owner must not read project invocation history");
        assert_eq!(denied.0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unauthorized_actor_cannot_create_project_scoped_task() {
        let db = setup_project_db().await;
        insert_project(&db, "project-1", "owner-1").await;
        let registry = TaskRegistry::new();

        let error = create_autopilot_action(
            Extension(claims("attacker-1")),
            Extension(db),
            Extension(registry.clone()),
            Extension(TaskStreamHub::new()),
            Extension(Arc::new(BookImportService::new())),
            Path("project-1".to_string()),
            Json(valid_request()),
        )
        .await
        .expect_err("unrelated actor must not create an autopilot task");

        assert_eq!(error.0, StatusCode::NOT_FOUND);
        assert!(
            registry
                .list_for_user("attacker-1", None, None, false, None)
                .await
                .is_empty(),
            "authorization failure must occur before generic task creation"
        );
    }
    #[tokio::test]
    async fn queued_audit_write_failure_does_not_create_task_or_mutate_workflow() {
        let db = setup_project_db_without_autopilot_audit().await;
        insert_project(&db, "project-1", "owner-1").await;
        let registry = TaskRegistry::new();

        let error = create_autopilot_action(
            Extension(claims("owner-1")),
            Extension(db.clone()),
            Extension(registry.clone()),
            Extension(TaskStreamHub::new()),
            Extension(Arc::new(BookImportService::new())),
            Path("project-1".to_string()),
            Json(valid_request()),
        )
        .await
        .expect_err("missing audit table must reject task creation safely");

        assert_eq!(error.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.1 .0["code"], "autopilot_task_creation_failed");
        assert!(
            registry
                .list_for_user("owner-1", Some("project-1"), None, false, None)
                .await
                .is_empty(),
            "queued audit failure must occur before generic task registration"
        );
        assert_eq!(
            get_state(&db, "project-1", "owner-1")
                .await
                .expect("workflow state remains readable")
                .phase,
            NovelWorkflowPhase::Foundation
        );
    }

    #[tokio::test]
    async fn owner_history_read_failure_is_safe_and_non_mutating() {
        let db = setup_project_db_without_autopilot_audit().await;
        insert_project(&db, "project-1", "owner-1").await;

        let error = list_project_autopilot_invocations(
            Extension(claims("owner-1")),
            Extension(db.clone()),
            Path("project-1".to_string()),
        )
        .await
        .expect_err("missing audit table must not expose a partial history");

        assert_eq!(error.0, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.1 .0["code"], "autopilot_invocation_audit_unavailable");
        assert_eq!(
            get_state(&db, "project-1", "owner-1")
                .await
                .expect("workflow state remains readable")
                .phase,
            NovelWorkflowPhase::Foundation
        );
    }
}
