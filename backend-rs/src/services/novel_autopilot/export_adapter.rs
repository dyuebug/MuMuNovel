use sea_orm::DatabaseConnection;
use serde_json::{json, Value};

use crate::{
    models::novel_autopilot_run,
    services::{
        cooperative_cancellation_service::CooperativeCancellationToken,
        project_export_service::{
            build_project_export_artifact, ProjectExportArtifact, ProjectExportServiceError,
        },
    },
    tasks::types::TaskRecord,
};

use super::{
    export_repository::NovelAutopilotExportCommit,
    repository::{
        ClaimedNovelAutopilotStep, NovelAutopilotRepository, NovelAutopilotRepositoryError,
        NovelAutopilotStepTerminalPatch,
    },
    router::AutopilotStepPlan,
    types::{
        NovelAutopilotRunConfig, NovelAutopilotRunStatus, NovelAutopilotStepStatus,
        NovelAutopilotStepType,
    },
};

const EXPORT_FACTS_INVALID: &str = "export_step_facts_invalid";
const UNSUPPORTED_EXPORT_FORMAT: &str = "unsupported_export_format";

#[derive(Debug)]
pub(crate) enum ExportAdapterError {
    Cancelled,
    Service(ProjectExportServiceError),
    Repository(NovelAutopilotRepositoryError),
}

impl ExportAdapterError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "export_cancelled",
            Self::Service(error) => error.code(),
            Self::Repository(error) => error.code(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ExportAdapterOutcome {
    StepCompleted {
        result: Value,
        run: novel_autopilot_run::Model,
    },
    WaitingHuman {
        result: Value,
    },
}

pub(crate) async fn execute_export_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<ExportAdapterOutcome, ExportAdapterError> {
    ensure_not_cancelled(cancellation_token)?;
    if step.step_type != NovelAutopilotStepType::Export
        || step.chapter_id.is_some()
        || step.chapter_number.is_some()
        || claimed.step.step_type != NovelAutopilotStepType::Export.as_str()
    {
        return finish_waiting_human(db, record, claimed, step, None, EXPORT_FACTS_INVALID).await;
    }

    let artifact = match build_project_export_artifact(
        db,
        &claimed.run.project_id,
        &record.user_id,
        &config.export_format,
    )
    .await
    {
        Ok(artifact) => artifact,
        Err(ProjectExportServiceError::UnsupportedFormat(_)) => {
            return finish_waiting_human(
                db,
                record,
                claimed,
                step,
                None,
                UNSUPPORTED_EXPORT_FORMAT,
            )
            .await;
        }
        Err(error) => return Err(ExportAdapterError::Service(error)),
    };
    ensure_not_cancelled(cancellation_token)?;
    let descriptor_json = artifact
        .descriptor_json()
        .map_err(ExportAdapterError::Service)?;
    let committed = NovelAutopilotRepository::commit_export_step(
        db,
        &claimed,
        &record.user_id,
        &step.step_key,
        Some(&record.task_id),
        NovelAutopilotExportCommit {
            descriptor_json,
            descriptor: artifact.descriptor.clone(),
        },
    )
    .await
    .map_err(ExportAdapterError::Repository)?;

    Ok(ExportAdapterOutcome::StepCompleted {
        result: artifact_result(&committed.run, &artifact, "completed", None),
        run: committed.run,
    })
}

async fn finish_waiting_human(
    db: &DatabaseConnection,
    record: &TaskRecord,
    claimed: ClaimedNovelAutopilotStep,
    step: &AutopilotStepPlan,
    artifact: Option<&ProjectExportArtifact>,
    reason_code: &str,
) -> Result<ExportAdapterOutcome, ExportAdapterError> {
    let terminal = NovelAutopilotRepository::complete_step(
        db,
        &claimed.step.id,
        &record.user_id,
        claimed.run.version,
        claimed.run.epoch,
        &step.step_key,
        Some(&record.task_id),
        NovelAutopilotStepStatus::Skipped,
        NovelAutopilotStepTerminalPatch {
            result_digest: artifact.map(|artifact| artifact.descriptor.content_digest.clone()),
            quality_decision: None,
            error_code: Some(reason_code.to_string()),
        },
    )
    .await
    .map_err(ExportAdapterError::Repository)?;
    let waiting = NovelAutopilotRepository::transition_owned(
        db,
        &terminal.run.id,
        &record.user_id,
        terminal.run.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    .map_err(ExportAdapterError::Repository)?;

    Ok(ExportAdapterOutcome::WaitingHuman {
        result: match artifact {
            Some(artifact) => {
                artifact_result(&waiting, artifact, "waiting_human", Some(reason_code))
            }
            None => json!({
                "run_id": waiting.id,
                "run_status": waiting.status,
                "run_epoch": waiting.epoch,
                "run_version": waiting.version,
                "dispatch_status": "waiting_human",
                "reason_code": reason_code,
                "step_type": NovelAutopilotStepType::Export.as_str(),
            }),
        },
    })
}

fn artifact_result(
    run: &novel_autopilot_run::Model,
    artifact: &ProjectExportArtifact,
    dispatch_status: &str,
    reason_code: Option<&str>,
) -> Value {
    json!({
        "run_id": run.id,
        "run_status": run.status,
        "run_epoch": run.epoch,
        "run_version": run.version,
        "dispatch_status": dispatch_status,
        "reason_code": reason_code,
        "step_type": NovelAutopilotStepType::Export.as_str(),
        "export": artifact.descriptor,
    })
}

fn ensure_not_cancelled(
    cancellation_token: &CooperativeCancellationToken,
) -> Result<(), ExportAdapterError> {
    if cancellation_token.is_cancelled() {
        Err(ExportAdapterError::Cancelled)
    } else {
        Ok(())
    }
}
