use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::{
    models::novel_autopilot_run,
    services::{
        cooperative_cancellation_service::CooperativeCancellationToken,
        wizard_career_generation_service::{
            generate_career_system_for_project_with_guidance, GenerateCareerSystemForProject,
            GeneratedCareer, GeneratedCareerSystem, WizardCareerGenerationError,
        },
    },
    tasks::types::TaskRecord,
};

use super::{
    output_observer::NovelAutopilotOutputObserver,
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotCareerCommit, NovelAutopilotCareerItemCommit,
        NovelAutopilotCareerSnapshot, NovelAutopilotRepository, NovelAutopilotRepositoryError,
        NovelAutopilotStepTerminalPatch,
    },
    router::AutopilotStepPlan,
    types::{NovelAutopilotRunStatus, NovelAutopilotStepStatus},
};

const CAREER_MANUAL_CONTENT_PRESENT: &str = "career_design_manual_content_present";
const CAREER_GENERATION_INCOMPLETE: &str = "career_generation_incomplete";
const CAREER_BUSINESS_DATA_CHANGED: &str = "career_design_business_data_changed";

#[derive(Debug)]
pub(crate) enum CareerAdapterError {
    Cancelled,
    Repository(NovelAutopilotRepositoryError),
}

impl CareerAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "career_generation_cancelled",
            Self::Repository(error) => error.code(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum CareerAdapterOutcome {
    StepCompleted {
        result: Value,
        run: novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

pub(crate) async fn execute_career_design_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    additional_guidance: Option<&str>,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<CareerAdapterOutcome, CareerAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    let expected_careers = NovelAutopilotCareerSnapshot::load(db, &claimed.run.project_id)
        .await
        .map_err(CareerAdapterError::Repository)?;

    if !expected_careers.is_blank() {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Skipped,
            CAREER_MANUAL_CONTENT_PRESENT,
            None,
        )
        .await;
    }

    let generated = match generate_career_system_for_project_with_guidance(
        db,
        GenerateCareerSystemForProject {
            user_id: &record.user_id,
            project_id: &claimed.run.project_id,
            provider_override: None,
            model_override: None,
        },
        additional_guidance,
        Some(cancellation_token),
        |progress| async move {
            tracing::debug!(
                event = "novel_book_autopilot_career_progress",
                progress = progress.progress,
                status = progress.status,
                message = %progress.message,
                "durable career generation progress updated"
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
        Err(WizardCareerGenerationError::Cancelled) => return Err(CareerAdapterError::Cancelled),
        Err(error) => {
            let error_code = error.code();
            tracing::warn!(
                event = "novel_book_autopilot_career_generation_failed",
                error_code,
                run_id = %claimed.run.id,
                step_id = %claimed.step.id,
                "durable career generation failed before business commit"
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
    let Some(career_commit) = career_commit(&generated) else {
        return finish_waiting_human(
            db,
            record,
            claimed,
            step,
            NovelAutopilotStepStatus::Failed,
            CAREER_GENERATION_INCOMPLETE,
            Some(generated.content_digest),
        )
        .await;
    };

    let provider = generated.provider;
    let model = generated.model;
    let attempts = generated.attempts;
    let committed = match NovelAutopilotRepository::commit_career_design_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        &expected_careers,
        career_commit,
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
                CAREER_BUSINESS_DATA_CHANGED,
                None,
            )
            .await;
        }
        Err(error) => return Err(CareerAdapterError::Repository(error)),
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
        "career_count": generated.main_careers.len() + generated.sub_careers.len(),
        "result_digest": committed.step.result_digest,
    });
    Ok(CareerAdapterOutcome::StepCompleted {
        result,
        run: committed.run,
    })
}

fn career_commit(generated: &GeneratedCareerSystem) -> Option<NovelAutopilotCareerCommit> {
    if !generated.is_complete() {
        return None;
    }

    let mut careers =
        Vec::with_capacity(generated.main_careers.len() + generated.sub_careers.len());
    for career in &generated.main_careers {
        careers.push(career_item_commit(career, "main")?);
    }
    for career in &generated.sub_careers {
        careers.push(career_item_commit(career, "sub")?);
    }

    Some(NovelAutopilotCareerCommit {
        careers,
        result_digest: generated.content_digest.clone(),
    })
}

fn career_item_commit(
    generated: &GeneratedCareer,
    career_type: &str,
) -> Option<NovelAutopilotCareerItemCommit> {
    Some(NovelAutopilotCareerItemCommit {
        name: generated.name.clone(),
        career_type: career_type.to_string(),
        description: generated.description.clone(),
        category: generated.category.clone(),
        stages: serde_json::to_string(&generated.stages).ok()?,
        max_stage: generated.max_stage,
        requirements: generated.requirements.clone(),
        special_abilities: generated.special_abilities.clone(),
        worldview_rules: generated.worldview_rules.clone(),
        attribute_bonuses: generated
            .attribute_bonuses
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok()),
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
) -> Result<CareerAdapterOutcome, CareerAdapterError> {
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
    .map_err(CareerAdapterError::Repository)?;
    let waiting = NovelAutopilotRepository::transition_owned(
        db,
        &terminal.run.id,
        &record.user_id,
        terminal.run.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    .map_err(CareerAdapterError::Repository)?;

    Ok(CareerAdapterOutcome::WaitingHuman {
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
) -> Result<(), CareerAdapterError> {
    if cancellation_token.is_cancelled() {
        Err(CareerAdapterError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::career_commit;
    use crate::services::wizard_career_generation_service::{
        GeneratedCareer, GeneratedCareerStage, GeneratedCareerSystem,
    };

    #[test]
    fn complete_generated_careers_map_to_repository_commit() {
        let career = GeneratedCareer {
            name: "巡界师".to_string(),
            description: Some("维护浮空航路".to_string()),
            category: Some("战斗".to_string()),
            stages: vec![GeneratedCareerStage {
                level: 1,
                name: "见习".to_string(),
                description: None,
            }],
            max_stage: 1,
            requirements: None,
            special_abilities: Some("锚定航路".to_string()),
            worldview_rules: None,
            attribute_bonuses: Some(json!({"perception": 2})),
        };
        let generated = GeneratedCareerSystem {
            main_careers: vec![career.clone()],
            sub_careers: vec![GeneratedCareer {
                name: "星图师".to_string(),
                ..career
            }],
            provider: "provider".to_string(),
            model: "model".to_string(),
            attempts: 1,
            content_digest: "digest".to_string(),
        };

        let commit = career_commit(&generated).expect("complete careers map to commit");
        assert_eq!(commit.careers.len(), 2);
        assert_eq!(commit.careers[0].career_type, "main");
        assert_eq!(commit.careers[1].career_type, "sub");
        assert_eq!(commit.result_digest, "digest");
        assert_eq!(
            commit.careers[0].attribute_bonuses.as_deref(),
            Some("{\"perception\":2}")
        );
    }
}
