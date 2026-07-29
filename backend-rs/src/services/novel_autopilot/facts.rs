use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};

use crate::{
    models::{
        career, chapter, character, novel_autopilot_run, novel_autopilot_step_run, organization,
        outline, plot_analysis, project,
    },
    services::{
        chapter_content_digest_service::chapter_content_digest,
        project_export_service::{
            project_export_descriptor_matches_current_artifact, ProjectExportArtifactDescriptorV1,
            ProjectExportServiceError,
        },
    },
};

use super::{
    book_review_service::{
        load_book_review_summary, BookReviewRewriteReference, BookReviewServiceError,
    },
    router::NovelAutopilotBusinessFacts,
    types::{NovelAutopilotRunConfig, NovelAutopilotStepStatus, NovelAutopilotStepType},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NovelAutopilotFactsError {
    NotFoundOrAccessDenied,
    Database,
    InvalidPendingRewrites,
    BookReview(BookReviewServiceError),
    ProjectExport(ProjectExportServiceError),
}

impl NovelAutopilotFactsError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::NotFoundOrAccessDenied => "not_found_or_access_denied",
            Self::Database => "database_error",
            Self::InvalidPendingRewrites => "invalid_pending_rewrites",
            Self::BookReview(error) => error.code(),
            Self::ProjectExport(error) => error.code(),
        }
    }
}

/// Selects which chapter bodies may participate in the per-chapter quality loop.
///
/// Full-book runs intentionally inspect all chapters. Partial scopes pass `CurrentChapter` so
/// pre-existing manual chapters cannot become analysis/repair targets for the new Run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NovelAutopilotQualityFactScope<'a> {
    AllChapters,
    CurrentChapter(Option<&'a str>),
}

impl NovelAutopilotQualityFactScope<'_> {
    fn includes(self, chapter_id: &str) -> bool {
        match self {
            Self::AllChapters => true,
            Self::CurrentChapter(current_chapter_id) => current_chapter_id == Some(chapter_id),
        }
    }
}

/// Reads the minimum project facts required by the pure durable-router.
///
/// This intentionally returns only booleans, counters, and chapter identifiers. It never
/// copies project prose, outline contents, prompts, or model output into the Durable Run.
pub(crate) async fn load_novel_autopilot_business_facts(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
    target_chapter_count: u32,
    chapters_completed_in_run: u32,
    quality_scope: NovelAutopilotQualityFactScope<'_>,
) -> Result<NovelAutopilotBusinessFacts, NovelAutopilotFactsError> {
    let project = project::Entity::find_by_id(project_id)
        .filter(project::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(|_| NovelAutopilotFactsError::Database)?
        .ok_or(NovelAutopilotFactsError::NotFoundOrAccessDenied)?;

    let careers_ready = career::Entity::find()
        .filter(career::Column::ProjectId.eq(project_id))
        .one(db)
        .await
        .map_err(|_| NovelAutopilotFactsError::Database)?
        .is_some();
    let characters_ready = character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .filter(character::Column::IsOrganization.eq(false))
        .one(db)
        .await
        .map_err(|_| NovelAutopilotFactsError::Database)?
        .is_some();
    let organization_record_exists = organization::Entity::find()
        .filter(organization::Column::ProjectId.eq(project_id))
        .one(db)
        .await
        .map_err(|_| NovelAutopilotFactsError::Database)?
        .is_some();
    let organization_character_exists = character::Entity::find()
        .filter(character::Column::ProjectId.eq(project_id))
        .filter(character::Column::IsOrganization.eq(true))
        .one(db)
        .await
        .map_err(|_| NovelAutopilotFactsError::Database)?
        .is_some();

    let outlines = outline::Entity::find()
        .filter(outline::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(|_| NovelAutopilotFactsError::Database)?;
    let chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .order_by_asc(chapter::Column::ChapterNumber)
        .order_by_asc(chapter::Column::SubIndex)
        .all(db)
        .await
        .map_err(|_| NovelAutopilotFactsError::Database)?;
    let analyses = plot_analysis::Entity::find()
        .filter(plot_analysis::Column::ProjectId.eq(project_id))
        .all(db)
        .await
        .map_err(|_| NovelAutopilotFactsError::Database)?;

    let next_incomplete_chapter = chapters
        .iter()
        .find(|chapter| !has_non_blank_text(chapter.content.as_deref()));
    let mut ordered_outlines = outlines.iter().collect::<Vec<_>>();
    ordered_outlines
        .sort_by_key(|outline| (outline.order_index.unwrap_or(i32::MAX), outline.id.as_str()));
    let unexpanded_outlines = ordered_outlines
        .into_iter()
        .filter(|outline| {
            !chapters
                .iter()
                .any(|chapter| chapter.outline_id.as_deref() == Some(outline.id.as_str()))
        })
        .collect::<Vec<_>>();
    let next_unexpanded_outline = unexpanded_outlines.first().copied();

    let foundation_ready = has_non_blank_text(Some(&project.title))
        && project.target_words > 0
        && [
            project.description.as_deref(),
            project.theme.as_deref(),
            project.genre.as_deref(),
        ]
        .into_iter()
        .any(has_non_blank_text);
    let world_ready = [
        project.world_time_period.as_deref(),
        project.world_location.as_deref(),
        project.world_atmosphere.as_deref(),
        project.world_rules.as_deref(),
    ]
    .into_iter()
    .all(has_non_blank_text);

    let pending_analysis = chapters.iter().find(|chapter| {
        quality_scope.includes(&chapter.id)
            && has_non_blank_text(chapter.content.as_deref())
            && !analyses
                .iter()
                .any(|analysis| analysis_matches_chapter_content(analysis, chapter))
    });
    let pending_repair = chapters.iter().find(|chapter| {
        quality_scope.includes(&chapter.id)
            && analyses.iter().any(|analysis| {
                analysis_matches_chapter_content(analysis, chapter)
                    && analysis
                        .overall_quality_score
                        .is_some_and(|score| (6.0..8.0).contains(&score))
            })
    });

    Ok(NovelAutopilotBusinessFacts {
        foundation_ready,
        world_ready,
        careers_ready,
        characters_ready,
        organizations_ready: organization_record_exists || organization_character_exists,
        outline_ready: outlines.iter().any(|outline| {
            has_non_blank_text(outline.content.as_deref())
                || has_non_blank_text(outline.structure.as_deref())
        }),
        outline_mode: project.outline_mode.clone(),
        current_chapter_count: u32::try_from(chapters.len()).unwrap_or(u32::MAX),
        next_unexpanded_outline_id: next_unexpanded_outline.map(|outline| outline.id.clone()),
        next_unexpanded_outline_order: next_unexpanded_outline
            .and_then(|outline| u32::try_from(outline.order_index.unwrap_or_default()).ok()),
        remaining_unexpanded_outline_count: u32::try_from(unexpanded_outlines.len())
            .unwrap_or(u32::MAX),
        next_incomplete_chapter_id: next_incomplete_chapter.map(|chapter| chapter.id.clone()),
        next_incomplete_chapter_number: next_incomplete_chapter
            .and_then(|chapter| u32::try_from(chapter.chapter_number).ok()),
        target_chapter_count,
        completed_chapter_count: u32::try_from(
            chapters
                .iter()
                .filter(|chapter| has_non_blank_text(chapter.content.as_deref()))
                .count(),
        )
        .unwrap_or(u32::MAX),
        chapters_completed_in_run,
        pending_analysis_chapter_id: pending_analysis.map(|chapter| chapter.id.clone()),
        pending_analysis_chapter_number: pending_analysis
            .and_then(|chapter| u32::try_from(chapter.chapter_number).ok()),
        pending_repair_chapter_id: pending_repair.map(|chapter| chapter.id.clone()),
        pending_repair_chapter_number: pending_repair
            .and_then(|chapter| u32::try_from(chapter.chapter_number).ok()),
        // Book review, polish, and export facts still require their completion owners.
        ..NovelAutopilotBusinessFacts::default()
    })
}

/// Adds completion-phase facts that must survive service restarts.
///
/// A historical completed step is valid only while its result digest still matches the current
/// consistency report and chapter-analysis facts. The Run stores references, never book prose.
pub(crate) async fn enrich_novel_autopilot_completion_facts(
    db: &DatabaseConnection,
    run: &novel_autopilot_run::Model,
    user_id: &str,
    config: &NovelAutopilotRunConfig,
    facts: &mut NovelAutopilotBusinessFacts,
) -> Result<(), NovelAutopilotFactsError> {
    facts.export_completed = false;

    let completion_facts_ready = facts.target_chapter_count > 0
        && facts.completed_chapter_count == facts.target_chapter_count
        && facts.next_incomplete_chapter_id.is_none()
        && facts.pending_analysis_chapter_id.is_none()
        && facts.pending_repair_chapter_id.is_none();
    if !completion_facts_ready {
        return Ok(());
    }

    let summary =
        load_book_review_summary(db, &run.project_id, user_id, facts.target_chapter_count)
            .await
            .map_err(NovelAutopilotFactsError::BookReview)?;
    if !summary.ready {
        return Ok(());
    }

    facts.book_review_completed = novel_autopilot_step_run::Entity::find()
        .filter(novel_autopilot_step_run::Column::RunId.eq(&run.id))
        .filter(
            novel_autopilot_step_run::Column::StepType
                .eq(NovelAutopilotStepType::BookReview.as_str()),
        )
        .filter(
            novel_autopilot_step_run::Column::Status
                .eq(NovelAutopilotStepStatus::Completed.as_str()),
        )
        .filter(
            novel_autopilot_step_run::Column::ResultDigest.eq(Some(summary.result_digest.clone())),
        )
        .order_by_desc(novel_autopilot_step_run::Column::CreatedAt)
        .one(db)
        .await
        .map_err(|_| NovelAutopilotFactsError::Database)?
        .is_some();
    let pending_rewrites =
        serde_json::from_value::<Vec<BookReviewRewriteReference>>(run.pending_rewrites.clone())
            .map_err(|_| NovelAutopilotFactsError::InvalidPendingRewrites)?;
    facts.book_polish_completed = pending_rewrites.is_empty();
    if facts.book_review_completed {
        if let Some(rewrite) = pending_rewrites.first() {
            facts.pending_polish_chapter_id = Some(rewrite.chapter_id.clone());
            facts.pending_polish_chapter_number = u32::try_from(rewrite.chapter_number).ok();
        }
    }
    facts.export_completed = final_export_matches_current_project(db, run, user_id, config).await?;
    Ok(())
}

async fn final_export_matches_current_project(
    db: &DatabaseConnection,
    run: &novel_autopilot_run::Model,
    user_id: &str,
    config: &NovelAutopilotRunConfig,
) -> Result<bool, NovelAutopilotFactsError> {
    let Some(raw_descriptor) = run
        .final_export_ref
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(false);
    };
    let Ok(descriptor) = serde_json::from_str::<ProjectExportArtifactDescriptorV1>(raw_descriptor)
    else {
        return Ok(false);
    };

    project_export_descriptor_matches_current_artifact(
        db,
        &run.project_id,
        user_id,
        &config.export_format,
        &descriptor,
    )
    .await
    .map_err(NovelAutopilotFactsError::ProjectExport)
}

fn analysis_matches_chapter_content(
    analysis: &plot_analysis::Model,
    chapter: &chapter::Model,
) -> bool {
    let Some(content) = chapter.content.as_deref() else {
        return false;
    };
    let current_digest = chapter_content_digest(content);
    analysis.chapter_id == chapter.id
        && analysis.source_content_digest.as_deref() == Some(current_digest.as_str())
}

fn has_non_blank_text(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
        IntoActiveModel, Schema, Set,
    };

    use crate::{
        models::{career, chapter, character, organization, outline, plot_analysis, project},
        services::chapter_content_digest_service::chapter_content_digest,
    };

    use super::{
        load_novel_autopilot_business_facts, NovelAutopilotFactsError,
        NovelAutopilotQualityFactScope,
    };

    async fn setup_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect facts sqlite memory db");
        let builder = DbBackend::Sqlite;
        let schema = Schema::new(builder);
        for statement in [
            builder.build(&schema.create_table_from_entity(project::Entity)),
            builder.build(&schema.create_table_from_entity(career::Entity)),
            builder.build(&schema.create_table_from_entity(character::Entity)),
            builder.build(&schema.create_table_from_entity(organization::Entity)),
            builder.build(&schema.create_table_from_entity(outline::Entity)),
            builder.build(&schema.create_table_from_entity(chapter::Entity)),
            builder.build(&schema.create_table_from_entity(plot_analysis::Entity)),
        ] {
            db.execute(statement).await.expect("create facts table");
        }
        db
    }

    fn created_at() -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 7, 19)
            .expect("valid date")
            .and_hms_opt(8, 0, 0)
            .expect("valid time")
    }

    async fn insert_ready_project(db: &DatabaseConnection) {
        let created_at = created_at();
        project::ActiveModel {
            id: Set("project-1".to_string()),
            user_id: Set("owner-1".to_string()),
            title: Set("Autopilot facts".to_string()),
            description: Set(Some("A complete project brief".to_string())),
            theme: Set(Some("growth".to_string())),
            genre: Set(Some("fantasy".to_string())),
            target_words: Set(100_000),
            current_words: Set(0),
            status: Set("foundation".to_string()),
            wizard_status: Set("incomplete".to_string()),
            wizard_step: Set(1),
            outline_mode: Set("linear".to_string()),
            world_time_period: Set(Some("future".to_string())),
            world_location: Set(Some("city".to_string())),
            world_atmosphere: Set(Some("tense".to_string())),
            world_rules: Set(Some("magic has a cost".to_string())),
            character_count: Set(1),
            created_at: Set(created_at),
            updated_at: Set(Some(created_at)),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert project");
    }

    #[tokio::test]
    async fn reads_safe_business_facts_without_copying_project_content() {
        let db = setup_db().await;
        let created_at = created_at();
        insert_ready_project(&db).await;

        career::ActiveModel {
            id: Set("career-1".to_string()),
            project_id: Set("project-1".to_string()),
            name: Set("mage".to_string()),
            career_type: Set("combat".to_string()),
            stages: Set("[]".to_string()),
            max_stage: Set(1),
            source: Set("manual".to_string()),
            created_at: Set(created_at),
            updated_at: Set(Some(created_at)),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert career");
        character::ActiveModel {
            id: Set("character-1".to_string()),
            project_id: Set("project-1".to_string()),
            name: Set("hero".to_string()),
            is_organization: Set(false),
            status: Set("active".to_string()),
            created_at: Set(created_at),
            updated_at: Set(Some(created_at)),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert character");
        organization::ActiveModel {
            id: Set("organization-1".to_string()),
            character_id: Set("character-1".to_string()),
            project_id: Set("project-1".to_string()),
            level: Set(1),
            power_level: Set(1),
            member_count: Set(1),
            created_at: Set(created_at),
            updated_at: Set(Some(created_at)),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert organization");
        outline::ActiveModel {
            id: Set("outline-1".to_string()),
            project_id: Set("project-1".to_string()),
            title: Set("Act one".to_string()),
            content: Set(Some("The story begins".to_string())),
            created_at: Set(created_at),
            updated_at: Set(Some(created_at)),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert outline");
        for (id, number, content) in [
            ("chapter-1", 1, Some("Already written")),
            ("chapter-2", 2, Some("   ")),
        ] {
            chapter::ActiveModel {
                id: Set(id.to_string()),
                project_id: Set("project-1".to_string()),
                chapter_number: Set(number),
                title: Set(format!("Chapter {number}")),
                content: Set(content.map(str::to_string)),
                word_count: Set(0),
                status: Set("draft".to_string()),
                sub_index: Set(0),
                created_at: Set(created_at),
                updated_at: Set(Some(created_at)),
                ..Default::default()
            }
            .insert(&db)
            .await
            .expect("insert chapter");
        }

        let facts = load_novel_autopilot_business_facts(
            &db,
            "project-1",
            "owner-1",
            2,
            1,
            NovelAutopilotQualityFactScope::AllChapters,
        )
        .await
        .expect("load facts");
        assert!(facts.foundation_ready);
        assert!(facts.world_ready);
        assert!(facts.careers_ready);
        assert!(facts.characters_ready);
        assert!(facts.organizations_ready);
        assert!(facts.outline_ready);
        assert_eq!(facts.target_chapter_count, 2);
        assert_eq!(facts.completed_chapter_count, 1);
        assert_eq!(facts.chapters_completed_in_run, 1);
        assert_eq!(
            facts.next_incomplete_chapter_id.as_deref(),
            Some("chapter-2")
        );
        assert_eq!(facts.next_incomplete_chapter_number, Some(2));

        let partial_scope = load_novel_autopilot_business_facts(
            &db,
            "project-1",
            "owner-1",
            2,
            0,
            NovelAutopilotQualityFactScope::CurrentChapter(None),
        )
        .await
        .expect("load continue-from-current facts before generating a chapter");
        assert!(partial_scope.pending_analysis_chapter_id.is_none());
        assert!(partial_scope.pending_repair_chapter_id.is_none());
        assert_eq!(
            partial_scope.next_incomplete_chapter_id.as_deref(),
            Some("chapter-2")
        );
    }

    #[tokio::test]
    async fn analysis_digest_controls_pending_analysis_and_repair_facts() {
        let db = setup_db().await;
        let created_at = created_at();
        insert_ready_project(&db).await;
        chapter::ActiveModel {
            id: Set("chapter-1".to_string()),
            project_id: Set("project-1".to_string()),
            chapter_number: Set(1),
            title: Set("Chapter 1".to_string()),
            content: Set(Some("current chapter body".to_string())),
            word_count: Set(20),
            status: Set("completed".to_string()),
            sub_index: Set(0),
            created_at: Set(created_at),
            updated_at: Set(Some(created_at)),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert analyzed chapter");

        let no_analysis = load_novel_autopilot_business_facts(
            &db,
            "project-1",
            "owner-1",
            1,
            1,
            NovelAutopilotQualityFactScope::AllChapters,
        )
        .await
        .expect("load facts without analysis");
        assert_eq!(
            no_analysis.pending_analysis_chapter_id.as_deref(),
            Some("chapter-1")
        );
        assert!(no_analysis.pending_repair_chapter_id.is_none());

        plot_analysis::ActiveModel {
            id: Set("analysis-1".to_string()),
            project_id: Set("project-1".to_string()),
            chapter_id: Set("chapter-1".to_string()),
            source_content_digest: Set(None),
            hooks_count: Set(0),
            foreshadows_planted: Set(0),
            foreshadows_resolved: Set(0),
            plot_points_count: Set(0),
            overall_quality_score: Set(Some(7.0)),
            created_at: Set(Some(created_at)),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert legacy analysis without digest");

        let legacy = load_novel_autopilot_business_facts(
            &db,
            "project-1",
            "owner-1",
            1,
            1,
            NovelAutopilotQualityFactScope::AllChapters,
        )
        .await
        .expect("load facts with legacy analysis");
        assert_eq!(
            legacy.pending_analysis_chapter_id.as_deref(),
            Some("chapter-1")
        );
        assert!(legacy.pending_repair_chapter_id.is_none());

        let analysis = plot_analysis::Entity::find_by_id("analysis-1")
            .one(&db)
            .await
            .expect("load analysis")
            .expect("analysis exists");
        let mut stale = analysis.into_active_model();
        stale.source_content_digest = Set(Some(chapter_content_digest("old chapter body")));
        stale.update(&db).await.expect("set stale analysis digest");
        let stale_facts = load_novel_autopilot_business_facts(
            &db,
            "project-1",
            "owner-1",
            1,
            1,
            NovelAutopilotQualityFactScope::AllChapters,
        )
        .await
        .expect("load facts with stale analysis");
        assert_eq!(
            stale_facts.pending_analysis_chapter_id.as_deref(),
            Some("chapter-1")
        );
        assert!(stale_facts.pending_repair_chapter_id.is_none());

        let analysis = plot_analysis::Entity::find_by_id("analysis-1")
            .one(&db)
            .await
            .expect("reload analysis")
            .expect("analysis exists");
        let mut current = analysis.into_active_model();
        current.source_content_digest = Set(Some(chapter_content_digest("current chapter body")));
        current
            .update(&db)
            .await
            .expect("set current analysis digest");
        let current_facts = load_novel_autopilot_business_facts(
            &db,
            "project-1",
            "owner-1",
            1,
            1,
            NovelAutopilotQualityFactScope::AllChapters,
        )
        .await
        .expect("load facts with current analysis");
        assert!(current_facts.pending_analysis_chapter_id.is_none());
        assert_eq!(
            current_facts.pending_repair_chapter_id.as_deref(),
            Some("chapter-1")
        );
    }

    #[tokio::test]
    async fn rejects_project_facts_for_another_owner() {
        let db = setup_db().await;
        insert_ready_project(&db).await;

        assert_eq!(
            load_novel_autopilot_business_facts(
                &db,
                "project-1",
                "owner-2",
                1,
                0,
                NovelAutopilotQualityFactScope::AllChapters,
            )
            .await
            .unwrap_err(),
            NovelAutopilotFactsError::NotFoundOrAccessDenied
        );
    }
}
