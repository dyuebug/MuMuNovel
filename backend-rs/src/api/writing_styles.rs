use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::services::auth::Claims;
use crate::services::writing_style_service::{
    build_create_writing_style_request_from_route_payload,
    build_update_writing_style_request_from_route_payload, CreateWritingStyleRequest,
    UpdateWritingStyleRequest, WritingStyleService,
};

const WRITING_STYLES_PRESETS_ROUTE: &str = "/writing-styles/presets/list";
const WRITING_STYLES_USER_ROUTE: &str = "/writing-styles/user";
const WRITING_STYLES_PROJECT_ROUTE: &str = "/writing-styles/project/{project_id}";
const WRITING_STYLES_PROJECT_INITIALIZE_ROUTE: &str =
    "/writing-styles/project/{project_id}/initialize";
const WRITING_STYLES_PROJECT_INIT_DEFAULTS_ROUTE: &str =
    "/writing-styles/project/{project_id}/init-defaults";
const WRITING_STYLES_CREATE_ROUTE: &str = "/writing-styles";
const WRITING_STYLES_DETAIL_ROUTE: &str = "/writing-styles/{style_id}";
const WRITING_STYLES_SET_DEFAULT_ROUTE: &str = "/writing-styles/{style_id}/set-default";

#[cfg(test)]
fn build_writing_styles_route_owner_contract() -> Value {
    json!({
        "owner": "writing_styles",
        "rust_owner": "backend-rs/src/api/writing_styles.rs",
        "routes": {
            "presets": WRITING_STYLES_PRESETS_ROUTE,
            "user": WRITING_STYLES_USER_ROUTE,
            "project": WRITING_STYLES_PROJECT_ROUTE,
            "project_initialize": WRITING_STYLES_PROJECT_INITIALIZE_ROUTE,
            "project_init_defaults": WRITING_STYLES_PROJECT_INIT_DEFAULTS_ROUTE,
            "create": WRITING_STYLES_CREATE_ROUTE,
            "detail": WRITING_STYLES_DETAIL_ROUTE,
            "update": WRITING_STYLES_DETAIL_ROUTE,
            "delete": WRITING_STYLES_DETAIL_ROUTE,
            "set_default": WRITING_STYLES_SET_DEFAULT_ROUTE
        },
        "methods": {
            "presets": ["GET"],
            "user": ["GET"],
            "project": ["GET"],
            "project_initialize": ["POST"],
            "project_init_defaults": ["POST"],
            "create": ["POST"],
            "detail": ["GET", "PUT", "DELETE"],
            "set_default": ["POST"]
        },
        "service_owners": [
            "backend-rs/src/services/writing_style_service.rs",
            "backend-rs/src/models/writing_style.rs",
            "backend-rs/src/models/project_default_style.rs"
        ],
        "readiness_probes": [
            "writing-styles-user-auth-guard-rust",
            "writing-styles-project-auth-guard-rust",
            "writing-styles-setup-project-business-rust",
            "writing-styles-presets-business-rust",
            "writing-styles-user-list-business-rust",
            "writing-styles-project-list-business-rust",
            "writing-styles-project-initialize-business-rust",
            "writing-styles-create-business-rust",
            "writing-styles-detail-business-rust",
            "writing-styles-update-business-rust",
            "writing-styles-set-default-business-rust",
            "writing-styles-project-list-after-default-business-rust",
            "writing-styles-reset-default-to-preset-business-rust",
            "writing-styles-delete-business-rust",
            "writing-styles-missing-detail-business-rust"
        ],
        "owner_profile": {
            "name": "phase5-writing-styles-business-owner",
            "business_probes": [
                "writing-styles-setup-project-business-rust",
                "writing-styles-presets-business-rust",
                "writing-styles-user-list-business-rust",
                "writing-styles-project-list-business-rust",
                "writing-styles-project-initialize-business-rust",
                "writing-styles-create-business-rust",
                "writing-styles-detail-business-rust",
                "writing-styles-update-business-rust",
                "writing-styles-set-default-business-rust",
                "writing-styles-project-list-after-default-business-rust",
                "writing-styles-reset-default-to-preset-business-rust",
                "writing-styles-delete-business-rust",
                "writing-styles-missing-detail-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "source_map_files": [
            "backend/app/api/writing_styles.py",
            "backend/app/models/writing_style.py",
            "backend/app/models/project_default_style.py",
            "backend/app/schemas/writing_style.py",
            "backend/app/services/writing_style_sync_service.py"
        ],
        "rollback_boundary": {
            "source_map_policy": "keep_python_writing_styles_route_model_schema_sync_files_as_source_map_until_explicit_freeze_delete_round",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": false,
            "python_fallback_removal_ready": false,
            "remaining_blockers": [
                "explicit source-map freeze/delete/repoint approval"
            ],
            "freeze_reason": "Rust writing_styles route group has dedicated phase5-writing-styles-business-owner probes for setup, presets, user/project list, initialize, create/detail/update, set-default, reset-default, delete, and missing-detail behavior; final Python source-map freeze/delete/repoint still requires explicit approval and rollback policy."
        },
        "business_smoke_status": {
            "owner_profile": "phase5-writing-styles-business-owner",
            "readiness_probe_count": 15,
            "business_probe_count": 13,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Writing styles route business smoke is covered by phase5-writing-styles-business-owner; final completion now requires explicit source-map freeze/delete/repoint approval with same-round rollback policy."
    })
}

#[derive(Deserialize, Default, Clone, Debug)]
pub struct SetDefaultStyleRouteQuery {
    pub project_id: Option<String>,
}

#[derive(Deserialize, Default, Clone, Debug)]
pub struct SetDefaultStyleRouteBody {
    pub project_id: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum BuildSetDefaultStyleRequestError {
    MissingProjectId,
}

fn build_set_default_style_project_id(
    query: SetDefaultStyleRouteQuery,
    body: Option<SetDefaultStyleRouteBody>,
) -> Result<String, BuildSetDefaultStyleRequestError> {
    let project_id = body
        .and_then(|payload| payload.project_id)
        .or(query.project_id)
        .unwrap_or_default();

    if project_id.is_empty() {
        return Err(BuildSetDefaultStyleRequestError::MissingProjectId);
    }

    Ok(project_id)
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct CreateWritingStyleRouteRequest {
    #[serde(default)]
    pub preset_id: Option<Value>,
    #[serde(default)]
    pub name: Option<Value>,
    #[serde(default)]
    pub description: Option<Value>,
    #[serde(default)]
    pub prompt_content: Option<Value>,
    #[serde(default)]
    pub style_type: Option<Value>,
}

impl CreateWritingStyleRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "preset_id": self.preset_id,
            "name": self.name,
            "description": self.description,
            "prompt_content": self.prompt_content,
            "style_type": self.style_type,
        })
    }
}

#[derive(Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct UpdateWritingStyleRouteRequest {
    #[serde(default)]
    pub name: Option<Value>,
    #[serde(default)]
    pub description: Option<Value>,
    #[serde(default)]
    pub prompt_content: Option<Value>,
    #[serde(default)]
    pub order_index: Option<Value>,
}

impl UpdateWritingStyleRouteRequest {
    fn into_body(self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "prompt_content": self.prompt_content,
            "order_index": self.order_index,
        })
    }
}

fn build_create_writing_style_request_from_typed_route_payload(
    route_request: CreateWritingStyleRouteRequest,
) -> CreateWritingStyleRequest {
    build_create_writing_style_request_from_route_payload(&route_request.into_body())
}

fn build_update_writing_style_request_from_typed_route_payload(
    route_request: UpdateWritingStyleRouteRequest,
) -> UpdateWritingStyleRequest {
    build_update_writing_style_request_from_route_payload(&route_request.into_body())
}

async fn list_presets(
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    WritingStyleService::list_presets(&db)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn list_user_styles(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    WritingStyleService::list_user_styles(&db, &claims.sub)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn list_project_styles(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    WritingStyleService::list_project_styles(&db, &claims.sub, &project_id)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn get_style(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(style_id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    WritingStyleService::get_style(&db, &claims.sub, style_id)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn create_style(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Json(body): Json<CreateWritingStyleRouteRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let request = build_create_writing_style_request_from_typed_route_payload(body);
    WritingStyleService::create_style(&db, &claims.sub, &request)
        .await
        .map(|v| (StatusCode::CREATED, Json(v)))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn update_style(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(style_id): Path<i32>,
    Json(body): Json<UpdateWritingStyleRouteRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let request = build_update_writing_style_request_from_typed_route_payload(body);
    WritingStyleService::update_style(&db, &claims.sub, style_id, &request)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn delete_style(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(style_id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    WritingStyleService::delete_style(&db, &claims.sub, style_id)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn set_default_style(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(style_id): Path<i32>,
    Query(params): Query<SetDefaultStyleRouteQuery>,
    body: Option<Json<SetDefaultStyleRouteBody>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let project_id = build_set_default_style_project_id(params, body.map(|Json(payload)| payload))
        .map_err(|error| match error {
            BuildSetDefaultStyleRequestError::MissingProjectId => (
                StatusCode::BAD_REQUEST,
                Json(json!({"detail": "project_id is required"})),
            ),
        })?;

    WritingStyleService::set_default_style(&db, &claims.sub, style_id, &project_id)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

async fn initialize_defaults(
    Extension(claims): Extension<Claims>,
    Extension(db): Extension<DatabaseConnection>,
    Path(project_id): Path<String>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    WritingStyleService::initialize_defaults(&db, &claims.sub, &project_id)
        .await
        .map(|v| (StatusCode::CREATED, Json(v)))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"detail": format!("{}", e)})),
            )
        })
}

pub fn routes() -> Router {
    Router::new()
        .route(WRITING_STYLES_PRESETS_ROUTE, get(list_presets))
        .route(WRITING_STYLES_USER_ROUTE, get(list_user_styles))
        .route(WRITING_STYLES_PROJECT_ROUTE, get(list_project_styles))
        .route(
            WRITING_STYLES_PROJECT_INITIALIZE_ROUTE,
            post(initialize_defaults),
        )
        .route(
            WRITING_STYLES_PROJECT_INIT_DEFAULTS_ROUTE,
            post(initialize_defaults),
        )
        .route(WRITING_STYLES_CREATE_ROUTE, post(create_style))
        .route(
            WRITING_STYLES_DETAIL_ROUTE,
            get(get_style).put(update_style).delete(delete_style),
        )
        .route(WRITING_STYLES_SET_DEFAULT_ROUTE, post(set_default_style))
}

#[cfg(test)]
mod tests {
    use super::{
        build_create_writing_style_request_from_typed_route_payload,
        build_set_default_style_project_id,
        build_update_writing_style_request_from_typed_route_payload,
        build_writing_styles_route_owner_contract, BuildSetDefaultStyleRequestError,
        CreateWritingStyleRouteRequest, SetDefaultStyleRouteBody, SetDefaultStyleRouteQuery,
        UpdateWritingStyleRouteRequest, WRITING_STYLES_CREATE_ROUTE, WRITING_STYLES_DETAIL_ROUTE,
        WRITING_STYLES_PRESETS_ROUTE, WRITING_STYLES_PROJECT_INITIALIZE_ROUTE,
        WRITING_STYLES_PROJECT_INIT_DEFAULTS_ROUTE, WRITING_STYLES_PROJECT_ROUTE,
        WRITING_STYLES_SET_DEFAULT_ROUTE, WRITING_STYLES_USER_ROUTE,
    };
    use serde_json::json;

    #[test]
    fn build_set_default_style_project_id_prefers_body_over_query() {
        let project_id = build_set_default_style_project_id(
            SetDefaultStyleRouteQuery {
                project_id: Some("project-from-query".to_string()),
            },
            Some(SetDefaultStyleRouteBody {
                project_id: Some("project-from-body".to_string()),
            }),
        )
        .expect("project_id should be built");

        assert_eq!(project_id, "project-from-body");
    }

    #[test]
    fn build_set_default_style_project_id_accepts_query_only() {
        let project_id = build_set_default_style_project_id(
            SetDefaultStyleRouteQuery {
                project_id: Some("project-only".to_string()),
            },
            None,
        )
        .expect("project_id should be built");

        assert_eq!(project_id, "project-only");
    }

    #[test]
    fn build_set_default_style_project_id_rejects_missing_value() {
        let error = build_set_default_style_project_id(
            SetDefaultStyleRouteQuery { project_id: None },
            Some(SetDefaultStyleRouteBody { project_id: None }),
        )
        .expect_err("missing project_id should fail");

        assert_eq!(error, BuildSetDefaultStyleRequestError::MissingProjectId);
    }

    #[test]
    fn build_create_writing_style_request_from_typed_route_payload_keeps_existing_shape() {
        let request = build_create_writing_style_request_from_typed_route_payload(
            CreateWritingStyleRouteRequest {
                preset_id: Some(json!("preset-1")),
                name: Some(json!(" 风格A ")),
                description: Some(json!("描述")),
                prompt_content: Some(json!("正文")),
                style_type: Some(json!(" custom ")),
            },
        );

        assert_eq!(request.preset_id(), Some("preset-1"));
        assert_eq!(request.name(), Some(" 风格A "));
        assert_eq!(request.description(), Some("描述"));
        assert_eq!(request.prompt_content(), Some("正文"));
        assert_eq!(request.style_type(), Some("custom"));
    }

    #[test]
    fn build_update_writing_style_request_from_typed_route_payload_keeps_compat_parsing() {
        let request = build_update_writing_style_request_from_typed_route_payload(
            UpdateWritingStyleRouteRequest {
                name: Some(json!("新标题")),
                description: Some(json!("新描述")),
                prompt_content: Some(json!("新内容")),
                order_index: Some(json!("invalid")),
            },
        );

        assert_eq!(request.name(), Some("新标题"));
        assert_eq!(request.description(), Some("新描述"));
        assert_eq!(request.prompt_content(), Some("新内容"));
        assert_eq!(request.order_index(), None);
    }

    #[test]
    fn should_publish_writing_styles_route_owner_contract() {
        let contract = build_writing_styles_route_owner_contract();

        assert_eq!(contract["owner"], "writing_styles");
        assert_eq!(
            contract["rust_owner"],
            "backend-rs/src/api/writing_styles.rs"
        );
        assert_eq!(contract["routes"]["presets"], WRITING_STYLES_PRESETS_ROUTE);
        assert_eq!(contract["routes"]["user"], WRITING_STYLES_USER_ROUTE);
        assert_eq!(contract["routes"]["project"], WRITING_STYLES_PROJECT_ROUTE);
        assert_eq!(
            contract["routes"]["project_initialize"],
            WRITING_STYLES_PROJECT_INITIALIZE_ROUTE
        );
        assert_eq!(
            contract["routes"]["project_init_defaults"],
            WRITING_STYLES_PROJECT_INIT_DEFAULTS_ROUTE
        );
        assert_eq!(contract["routes"]["create"], WRITING_STYLES_CREATE_ROUTE);
        assert_eq!(contract["routes"]["detail"], WRITING_STYLES_DETAIL_ROUTE);
        assert_eq!(
            contract["routes"]["set_default"],
            WRITING_STYLES_SET_DEFAULT_ROUTE
        );
        assert_eq!(contract["service_owners"].as_array().unwrap().len(), 3);
        assert_eq!(contract["readiness_probes"].as_array().unwrap().len(), 15);
        assert_eq!(
            contract["readiness_probes"][14],
            "writing-styles-missing-detail-business-rust"
        );
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-writing-styles-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"]
                .as_array()
                .expect("business probes should be present")
                .len(),
            13
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"][8],
            "writing-styles-set-default-business-rust"
        );
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(contract["source_map_files"].as_array().unwrap().len(), 5);
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            false
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["readiness_probe_count"],
            15
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            13
        );
        assert_eq!(
            contract["business_smoke_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("business smoke is covered"));
    }

    #[test]
    fn should_keep_writing_styles_route_group_paths_stable() {
        assert_eq!(WRITING_STYLES_PRESETS_ROUTE, "/writing-styles/presets/list");
        assert_eq!(WRITING_STYLES_USER_ROUTE, "/writing-styles/user");
        assert_eq!(
            WRITING_STYLES_PROJECT_ROUTE,
            "/writing-styles/project/{project_id}"
        );
        assert_eq!(
            WRITING_STYLES_PROJECT_INITIALIZE_ROUTE,
            "/writing-styles/project/{project_id}/initialize"
        );
        assert_eq!(
            WRITING_STYLES_PROJECT_INIT_DEFAULTS_ROUTE,
            "/writing-styles/project/{project_id}/init-defaults"
        );
        assert_eq!(WRITING_STYLES_CREATE_ROUTE, "/writing-styles");
        assert_eq!(WRITING_STYLES_DETAIL_ROUTE, "/writing-styles/{style_id}");
        assert_eq!(
            WRITING_STYLES_SET_DEFAULT_ROUTE,
            "/writing-styles/{style_id}/set-default"
        );
    }
}
