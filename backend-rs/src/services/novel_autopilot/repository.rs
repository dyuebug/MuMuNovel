use std::fmt;

use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    models::{
        career, character, novel_autopilot_run, novel_autopilot_step_run, organization,
        organization_member, project, relationship,
    },
    services::project_service::{ProjectAccessQueryError, ProjectService},
};

pub(crate) use super::{
    chapter_repository::{
        ChapterBusinessSnapshot, NovelAutopilotChapterGenerateCommit,
        NovelAutopilotManualReviewCandidate,
    },
    character_repository::{
        NovelAutopilotCharacterCareerAssignmentCommit, NovelAutopilotCharacterCommit,
        NovelAutopilotCharacterItemCommit, NovelAutopilotCharacterOrganizationCommit,
        NovelAutopilotCharacterOrganizationMembershipCommit,
        NovelAutopilotCharacterRelationshipCommit, NovelAutopilotCharacterSnapshot,
        NovelAutopilotCharacterSubCareerCommit,
    },
    outline_expansion_repository::{
        NovelAutopilotExpandedChapterCommit, NovelAutopilotOutlineExpansionCommit,
    },
    outline_repository::{
        NovelAutopilotOutlineCommit, NovelAutopilotOutlineItemCommit,
        NovelAutopilotOutlineSnapshot, NovelAutopilotPendingChapterCommit,
    },
};

use super::types::{
    NovelAutopilotPhase, NovelAutopilotPrivateSnapshot, NovelAutopilotRunConfig,
    NovelAutopilotRunStatus, NovelAutopilotStepStatus, NovelAutopilotStepType,
    NOVEL_AUTOPILOT_SCHEMA_VERSION,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NovelAutopilotRepositoryError {
    NotFoundOrAccessDenied,
    InvalidConfig {
        field: &'static str,
        code: &'static str,
    },
    InvalidTransition,
    StaleVersion,
    StaleEpoch,
    BusinessDataChanged,
    Database(String),
}

impl NovelAutopilotRepositoryError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::NotFoundOrAccessDenied => "not_found_or_access_denied",
            Self::InvalidConfig { .. } => "invalid_config",
            Self::InvalidTransition => "invalid_transition",
            Self::StaleVersion => "stale_version",
            Self::StaleEpoch => "stale_epoch",
            Self::BusinessDataChanged => "business_data_changed",
            Self::Database(_) => "database_error",
        }
    }
}

impl fmt::Display for NovelAutopilotRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFoundOrAccessDenied => formatter.write_str("run not found or access denied"),
            Self::InvalidConfig { field, code } => write!(formatter, "invalid {field}: {code}"),
            Self::InvalidTransition => formatter.write_str("invalid run state transition"),
            Self::StaleVersion => formatter.write_str("run version is stale"),
            Self::StaleEpoch => formatter.write_str("run epoch is stale"),
            Self::BusinessDataChanged => {
                formatter.write_str("business data changed during step execution")
            }
            Self::Database(_) => formatter.write_str("database operation failed"),
        }
    }
}

impl std::error::Error for NovelAutopilotRepositoryError {}

#[derive(Debug, Clone)]
pub(crate) struct CreateNovelAutopilotRun {
    pub project_id: String,
    pub user_id: String,
    pub total_chapters: u32,
    pub config: NovelAutopilotRunConfig,
}

#[derive(Debug, Clone)]
pub(crate) struct CreateOrGetActiveRunResult {
    pub run: novel_autopilot_run::Model,
    pub created: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct CreateNovelAutopilotStepAttempt {
    pub run_id: String,
    pub user_id: String,
    pub step_key: String,
    pub step_type: NovelAutopilotStepType,
    pub phase: NovelAutopilotPhase,
    pub chapter_id: Option<String>,
    pub chapter_number: Option<u32>,
    pub run_epoch: i64,
    pub input_digest: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ClaimedNovelAutopilotStep {
    pub run: novel_autopilot_run::Model,
    pub step: novel_autopilot_step_run::Model,
}

/// Creates the next attempt and claims it in a single transaction.
///
/// A durable coordinator must use this instead of `create_step_attempt` followed by
/// `claim_step`: the latter is intentionally still available for reconciliation and
/// tests, but has a scheduling gap between the two operations.
#[derive(Debug, Clone)]
pub(crate) struct PrepareAndClaimNovelAutopilotStep {
    pub attempt: CreateNovelAutopilotStepAttempt,
    pub expected_run_version: i64,
    pub background_task_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct NovelAutopilotStepTerminalPatch {
    pub result_digest: Option<String>,
    pub quality_decision: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotFoundationSnapshot {
    pub title: String,
    pub description: Option<String>,
    pub theme: Option<String>,
    pub genre: Option<String>,
    pub narrative_perspective: Option<String>,
}

impl NovelAutopilotFoundationSnapshot {
    pub(crate) fn from_project(project: &project::Model) -> Self {
        Self {
            title: project.title.clone(),
            description: project.description.clone(),
            theme: project.theme.clone(),
            genre: project.genre.clone(),
            narrative_perspective: project.narrative_perspective.clone(),
        }
    }

    pub(crate) fn is_complete(&self) -> bool {
        !self.title.trim().is_empty()
            && [
                self.description.as_deref(),
                self.theme.as_deref(),
                self.genre.as_deref(),
                self.narrative_perspective.as_deref(),
            ]
            .into_iter()
            .all(|value| value.is_some_and(|value| !value.trim().is_empty()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotFoundationCommit {
    pub title: String,
    pub description: String,
    pub theme: String,
    pub genre: String,
    pub narrative_perspective: String,
    pub result_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotWorldSnapshot {
    pub time_period: Option<String>,
    pub location: Option<String>,
    pub atmosphere: Option<String>,
    pub rules: Option<String>,
}

impl NovelAutopilotWorldSnapshot {
    pub(crate) fn from_project(project: &project::Model) -> Self {
        Self {
            time_period: project.world_time_period.clone(),
            location: project.world_location.clone(),
            atmosphere: project.world_atmosphere.clone(),
            rules: project.world_rules.clone(),
        }
    }

    pub(crate) fn is_blank(&self) -> bool {
        [
            self.time_period.as_deref(),
            self.location.as_deref(),
            self.atmosphere.as_deref(),
            self.rules.as_deref(),
        ]
        .into_iter()
        .all(|value| value.is_none_or(|value| value.trim().is_empty()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotWorldCommit {
    pub time_period: String,
    pub location: String,
    pub atmosphere: String,
    pub rules: String,
    pub result_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotCareerSnapshot {
    pub career_ids: Vec<String>,
}

impl NovelAutopilotCareerSnapshot {
    pub(crate) async fn load(
        db: &DatabaseConnection,
        project_id: &str,
    ) -> Result<Self, NovelAutopilotRepositoryError> {
        let mut career_ids = career::Entity::find()
            .filter(career::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|career| career.id)
            .collect::<Vec<_>>();
        career_ids.sort();
        Ok(Self { career_ids })
    }

    pub(crate) fn is_blank(&self) -> bool {
        self.career_ids.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotCareerItemCommit {
    pub name: String,
    pub career_type: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub stages: String,
    pub max_stage: i32,
    pub requirements: Option<String>,
    pub special_abilities: Option<String>,
    pub worldview_rules: Option<String>,
    pub attribute_bonuses: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotCareerCommit {
    pub careers: Vec<NovelAutopilotCareerItemCommit>,
    pub result_digest: String,
}

/// Snapshot of organization ownership and the existing project characters that a
/// generated organization may reference. It is intentionally internal-only and
/// never exposed in API/task results.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NovelAutopilotOrganizationSnapshot {
    pub organization_ids: Vec<String>,
    pub legacy_organization_character_ids: Vec<String>,
    character_fingerprints: Vec<NovelAutopilotOrganizationCharacterFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NovelAutopilotOrganizationCharacterFingerprint {
    id: String,
    name: String,
    is_organization: bool,
    fingerprint: String,
}

impl NovelAutopilotOrganizationSnapshot {
    pub(crate) async fn load(
        db: &DatabaseConnection,
        project_id: &str,
    ) -> Result<Self, NovelAutopilotRepositoryError> {
        let organization_ids = organization::Entity::find()
            .filter(organization::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|organization| organization.id)
            .collect::<Vec<_>>();
        let characters = character::Entity::find()
            .filter(character::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(database_error)?;
        Ok(Self::from_models(organization_ids, characters))
    }

    fn from_models(mut organization_ids: Vec<String>, characters: Vec<character::Model>) -> Self {
        organization_ids.sort();
        let mut legacy_organization_character_ids = characters
            .iter()
            .filter(|character| character.is_organization)
            .map(|character| character.id.clone())
            .collect::<Vec<_>>();
        legacy_organization_character_ids.sort();
        let mut character_fingerprints = characters
            .into_iter()
            .map(|character| NovelAutopilotOrganizationCharacterFingerprint {
                id: character.id.clone(),
                name: character.name.clone(),
                is_organization: character.is_organization,
                // The snapshot is a CAS guard, not a public DTO. Debug formatting captures
                // every business field, including edits made while the model is running.
                fingerprint: format!("{character:?}"),
            })
            .collect::<Vec<_>>();
        character_fingerprints.sort_by(|left, right| left.id.cmp(&right.id));
        Self {
            organization_ids,
            legacy_organization_character_ids,
            character_fingerprints,
        }
    }

    pub(crate) fn is_blank(&self) -> bool {
        self.organization_ids.is_empty() && self.legacy_organization_character_ids.is_empty()
    }

    pub(crate) fn find_unique_non_organization_character_id(&self, name: &str) -> Option<String> {
        self.find_unique_character_id(name, false)
    }

    pub(crate) fn find_unique_organization_character_id(&self, name: &str) -> Option<String> {
        self.find_unique_character_id(name, true)
    }

    fn find_unique_character_id(&self, name: &str, is_organization: bool) -> Option<String> {
        let mut matches = self.character_fingerprints.iter().filter(|character| {
            character.is_organization == is_organization && character.name == name
        });
        let id = matches.next()?.id.clone();
        if matches.next().is_some() {
            None
        } else {
            Some(id)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotOrganizationMemberCommit {
    pub character_id: String,
    pub position: String,
    pub rank: i32,
    pub status: String,
    pub joined_at: Option<String>,
    pub loyalty: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotOrganizationRelationshipCommit {
    pub target_organization_character_id: String,
    pub relationship_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotOrganizationCommit {
    pub name: String,
    pub organization_type: String,
    pub personality: Option<String>,
    pub background: Option<String>,
    pub appearance: Option<String>,
    pub organization_purpose: Option<String>,
    pub traits: String,
    pub power_level: i32,
    pub location: Option<String>,
    pub motto: Option<String>,
    pub color: Option<String>,
    pub members: Vec<NovelAutopilotOrganizationMemberCommit>,
    pub relationships: Vec<NovelAutopilotOrganizationRelationshipCommit>,
    pub result_digest: String,
}

pub(crate) struct NovelAutopilotRepository;

impl NovelAutopilotRepository {
    pub(crate) async fn create_or_get_active(
        db: &DatabaseConnection,
        input: CreateNovelAutopilotRun,
    ) -> Result<CreateOrGetActiveRunResult, NovelAutopilotRepositoryError> {
        input
            .config
            .validate()
            .map_err(|error| NovelAutopilotRepositoryError::InvalidConfig {
                field: error.field,
                code: error.code,
            })?;
        if input.total_chapters > input.config.max_chapters {
            return Err(NovelAutopilotRepositoryError::InvalidConfig {
                field: "total_chapters",
                code: "exceeds_max_chapters",
            });
        }
        ensure_project_owned(db, &input.project_id, &input.user_id).await?;

        if let Some(existing) = find_active_by_project(db, &input.project_id).await? {
            if existing.user_id == input.user_id {
                return Ok(CreateOrGetActiveRunResult {
                    run: existing,
                    created: false,
                });
            }
            return Err(NovelAutopilotRepositoryError::NotFoundOrAccessDenied);
        }

        let now = Utc::now().naive_utc();
        let max_chapters = i32::try_from(input.config.max_chapters).map_err(|_| {
            NovelAutopilotRepositoryError::InvalidConfig {
                field: "max_chapters",
                code: "out_of_range",
            }
        })?;
        let max_tokens = i64::try_from(input.config.max_tokens).map_err(|_| {
            NovelAutopilotRepositoryError::InvalidConfig {
                field: "max_tokens",
                code: "out_of_range",
            }
        })?;
        let max_runtime_seconds =
            i64::try_from(input.config.max_runtime_seconds).map_err(|_| {
                NovelAutopilotRepositoryError::InvalidConfig {
                    field: "max_runtime_seconds",
                    code: "out_of_range",
                }
            })?;
        let total_chapters = i32::try_from(input.total_chapters).map_err(|_| {
            NovelAutopilotRepositoryError::InvalidConfig {
                field: "total_chapters",
                code: "out_of_range",
            }
        })?;
        let config_snapshot =
            serde_json::to_value(NovelAutopilotPrivateSnapshot::new(input.config.clone()))
                .map_err(database_error)?;
        let active = novel_autopilot_run::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(input.project_id.clone()),
            user_id: Set(input.user_id.clone()),
            schema_version: Set(NOVEL_AUTOPILOT_SCHEMA_VERSION.to_string()),
            status: Set(NovelAutopilotRunStatus::Queued.as_str().to_string()),
            current_phase: Set(NovelAutopilotPhase::Validate.as_str().to_string()),
            current_step: Set(None),
            active_scope_key: Set(Some(input.project_id.clone())),
            current_chapter_id: Set(None),
            current_chapter_number: Set(None),
            total_chapters: Set(total_chapters),
            completed_chapters: Set(0),
            failed_chapters: Set(json!([])),
            pending_rewrites: Set(json!([])),
            total_word_count: Set(0),
            execution_scope: Set(input.config.execution_scope.as_str().to_string()),
            human_gate_mode: Set(input.config.human_gate_mode.as_str().to_string()),
            gate_interval: Set(Some(input.config.gate_interval as i32)),
            config_snapshot: Set(config_snapshot),
            max_chapters: Set(Some(max_chapters)),
            max_tokens: Set(Some(max_tokens)),
            max_estimated_cost: Set(input.config.max_estimated_cost),
            max_runtime_seconds: Set(Some(max_runtime_seconds)),
            used_tokens: Set(0),
            estimated_cost: Set(0.0),
            epoch: Set(0),
            version: Set(0),
            consecutive_provider_failures: Set(0),
            consecutive_quality_failures: Set(0),
            last_error_code: Set(None),
            guidance_digest: Set(None),
            active_background_task_id: Set(None),
            final_export_ref: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            started_at: Set(None),
            paused_at: Set(None),
            completed_at: Set(None),
        };

        match active.insert(db).await {
            Ok(run) => Ok(CreateOrGetActiveRunResult { run, created: true }),
            Err(insert_error) => {
                if let Some(existing) = find_active_by_project(db, &input.project_id).await? {
                    if existing.user_id == input.user_id {
                        return Ok(CreateOrGetActiveRunResult {
                            run: existing,
                            created: false,
                        });
                    }
                }
                Err(database_error(insert_error))
            }
        }
    }

    pub(crate) async fn find_owned(
        db: &DatabaseConnection,
        run_id: &str,
        user_id: &str,
    ) -> Result<novel_autopilot_run::Model, NovelAutopilotRepositoryError> {
        let run = novel_autopilot_run::Entity::find_by_id(run_id)
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        ensure_project_owned(db, &run.project_id, user_id).await?;
        Ok(run)
    }

    pub(crate) async fn list_owned(
        db: &DatabaseConnection,
        project_id: &str,
        user_id: &str,
    ) -> Result<Vec<novel_autopilot_run::Model>, NovelAutopilotRepositoryError> {
        ensure_project_owned(db, project_id, user_id).await?;
        novel_autopilot_run::Entity::find()
            .filter(novel_autopilot_run::Column::ProjectId.eq(project_id))
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .order_by_desc(novel_autopilot_run::Column::CreatedAt)
            .all(db)
            .await
            .map_err(database_error)
    }

    pub(crate) async fn list_steps_owned(
        db: &DatabaseConnection,
        run_id: &str,
        user_id: &str,
    ) -> Result<Vec<novel_autopilot_step_run::Model>, NovelAutopilotRepositoryError> {
        Self::find_owned(db, run_id, user_id).await?;
        novel_autopilot_step_run::Entity::find()
            .filter(novel_autopilot_step_run::Column::RunId.eq(run_id))
            .order_by_asc(novel_autopilot_step_run::Column::CreatedAt)
            .order_by_asc(novel_autopilot_step_run::Column::Attempt)
            .all(db)
            .await
            .map_err(database_error)
    }

    pub(crate) async fn list_startup_recoverable(
        db: &DatabaseConnection,
    ) -> Result<Vec<novel_autopilot_run::Model>, NovelAutopilotRepositoryError> {
        novel_autopilot_run::Entity::find()
            .filter(novel_autopilot_run::Column::Status.is_in([
                NovelAutopilotRunStatus::Queued.as_str(),
                NovelAutopilotRunStatus::Running.as_str(),
            ]))
            .order_by_asc(novel_autopilot_run::Column::CreatedAt)
            .all(db)
            .await
            .map_err(database_error)
    }

    /// Fences work that belonged to a previous server process and returns the Run
    /// to a schedulable state.  This is intentionally a dedicated recovery CAS:
    /// normal product transitions must continue to use `transition_owned`.
    pub(crate) async fn prepare_startup_recovery(
        db: &DatabaseConnection,
        run_id: &str,
        expected_version: i64,
        expected_epoch: i64,
    ) -> Result<novel_autopilot_run::Model, NovelAutopilotRepositoryError> {
        let current = novel_autopilot_run::Entity::find_by_id(run_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        if current.version != expected_version {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        if current.epoch != expected_epoch {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }
        let current_status = current
            .status
            .parse::<NovelAutopilotRunStatus>()
            .map_err(|_| NovelAutopilotRepositoryError::InvalidTransition)?;
        if !current_status.can_schedule() {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(database_error)?;
        if let Some(current_step) = current.current_step.as_deref() {
            novel_autopilot_step_run::Entity::update_many()
                .col_expr(
                    novel_autopilot_step_run::Column::Status,
                    Expr::value(NovelAutopilotStepStatus::Stale.as_str()),
                )
                .col_expr(
                    novel_autopilot_step_run::Column::ErrorCode,
                    Expr::value(Some("service_restarted".to_string())),
                )
                .col_expr(
                    novel_autopilot_step_run::Column::CompletedAt,
                    Expr::value(Some(now)),
                )
                .col_expr(
                    novel_autopilot_step_run::Column::UpdatedAt,
                    Expr::value(now),
                )
                .filter(novel_autopilot_step_run::Column::RunId.eq(run_id))
                .filter(novel_autopilot_step_run::Column::StepKey.eq(current_step))
                .filter(novel_autopilot_step_run::Column::RunEpoch.eq(expected_epoch))
                .filter(
                    novel_autopilot_step_run::Column::Status
                        .eq(NovelAutopilotStepStatus::Running.as_str()),
                )
                .exec(&txn)
                .await
                .map_err(database_error)?;
        }

        let updated = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::Status,
                Expr::value(NovelAutopilotRunStatus::Queued.as_str()),
            )
            .col_expr(
                novel_autopilot_run::Column::CurrentStep,
                Expr::value(None::<String>),
            )
            .col_expr(
                novel_autopilot_run::Column::ActiveBackgroundTaskId,
                Expr::value(None::<String>),
            )
            .col_expr(
                novel_autopilot_run::Column::Epoch,
                Expr::col(novel_autopilot_run::Column::Epoch).add(1),
            )
            .col_expr(
                novel_autopilot_run::Column::Version,
                Expr::col(novel_autopilot_run::Column::Version).add(1),
            )
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now))
            .filter(novel_autopilot_run::Column::Id.eq(run_id))
            .filter(novel_autopilot_run::Column::Version.eq(expected_version))
            .filter(novel_autopilot_run::Column::Epoch.eq(expected_epoch))
            .filter(novel_autopilot_run::Column::Status.eq(current_status.as_str()))
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if updated.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        txn.commit().await.map_err(database_error)?;
        Self::find_owned(db, run_id, &current.user_id).await
    }

    pub(crate) async fn transition_owned(
        db: &DatabaseConnection,
        run_id: &str,
        user_id: &str,
        expected_version: i64,
        target: NovelAutopilotRunStatus,
    ) -> Result<novel_autopilot_run::Model, NovelAutopilotRepositoryError> {
        let current = Self::find_owned(db, run_id, user_id).await?;
        if current.version != expected_version {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        let current_status = current
            .status
            .parse::<NovelAutopilotRunStatus>()
            .map_err(|_| NovelAutopilotRepositoryError::InvalidTransition)?;
        if !current_status.can_transition_to(target) {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        let now = Utc::now().naive_utc();
        let bump_epoch = matches!(
            target,
            NovelAutopilotRunStatus::Paused
                | NovelAutopilotRunStatus::Queued
                | NovelAutopilotRunStatus::Cancelled
        );
        let txn = db.begin().await.map_err(database_error)?;

        // A paused or cancelled Run must never retain a running Step cursor.  The
        // epoch fence alone rejects late writes, but terminalising the active Step
        // is also required so a later resume can schedule a fresh attempt.
        if matches!(
            target,
            NovelAutopilotRunStatus::Paused | NovelAutopilotRunStatus::Cancelled
        ) {
            if let Some(current_step) = current.current_step.as_deref() {
                let (step_status, error_code) = match target {
                    NovelAutopilotRunStatus::Paused => {
                        (NovelAutopilotStepStatus::Stale, "run_paused")
                    }
                    NovelAutopilotRunStatus::Cancelled => {
                        (NovelAutopilotStepStatus::Cancelled, "run_cancelled")
                    }
                    _ => unreachable!("guarded by target match"),
                };
                let interrupted = novel_autopilot_step_run::Entity::update_many()
                    .col_expr(
                        novel_autopilot_step_run::Column::Status,
                        Expr::value(step_status.as_str()),
                    )
                    .col_expr(
                        novel_autopilot_step_run::Column::ErrorCode,
                        Expr::value(Some(error_code.to_string())),
                    )
                    .col_expr(
                        novel_autopilot_step_run::Column::CompletedAt,
                        Expr::value(Some(now)),
                    )
                    .col_expr(
                        novel_autopilot_step_run::Column::UpdatedAt,
                        Expr::value(now),
                    )
                    .filter(novel_autopilot_step_run::Column::RunId.eq(run_id))
                    .filter(novel_autopilot_step_run::Column::StepKey.eq(current_step))
                    .filter(novel_autopilot_step_run::Column::RunEpoch.eq(current.epoch))
                    .filter(
                        novel_autopilot_step_run::Column::Status
                            .eq(NovelAutopilotStepStatus::Running.as_str()),
                    )
                    .exec(&txn)
                    .await
                    .map_err(database_error)?;
                if interrupted.rows_affected != 1 {
                    // The Step may have completed between the owner-scoped read and
                    // this transition attempt. Its Run version is then stale, so do
                    // not misclassify the caller-visible CAS conflict as a malformed
                    // state transition.
                    return Err(NovelAutopilotRepositoryError::StaleVersion);
                }
            }
        }

        let mut update = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::Status,
                Expr::value(target.as_str()),
            )
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                novel_autopilot_run::Column::Version,
                Expr::col(novel_autopilot_run::Column::Version).add(1),
            )
            .filter(novel_autopilot_run::Column::Id.eq(run_id))
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .filter(novel_autopilot_run::Column::Version.eq(expected_version))
            .filter(novel_autopilot_run::Column::Status.eq(current_status.as_str()));
        if bump_epoch {
            update = update.col_expr(
                novel_autopilot_run::Column::Epoch,
                Expr::col(novel_autopilot_run::Column::Epoch).add(1),
            );
        }
        if target == NovelAutopilotRunStatus::Running {
            update = update
                .col_expr(
                    novel_autopilot_run::Column::StartedAt,
                    Expr::value(current.started_at.or(Some(now))),
                )
                .col_expr(
                    novel_autopilot_run::Column::PausedAt,
                    Expr::value(None::<chrono::NaiveDateTime>),
                );
        }
        if target == NovelAutopilotRunStatus::Paused {
            update = update
                .col_expr(
                    novel_autopilot_run::Column::PausedAt,
                    Expr::value(Some(now)),
                )
                .col_expr(
                    novel_autopilot_run::Column::CurrentStep,
                    Expr::value(None::<String>),
                );
        }
        if matches!(
            target,
            NovelAutopilotRunStatus::Queued
                | NovelAutopilotRunStatus::Paused
                | NovelAutopilotRunStatus::WaitingHuman
        ) || target.is_terminal()
        {
            update = update.col_expr(
                novel_autopilot_run::Column::ActiveBackgroundTaskId,
                Expr::value(None::<String>),
            );
        }
        if target.is_terminal() {
            update = update
                .col_expr(
                    novel_autopilot_run::Column::ActiveScopeKey,
                    Expr::value(None::<String>),
                )
                .col_expr(
                    novel_autopilot_run::Column::CurrentStep,
                    Expr::value(None::<String>),
                )
                .col_expr(
                    novel_autopilot_run::Column::CompletedAt,
                    Expr::value(Some(now)),
                );
        }

        let result = update.exec(&txn).await.map_err(database_error)?;
        if result.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        txn.commit().await.map_err(database_error)?;
        Self::find_owned(db, run_id, user_id).await
    }

    pub(crate) async fn set_active_background_task_owned(
        db: &DatabaseConnection,
        run_id: &str,
        user_id: &str,
        expected_version: i64,
        expected_epoch: i64,
        task_id: Option<&str>,
    ) -> Result<novel_autopilot_run::Model, NovelAutopilotRepositoryError> {
        let current = Self::find_owned(db, run_id, user_id).await?;
        if current.version != expected_version {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        if current.epoch != expected_epoch {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }
        let status = current
            .status
            .parse::<NovelAutopilotRunStatus>()
            .map_err(|_| NovelAutopilotRepositoryError::InvalidTransition)?;
        if !matches!(
            status,
            NovelAutopilotRunStatus::Queued | NovelAutopilotRunStatus::Running
        ) {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }
        if task_id.is_some_and(str::is_empty) {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        let now = Utc::now().naive_utc();
        let result = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::ActiveBackgroundTaskId,
                Expr::value(task_id.map(str::to_string)),
            )
            .col_expr(
                novel_autopilot_run::Column::Version,
                Expr::col(novel_autopilot_run::Column::Version).add(1),
            )
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now))
            .filter(novel_autopilot_run::Column::Id.eq(run_id))
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .filter(novel_autopilot_run::Column::Version.eq(expected_version))
            .filter(novel_autopilot_run::Column::Epoch.eq(expected_epoch))
            .filter(novel_autopilot_run::Column::Status.eq(status.as_str()))
            .exec(db)
            .await
            .map_err(database_error)?;
        if result.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        Self::find_owned(db, run_id, user_id).await
    }

    pub(crate) async fn latest_step_attempt_owned(
        db: &DatabaseConnection,
        run_id: &str,
        user_id: &str,
        step_key: &str,
    ) -> Result<Option<i32>, NovelAutopilotRepositoryError> {
        Self::find_owned(db, run_id, user_id).await?;
        let latest = novel_autopilot_step_run::Entity::find()
            .filter(novel_autopilot_step_run::Column::RunId.eq(run_id))
            .filter(novel_autopilot_step_run::Column::StepKey.eq(step_key))
            .order_by_desc(novel_autopilot_step_run::Column::Attempt)
            .one(db)
            .await
            .map_err(database_error)?;
        Ok(latest.map(|step| step.attempt))
    }

    pub(crate) async fn wait_for_budget_owned(
        db: &DatabaseConnection,
        run_id: &str,
        user_id: &str,
        expected_version: i64,
        expected_epoch: i64,
        expected_background_task_id: Option<&str>,
        error_code: &str,
    ) -> Result<novel_autopilot_run::Model, NovelAutopilotRepositoryError> {
        if error_code.is_empty() || error_code.len() > 160 {
            return Err(NovelAutopilotRepositoryError::InvalidConfig {
                field: "error_code",
                code: "invalid_length",
            });
        }
        let now = Utc::now().naive_utc();
        let update = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::Status,
                Expr::value(NovelAutopilotRunStatus::WaitingHuman.as_str()),
            )
            .col_expr(
                novel_autopilot_run::Column::LastErrorCode,
                Expr::value(Some(error_code.to_string())),
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
            .filter(novel_autopilot_run::Column::Id.eq(run_id))
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .filter(novel_autopilot_run::Column::Version.eq(expected_version))
            .filter(novel_autopilot_run::Column::Epoch.eq(expected_epoch))
            .filter(novel_autopilot_run::Column::CurrentStep.is_null())
            .filter(
                novel_autopilot_run::Column::Status.eq(NovelAutopilotRunStatus::Running.as_str()),
            );
        let update = match expected_background_task_id {
            Some(task_id) => {
                update.filter(novel_autopilot_run::Column::ActiveBackgroundTaskId.eq(task_id))
            }
            None => update.filter(novel_autopilot_run::Column::ActiveBackgroundTaskId.is_null()),
        };
        let result = update.exec(db).await.map_err(database_error)?;
        if result.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        Self::find_owned(db, run_id, user_id).await
    }

    pub(crate) async fn increment_estimated_usage_owned(
        db: &DatabaseConnection,
        run_id: &str,
        user_id: &str,
        expected_version: i64,
        expected_epoch: i64,
        expected_background_task_id: Option<&str>,
        estimated_tokens_delta: u64,
    ) -> Result<novel_autopilot_run::Model, NovelAutopilotRepositoryError> {
        let current = Self::find_owned(db, run_id, user_id).await?;
        if current.version != expected_version {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        if current.epoch != expected_epoch {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }
        if current.active_background_task_id.as_deref() != expected_background_task_id {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }
        let delta = i64::try_from(estimated_tokens_delta).unwrap_or(i64::MAX);
        let used_tokens = current.used_tokens.saturating_add(delta);
        let now = Utc::now().naive_utc();
        let update = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::UsedTokens,
                Expr::value(used_tokens),
            )
            .col_expr(
                novel_autopilot_run::Column::Version,
                Expr::col(novel_autopilot_run::Column::Version).add(1),
            )
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now))
            .filter(novel_autopilot_run::Column::Id.eq(run_id))
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .filter(novel_autopilot_run::Column::Version.eq(expected_version))
            .filter(novel_autopilot_run::Column::Epoch.eq(expected_epoch));
        let update = match expected_background_task_id {
            Some(task_id) => {
                update.filter(novel_autopilot_run::Column::ActiveBackgroundTaskId.eq(task_id))
            }
            None => update.filter(novel_autopilot_run::Column::ActiveBackgroundTaskId.is_null()),
        };
        let result = update.exec(db).await.map_err(database_error)?;
        if result.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        Self::find_owned(db, run_id, user_id).await
    }

    pub(crate) async fn update_guidance(
        db: &DatabaseConnection,
        run_id: &str,
        user_id: &str,
        expected_version: i64,
        guidance: &str,
        guidance_digest: &str,
    ) -> Result<novel_autopilot_run::Model, NovelAutopilotRepositoryError> {
        let current = Self::find_owned(db, run_id, user_id).await?;
        let status = current
            .status
            .parse::<NovelAutopilotRunStatus>()
            .map_err(|_| NovelAutopilotRepositoryError::InvalidTransition)?;
        if current.version != expected_version {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        if !matches!(
            status,
            NovelAutopilotRunStatus::Paused | NovelAutopilotRunStatus::WaitingHuman
        ) || guidance.is_empty()
            || guidance_digest.is_empty()
            || guidance_digest.len() > 128
        {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }
        let mut private_snapshot = NovelAutopilotPrivateSnapshot::decode(&current.config_snapshot)
            .map_err(|_| NovelAutopilotRepositoryError::InvalidConfig {
                field: "config_snapshot",
                code: "invalid_private_snapshot",
            })?;
        private_snapshot.guidance = Some(guidance.to_string());
        let config_snapshot = serde_json::to_value(private_snapshot).map_err(database_error)?;

        let now = Utc::now().naive_utc();
        let result = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::ConfigSnapshot,
                Expr::value(config_snapshot),
            )
            .col_expr(
                novel_autopilot_run::Column::GuidanceDigest,
                Expr::value(Some(guidance_digest.to_string())),
            )
            .col_expr(
                novel_autopilot_run::Column::Epoch,
                Expr::col(novel_autopilot_run::Column::Epoch).add(1),
            )
            .col_expr(
                novel_autopilot_run::Column::Version,
                Expr::col(novel_autopilot_run::Column::Version).add(1),
            )
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now))
            .filter(novel_autopilot_run::Column::Id.eq(run_id))
            .filter(novel_autopilot_run::Column::UserId.eq(user_id))
            .filter(novel_autopilot_run::Column::Version.eq(expected_version))
            .filter(novel_autopilot_run::Column::Status.eq(status.as_str()))
            .exec(db)
            .await
            .map_err(database_error)?;
        if result.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        Self::find_owned(db, run_id, user_id).await
    }

    pub(crate) async fn prepare_and_claim_step(
        db: &DatabaseConnection,
        input: PrepareAndClaimNovelAutopilotStep,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        let attempt = input.attempt;
        if attempt.step_key.is_empty() || attempt.step_key.len() > 160 {
            return Err(NovelAutopilotRepositoryError::InvalidConfig {
                field: "step_key",
                code: "invalid_length",
            });
        }
        if attempt.input_digest.is_empty() || attempt.input_digest.len() > 128 {
            return Err(NovelAutopilotRepositoryError::InvalidConfig {
                field: "input_digest",
                code: "invalid_length",
            });
        }
        let chapter_number = attempt
            .chapter_number
            .map(i32::try_from)
            .transpose()
            .map_err(|_| NovelAutopilotRepositoryError::InvalidConfig {
                field: "chapter_number",
                code: "out_of_range",
            })?;

        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(database_error)?;
        let run = novel_autopilot_run::Entity::find_by_id(&attempt.run_id)
            .filter(novel_autopilot_run::Column::UserId.eq(&attempt.user_id))
            .one(&txn)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let run_status = run
            .status
            .parse::<NovelAutopilotRunStatus>()
            .map_err(|_| NovelAutopilotRepositoryError::InvalidTransition)?;
        if run.version != input.expected_run_version {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        if run.epoch != attempt.run_epoch {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }
        if !run_status.can_schedule() || run.current_step.is_some() {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        let previous = novel_autopilot_step_run::Entity::find()
            .filter(novel_autopilot_step_run::Column::RunId.eq(&attempt.run_id))
            .filter(novel_autopilot_step_run::Column::StepKey.eq(&attempt.step_key))
            .order_by_desc(novel_autopilot_step_run::Column::Attempt)
            .one(&txn)
            .await
            .map_err(database_error)?;
        let next_attempt = previous.map_or(1, |step| step.attempt.saturating_add(1));
        let step_id = Uuid::new_v4().to_string();
        novel_autopilot_step_run::ActiveModel {
            id: Set(step_id.clone()),
            run_id: Set(attempt.run_id.clone()),
            step_key: Set(attempt.step_key.clone()),
            step_type: Set(attempt.step_type.as_str().to_string()),
            phase: Set(attempt.phase.as_str().to_string()),
            chapter_id: Set(attempt.chapter_id),
            chapter_number: Set(chapter_number),
            attempt: Set(next_attempt),
            run_epoch: Set(attempt.run_epoch),
            status: Set(NovelAutopilotStepStatus::Running.as_str().to_string()),
            background_task_id: Set(input.background_task_id.clone()),
            input_digest: Set(attempt.input_digest),
            result_digest: Set(None),
            quality_decision: Set(None),
            error_code: Set(None),
            started_at: Set(Some(now)),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&txn)
        .await
        .map_err(database_error)?;

        let run_update = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::Status,
                Expr::value(NovelAutopilotRunStatus::Running.as_str()),
            )
            .col_expr(
                novel_autopilot_run::Column::CurrentPhase,
                Expr::value(attempt.phase.as_str()),
            )
            .col_expr(
                novel_autopilot_run::Column::CurrentStep,
                Expr::value(Some(attempt.step_key)),
            )
            .col_expr(
                novel_autopilot_run::Column::ActiveBackgroundTaskId,
                Expr::value(input.background_task_id),
            )
            .col_expr(
                novel_autopilot_run::Column::StartedAt,
                Expr::value(run.started_at.or(Some(now))),
            )
            .col_expr(
                novel_autopilot_run::Column::Version,
                Expr::col(novel_autopilot_run::Column::Version).add(1),
            )
            .col_expr(novel_autopilot_run::Column::UpdatedAt, Expr::value(now))
            .filter(novel_autopilot_run::Column::Id.eq(&run.id))
            .filter(novel_autopilot_run::Column::UserId.eq(&attempt.user_id))
            .filter(novel_autopilot_run::Column::Version.eq(input.expected_run_version))
            .filter(novel_autopilot_run::Column::Epoch.eq(attempt.run_epoch))
            .filter(novel_autopilot_run::Column::CurrentStep.is_null())
            .filter(novel_autopilot_run::Column::Status.is_in([
                NovelAutopilotRunStatus::Queued.as_str(),
                NovelAutopilotRunStatus::Running.as_str(),
            ]))
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if run_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        txn.commit().await.map_err(database_error)?;

        Ok(ClaimedNovelAutopilotStep {
            run: Self::find_owned(db, &run.id, &attempt.user_id).await?,
            step: novel_autopilot_step_run::Entity::find_by_id(step_id)
                .one(db)
                .await
                .map_err(database_error)?
                .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?,
        })
    }

    pub(crate) async fn create_step_attempt(
        db: &DatabaseConnection,
        input: CreateNovelAutopilotStepAttempt,
    ) -> Result<novel_autopilot_step_run::Model, NovelAutopilotRepositoryError> {
        let run = Self::find_owned(db, &input.run_id, &input.user_id).await?;
        let run_status = run
            .status
            .parse::<NovelAutopilotRunStatus>()
            .map_err(|_| NovelAutopilotRepositoryError::InvalidTransition)?;
        if run.epoch != input.run_epoch {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }
        if !run_status.can_schedule() {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }
        if input.step_key.is_empty() || input.step_key.len() > 160 {
            return Err(NovelAutopilotRepositoryError::InvalidConfig {
                field: "step_key",
                code: "invalid_length",
            });
        }
        if input.input_digest.is_empty() || input.input_digest.len() > 128 {
            return Err(NovelAutopilotRepositoryError::InvalidConfig {
                field: "input_digest",
                code: "invalid_length",
            });
        }

        let previous = novel_autopilot_step_run::Entity::find()
            .filter(novel_autopilot_step_run::Column::RunId.eq(&input.run_id))
            .filter(novel_autopilot_step_run::Column::StepKey.eq(&input.step_key))
            .order_by_desc(novel_autopilot_step_run::Column::Attempt)
            .one(db)
            .await
            .map_err(database_error)?;
        let attempt = previous.map_or(1, |step| step.attempt.saturating_add(1));
        let chapter_number = input
            .chapter_number
            .map(i32::try_from)
            .transpose()
            .map_err(|_| NovelAutopilotRepositoryError::InvalidConfig {
                field: "chapter_number",
                code: "out_of_range",
            })?;
        let now = Utc::now().naive_utc();
        novel_autopilot_step_run::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            run_id: Set(input.run_id),
            step_key: Set(input.step_key),
            step_type: Set(input.step_type.as_str().to_string()),
            phase: Set(input.phase.as_str().to_string()),
            chapter_id: Set(input.chapter_id),
            chapter_number: Set(chapter_number),
            attempt: Set(attempt),
            run_epoch: Set(input.run_epoch),
            status: Set(NovelAutopilotStepStatus::Queued.as_str().to_string()),
            background_task_id: Set(None),
            input_digest: Set(input.input_digest),
            result_digest: Set(None),
            quality_decision: Set(None),
            error_code: Set(None),
            started_at: Set(None),
            completed_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .map_err(database_error)
    }

    pub(crate) async fn claim_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        background_task_id: Option<&str>,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        let step = novel_autopilot_step_run::Entity::find_by_id(step_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let run = Self::find_owned(db, &step.run_id, user_id).await?;
        if run.version != expected_run_version {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        if run.epoch != expected_run_epoch || step.run_epoch != expected_run_epoch {
            return Err(NovelAutopilotRepositoryError::StaleEpoch);
        }
        let status = run
            .status
            .parse::<NovelAutopilotRunStatus>()
            .map_err(|_| NovelAutopilotRepositoryError::InvalidTransition)?;
        if !status.can_schedule() || step.status != NovelAutopilotStepStatus::Queued.as_str() {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }

        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(database_error)?;
        let run_update = novel_autopilot_run::Entity::update_many()
            .col_expr(
                novel_autopilot_run::Column::Status,
                Expr::value(NovelAutopilotRunStatus::Running.as_str()),
            )
            .col_expr(
                novel_autopilot_run::Column::CurrentPhase,
                Expr::value(step.phase.clone()),
            )
            .col_expr(
                novel_autopilot_run::Column::CurrentStep,
                Expr::value(Some(step.step_key.clone())),
            )
            .col_expr(
                novel_autopilot_run::Column::ActiveBackgroundTaskId,
                Expr::value(background_task_id.map(str::to_string)),
            )
            .col_expr(
                novel_autopilot_run::Column::StartedAt,
                Expr::value(run.started_at.or(Some(now))),
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
            .filter(novel_autopilot_run::Column::CurrentStep.is_null())
            .filter(novel_autopilot_run::Column::Status.is_in([
                NovelAutopilotRunStatus::Queued.as_str(),
                NovelAutopilotRunStatus::Running.as_str(),
            ]))
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if run_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::StaleVersion);
        }
        let step_update = novel_autopilot_step_run::Entity::update_many()
            .col_expr(
                novel_autopilot_step_run::Column::Status,
                Expr::value(NovelAutopilotStepStatus::Running.as_str()),
            )
            .col_expr(
                novel_autopilot_step_run::Column::BackgroundTaskId,
                Expr::value(background_task_id.map(str::to_string)),
            )
            .col_expr(
                novel_autopilot_step_run::Column::StartedAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                novel_autopilot_step_run::Column::UpdatedAt,
                Expr::value(now),
            )
            .filter(novel_autopilot_step_run::Column::Id.eq(step_id))
            .filter(novel_autopilot_step_run::Column::RunEpoch.eq(expected_run_epoch))
            .filter(
                novel_autopilot_step_run::Column::Status
                    .eq(NovelAutopilotStepStatus::Queued.as_str()),
            )
            .exec(&txn)
            .await
            .map_err(database_error)?;
        if step_update.rows_affected != 1 {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }
        txn.commit().await.map_err(database_error)?;

        Ok(ClaimedNovelAutopilotStep {
            run: Self::find_owned(db, &run.id, user_id).await?,
            step: novel_autopilot_step_run::Entity::find_by_id(step_id)
                .one(db)
                .await
                .map_err(database_error)?
                .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn commit_foundation_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        expected_foundation: &NovelAutopilotFoundationSnapshot,
        foundation: NovelAutopilotFoundationCommit,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        if [
            foundation.title.as_str(),
            foundation.description.as_str(),
            foundation.theme.as_str(),
            foundation.genre.as_str(),
            foundation.narrative_perspective.as_str(),
        ]
        .into_iter()
        .any(|value| value.trim().is_empty())
        {
            return Err(NovelAutopilotRepositoryError::InvalidConfig {
                field: "foundation",
                code: "incomplete",
            });
        }

        let step = novel_autopilot_step_run::Entity::find_by_id(step_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let run = Self::find_owned(db, &step.run_id, user_id).await?;
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

        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(database_error)?;
        let project_update = project::Entity::update_many()
            .col_expr(project::Column::Title, Expr::value(foundation.title))
            .col_expr(
                project::Column::Description,
                Expr::value(Some(foundation.description)),
            )
            .col_expr(project::Column::Theme, Expr::value(Some(foundation.theme)))
            .col_expr(project::Column::Genre, Expr::value(Some(foundation.genre)))
            .col_expr(
                project::Column::NarrativePerspective,
                Expr::value(Some(foundation.narrative_perspective)),
            )
            .col_expr(project::Column::UpdatedAt, Expr::value(Some(now)))
            .filter(project::Column::Id.eq(&run.project_id))
            .filter(project::Column::UserId.eq(user_id))
            .filter(project_foundation_snapshot_condition(expected_foundation))
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
                Expr::value(Some(foundation.result_digest)),
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
            run: Self::find_owned(db, &run.id, user_id).await?,
            step: novel_autopilot_step_run::Entity::find_by_id(step_id)
                .one(db)
                .await
                .map_err(database_error)?
                .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?,
        })
    }

    pub(crate) async fn commit_world_building_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        expected_world: &NovelAutopilotWorldSnapshot,
        world: NovelAutopilotWorldCommit,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        if [
            world.time_period.as_str(),
            world.location.as_str(),
            world.atmosphere.as_str(),
            world.rules.as_str(),
        ]
        .into_iter()
        .any(|value| value.trim().is_empty())
        {
            return Err(NovelAutopilotRepositoryError::InvalidConfig {
                field: "world_building",
                code: "incomplete",
            });
        }

        let step = novel_autopilot_step_run::Entity::find_by_id(step_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let run = Self::find_owned(db, &step.run_id, user_id).await?;
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

        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(database_error)?;
        let project_update = project::Entity::update_many()
            .col_expr(
                project::Column::WorldTimePeriod,
                Expr::value(Some(world.time_period)),
            )
            .col_expr(
                project::Column::WorldLocation,
                Expr::value(Some(world.location)),
            )
            .col_expr(
                project::Column::WorldAtmosphere,
                Expr::value(Some(world.atmosphere)),
            )
            .col_expr(project::Column::WorldRules, Expr::value(Some(world.rules)))
            .col_expr(project::Column::UpdatedAt, Expr::value(Some(now)))
            .filter(project::Column::Id.eq(&run.project_id))
            .filter(project::Column::UserId.eq(user_id))
            .filter(project_world_snapshot_condition(expected_world))
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
                Expr::value(Some(world.result_digest)),
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
            run: Self::find_owned(db, &run.id, user_id).await?,
            step: novel_autopilot_step_run::Entity::find_by_id(step_id)
                .one(db)
                .await
                .map_err(database_error)?
                .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn commit_career_design_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        expected_careers: &NovelAutopilotCareerSnapshot,
        career_commit: NovelAutopilotCareerCommit,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        if career_commit.careers.is_empty() {
            return Err(NovelAutopilotRepositoryError::InvalidConfig {
                field: "career_system",
                code: "empty",
            });
        }
        if career_commit.careers.iter().any(|career| {
            career.name.trim().is_empty()
                || !matches!(career.career_type.as_str(), "main" | "sub")
                || career.stages.trim().is_empty()
                || career.max_stage <= 0
        }) {
            return Err(NovelAutopilotRepositoryError::InvalidConfig {
                field: "career_system",
                code: "invalid_item",
            });
        }

        let step = novel_autopilot_step_run::Entity::find_by_id(step_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let run = Self::find_owned(db, &step.run_id, user_id).await?;
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

        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(database_error)?;
        let mut current_career_ids = career::Entity::find()
            .filter(career::Column::ProjectId.eq(&run.project_id))
            .all(&txn)
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|career| career.id)
            .collect::<Vec<_>>();
        current_career_ids.sort();
        if current_career_ids != expected_careers.career_ids {
            return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
        }

        let career_models = career_commit
            .careers
            .into_iter()
            .map(|career| career::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                project_id: Set(run.project_id.clone()),
                name: Set(career.name),
                career_type: Set(career.career_type),
                description: Set(career.description),
                category: Set(career.category),
                stages: Set(career.stages),
                max_stage: Set(career.max_stage),
                requirements: Set(career.requirements),
                special_abilities: Set(career.special_abilities),
                worldview_rules: Set(career.worldview_rules),
                attribute_bonuses: Set(career.attribute_bonuses),
                source: Set("ai".to_string()),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            })
            .collect::<Vec<_>>();
        career::Entity::insert_many(career_models)
            .exec(&txn)
            .await
            .map_err(database_error)?;

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
                Expr::value(Some(career_commit.result_digest)),
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
            .filter(novel_autopilot_step_run::Column::RunId.eq(&run.id))
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
            run: Self::find_owned(db, &run.id, user_id).await?,
            step: novel_autopilot_step_run::Entity::find_by_id(step_id)
                .one(db)
                .await
                .map_err(database_error)?
                .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?,
        })
    }

    pub(crate) async fn complete_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        terminal_status: NovelAutopilotStepStatus,
        patch: NovelAutopilotStepTerminalPatch,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        if !terminal_status.is_terminal() {
            return Err(NovelAutopilotRepositoryError::InvalidTransition);
        }
        let step = novel_autopilot_step_run::Entity::find_by_id(step_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let run = Self::find_owned(db, &step.run_id, user_id).await?;
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

        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(database_error)?;
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
                Expr::value(terminal_status.as_str()),
            )
            .col_expr(
                novel_autopilot_step_run::Column::ResultDigest,
                Expr::value(patch.result_digest),
            )
            .col_expr(
                novel_autopilot_step_run::Column::QualityDecision,
                Expr::value(patch.quality_decision),
            )
            .col_expr(
                novel_autopilot_step_run::Column::ErrorCode,
                Expr::value(patch.error_code),
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
            run: Self::find_owned(db, &run.id, user_id).await?,
            step: novel_autopilot_step_run::Entity::find_by_id(step_id)
                .one(db)
                .await
                .map_err(database_error)?
                .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn commit_organization_design_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        expected_organizations: &NovelAutopilotOrganizationSnapshot,
        organization_commit: NovelAutopilotOrganizationCommit,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        validate_organization_commit(&organization_commit)?;

        let step = novel_autopilot_step_run::Entity::find_by_id(step_id)
            .one(db)
            .await
            .map_err(database_error)?
            .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?;
        let run = Self::find_owned(db, &step.run_id, user_id).await?;
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

        let now = Utc::now().naive_utc();
        let txn = db.begin().await.map_err(database_error)?;
        let current_organization_ids = organization::Entity::find()
            .filter(organization::Column::ProjectId.eq(&run.project_id))
            .all(&txn)
            .await
            .map_err(database_error)?
            .into_iter()
            .map(|organization| organization.id)
            .collect::<Vec<_>>();
        let current_characters = character::Entity::find()
            .filter(character::Column::ProjectId.eq(&run.project_id))
            .all(&txn)
            .await
            .map_err(database_error)?;
        let current_snapshot = NovelAutopilotOrganizationSnapshot::from_models(
            current_organization_ids,
            current_characters,
        );
        if &current_snapshot != expected_organizations {
            return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
        }

        let new_organization_character_id = Uuid::new_v4().to_string();
        let new_organization_id = Uuid::new_v4().to_string();
        let member_ids = organization_commit
            .members
            .iter()
            .map(|member| member.character_id.clone())
            .collect::<Vec<_>>();
        if !member_ids.is_empty() {
            let found_member_ids = character::Entity::find()
                .filter(character::Column::ProjectId.eq(&run.project_id))
                .filter(character::Column::IsOrganization.eq(false))
                .filter(character::Column::Id.is_in(member_ids.clone()))
                .all(&txn)
                .await
                .map_err(database_error)?
                .into_iter()
                .map(|character| character.id)
                .collect::<std::collections::HashSet<_>>();
            if found_member_ids.len() != member_ids.len()
                || member_ids.iter().any(|id| !found_member_ids.contains(id))
            {
                return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
            }
        }
        let target_organization_character_ids = organization_commit
            .relationships
            .iter()
            .map(|relationship| relationship.target_organization_character_id.clone())
            .collect::<Vec<_>>();
        if !target_organization_character_ids.is_empty() {
            let found_target_ids = character::Entity::find()
                .filter(character::Column::ProjectId.eq(&run.project_id))
                .filter(character::Column::IsOrganization.eq(true))
                .filter(character::Column::Id.is_in(target_organization_character_ids.clone()))
                .all(&txn)
                .await
                .map_err(database_error)?
                .into_iter()
                .map(|character| character.id)
                .collect::<std::collections::HashSet<_>>();
            if found_target_ids.len() != target_organization_character_ids.len()
                || target_organization_character_ids
                    .iter()
                    .any(|id| !found_target_ids.contains(id))
            {
                return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
            }
        }

        character::ActiveModel {
            id: Set(new_organization_character_id.clone()),
            project_id: Set(run.project_id.clone()),
            name: Set(organization_commit.name.clone()),
            age: Set(None),
            gender: Set(None),
            is_organization: Set(true),
            role_type: Set(Some("supporting".to_string())),
            personality: Set(organization_commit.personality.clone()),
            background: Set(organization_commit.background.clone()),
            appearance: Set(organization_commit.appearance.clone()),
            relationships: Set(None),
            organization_type: Set(Some(organization_commit.organization_type.clone())),
            organization_purpose: Set(organization_commit.organization_purpose.clone()),
            organization_members: Set(None),
            status: Set("active".to_string()),
            status_changed_chapter: Set(None),
            current_state: Set(None),
            state_updated_chapter: Set(None),
            main_career_id: Set(None),
            main_career_stage: Set(None),
            sub_careers: Set(None),
            avatar_url: Set(None),
            traits: Set(Some(organization_commit.traits.clone())),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&txn)
        .await
        .map_err(database_error)?;
        organization::ActiveModel {
            id: Set(new_organization_id.clone()),
            character_id: Set(new_organization_character_id.clone()),
            project_id: Set(run.project_id.clone()),
            parent_org_id: Set(None),
            level: Set(0),
            power_level: Set(organization_commit.power_level),
            member_count: Set(
                i32::try_from(organization_commit.members.len()).map_err(|_| {
                    NovelAutopilotRepositoryError::InvalidConfig {
                        field: "organization_members",
                        code: "too_many",
                    }
                })?,
            ),
            location: Set(organization_commit.location.clone()),
            motto: Set(organization_commit.motto.clone()),
            color: Set(organization_commit.color.clone()),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        }
        .insert(&txn)
        .await
        .map_err(database_error)?;

        if !organization_commit.members.is_empty() {
            let members = organization_commit
                .members
                .iter()
                .map(|member| organization_member::ActiveModel {
                    id: Set(Uuid::new_v4().to_string()),
                    organization_id: Set(new_organization_id.clone()),
                    character_id: Set(member.character_id.clone()),
                    position: Set(member.position.clone()),
                    rank: Set(member.rank),
                    status: Set(member.status.clone()),
                    joined_at: Set(member.joined_at.clone()),
                    left_at: Set(None),
                    loyalty: Set(member.loyalty),
                    contribution: Set(0),
                    source: Set("novel_autopilot".to_string()),
                    notes: Set(None),
                    created_at: Set(now),
                    updated_at: Set(Some(now)),
                })
                .collect::<Vec<_>>();
            organization_member::Entity::insert_many(members)
                .exec(&txn)
                .await
                .map_err(database_error)?;
        }
        if !organization_commit.relationships.is_empty() {
            let relationships = organization_commit
                .relationships
                .iter()
                .map(|relationship| relationship::ActiveModel {
                    id: Set(Uuid::new_v4().to_string()),
                    project_id: Set(run.project_id.clone()),
                    character_from_id: Set(new_organization_character_id.clone()),
                    character_to_id: Set(relationship.target_organization_character_id.clone()),
                    relationship_type_id: Set(None),
                    relationship_name: Set(relationship.relationship_name.clone()),
                    intimacy_level: Set(0),
                    status: Set("active".to_string()),
                    description: Set(relationship.description.clone()),
                    started_at: Set(None),
                    ended_at: Set(None),
                    source: Set("novel_autopilot".to_string()),
                    created_at: Set(now),
                    updated_at: Set(Some(now)),
                })
                .collect::<Vec<_>>();
            relationship::Entity::insert_many(relationships)
                .exec(&txn)
                .await
                .map_err(database_error)?;
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
                Expr::value(Some(organization_commit.result_digest)),
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
            run: Self::find_owned(db, &run.id, user_id).await?,
            step: novel_autopilot_step_run::Entity::find_by_id(step_id)
                .one(db)
                .await
                .map_err(database_error)?
                .ok_or(NovelAutopilotRepositoryError::NotFoundOrAccessDenied)?,
        })
    }
}

fn validate_organization_commit(
    organization_commit: &NovelAutopilotOrganizationCommit,
) -> Result<(), NovelAutopilotRepositoryError> {
    if organization_commit.name.trim().is_empty()
        || organization_commit.organization_type.trim().is_empty()
        || organization_commit.result_digest.trim().is_empty()
        || !(0..=100).contains(&organization_commit.power_level)
    {
        return Err(NovelAutopilotRepositoryError::InvalidConfig {
            field: "organization",
            code: "invalid",
        });
    }
    let member_ids = organization_commit
        .members
        .iter()
        .map(|member| member.character_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    if member_ids.len() != organization_commit.members.len()
        || organization_commit.members.iter().any(|member| {
            member.character_id.trim().is_empty()
                || member.position.trim().is_empty()
                || member.status.trim().is_empty()
                || member.rank < 0
                || !(0..=100).contains(&member.loyalty)
        })
    {
        return Err(NovelAutopilotRepositoryError::InvalidConfig {
            field: "organization_members",
            code: "invalid",
        });
    }
    let relationship_ids = organization_commit
        .relationships
        .iter()
        .map(|relationship| relationship.target_organization_character_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    if relationship_ids.len() != organization_commit.relationships.len()
        || organization_commit
            .relationships
            .iter()
            .any(|relationship| {
                relationship
                    .target_organization_character_id
                    .trim()
                    .is_empty()
            })
    {
        return Err(NovelAutopilotRepositoryError::InvalidConfig {
            field: "organization_relationships",
            code: "invalid",
        });
    }
    Ok(())
}

fn project_foundation_snapshot_condition(snapshot: &NovelAutopilotFoundationSnapshot) -> Condition {
    let mut condition = Condition::all().add(project::Column::Title.eq(&snapshot.title));
    condition = condition.add(match snapshot.description.as_deref() {
        Some(value) => project::Column::Description.eq(value),
        None => project::Column::Description.is_null(),
    });
    condition = condition.add(match snapshot.theme.as_deref() {
        Some(value) => project::Column::Theme.eq(value),
        None => project::Column::Theme.is_null(),
    });
    condition = condition.add(match snapshot.genre.as_deref() {
        Some(value) => project::Column::Genre.eq(value),
        None => project::Column::Genre.is_null(),
    });
    condition.add(match snapshot.narrative_perspective.as_deref() {
        Some(value) => project::Column::NarrativePerspective.eq(value),
        None => project::Column::NarrativePerspective.is_null(),
    })
}

fn project_world_snapshot_condition(snapshot: &NovelAutopilotWorldSnapshot) -> Condition {
    let mut condition = Condition::all();
    condition = condition.add(match snapshot.time_period.as_deref() {
        Some(value) => project::Column::WorldTimePeriod.eq(value),
        None => project::Column::WorldTimePeriod.is_null(),
    });
    condition = condition.add(match snapshot.location.as_deref() {
        Some(value) => project::Column::WorldLocation.eq(value),
        None => project::Column::WorldLocation.is_null(),
    });
    condition = condition.add(match snapshot.atmosphere.as_deref() {
        Some(value) => project::Column::WorldAtmosphere.eq(value),
        None => project::Column::WorldAtmosphere.is_null(),
    });
    condition.add(match snapshot.rules.as_deref() {
        Some(value) => project::Column::WorldRules.eq(value),
        None => project::Column::WorldRules.is_null(),
    })
}

async fn ensure_project_owned(
    db: &DatabaseConnection,
    project_id: &str,
    user_id: &str,
) -> Result<(), NovelAutopilotRepositoryError> {
    ProjectService::ensure_owned_access(db, project_id, user_id)
        .await
        .map_err(|error| match error {
            ProjectAccessQueryError::NotFoundOrAccessDenied => {
                NovelAutopilotRepositoryError::NotFoundOrAccessDenied
            }
            ProjectAccessQueryError::Internal(message) => {
                NovelAutopilotRepositoryError::Database(message)
            }
        })
}

async fn find_active_by_project(
    db: &DatabaseConnection,
    project_id: &str,
) -> Result<Option<novel_autopilot_run::Model>, NovelAutopilotRepositoryError> {
    novel_autopilot_run::Entity::find()
        .filter(novel_autopilot_run::Column::ActiveScopeKey.eq(project_id))
        .one(db)
        .await
        .map_err(database_error)
}

fn database_error(error: impl fmt::Display) -> NovelAutopilotRepositoryError {
    NovelAutopilotRepositoryError::Database(error.to_string())
}
