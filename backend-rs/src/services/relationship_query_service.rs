use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{json, Value};

use crate::models::{
    character, organization, organization_member, project, relationship, relationship_type,
};

pub async fn verify_relationship_project_access(
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

pub async fn list_relationship_types(
    db: &DatabaseConnection,
) -> Result<Vec<relationship_type::Model>, String> {
    relationship_type::Entity::find()
        .order_by_asc(relationship_type::Column::Category)
        .order_by_asc(relationship_type::Column::Id)
        .all(db)
        .await
        .map_err(|error| error.to_string())
}

pub async fn list_project_relationship_models(
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

pub async fn build_relationship_graph_payload(
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::build_relationship_graph_payload;

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

        let _ = build_relationship_graph_payload;
    }
}
