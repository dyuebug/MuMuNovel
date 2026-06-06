use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::models::{
    career, chapter, character, character_career, generation_history, organization,
    organization_member, outline, plot_analysis, project, project_default_style, relationship,
    story_memory, writing_style,
};
use crate::services::project_consistency_query_service::ProjectQueryContextError;
use crate::services::project_service::ProjectService;

#[derive(Debug, Clone)]
pub struct ProjectExportOptions {
    pub include_generation_history: bool,
    pub include_writing_styles: bool,
    pub include_careers: bool,
    pub include_memories: bool,
    pub include_plot_analysis: bool,
}

#[derive(Debug, Clone)]
pub struct ProjectExportContext {
    pub project: project::Model,
    pub chapters: Vec<chapter::Model>,
    pub characters: Vec<character::Model>,
    pub outlines: Vec<outline::Model>,
    pub relationships: Vec<relationship::Model>,
    pub organizations: Vec<organization::Model>,
    pub organization_members: Vec<organization_member::Model>,
    pub writing_styles: Vec<writing_style::Model>,
    pub generation_history: Vec<generation_history::Model>,
    pub careers: Vec<career::Model>,
    pub character_careers: Vec<character_career::Model>,
    pub story_memories: Vec<story_memory::Model>,
    pub plot_analysis: Vec<plot_analysis::Model>,
    pub project_default_style: Option<project_default_style::Model>,
    pub project_default_style_style: Option<writing_style::Model>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadProjectExportContextError {
    Context(ProjectQueryContextError),
    ProjectHasNoChapters,
}

pub async fn load_project_export_context(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    options: &ProjectExportOptions,
) -> Result<ProjectExportContext, LoadProjectExportContextError> {
    let project = ProjectService::get(db, project_id, user_id)
        .await
        .map_err(ProjectQueryContextError::Internal)
        .map_err(LoadProjectExportContextError::Context)?
        .ok_or(LoadProjectExportContextError::Context(
            ProjectQueryContextError::ProjectNotFound,
        ))?;

    let chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .order_by_asc(chapter::Column::ChapterNumber)
        .all(db)
        .await
        .map_err(map_internal_error)?;

    let characters = character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(map_internal_error)?;

    let outlines = outline::Entity::find()
        .filter(outline::Column::ProjectId.eq(project_id))
        .order_by_asc(outline::Column::OrderIndex)
        .all(db)
        .await
        .map_err(map_internal_error)?;

    let relationships = relationship::Entity::find()
        .filter(relationship::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(map_internal_error)?;

    let organizations = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(map_internal_error)?;

    let organization_ids = organizations
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let organization_members = if organization_ids.is_empty() {
        Vec::new()
    } else {
        organization_member::Entity::find()
            .filter(organization_member::Column::OrganizationId.is_in(organization_ids))
            .all(db)
            .await
            .map_err(map_internal_error)?
    };

    let writing_styles = if options.include_writing_styles {
        writing_style::Entity::find()
            .filter(writing_style::Column::UserId.eq(project.user_id.clone()))
            .order_by_asc(writing_style::Column::OrderIndex)
            .all(db)
            .await
            .map_err(map_internal_error)?
    } else {
        Vec::new()
    };

    let generation_history = if options.include_generation_history {
        generation_history::Entity::find()
            .filter(generation_history::Column::ProjectId.eq(project_id))
            .order_by_desc(generation_history::Column::CreatedAt)
            .limit(100)
            .all(db)
            .await
            .map_err(map_internal_error)?
    } else {
        Vec::new()
    };

    let careers = if options.include_careers {
        career::Entity::find()
            .filter(career::Column::ProjectId.eq(project_id))
            .order_by_asc(career::Column::CareerType)
            .order_by_asc(career::Column::CreatedAt)
            .all(db)
            .await
            .map_err(map_internal_error)?
    } else {
        Vec::new()
    };

    let character_ids = characters
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let character_careers = if options.include_careers && !character_ids.is_empty() {
        character_career::Entity::find()
            .filter(character_career::Column::CharacterId.is_in(character_ids))
            .all(db)
            .await
            .map_err(map_internal_error)?
    } else {
        Vec::new()
    };

    let story_memories = if options.include_memories {
        story_memory::Entity::find()
            .filter(story_memory::Column::ProjectId.eq(project_id))
            .order_by_asc(story_memory::Column::StoryTimeline)
            .order_by_asc(story_memory::Column::ChapterPosition)
            .all(db)
            .await
            .map_err(map_internal_error)?
    } else {
        Vec::new()
    };

    let plot_analysis = if options.include_plot_analysis {
        plot_analysis::Entity::find()
            .filter(plot_analysis::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(map_internal_error)?
    } else {
        Vec::new()
    };

    let project_default_style = project_default_style::Entity::find()
        .filter(project_default_style::Column::ProjectId.eq(project_id))
        .one(db)
        .await
        .map_err(map_internal_error)?;
    let project_default_style_style = if let Some(default_style) = project_default_style.as_ref() {
        writing_style::Entity::find_by_id(default_style.style_id)
            .one(db)
            .await
            .map_err(map_internal_error)?
    } else {
        None
    };

    Ok(ProjectExportContext {
        project,
        chapters,
        characters,
        outlines,
        relationships,
        organizations,
        organization_members,
        writing_styles,
        generation_history,
        careers,
        character_careers,
        story_memories,
        plot_analysis,
        project_default_style,
        project_default_style_style,
    })
}

pub async fn load_project_export_context_with_non_empty_chapters(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<ProjectExportContext, LoadProjectExportContextError> {
    let context = load_project_export_context(
        db,
        project_id,
        user_id,
        &ProjectExportOptions {
            include_generation_history: false,
            include_writing_styles: false,
            include_careers: false,
            include_memories: false,
            include_plot_analysis: false,
        },
    )
    .await?;
    if context.chapters.is_empty() {
        return Err(LoadProjectExportContextError::ProjectHasNoChapters);
    }

    Ok(context)
}

fn map_internal_error(error: sea_orm::DbErr) -> LoadProjectExportContextError {
    LoadProjectExportContextError::Context(ProjectQueryContextError::Internal(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{LoadProjectExportContextError, ProjectExportOptions};
    use crate::services::project_consistency_query_service::ProjectQueryContextError;

    #[test]
    fn project_export_context_error_equality_is_stable() {
        assert_eq!(
            LoadProjectExportContextError::Context(ProjectQueryContextError::ProjectNotFound),
            LoadProjectExportContextError::Context(ProjectQueryContextError::ProjectNotFound)
        );
        assert_eq!(
            LoadProjectExportContextError::ProjectHasNoChapters,
            LoadProjectExportContextError::ProjectHasNoChapters
        );
        assert_eq!(
            LoadProjectExportContextError::Context(ProjectQueryContextError::Internal(
                "boom".to_string(),
            )),
            LoadProjectExportContextError::Context(ProjectQueryContextError::Internal(
                "boom".to_string(),
            ))
        );
    }

    #[test]
    fn project_export_options_clone_keeps_flags() {
        let options = ProjectExportOptions {
            include_generation_history: true,
            include_writing_styles: false,
            include_careers: true,
            include_memories: false,
            include_plot_analysis: true,
        };

        let cloned = options.clone();

        assert!(cloned.include_generation_history);
        assert!(!cloned.include_writing_styles);
        assert!(cloned.include_careers);
        assert!(!cloned.include_memories);
        assert!(cloned.include_plot_analysis);
    }
}
