use std::{collections::HashSet, fmt};

use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    QueryFilter, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::{
    models::{chapter, novel_autopilot_run, novel_autopilot_step_run, outline, project},
    services::novel_workflow_service::resolve_internal_writing_transition,
};

use super::{
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotRepository, NovelAutopilotRepositoryError,
    },
    types::{NovelAutopilotRunStatus, NovelAutopilotStepStatus},
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NovelAutopilotOutlineSnapshot {
    project: project::Model,
    outline_fingerprints: Vec<NovelAutopilotOutlineFingerprint>,
    chapter_fingerprints: Vec<NovelAutopilotChapterFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NovelAutopilotOutlineFingerprint {
    id: String,
    fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NovelAutopilotChapterFingerprint {
    id: String,
    outline_id: Option<String>,
    fingerprint: String,
}

impl NovelAutopilotOutlineSnapshot {
    pub(crate) async fn load(
        db: &DatabaseConnection,
        project_id: &str,
    ) -> Result<Self, NovelAutopilotRepositoryError> {
        let project = project::Entity::find_by_id(project_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let outlines = outline::Entity::find()
            .filter(outline::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(database_error)?;
        let chapters = chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(database_error)?;

        Ok(Self::from_models(project, outlines, chapters))
    }

    pub(crate) fn from_models(
        project: project::Model,
        outlines: Vec<outline::Model>,
        chapters: Vec<chapter::Model>,
    ) -> Self {
        let mut outline_fingerprints = outlines
            .into_iter()
            .map(|outline| NovelAutopilotOutlineFingerprint {
                id: outline.id.clone(),
                // Snapshot 只用于 CAS；Debug fingerprint 覆盖完整业务字段。
                fingerprint: format!("{outline:?}"),
            })
            .collect::<Vec<_>>();
        outline_fingerprints.sort_by(|left, right| left.id.cmp(&right.id));

        let mut chapter_fingerprints = chapters
            .into_iter()
            .map(|chapter| NovelAutopilotChapterFingerprint {
                id: chapter.id.clone(),
                outline_id: chapter.outline_id.clone(),
                // 模型运行期间的任意章节编辑都必须阻止自动结果覆盖。
                fingerprint: format!("{chapter:?}"),
            })
            .collect::<Vec<_>>();
        chapter_fingerprints.sort_by(|left, right| left.id.cmp(&right.id));

        Self {
            project,
            outline_fingerprints,
            chapter_fingerprints,
        }
    }

    pub(crate) fn is_blank(&self) -> bool {
        self.outline_fingerprints.is_empty() && self.chapter_fingerprints.is_empty()
    }

    pub(crate) fn contains_outline(&self, outline_id: &str) -> bool {
        self.outline_fingerprints
            .iter()
            .any(|outline| outline.id == outline_id)
    }

    pub(crate) fn has_chapters_for_outline(&self, outline_id: &str) -> bool {
        self.chapter_fingerprints
            .iter()
            .any(|chapter| chapter.outline_id.as_deref() == Some(outline_id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotOutlineItemCommit {
    pub title: String,
    pub content: String,
    pub structure: String,
    pub order_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotPendingChapterCommit {
    pub chapter_number: i32,
    pub title: String,
    pub summary: String,
    pub outline_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotOutlineCommit {
    pub outlines: Vec<NovelAutopilotOutlineItemCommit>,
    pub pending_chapters: Vec<NovelAutopilotPendingChapterCommit>,
    pub outline_mode: String,
    pub narrative_perspective: Option<String>,
    pub target_words: i32,
    pub result_digest: String,
}

impl NovelAutopilotRepository {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn commit_outline_design_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        expected_outline: &NovelAutopilotOutlineSnapshot,
        outline_commit: NovelAutopilotOutlineCommit,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        validate_outline_commit(&outline_commit)?;

        let step = novel_autopilot_step_run::Entity::find_by_id(step_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let run = find_owned_run(db, &step.run_id, user_id).await?;
        if run.version != expected_run_version {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        if run.epoch != expected_run_epoch || step.run_epoch != expected_run_epoch {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }
        if run.status != NovelAutopilotRunStatus::Running.as_str()
            || step.status != NovelAutopilotStepStatus::Running.as_str()
            || run.current_step.as_deref() != Some(expected_step_key)
            || step.step_key != expected_step_key
            || run.active_background_task_id.as_deref() != expected_background_task_id
            || step.background_task_id.as_deref() != expected_background_task_id
        {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        let chapter_count = i32::try_from(outline_commit.outlines.len()).map_err(|_| {
            NovelAutopilotRepositoryError::InvalidConfig {
                field: "outlines",
                code: "too_many",
            }
        })?;

        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(database_error)?;
        let current_project = project::Entity::find_by_id(&run.project_id)
            .filter(project::Column::UserId.eq(user_id))
            .one(&txn)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::BusinessDataChanged)?;
        let current_outlines = outline::Entity::find()
            .filter(outline::Column::ProjectId.eq(&run.project_id))
            .all(&txn)
            .await
            .map_err(database_error)?;
        let current_chapters = chapter::Entity::find()
            .filter(chapter::Column::ProjectId.eq(&run.project_id))
            .all(&txn)
            .await
            .map_err(database_error)?;
        let current_snapshot = NovelAutopilotOutlineSnapshot::from_models(
            current_project,
            current_outlines,
            current_chapters,
        );
        if &current_snapshot != expected_outline {
            return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
        }
        let writing_phase = resolve_internal_writing_transition(&current_snapshot.project.status)
            .map_err(|_| NovelAutopilotRepositoryError::InvalidTransition)?;

        let mut created_outline_ids = Vec::with_capacity(outline_commit.outlines.len());
        for item in &outline_commit.outlines {
            let outline_id = Uuid::new_v4().to_string();
            outline::ActiveModel {
                id: Set(outline_id.clone()),
                project_id: Set(run.project_id.clone()),
                title: Set(item.title.clone()),
                content: Set(Some(item.content.clone())),
                structure: Set(Some(item.structure.clone())),
                order_index: Set(Some(item.order_index)),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            }
            .insert(&txn)
            .await
            .map_err(database_error)?;
            created_outline_ids.push(outline_id);
        }

        for pending in &outline_commit.pending_chapters {
            chapter::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                project_id: Set(run.project_id.clone()),
                chapter_number: Set(pending.chapter_number),
                title: Set(pending.title.clone()),
                content: Set(Some(String::new())),
                summary: Set(Some(pending.summary.clone())),
                word_count: Set(0),
                status: Set("pending".to_string()),
                outline_id: Set(Some(created_outline_ids[pending.outline_index].clone())),
                sub_index: Set(0),
                expansion_plan: Set(None),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            }
            .insert(&txn)
            .await
            .map_err(database_error)?;
        }

        let project_update = project::Entity::update_many()
            .col_expr(
                project::Column::ChapterCount,
                Expr::value(Some(chapter_count)),
            )
            .col_expr(
                project::Column::OutlineMode,
                Expr::value(outline_commit.outline_mode.clone()),
            )
            .col_expr(
                project::Column::NarrativePerspective,
                Expr::value(outline_commit.narrative_perspective.clone()),
            )
            .col_expr(
                project::Column::TargetWords,
                Expr::value(outline_commit.target_words),
            )
            .col_expr(project::Column::Status, Expr::value(writing_phase.as_str()))
            .col_expr(project::Column::WizardStatus, Expr::value("completed"))
            .col_expr(project::Column::WizardStep, Expr::value(4))
            .col_expr(project::Column::UpdatedAt, Expr::value(Some(now)))
            .filter(project::Column::Id.eq(&run.project_id))
            .filter(project::Column::UserId.eq(user_id))
            .filter(project_snapshot_condition(&expected_outline.project))
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if project_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
        }

        let run_update = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::CurrentStep,
                Expr::value(None::<String>),
            )
            .col_expr(
                novel_autopilot_run::Column::ActiveBackgroundTaskId,
                Expr::value(None::<String>),
            )
            .col_expr(
                novel_autopilot_run::Column::Version,
                Expr::col(novel_autopilot_run::Column::Version).add(1),
            )
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now))
            .filter(novel_autopilot_run::Column::Id.eq(&run.id))
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .filter(novel_autopilot_run::Column::Version.eq(expected_run_version))
            .filter(novel_autopilot_run::Column::Epoch.eq(expected_run_epoch))
            .filter(novel_autopilot_run::Column::CurrentStep.eq(expected_step_key))
            .filter(
                novel_autopilot_run::Column::ActiveBackgroundTaskId
                    .eq(expected_background_task_id.map(str::to_string)),
            )
            .filter(
                novel_autopilot_run::Column::Status.eq(NovelAutopilotRunStatus::Running.as_str()),
            )
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if run_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }

        let step_update = novel_autopilot_step_run::Entity::update_many()
            .col_expr(
                novel_autopilot_step_run::Column::Status,
                Expr::value(NovelAutopilotStepStatus::Completed.as_str()),
            )
            .col_expr(
                novel_autopilot_step_run::Column::ResultDigest,
                Expr::value(Some(outline_commit.result_digest)),
            )
            .col_expr(
                novel_autopilot_step_run::Column::QualityDecision,
                Expr::value(None::<String>),
            )
            .col_expr(
                novel_autopilot_step_run::Column::ErrorCode,
                Expr::value(None::<String>),
            )
            .col_expr(
                novel_autopilot_step_run::Column::CompletedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                novel_autopilot_step_run::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(novel_autopilot_step_run::Column::Id.eq(step_id))
            .filter(novel_autopilot_step_run::Column::RunEpoch.eq(expected_run_epoch))
            .filter(novel_autopilot_step_run::Column::StepKey.eq(expected_step_key))
            .filter(
                novel_autopilot_step_run::Column::BackgroundTaskId
                    .eq(expected_background_task_id.map(str::to_string)),
            )
            .filter(
                novel_autopilot_step_run::Column::Status
                    .eq(NovelAutopilotStepStatus::Running.as_str()),
            )
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if step_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        txn.commit().await.map_err(database_error)?;

        Ok(ClaimedNovelAutopilotStep {
            run: find_owned_run(db, &run.id, user_id).await?,
            step: novel_autopilot_step_run::Entity::find_by_id(step_id)
                .one(db)
                .await
                .map_err(database_error)?
                .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?,
        })
    }
}

fn project_snapshot_condition(snapshot: &project::Model) -> Condition {
    let mut condition = Condition::all()
        .add(project::Column::Title.eq(&snapshot.title))
        .add(project::Column::TargetWords.eq(snapshot.target_words))
        .add(project::Column::CurrentWords.eq(snapshot.current_words))
        .add(project::Column::Status.eq(&snapshot.status))
        .add(project::Column::WizardStatus.eq(&snapshot.wizard_status))
        .add(project::Column::WizardStep.eq(snapshot.wizard_step))
        .add(project::Column::OutlineMode.eq(&snapshot.outline_mode))
        .add(project::Column::CharacterCount.eq(snapshot.character_count))
        .add(project::Column::CreatedAt.eq(snapshot.created_at));
    condition = add_optional_string_condition(
        condition,
        project::Column::Description,
        &snapshot.description,
    );
    condition = add_optional_string_condition(condition, project::Column::Theme, &snapshot.theme);
    condition = add_optional_string_condition(condition, project::Column::Genre, &snapshot.genre);
    condition = add_optional_string_condition(
        condition,
        project::Column::WorldTimePeriod,
        &snapshot.world_time_period,
    );
    condition = add_optional_string_condition(
        condition,
        project::Column::WorldLocation,
        &snapshot.world_location,
    );
    condition = add_optional_string_condition(
        condition,
        project::Column::WorldAtmosphere,
        &snapshot.world_atmosphere,
    );
    condition = add_optional_string_condition(
        condition,
        project::Column::WorldRules,
        &snapshot.world_rules,
    );
    condition = condition.add(match snapshot.chapter_count {
        Some(value) => project::Column::ChapterCount.eq(value),
        None => project::Column::ChapterCount.is_null(),
    });
    condition = add_optional_string_condition(
        condition,
        project::Column::NarrativePerspective,
        &snapshot.narrative_perspective,
    );
    condition = add_optional_string_condition(
        condition,
        project::Column::DefaultCreativeMode,
        &snapshot.default_creative_mode,
    );
    condition = add_optional_string_condition(
        condition,
        project::Column::DefaultStoryFocus,
        &snapshot.default_story_focus,
    );
    condition = add_optional_string_condition(
        condition,
        project::Column::DefaultPlotStage,
        &snapshot.default_plot_stage,
    );
    condition = add_optional_string_condition(
        condition,
        project::Column::DefaultStoryCreationBrief,
        &snapshot.default_story_creation_brief,
    );
    condition = add_optional_string_condition(
        condition,
        project::Column::DefaultQualityPreset,
        &snapshot.default_quality_preset,
    );
    condition = add_optional_string_condition(
        condition,
        project::Column::DefaultQualityNotes,
        &snapshot.default_quality_notes,
    );
    condition.add(match snapshot.updated_at {
        Some(value) => project::Column::UpdatedAt.eq(value),
        None => project::Column::UpdatedAt.is_null(),
    })
}

fn add_optional_string_condition(
    condition: Condition,
    column: project::Column,
    value: &Option<String>,
) -> Condition {
    condition.add(match value.as_deref() {
        Some(value) => column.eq(value),
        None => column.is_null(),
    })
}
fn validate_outline_commit(
    outline_commit: &NovelAutopilotOutlineCommit,
) -> Result<(), NovelAutopilotRepositoryError> {
    if outline_commit.outlines.is_empty()
        || outline_commit.outline_mode.trim().is_empty()
        || outline_commit.target_words <= 0
        || outline_commit.result_digest.trim().is_empty()
    {
        return Err(invalid_config("outline"));
    }

    let mut order_indexes = HashSet::with_capacity(outline_commit.outlines.len());
    if outline_commit.outlines.iter().any(|item| {
        item.title.trim().is_empty()
            || item.content.trim().is_empty()
            || item.structure.trim().is_empty()
            || item.order_index <= 0
            || !order_indexes.insert(item.order_index)
            || serde_json::from_str::<serde_json::Value>(&item.structure).is_err()
    }) {
        return Err(invalid_config("outlines"));
    }

    let mut chapter_numbers = HashSet::with_capacity(outline_commit.pending_chapters.len());
    let mut outline_indexes = HashSet::with_capacity(outline_commit.pending_chapters.len());
    if outline_commit.pending_chapters.iter().any(|chapter| {
        chapter.chapter_number <= 0
            || chapter.title.trim().is_empty()
            || chapter.outline_index >= outline_commit.outlines.len()
            || !chapter_numbers.insert(chapter.chapter_number)
            || !outline_indexes.insert(chapter.outline_index)
    }) {
        return Err(invalid_config("pending_chapters"));
    }

    if outline_commit.outline_mode == "one-to-one" {
        if outline_commit.pending_chapters.len() != outline_commit.outlines.len() {
            return Err(invalid_config("pending_chapters"));
        }
    } else if !outline_commit.pending_chapters.is_empty() {
        return Err(invalid_config("pending_chapters"));
    }

    Ok(())
}

const fn invalid_config(field: &'static str) -> NovelAutopilotRepositoryError {
    NovelAutopilotRepositoryError::InvalidConfig {
        field,
        code: "invalid",
    }
}

async fn find_owned_run(
    db: &DatabaseConnection,
    run_id: &str,
    user_id: &str,
) -> Result<novel_autopilot_run::Model, NovelAutopilotRepositoryError> {
    novel_autopilot_run::Entity::find_by_id(run_id)
        .filter(novel_autopilot_run::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(database_error)?
        .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)
}

fn database_error(error: impl fmt::Display) -> NovelAutopilotRepositoryError {
    NovelAutopilotRepositoryError::Database(error.to_string())
}
