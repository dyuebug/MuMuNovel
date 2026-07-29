use std::collections::{HashMap, HashSet};
use std::fmt;

use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set, TransactionTrait,
};
use serde_json::Value;
use uuid::Uuid;

use crate::models::{
    career, character, novel_autopilot_run, novel_autopilot_step_run, organization,
    organization_member, relationship,
};

use super::{
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotRepository, NovelAutopilotRepositoryError,
    },
    types::{NovelAutopilotRunStatus, NovelAutopilotStepStatus},
};

/// Internal CAS snapshot for the character-design step.  It is deliberately a
/// full business-data fingerprint rather than an API DTO: any human edit made
/// while generation is in flight rejects the write instead of being overwritten.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NovelAutopilotCharacterSnapshot {
    character_fingerprints: Vec<String>,
    career_fingerprints: Vec<String>,
}

impl NovelAutopilotCharacterSnapshot {
    pub(crate) async fn load(
        db: &DatabaseConnection,
        project_id: &str,
    ) -> Result<Self, NovelAutopilotRepositoryError> {
        let characters = character::Entity::find()
            .filter(character::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(database_error)?;
        let careers = career::Entity::find()
            .filter(career::Column::ProjectId.eq(project_id))
            .all(db)
            .await
            .map_err(database_error)?;
        Ok(Self::from_models(characters, careers))
    }

    pub(crate) fn is_blank(&self) -> bool {
        self.character_fingerprints.is_empty()
    }

    fn from_models(characters: Vec<character::Model>, careers: Vec<career::Model>) -> Self {
        let mut character_fingerprints = characters
            .into_iter()
            .map(|item| format!("{item:?}"))
            .collect::<Vec<_>>();
        character_fingerprints.sort();
        let mut career_fingerprints = careers
            .into_iter()
            .map(|item| format!("{item:?}"))
            .collect::<Vec<_>>();
        career_fingerprints.sort();
        Self {
            character_fingerprints,
            career_fingerprints,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotCharacterItemCommit {
    pub name: String,
    pub age: String,
    pub gender: String,
    pub role_type: String,
    pub personality: String,
    pub background: String,
    pub appearance: String,
    pub traits: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotCharacterOrganizationCommit {
    pub name: String,
    pub role_type: String,
    pub personality: String,
    pub background: String,
    pub appearance: String,
    pub organization_type: String,
    pub organization_purpose: String,
    pub member_names: Vec<String>,
    pub power_level: i32,
    pub location: String,
    pub motto: String,
    pub color: String,
    pub traits: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotCharacterSubCareerCommit {
    pub career: String,
    pub stage: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotCharacterCareerAssignmentCommit {
    pub character_name: String,
    pub main_career: String,
    pub main_stage: i32,
    pub sub_careers: Vec<NovelAutopilotCharacterSubCareerCommit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotCharacterRelationshipCommit {
    pub source_character_name: String,
    pub target_character_name: String,
    pub relationship_type: String,
    pub intimacy_level: i32,
    pub description: String,
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotCharacterOrganizationMembershipCommit {
    pub character_name: String,
    pub organization_name: String,
    pub position: String,
    pub rank: i32,
    pub loyalty: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotCharacterCommit {
    pub characters: Vec<NovelAutopilotCharacterItemCommit>,
    pub organizations: Vec<NovelAutopilotCharacterOrganizationCommit>,
    pub career_assignments: Vec<NovelAutopilotCharacterCareerAssignmentCommit>,
    pub relationships: Vec<NovelAutopilotCharacterRelationshipCommit>,
    pub organization_memberships: Vec<NovelAutopilotCharacterOrganizationMembershipCommit>,
    pub result_digest: String,
}

#[derive(Debug, Clone)]
struct ResolvedCareerAssignment {
    character_name: String,
    main_career_id: String,
    main_stage: i32,
    sub_careers: Vec<(String, i32)>,
}

#[derive(Debug, Clone)]
struct CareerCatalogEntry {
    id: String,
    career_type: String,
    max_stage: i32,
}

impl NovelAutopilotRepository {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn commit_character_design_step(
        db: &DatabaseConnection,
        step_id: &str,
        user_id: &str,
        expected_run_version: i64,
        expected_run_epoch: i64,
        expected_step_key: &str,
        expected_background_task_id: Option<&str>,
        expected_characters: &NovelAutopilotCharacterSnapshot,
        character_commit: NovelAutopilotCharacterCommit,
    ) -> Result<ClaimedNovelAutopilotStep, NovelAutopilotRepositoryError> {
        validate_character_commit(&character_commit)?;

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
        let current_characters = character::Entity::find()
            .filter(character::Column::ProjectId.eq(&run.project_id))
            .all(&txn)
            .await
            .map_err(database_error)?;
        let current_careers = career::Entity::find()
            .filter(career::Column::ProjectId.eq(&run.project_id))
            .all(&txn)
            .await
            .map_err(database_error)?;
        let current_snapshot = NovelAutopilotCharacterSnapshot::from_models(
            current_characters,
            current_careers.clone(),
        );
        if &current_snapshot != expected_characters || !expected_characters.is_blank() {
            return Err(NovelAutopilotRepositoryError::BusinessDataChanged);
        }

        let career_assignments =
            resolve_career_assignments(&character_commit.career_assignments, &current_careers)?;
        let assignment_by_character = career_assignments
            .iter()
            .map(|assignment| (assignment.character_name.as_str(), assignment))
            .collect::<HashMap<_, _>>();

        let mut character_ids = HashMap::new();
        let character_models = character_commit
            .characters
            .iter()
            .map(|item| {
                let id = Uuid::new_v4().to_string();
                character_ids.insert(item.name.clone(), id.clone());
                let assignment = assignment_by_character.get(item.name.as_str()).copied();
                character::ActiveModel {
                    id: Set(id),
                    project_id: Set(run.project_id.clone()),
                    name: Set(item.name.clone()),
                    age: Set(Some(item.age.clone())),
                    gender: Set(Some(item.gender.clone())),
                    is_organization: Set(false),
                    role_type: Set(Some(item.role_type.clone())),
                    personality: Set(Some(item.personality.clone())),
                    background: Set(Some(item.background.clone())),
                    appearance: Set(Some(item.appearance.clone())),
                    relationships: Set(None),
                    organization_type: Set(None),
                    organization_purpose: Set(None),
                    organization_members: Set(None),
                    status: Set("active".to_string()),
                    status_changed_chapter: Set(None),
                    current_state: Set(None),
                    state_updated_chapter: Set(None),
                    main_career_id: Set(assignment.map(|item| item.main_career_id.clone())),
                    main_career_stage: Set(assignment.map(|item| item.main_stage)),
                    sub_careers: Set(assignment.map(|item| {
                        Value::Array(
                            item.sub_careers
                                .iter()
                                .map(|(career_id, stage)| {
                                    serde_json::json!({"career_id": career_id, "stage": stage})
                                })
                                .collect(),
                        )
                        .to_string()
                    })),
                    avatar_url: Set(None),
                    traits: Set(Some(item.traits.clone())),
                    created_at: Set(now),
                    updated_at: Set(Some(now)),
                }
            })
            .collect::<Vec<_>>();
        character::Entity::insert_many(character_models)
            .exec(&txn)
            .await
            .map_err(database_error)?;

        let mut organization_ids = HashMap::new();
        let mut organization_character_models =
            Vec::with_capacity(character_commit.organizations.len());
        let mut organization_models = Vec::with_capacity(character_commit.organizations.len());
        for item in &character_commit.organizations {
            let organization_character_id = Uuid::new_v4().to_string();
            let organization_id = Uuid::new_v4().to_string();
            organization_ids.insert(item.name.clone(), organization_id.clone());
            organization_character_models.push(character::ActiveModel {
                id: Set(organization_character_id.clone()),
                project_id: Set(run.project_id.clone()),
                name: Set(item.name.clone()),
                age: Set(None),
                gender: Set(None),
                is_organization: Set(true),
                role_type: Set(Some(item.role_type.clone())),
                personality: Set(Some(item.personality.clone())),
                background: Set(Some(item.background.clone())),
                appearance: Set(Some(item.appearance.clone())),
                relationships: Set(None),
                organization_type: Set(Some(item.organization_type.clone())),
                organization_purpose: Set(Some(item.organization_purpose.clone())),
                organization_members: Set(Some(
                    Value::Array(
                        item.member_names
                            .iter()
                            .cloned()
                            .map(Value::String)
                            .collect(),
                    )
                    .to_string(),
                )),
                status: Set("active".to_string()),
                status_changed_chapter: Set(None),
                current_state: Set(None),
                state_updated_chapter: Set(None),
                main_career_id: Set(None),
                main_career_stage: Set(None),
                sub_careers: Set(None),
                avatar_url: Set(None),
                traits: Set(Some(item.traits.clone())),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            });
            let member_count = character_commit
                .organization_memberships
                .iter()
                .filter(|membership| membership.organization_name == item.name)
                .count();
            organization_models.push(organization::ActiveModel {
                id: Set(organization_id),
                character_id: Set(organization_character_id),
                project_id: Set(run.project_id.clone()),
                parent_org_id: Set(None),
                level: Set(0),
                power_level: Set(item.power_level),
                member_count: Set(i32::try_from(member_count).map_err(|_| {
                    NovelAutopilotRepositoryError::InvalidConfig {
                        field: "organization_memberships",
                        code: "too_many",
                    }
                })?),
                location: Set(Some(item.location.clone())),
                motto: Set(Some(item.motto.clone())),
                color: Set(Some(item.color.clone())),
                created_at: Set(now),
                updated_at: Set(Some(now)),
            });
        }
        if !organization_character_models.is_empty() {
            character::Entity::insert_many(organization_character_models)
                .exec(&txn)
                .await
                .map_err(database_error)?;
            organization::Entity::insert_many(organization_models)
                .exec(&txn)
                .await
                .map_err(database_error)?;
        }

        if !character_commit.relationships.is_empty() {
            let relationships = character_commit
                .relationships
                .iter()
                .map(|item| relationship::ActiveModel {
                    id: Set(Uuid::new_v4().to_string()),
                    project_id: Set(run.project_id.clone()),
                    character_from_id: Set(character_ids[&item.source_character_name].clone()),
                    character_to_id: Set(character_ids[&item.target_character_name].clone()),
                    relationship_type_id: Set(None),
                    relationship_name: Set(Some(item.relationship_type.clone())),
                    intimacy_level: Set(item.intimacy_level),
                    status: Set("active".to_string()),
                    description: Set(Some(item.description.clone())),
                    started_at: Set(item.started_at.clone()),
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

        if !character_commit.organization_memberships.is_empty() {
            let memberships = character_commit
                .organization_memberships
                .iter()
                .map(|item| organization_member::ActiveModel {
                    id: Set(Uuid::new_v4().to_string()),
                    organization_id: Set(organization_ids[&item.organization_name].clone()),
                    character_id: Set(character_ids[&item.character_name].clone()),
                    position: Set(item.position.clone()),
                    rank: Set(item.rank),
                    status: Set("active".to_string()),
                    joined_at: Set(None),
                    left_at: Set(None),
                    loyalty: Set(item.loyalty),
                    contribution: Set(0),
                    source: Set("novel_autopilot".to_string()),
                    notes: Set(None),
                    created_at: Set(now),
                    updated_at: Set(Some(now)),
                })
                .collect::<Vec<_>>();
            organization_member::Entity::insert_many(memberships)
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
                Expr::value(Some(character_commit.result_digest)),
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
}

fn validate_character_commit(
    character_commit: &NovelAutopilotCharacterCommit,
) -> Result<(), NovelAutopilotRepositoryError> {
    if character_commit.result_digest.trim().is_empty() || character_commit.characters.is_empty() {
        return invalid("character_graph", "empty");
    }

    let character_names = character_commit
        .characters
        .iter()
        .map(|item| item.name.as_str())
        .collect::<HashSet<_>>();
    let organization_names = character_commit
        .organizations
        .iter()
        .map(|item| item.name.as_str())
        .collect::<HashSet<_>>();
    if character_names.len() != character_commit.characters.len()
        || organization_names.len() != character_commit.organizations.len()
        || character_names.iter().any(|name| name.trim().is_empty())
        || organization_names.iter().any(|name| name.trim().is_empty())
        || character_names
            .iter()
            .any(|name| organization_names.contains(name))
    {
        return invalid("character_graph", "duplicate_or_empty_name");
    }
    if character_commit
        .characters
        .iter()
        .filter(|item| item.role_type == "protagonist")
        .count()
        != 1
    {
        return invalid("characters", "protagonist_required");
    }
    for item in &character_commit.characters {
        if !matches!(
            item.role_type.as_str(),
            "protagonist" | "supporting" | "antagonist"
        ) || item.gender.trim().is_empty()
            || item.personality.trim().is_empty()
            || item.background.trim().is_empty()
            || item.appearance.trim().is_empty()
            || !valid_age(&item.age)
            || !valid_traits(&item.traits)
        {
            return invalid("characters", "invalid_item");
        }
    }
    for item in &character_commit.organizations {
        if !matches!(
            item.role_type.as_str(),
            "protagonist" | "supporting" | "antagonist"
        ) || item.personality.trim().is_empty()
            || item.background.trim().is_empty()
            || item.appearance.trim().is_empty()
            || item.organization_type.trim().is_empty()
            || item.organization_purpose.trim().is_empty()
            || item.location.trim().is_empty()
            || item.motto.trim().is_empty()
            || item.color.trim().is_empty()
            || !(70..=95).contains(&item.power_level)
            || !valid_traits(&item.traits)
            || item
                .member_names
                .iter()
                .any(|name| !character_names.contains(name.as_str()))
            || item.member_names.len() != item.member_names.iter().collect::<HashSet<_>>().len()
        {
            return invalid("organizations", "invalid_item");
        }
    }

    let assignment_names = character_commit
        .career_assignments
        .iter()
        .map(|item| item.character_name.as_str())
        .collect::<HashSet<_>>();
    if assignment_names.len() != character_commit.career_assignments.len()
        || character_commit.career_assignments.iter().any(|item| {
            !character_names.contains(item.character_name.as_str())
                || item.main_career.trim().is_empty()
                || item.main_stage <= 0
                || item.sub_careers.len() > 2
                || item
                    .sub_careers
                    .iter()
                    .any(|sub| sub.career.trim().is_empty() || sub.stage <= 0)
                || item
                    .sub_careers
                    .iter()
                    .map(|sub| sub.career.as_str())
                    .collect::<HashSet<_>>()
                    .len()
                    != item.sub_careers.len()
                || item
                    .sub_careers
                    .iter()
                    .any(|sub| sub.career == item.main_career)
        })
    {
        return invalid("career_assignments", "invalid");
    }

    let relationship_pairs = character_commit
        .relationships
        .iter()
        .map(|item| {
            (
                item.source_character_name.as_str(),
                item.target_character_name.as_str(),
            )
        })
        .collect::<HashSet<_>>();
    if relationship_pairs.len() != character_commit.relationships.len()
        || character_commit.relationships.iter().any(|item| {
            item.relationship_type.trim().is_empty()
                || item.description.trim().is_empty()
                || item.source_character_name == item.target_character_name
                || !character_names.contains(item.source_character_name.as_str())
                || !character_names.contains(item.target_character_name.as_str())
                || !(-100..=100).contains(&item.intimacy_level)
        })
    {
        return invalid("relationships", "invalid");
    }

    let membership_pairs = character_commit
        .organization_memberships
        .iter()
        .map(|item| {
            (
                item.character_name.as_str(),
                item.organization_name.as_str(),
            )
        })
        .collect::<HashSet<_>>();
    if membership_pairs.len() != character_commit.organization_memberships.len()
        || character_commit
            .organization_memberships
            .iter()
            .any(|item| {
                !character_names.contains(item.character_name.as_str())
                    || !organization_names.contains(item.organization_name.as_str())
                    || item.position.trim().is_empty()
                    || !(0..=10).contains(&item.rank)
                    || !(0..=100).contains(&item.loyalty)
            })
    {
        return invalid("organization_memberships", "invalid");
    }
    Ok(())
}

fn resolve_career_assignments(
    assignments: &[NovelAutopilotCharacterCareerAssignmentCommit],
    careers: &[career::Model],
) -> Result<Vec<ResolvedCareerAssignment>, NovelAutopilotRepositoryError> {
    if assignments.is_empty() {
        return Ok(Vec::new());
    }
    let mut by_name = HashMap::new();
    for item in careers {
        if by_name
            .insert(
                item.name.as_str(),
                CareerCatalogEntry {
                    id: item.id.clone(),
                    career_type: item.career_type.clone(),
                    max_stage: item.max_stage,
                },
            )
            .is_some()
        {
            return invalid("career_catalog", "ambiguous_name");
        }
    }
    assignments
        .iter()
        .map(|assignment| {
            let main = by_name
                .get(assignment.main_career.as_str())
                .ok_or_else(|| NovelAutopilotRepositoryError::BusinessDataChanged)?;
            if main.career_type != "main" || assignment.main_stage > main.max_stage {
                return invalid("career_assignments", "invalid_main_career");
            }
            let sub_careers = assignment
                .sub_careers
                .iter()
                .map(|sub| {
                    let career = by_name
                        .get(sub.career.as_str())
                        .ok_or_else(|| NovelAutopilotRepositoryError::BusinessDataChanged)?;
                    if career.career_type != "sub" || sub.stage > career.max_stage {
                        return invalid("career_assignments", "invalid_sub_career");
                    }
                    Ok((career.id.clone(), sub.stage))
                })
                .collect::<Result<Vec<_>, NovelAutopilotRepositoryError>>()?;
            Ok(ResolvedCareerAssignment {
                character_name: assignment.character_name.clone(),
                main_career_id: main.id.clone(),
                main_stage: assignment.main_stage,
                sub_careers,
            })
        })
        .collect()
}

fn valid_age(value: &str) -> bool {
    value
        .trim()
        .parse::<i32>()
        .is_ok_and(|age| (0..=200).contains(&age))
}

fn valid_traits(value: &str) -> bool {
    serde_json::from_str::<Value>(value)
        .ok()
        .and_then(|value| value.as_array().cloned())
        .is_some_and(|items| {
            !items.is_empty()
                && items.iter().all(|item| {
                    item.as_str()
                        .map(str::trim)
                        .is_some_and(|item| !item.is_empty())
                })
        })
}

fn invalid<T>(field: &'static str, code: &'static str) -> Result<T, NovelAutopilotRepositoryError> {
    Err(NovelAutopilotRepositoryError::InvalidConfig { field, code })
}

fn database_error(error: impl fmt::Display) -> NovelAutopilotRepositoryError {
    NovelAutopilotRepositoryError::Database(error.to_string())
}
