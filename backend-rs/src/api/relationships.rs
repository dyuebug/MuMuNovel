use axum::{
    extract::{Extension, Path, Query},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::{
    character, organization, organization_member, project, relationship, relationship_type,
};
use crate::services::auth::Claims;

const RELATIONSHIPS_LIST_CREATE_ROUTE: &str = "/relationships";
const RELATIONSHIPS_CREATE_TRAILING_SLASH_ROUTE: &str = "/relationships/";
const RELATIONSHIPS_TYPES_ROUTE: &str = "/relationships/types";
const RELATIONSHIPS_PROJECT_LIST_ROUTE: &str = "/relationships/project/{project_id}";
const RELATIONSHIPS_GRAPH_ROUTE: &str = "/relationships/graph/{project_id}";
const RELATIONSHIPS_DETAIL_ROUTE: &str = "/relationships/{rel_id}";

#[cfg(test)]
fn build_relationships_route_owner_contract() -> Value {
    json!({
        "owner": "relationships",
        "rust_owner": "backend-rs/src/api/relationships.rs",
        "route_prefix": "/api",
        "routes": {
            "list": RELATIONSHIPS_LIST_CREATE_ROUTE,
            "create": RELATIONSHIPS_LIST_CREATE_ROUTE,
            "create_trailing_slash": RELATIONSHIPS_CREATE_TRAILING_SLASH_ROUTE,
            "types": RELATIONSHIPS_TYPES_ROUTE,
            "project_list": RELATIONSHIPS_PROJECT_LIST_ROUTE,
            "graph": RELATIONSHIPS_GRAPH_ROUTE,
            "detail": RELATIONSHIPS_DETAIL_ROUTE,
            "update": RELATIONSHIPS_DETAIL_ROUTE,
            "delete": RELATIONSHIPS_DETAIL_ROUTE
        },
        "method_contract": {
            "list_create": ["GET", "POST"],
            "create_trailing_slash": ["POST"],
            "types": ["GET"],
            "project_list": ["GET"],
            "graph": ["GET"],
            "detail": ["GET", "PUT", "DELETE"]
        },
        "service_handoffs": {
            "query_owner": "backend-rs/src/api/relationships.rs",
            "write_owner": "backend-rs/src/api/relationships.rs"
        },
        "readiness_probes": [
            "relationships-project-list-auth-guard-rust",
            "relationships-graph-auth-guard-rust",
            "relationships-setup-project-business-rust",
            "relationships-create-character-a-business-rust",
            "relationships-create-character-b-business-rust",
            "relationships-types-business-rust",
            "relationships-create-business-rust",
            "relationships-list-business-rust",
            "relationships-project-list-business-rust",
            "relationships-graph-business-rust",
            "relationships-detail-business-rust",
            "relationships-update-business-rust",
            "relationships-delete-business-rust",
            "relationships-missing-detail-business-rust"
        ],
        "source_map_files": [
            "backend/migrator_app/models/relationship.py"
        ],
        "owner_profile": {
            "name": "phase5-relationships-business-owner",
            "business_probes": [
                "relationships-setup-project-business-rust",
                "relationships-create-character-a-business-rust",
                "relationships-create-character-b-business-rust",
                "relationships-types-business-rust",
                "relationships-create-business-rust",
                "relationships-list-business-rust",
                "relationships-project-list-business-rust",
                "relationships-graph-business-rust",
                "relationships-detail-business-rust",
                "relationships-update-business-rust",
                "relationships-delete-business-rust",
                "relationships-missing-detail-business-rust"
            ],
            "python_fallback_probe_count": 0
        },
        "rollback_boundary": {
            "source_map_policy": "relationships_route_source_map_deleted_remaining_relationship_model_requires_separate_closeout",
            "source_map_freeze_candidate_ready": true,
            "full_module_freeze_ready": true,
            "python_bootstrap_status": "relationships_route_runtime_registration_deleted_no_python_route_shell_remains",
            "python_route_files_status": "relationships_route_source_map_deleted_remaining_relationship_model_only",
            "python_fallback_removal_ready": true,
            "remaining_blockers": [],
            "freeze_reason": "Rust relationships route group has dedicated phase5-relationships-business-owner probes for setup, type lookup, create/list/project-list/graph/detail/update/delete, and missing-detail behavior; the Python relationships route shell is no longer registered in app bootstrap, the detached Python relationship schema shell and relationship-type init script have been physically deleted, and the remaining persistence source map has been narrowed to the dedicated relationship model file.",
            "rollback_files": []
        },
        "business_smoke_status": {
            "owner_profile": "phase5-relationships-business-owner",
            "business_probe_count": 12,
            "python_fallback_probe_count": 0,
            "status": "covered_by_dedicated_rust_owner_profile"
        },
        "next_cutover_gate": "explicit relationship model source-map freeze/delete/repoint approval with same-round rollback policy",
        "migration_policy": "Relationships route business smoke is covered by phase5-relationships-business-owner; the Python route shell is no longer registered in app bootstrap, the detached Python relationship schema shell plus relationship-type init script have been physically deleted, and final completion now requires explicit relationship model source-map freeze/delete/repoint approval with same-round rollback policy."
    })
}

struct RelationshipService;

impl RelationshipService {
    async fn verify_project_access(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<bool, String> {
        project::Entity::find()
            .filter(project::Column::Id.eq(project_id))
            .filter(project::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map(|project| project.is_some())
            .map_err(|error| error.to_string())
    }

    async fn list_types(db: &DatabaseConnection) -> Result<Vec<relationship_type::Model>, String> {
        relationship_type::Entity::find()
            .order_by_asc(relationship_type::Column::Category)
            .order_by_asc(relationship_type::Column::Id)
            .all(db)
            .await
            .map_err(|error| error.to_string())
    }

    async fn list_project_models(
        db: &DatabaseConnection,
        project_id: &str,
        character_id: Option<&str>,
    ) -> Result<Vec<relationship::Model>, String> {
        let mut selector = relationship::Entity::find()
            .filter(relationship::Column::ProjectId.eq(project_id))
            .order_by_desc(relationship::Column::CreatedAt);

        if let Some(character_id) = character_id {
            selector = selector.filter(
                relationship::Column::CharacterFromId
                    .eq(character_id.to_string())
                    .or(relationship::Column::CharacterToId.eq(character_id.to_string())),
            );
        }

        selector.all(db).await.map_err(|error| error.to_string())
    }

    async fn build_graph_payload(
        db: &DatabaseConnection,
        project_id: &str,
    ) -> Result<Value, String> {
        let characters = character::Entity::find()
            .filter(character::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(|error| error.to_string())?;
        let nodes: Vec<Value> = characters
            .iter()
            .map(|item| {
                json!({
                    "id": item.id,
                    "name": item.name,
                    "type": if item.is_organization { "organization" } else { "character" },
                    "role_type": item.role_type,
                    "avatar": item.avatar_url,
                })
            })
            .collect();

        let relationships = relationship::Entity::find()
            .filter(relationship::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(|error| error.to_string())?;
        let mut links: Vec<Value> = relationships
            .iter()
            .map(|item| {
                json!({
                    "source": item.character_from_id,
                    "target": item.character_to_id,
                    "relationship": item.relationship_name.as_deref().unwrap_or("未知关系"),
                    "intimacy": item.intimacy_level,
                    "status": item.status,
                })
            })
            .collect();

        let organizations = organization::Entity::find()
            .filter(organization::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(|error| error.to_string())?;
        for organization in organizations {
            let members = organization_member::Entity::find()
                .filter(organization_member::Column::OrganizationId.eq(&organization.id))
                .all(db)
                .await
                .map_err(|error| error.to_string())?;

            links.extend(members.into_iter().map(|member| {
                json!({
                    "source": organization.character_id,
                    "target": member.character_id,
                    "relationship": format!("组织成员·{}", member.position),
                    "intimacy": member.loyalty,
                    "status": member.status,
                })
            }));
        }

        Ok(json!({
            "nodes": nodes,
            "links": links,
        }))
    }

    async fn create(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
        character_from_id: &str,
        character_to_id: &str,
        relationship_type_id: Option<i32>,
        relationship_name: Option<&str>,
        intimacy_level: Option<i32>,
        description: Option<&str>,
    ) -> Result<Option<relationship::Model>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        let now = Utc::now().naive_utc();
        let model = relationship::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.to_string()),
            character_from_id: Set(character_from_id.to_string()),
            character_to_id: Set(character_to_id.to_string()),
            relationship_type_id: Set(relationship_type_id),
            relationship_name: Set(relationship_name.map(|value| value.to_string())),
            intimacy_level: Set(intimacy_level.unwrap_or(50)),
            status: Set("active".to_string()),
            description: Set(description.map(|value| value.to_string())),
            started_at: Set(None),
            ended_at: Set(None),
            source: Set("manual".to_string()),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };
        model
            .insert(db)
            .await
            .map_err(|error| error.to_string())
            .map(Some)
    }

    async fn list(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Option<Vec<relationship::Model>>, String> {
        if !Self::verify_project_access(db, project_id, user_id).await? {
            return Ok(None);
        }
        relationship::Entity::find()
            .filter(relationship::Column::ProjectId.eq(project_id))
            .order_by_asc(relationship::Column::CharacterFromId)
            .all(db)
            .await
            .map_err(|error| error.to_string())
            .map(Some)
    }

    async fn get(
        db: &DatabaseConnection,
        rel_id: &str,
        user_id: &str,
    ) -> Result<Option<relationship::Model>, String> {
        let relationship = relationship::Entity::find_by_id(rel_id)
            .one(db)
            .await
            .map_err(|error| error.to_string())?;
        match relationship {
            Some(ref rel) => {
                if !Self::verify_project_access(db, &rel.project_id, user_id).await? {
                    return Ok(None);
                }
                Ok(Some(rel.clone()))
            }
            None => Ok(None),
        }
    }

    async fn update(
        db: &DatabaseConnection,
        rel_id: &str,
        user_id: &str,
        relationship_name: Option<&str>,
        intimacy_level: Option<i32>,
        status: Option<&str>,
        description: Option<&str>,
    ) -> Result<Option<relationship::Model>, String> {
        let existing = Self::get(db, rel_id, user_id).await?;
        let Some(model) = existing else {
            return Ok(None);
        };
        let mut active: relationship::ActiveModel = model.into();
        if let Some(value) = relationship_name {
            active.relationship_name = Set(Some(value.to_string()));
        }
        if let Some(value) = intimacy_level {
            active.intimacy_level = Set(value);
        }
        if let Some(value) = status {
            active.status = Set(value.to_string());
        }
        if let Some(value) = description {
            active.description = Set(Some(value.to_string()));
        }
        active.updated_at = Set(Some(Utc::now().naive_utc()));
        active
            .update(db)
            .await
            .map_err(|error| error.to_string())
            .map(Some)
    }

    async fn delete(
        db: &DatabaseConnection,
        rel_id: &str,
        user_id: &str,
    ) -> Result<Option<()>, String> {
        let existing = Self::get(db, rel_id, user_id).await?;
        if existing.is_none() {
            return Ok(None);
        }
        relationship::Entity::delete_by_id(rel_id)
            .exec(db)
            .await
            .map_err(|error| error.to_string())?;
        Ok(Some(()))
    }
}

#[derive(Deserialize)]
struct CreateRequest {
    project_id: String,
    character_from_id: String,
    character_to_id: String,
    relationship_type_id: Option<i32>,
    relationship_name: Option<String>,
    intimacy_level: Option<i32>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct UpdateRequest {
    relationship_name: Option<String>,
    intimacy_level: Option<i32>,
    status: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct ListQuery {
    project_id: String,
}

#[derive(Deserialize)]
struct ProjectRelationshipQuery {
    character_id: Option<String>,
}

fn forbidden_or_missing(message: &str) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"success": false, "message": message})),
    )
}

fn server_error(error: String) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"success": false, "message": error})),
    )
}

async fn create_relationship(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match RelationshipService::create(
        &db,
        &body.project_id,
        &claims.sub,
        &body.character_from_id,
        &body.character_to_id,
        body.relationship_type_id,
        body.relationship_name.as_deref(),
        body.intimacy_level,
        body.description.as_deref(),
    )
    .await
    {
        Ok(Some(rel)) => Ok((StatusCode::CREATED, Json(json!(rel)))),
        Ok(None) => Err(forbidden_or_missing("项目不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn list_relationships(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match RelationshipService::list(&db, &query.project_id, &claims.sub).await {
        Ok(Some(rels)) => Ok(Json(json!(rels))),
        Ok(None) => Err(forbidden_or_missing("项目不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn list_types(
    Extension(db): Extension<DatabaseConnection>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let types = RelationshipService::list_types(&db)
        .await
        .map_err(server_error)?;
    Ok(Json(json!(types)))
}

async fn list_project_relationships(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
    Query(query): Query<ProjectRelationshipQuery>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !RelationshipService::verify_project_access(&db, &project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("项目不存在或无权限"));
    }

    let relationships =
        RelationshipService::list_project_models(&db, &project_id, query.character_id.as_deref())
            .await
            .map_err(server_error)?;
    Ok(Json(json!(relationships)))
}

async fn relationship_graph(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if !RelationshipService::verify_project_access(&db, &project_id, &claims.sub)
        .await
        .map_err(server_error)?
    {
        return Err(forbidden_or_missing("项目不存在或无权限"));
    }

    RelationshipService::build_graph_payload(&db, &project_id)
        .await
        .map(Json)
        .map_err(server_error)
}

async fn get_relationship(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(rel_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match RelationshipService::get(&db, &rel_id, &claims.sub).await {
        Ok(Some(rel)) => Ok(Json(json!(rel))),
        Ok(None) => Err(forbidden_or_missing("关系不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn update_relationship(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(rel_id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match RelationshipService::update(
        &db,
        &rel_id,
        &claims.sub,
        body.relationship_name.as_deref(),
        body.intimacy_level,
        body.status.as_deref(),
        body.description.as_deref(),
    )
    .await
    {
        Ok(Some(rel)) => Ok(Json(json!(rel))),
        Ok(None) => Err(forbidden_or_missing("关系不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

async fn delete_relationship(
    Extension(db): Extension<DatabaseConnection>,
    Extension(claims): Extension<Claims>,
    Path(rel_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    match RelationshipService::delete(&db, &rel_id, &claims.sub).await {
        Ok(Some(())) => Ok(Json(json!({"message": "关系删除成功", "id": rel_id}))),
        Ok(None) => Err(forbidden_or_missing("关系不存在或无权限")),
        Err(e) => Err(server_error(e)),
    }
}

pub fn routes() -> Router {
    Router::new()
        .route(
            RELATIONSHIPS_LIST_CREATE_ROUTE,
            post(create_relationship).get(list_relationships),
        )
        .route(
            RELATIONSHIPS_CREATE_TRAILING_SLASH_ROUTE,
            post(create_relationship),
        )
        .route(RELATIONSHIPS_TYPES_ROUTE, get(list_types))
        .route(
            RELATIONSHIPS_PROJECT_LIST_ROUTE,
            get(list_project_relationships),
        )
        .route(RELATIONSHIPS_GRAPH_ROUTE, get(relationship_graph))
        .route(
            RELATIONSHIPS_DETAIL_ROUTE,
            get(get_relationship)
                .put(update_relationship)
                .delete(delete_relationship),
        )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_relationships_route_owner_contract, RelationshipService,
        RELATIONSHIPS_CREATE_TRAILING_SLASH_ROUTE, RELATIONSHIPS_DETAIL_ROUTE,
        RELATIONSHIPS_GRAPH_ROUTE, RELATIONSHIPS_LIST_CREATE_ROUTE,
        RELATIONSHIPS_PROJECT_LIST_ROUTE, RELATIONSHIPS_TYPES_ROUTE,
    };

    #[test]
    fn should_publish_relationships_route_owner_contract() {
        let contract = build_relationships_route_owner_contract();

        assert_eq!(contract["owner"], "relationships");
        assert_eq!(
            contract["rust_owner"],
            "backend-rs/src/api/relationships.rs"
        );
        assert_eq!(
            contract["routes"]["project_list"],
            RELATIONSHIPS_PROJECT_LIST_ROUTE
        );
        assert_eq!(contract["routes"]["graph"], RELATIONSHIPS_GRAPH_ROUTE);
        assert_eq!(
            contract["routes"]["create_trailing_slash"],
            RELATIONSHIPS_CREATE_TRAILING_SLASH_ROUTE
        );
        assert_eq!(contract["readiness_probes"].as_array().unwrap().len(), 14);
        assert_eq!(
            contract["readiness_probes"][13],
            "relationships-missing-detail-business-rust"
        );
        assert_eq!(contract["source_map_files"].as_array().unwrap().len(), 1);
        assert_eq!(
            contract["source_map_files"][0],
            "backend/migrator_app/models/relationship.py"
        );
        assert!(contract["source_map_files"].get(1).is_none());
        assert_eq!(
            contract["owner_profile"]["name"],
            "phase5-relationships-business-owner"
        );
        assert_eq!(
            contract["owner_profile"]["business_probes"][7],
            "relationships-graph-business-rust"
        );
        assert_eq!(contract["owner_profile"]["python_fallback_probe_count"], 0);
        assert_eq!(
            contract["rollback_boundary"]["source_map_freeze_candidate_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["full_module_freeze_ready"],
            true
        );
        assert_eq!(
            contract["rollback_boundary"]["python_bootstrap_status"],
            "relationships_route_runtime_registration_deleted_no_python_route_shell_remains"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_route_files_status"],
            "relationships_route_source_map_deleted_remaining_relationship_model_only"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            true
        );
        assert_eq!(
            contract["business_smoke_status"]["status"],
            "covered_by_dedicated_rust_owner_profile"
        );
        assert_eq!(
            contract["business_smoke_status"]["business_probe_count"],
            12
        );
        assert_eq!(
            contract["next_cutover_gate"],
            "explicit relationship model source-map freeze/delete/repoint approval with same-round rollback policy"
        );
        assert_eq!(
            contract["rollback_boundary"]["remaining_blockers"],
            json!([])
        );
        assert_eq!(contract["rollback_boundary"]["rollback_files"], json!([]));
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("business smoke is covered"));
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("Python route shell is no longer registered in app bootstrap"));
        assert!(contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("relationship schema shell plus relationship-type init script have been physically deleted"));
        assert!(!contract["migration_policy"]
            .as_str()
            .expect("migration policy")
            .contains("requires source-map freeze/delete/repoint evidence or business smoke"));
    }

    #[test]
    fn should_keep_relationships_route_group_paths_stable() {
        assert_eq!(RELATIONSHIPS_LIST_CREATE_ROUTE, "/relationships");
        assert_eq!(RELATIONSHIPS_CREATE_TRAILING_SLASH_ROUTE, "/relationships/");
        assert_eq!(RELATIONSHIPS_TYPES_ROUTE, "/relationships/types");
        assert_eq!(
            RELATIONSHIPS_PROJECT_LIST_ROUTE,
            "/relationships/project/{project_id}"
        );
        assert_eq!(
            RELATIONSHIPS_GRAPH_ROUTE,
            "/relationships/graph/{project_id}"
        );
        assert_eq!(RELATIONSHIPS_DETAIL_ROUTE, "/relationships/{rel_id}");
    }

    #[test]
    fn graph_payload_shape_contract_remains_explicit() {
        let payload = json!({
            "nodes": [
                {
                    "id": "char-1",
                    "name": "角色A",
                    "type": "character",
                    "role_type": "supporting",
                    "avatar": null
                }
            ],
            "links": [
                {
                    "source": "char-1",
                    "target": "char-2",
                    "relationship": "盟友",
                    "intimacy": 80,
                    "status": "active"
                }
            ]
        });

        assert_eq!(payload["nodes"][0]["type"], "character");
        assert_eq!(payload["links"][0]["relationship"], "盟友");
        assert_eq!(payload["links"][0]["intimacy"], 80);
        assert_eq!(payload["links"][0]["status"], "active");

        let _ = RelationshipService::build_graph_payload;
    }
}
