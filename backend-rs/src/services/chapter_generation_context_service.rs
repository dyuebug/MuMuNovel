use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
};

use crate::models::{chapter, project};

pub async fn load_generation_context(
    db: &DatabaseConnection,
    user_id: &str,
    chapter_id: &str,
) -> Result<(chapter::Model, project::Model, Option<chapter::Model>), String> {
    let chapter_model = chapter::Entity::find_by_id(chapter_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Chapter not found".to_string())?;

    let project_model = project::Entity::find_by_id(&chapter_model.project_id)
        .one(db)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Project not found".to_string())?;

    if project_model.user_id != user_id {
        return Err("Chapter not found or access denied".to_string());
    }

    let previous_chapter = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(&chapter_model.project_id))
        .filter(chapter::Column::ChapterNumber.lt(chapter_model.chapter_number))
        .order_by_desc(chapter::Column::ChapterNumber)
        .one(db)
        .await
        .map_err(|error| error.to_string())?;

    Ok((chapter_model, project_model, previous_chapter))
}
