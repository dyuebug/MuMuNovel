use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::{
    models::novel_autopilot_run,
    services::{
        cooperative_cancellation_service::CooperativeCancellationToken,
        wizard_character_generation_service::{
            generate_character_graph_for_project_with_guidance, GenerateCharacterGraphForProject,
            GeneratedCharacterGraph, WizardCharacterGenerationError,
        },
    },
    tasks::types::TaskRecord,
};

use super::{
    output_observer::NovelAutopilotOutputObserver,
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotCharacterCareerAssignmentCommit,
        NovelAutopilotCharacterCommit, NovelAutopilotCharacterItemCommit,
        NovelAutopilotCharacterOrganizationCommit,
        NovelAutopilotCharacterOrganizationMembershipCommit,
        NovelAutopilotCharacterRelationshipCommit, NovelAutopilotCharacterSnapshot,
        NovelAutopilotCharacterSubCareerCommit, NovelAutopilotRepository,
        NovelAutopilotRepositoryError, NovelAutopilotStepTerminalPatch,
    },
    router::AutopilotStepPlan,
    types::{NovelAutopilotRunStatus, NovelAutopilotStepStatus},
};

const CHARACTER_DESIGN_COUNT: usize = 5;
const CHARACTER_MANUAL_CONTENT_PRESENT: &str = "character_design_manual_content_present";
const CHARACTER_GENERATION_INCOMPLETE: &str = "character_generation_incomplete";
const CHARACTER_BUSINESS_DATA_CHANGED: &str = "character_design_business_data_changed";

#[derive(Debug)]
pub(crate) enum CharacterAdapterError {
    Cancelled,
    Repository(NovelAutopilotRepositoryError),
}

impl CharacterAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "character_generation_cancelled",
            Self::Repository(error) => error.code(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum CharacterAdapterOutcome {
    StepCompleted {
        result: Value,
        run: novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

pub(crate) async fn execute_character_design_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    additional_guidance: Option<&str>,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<CharacterAdapterOutcome, CharacterAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    let expected_characters = NovelAutopilotCharacterSnapshot::load(db, &claimed.run.project_id)
        .await
        .map_err(CharacterAdapterError::Repository)?;

    if !expected_characters.is_blank() {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Skipped,
            CHARACTER_MANUAL_CONTENT_PRESENT,
            None,
        )
        .await;
    }

    let generated = match generate_character_graph_for_project_with_guidance(
        db,
        GenerateCharacterGraphForProject {
            user_id: &record.user_id,
            project_id: &claimed.run.project_id,
            count: CHARACTER_DESIGN_COUNT,
            world_context: None,
            theme: None,
            genre: None,
            requirements: Some("优先生成主角与能直接推动主线冲突的核心配角。"),
            provider_override: None,
            model_override: None,
        },
        additional_guidance,
        Some(cancellation_token),
        |progress| async move {
            tracing::debug!(
                event = "novel_book_autopilot_character_progress",
                progress = progress.progress,
                status = progress.status,
                message = %progress.message,
                "durable character generation progress updated"
            );
            Ok(())
        },
        {
            let output_observer = output_observer.clone();
            move |content| {
                let output_observer = output_observer.clone();
                async move {
                    output_observer.content(content).await;
                    Ok(())
                }
            }
        },
        {
            let output_observer = output_observer.clone();
            move |reasoning| {
                let output_observer = output_observer.clone();
                async move {
                    output_observer.reasoning(reasoning).await;
                    Ok(())
                }
            }
        },
    )
    .await
    {
        Ok(generated) => generated,
        Err(WizardCharacterGenerationError::Cancelled) => {
            return Err(CharacterAdapterError::Cancelled)
        }
        Err(error) => {
            let error_code = error.code();
            tracing::warn!(
                event = "novel_book_autopilot_character_generation_failed",
                error_code,
                run_id = %claimed.run.id,
                step_id = %claimed.step.id,
                "durable character generation failed before business commit"
            );
            return finish_waiting_human(
                db,
                record,
                claimed,
                step,
                NovelAutopilotStepStatus::Failed,
                error_code,
                None,
            )
            .await;
        }
    };

    ensure_not_cancelled(cancellation_token)?;
    let Some(character_commit) = character_commit(&generated) else {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            CHARACTER_GENERATION_INCOMPLETE,
            Some(generated.content_digest),
        )
        .await;
    };

    let provider = generated.provider;
    let model = generated.model;
    let attempts = generated.attempts;
    let character_count = character_commit.characters.len();
    let organization_count = character_commit.organizations.len();
    let relationship_count = character_commit.relationships.len();
    let membership_count = character_commit.organization_memberships.len();
    let committed = match NovelAutopilotRepository::commit_character_design_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        &expected_characters,
        character_commit,
    )
    .await
    {
        Ok(committed) => committed,
        Err(NovelAutopilotRepositoryError::BusinessDataChanged) => {
            return finish_waiting_human(
                db,
                record,
                claimed,
                step,
                NovelAutopilotStepStatus::Skipped,
                CHARACTER_BUSINESS_DATA_CHANGED,
                None,
            )
            .await;
        }
        Err(error) => return Err(CharacterAdapterError::Repository(error)),
    };

    let result = json!({
        "run_id": committed.run.id,
        "run_status": committed.run.status,
        "run_epoch": committed.run.epoch,
        "run_version": committed.run.version,
        "dispatch_status": "step_completed",
        "step_id": committed.step.id,
        "step_type": step.step_type,
        "step_status": committed.step.status,
        "provider": provider,
        "model": model,
        "attempts": attempts,
        "character_count": character_count,
        "organization_count": organization_count,
        "relationship_count": relationship_count,
        "organization_membership_count": membership_count,
        "result_digest": committed.step.result_digest,
    });
    Ok(CharacterAdapterOutcome::StepCompleted {
        result,
        run: committed.run,
    })
}
fn character_commit(generated: &GeneratedCharacterGraph) -> Option<NovelAutopilotCharacterCommit> {
    if !generated.is_complete() {
        return None;
    }

    let characters = generated
        .characters
        .iter()
        .map(|character| {
            Some(NovelAutopilotCharacterItemCommit {
                name: character.name.clone(),
                age: character.age.to_string(),
                gender: character.gender.clone(),
                role_type: character.role_type.clone(),
                personality: character.personality.clone(),
                background: character.background.clone(),
                appearance: character.appearance.clone(),
                traits: serde_json::to_string(&character.traits).ok()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let organizations = generated
        .organizations
        .iter()
        .map(|organization| {
            Some(NovelAutopilotCharacterOrganizationCommit {
                name: organization.name.clone(),
                role_type: organization.role_type.clone(),
                personality: organization.personality.clone(),
                background: organization.background.clone(),
                appearance: organization.appearance.clone(),
                organization_type: organization.organization_type.clone(),
                organization_purpose: organization.organization_purpose.clone(),
                member_names: organization.member_names.clone(),
                power_level: organization.power_level,
                location: organization.location.clone(),
                motto: organization.motto.clone(),
                color: organization.color.clone(),
                traits: serde_json::to_string(&organization.traits).ok()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let career_assignments = generated
        .career_assignments
        .iter()
        .map(|assignment| NovelAutopilotCharacterCareerAssignmentCommit {
            character_name: assignment.character_name.clone(),
            main_career: assignment.main_career.clone(),
            main_stage: assignment.main_stage,
            sub_careers: assignment
                .sub_careers
                .iter()
                .map(|sub_career| NovelAutopilotCharacterSubCareerCommit {
                    career: sub_career.career.clone(),
                    stage: sub_career.stage,
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let relationships = generated
        .relationships
        .iter()
        .map(|relationship| NovelAutopilotCharacterRelationshipCommit {
            source_character_name: relationship.source_character_name.clone(),
            target_character_name: relationship.target_character_name.clone(),
            relationship_type: relationship.relationship_type.clone(),
            intimacy_level: relationship.intimacy_level,
            description: relationship.description.clone(),
            started_at: relationship.started_at.clone(),
        })
        .collect::<Vec<_>>();
    let organization_memberships = generated
        .organization_memberships
        .iter()
        .map(
            |membership| NovelAutopilotCharacterOrganizationMembershipCommit {
                character_name: membership.character_name.clone(),
                organization_name: membership.organization_name.clone(),
                position: membership.position.clone(),
                rank: membership.rank,
                loyalty: membership.loyalty,
            },
        )
        .collect::<Vec<_>>();

    if characters.is_empty()
        || characters.iter().any(|character| {
            character.name.trim().is_empty()
                || character.role_type.trim().is_empty()
                || character.traits.trim().is_empty()
        })
        || organizations.iter().any(|organization| {
            organization.name.trim().is_empty()
                || organization.organization_type.trim().is_empty()
                || organization.traits.trim().is_empty()
        })
    {
        return None;
    }

    Some(NovelAutopilotCharacterCommit {
        characters,
        organizations,
        career_assignments,
        relationships,
        organization_memberships,
        result_digest: generated.content_digest.clone(),
    })
}

async fn finish_waiting_human(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    terminal_status: NovelAutopilotStepStatus,
    reason_code: &str,
    result_digest: Option<String>,
) -> Result<CharacterAdapterOutcome, CharacterAdapterError> {
    let terminal = NovelAutopilotRepository::complete_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        terminal_status,
        NovelAutopilotStepTerminalPatch {
            result_digest,
            quality_decision: None,
            error_code: Some(reason_code.to_string()),
        },
    )
    .await
    .map_err(CharacterAdapterError::Repository)?;
    let waiting = NovelAutopilotRepository::transition_owned(
        db,
        &terminal.run.id,
        &record.user_id,
        terminal.run.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    .map_err(CharacterAdapterError::Repository)?;

    Ok(CharacterAdapterOutcome::WaitingHuman {
        result: waiting_human_view(&waiting, &terminal, step, reason_code),
    })
}

fn waiting_human_view(
    run: &novel_autopilot_run::Model,
    terminal: &ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    reason_code: &str,
) -> Value {
    json!({
        "run_id": run.id,
        "run_status": run.status,
        "run_epoch": run.epoch,
        "run_version": run.version,
        "dispatch_status": "waiting_human",
        "reason_code": reason_code,
        "step_id": terminal.step.id,
        "step_type": step.step_type,
        "step_status": terminal.step.status,
    })
}

fn ensure_not_cancelled(
    cancellation_token: &CooperativeCancellationToken,
) -> Result<(), CharacterAdapterError> {
    if cancellation_token.is_cancelled() {
        Err(CharacterAdapterError::Cancelled)
    } else {
        Ok(())
    }
}
