use std::collections::HashSet;

use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::{
    models::novel_autopilot_run,
    services::{
        cooperative_cancellation_service::CooperativeCancellationToken,
        organization_generation_service::{
            generate_organization_plan_for_project_with_guidance,
            GenerateOrganizationPlanForProject, GeneratedOrganizationPlan,
            OrganizationGenerationError,
        },
    },
    tasks::types::TaskRecord,
};

use super::{
    output_observer::NovelAutopilotOutputObserver,
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotOrganizationCommit,
        NovelAutopilotOrganizationMemberCommit, NovelAutopilotOrganizationRelationshipCommit,
        NovelAutopilotOrganizationSnapshot, NovelAutopilotRepository,
        NovelAutopilotRepositoryError, NovelAutopilotStepTerminalPatch,
    },
    router::AutopilotStepPlan,
    types::{NovelAutopilotRunStatus, NovelAutopilotStepStatus},
};

const ORGANIZATION_MANUAL_CONTENT_PRESENT: &str = "organization_design_manual_content_present";
const ORGANIZATION_GENERATION_INCOMPLETE: &str = "organization_generation_incomplete";
const ORGANIZATION_BUSINESS_DATA_CHANGED: &str = "organization_design_business_data_changed";

#[derive(Debug)]
pub(crate) enum OrganizationAdapterError {
    Cancelled,
    Repository(NovelAutopilotRepositoryError),
}

impl OrganizationAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "organization_generation_cancelled",
            Self::Repository(error) => error.code(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum OrganizationAdapterOutcome {
    StepCompleted {
        result: Value,
        run: novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

pub(crate) async fn execute_organization_design_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    additional_guidance: Option<&str>,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<OrganizationAdapterOutcome, OrganizationAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    let expected_organizations =
        NovelAutopilotOrganizationSnapshot::load(db, &claimed.run.project_id)
            .await
            .map_err(OrganizationAdapterError::Repository)?;

    if !expected_organizations.is_blank() {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Skipped,
            ORGANIZATION_MANUAL_CONTENT_PRESENT,
            None,
        )
        .await;
    }

    let generated = match generate_organization_plan_for_project_with_guidance(
        db,
        GenerateOrganizationPlanForProject {
            user_id: &record.user_id,
            project_id: &claimed.run.project_id,
            name: None,
            organization_type: None,
            background: None,
            requirements: None,
            provider_override: None,
            model_override: None,
        },
        additional_guidance,
        Some(cancellation_token),
    )
    .await
    {
        Ok(generated) => generated,
        Err(OrganizationGenerationError::Cancelled) => {
            return Err(OrganizationAdapterError::Cancelled)
        }
        Err(error) => {
            let error_code = error.code();
            tracing::warn!(
                event = "novel_book_autopilot_organization_generation_failed",
                error_code,
                run_id = %claimed.run.id,
                step_id = %claimed.step.id,
                "durable organization generation failed before business commit"
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
    if let Ok(content) = serde_json::to_string(&generated) {
        output_observer.content(content).await;
    }
    let Some(organization_commit) = organization_commit(&generated, &expected_organizations) else {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            ORGANIZATION_GENERATION_INCOMPLETE,
            Some(generated.content_digest),
        )
        .await;
    };

    let provider = generated.provider;
    let model = generated.model;
    let member_count = organization_commit.members.len();
    let relationship_count = organization_commit.relationships.len();
    let committed = match NovelAutopilotRepository::commit_organization_design_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        &expected_organizations,
        organization_commit,
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
                ORGANIZATION_BUSINESS_DATA_CHANGED,
                None,
            )
            .await;
        }
        Err(error) => return Err(OrganizationAdapterError::Repository(error)),
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
        "member_count": member_count,
        "relationship_count": relationship_count,
        "result_digest": committed.step.result_digest,
    });
    Ok(OrganizationAdapterOutcome::StepCompleted {
        result,
        run: committed.run,
    })
}

fn organization_commit(
    generated: &GeneratedOrganizationPlan,
    snapshot: &NovelAutopilotOrganizationSnapshot,
) -> Option<NovelAutopilotOrganizationCommit> {
    if generated.name.trim().is_empty() || generated.organization_type.trim().is_empty() {
        return None;
    }

    let mut member_ids = HashSet::new();
    let mut members = Vec::with_capacity(generated.initial_members.len());
    for member in &generated.initial_members {
        let character_id =
            snapshot.find_unique_non_organization_character_id(&member.character_name)?;
        if !member_ids.insert(character_id.clone()) {
            return None;
        }
        members.push(NovelAutopilotOrganizationMemberCommit {
            character_id,
            position: member
                .position
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("成员")
                .to_string(),
            rank: member.rank.unwrap_or(0).max(0),
            status: member
                .status
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("active")
                .to_string(),
            joined_at: member.joined_at.clone(),
            loyalty: member.loyalty.unwrap_or(50).clamp(0, 100),
        });
    }

    let mut relationship_ids = HashSet::new();
    let mut relationships = Vec::with_capacity(generated.relationships.len());
    for relationship in &generated.relationships {
        let target_organization_character_id = snapshot
            .find_unique_organization_character_id(&relationship.target_organization_name)?;
        if !relationship_ids.insert(target_organization_character_id.clone()) {
            return None;
        }
        relationships.push(NovelAutopilotOrganizationRelationshipCommit {
            target_organization_character_id,
            relationship_name: relationship
                .relationship_type
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string),
            description: relationship.description.clone(),
        });
    }

    Some(NovelAutopilotOrganizationCommit {
        name: generated.name.clone(),
        organization_type: generated.organization_type.clone(),
        personality: generated.personality.clone(),
        background: generated.background.clone(),
        appearance: generated.appearance.clone(),
        organization_purpose: generated.organization_purpose.clone(),
        traits: serde_json::to_string(&generated.traits).ok()?,
        power_level: generated.power_level.clamp(0, 100),
        location: generated.location.clone(),
        motto: generated.motto.clone(),
        color: generated.color.clone(),
        members,
        relationships,
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
) -> Result<OrganizationAdapterOutcome, OrganizationAdapterError> {
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
    .map_err(OrganizationAdapterError::Repository)?;
    let waiting = NovelAutopilotRepository::transition_owned(
        db,
        &terminal.run.id,
        &record.user_id,
        terminal.run.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    .map_err(OrganizationAdapterError::Repository)?;

    Ok(OrganizationAdapterOutcome::WaitingHuman {
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
) -> Result<(), OrganizationAdapterError> {
    if cancellation_token.is_cancelled() {
        Err(OrganizationAdapterError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::organization_commit;
    use crate::services::{
        novel_autopilot::repository::NovelAutopilotOrganizationSnapshot,
        organization_generation_service::{
            GeneratedOrganizationInitialMember, GeneratedOrganizationPlan,
            GeneratedOrganizationRelationship,
        },
    };

    #[test]
    fn generated_organization_with_unknown_member_never_maps_to_a_commit() {
        let generated = GeneratedOrganizationPlan {
            name: "浮港议会".to_string(),
            organization_type: "政治".to_string(),
            personality: None,
            background: None,
            appearance: None,
            organization_purpose: None,
            traits: vec![],
            power_level: 50,
            location: None,
            motto: None,
            color: None,
            initial_members: vec![GeneratedOrganizationInitialMember {
                character_name: "不存在的角色".to_string(),
                position: None,
                rank: None,
                loyalty: None,
                joined_at: None,
                status: None,
            }],
            relationships: vec![GeneratedOrganizationRelationship {
                target_organization_name: "不存在的组织".to_string(),
                relationship_type: None,
                description: None,
            }],
            provider: "provider".to_string(),
            model: "model".to_string(),
            content_digest: "digest".to_string(),
        };
        let snapshot = NovelAutopilotOrganizationSnapshot::default();

        assert!(organization_commit(&generated, &snapshot).is_none());
    }
}
