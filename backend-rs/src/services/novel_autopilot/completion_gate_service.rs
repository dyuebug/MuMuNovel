use sea_orm::DatabaseConnection;
use serde::Serialize;

use crate::{
    models::novel_autopilot_run,
    services::{
        book_completion_consistency_service::{
            load_book_completion_consistency, BookCompletionConsistencyError,
        },
        novel_workflow_service::{
            self, NovelWorkflowAuditContext, NovelWorkflowError, NovelWorkflowPhase,
            NovelWorkflowTransitionReceipt,
        },
        project_export_service::{
            build_project_export_artifact, ProjectExportArtifactDescriptorV1,
            ProjectExportServiceError, PROJECT_EXPORT_DESCRIPTOR_SCHEMA_VERSION,
        },
    },
};

use super::{
    book_review_service::{
        load_book_review_summary, BookReviewRewriteReference, BookReviewServiceError,
    },
    router::NovelAutopilotBusinessFacts,
    types::{NovelAutopilotExecutionScope, NovelAutopilotRunConfig},
};

const WORKFLOW_COMPLETION_REASON: &str = "durable_novel_autopilot_complete_book";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct NovelAutopilotCompletionGateReport {
    pub ready: bool,
    pub reason_codes: Vec<String>,
    pub consistency_digest: Option<String>,
    pub book_review_digest: Option<String>,
    pub export_digest: Option<String>,
    pub workflow_phase: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NovelAutopilotCompletionGateDecision {
    Ready(NovelAutopilotCompletionGateReport),
    Reroute(NovelAutopilotCompletionGateReport),
    AdvanceWorkflow {
        report: NovelAutopilotCompletionGateReport,
        expected: NovelWorkflowPhase,
        target: NovelWorkflowPhase,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NovelAutopilotCompletionGateError {
    Consistency(BookCompletionConsistencyError),
    BookReview(BookReviewServiceError),
    ProjectExport(ProjectExportServiceError),
    Workflow(NovelWorkflowError),
    InvalidPendingRewrites,
}

impl NovelAutopilotCompletionGateError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Consistency(error) => error.code(),
            Self::BookReview(error) => error.code(),
            Self::ProjectExport(error) => error.code(),
            Self::Workflow(_) => "novel_workflow_error",
            Self::InvalidPendingRewrites => "invalid_pending_rewrites",
        }
    }
}

pub(crate) async fn evaluate_complete_book_completion_gate(
    db: &DatabaseConnection,
    run: &novel_autopilot_run::Model,
    user_id: &str,
    config: &NovelAutopilotRunConfig,
    facts: &NovelAutopilotBusinessFacts,
) -> Result<NovelAutopilotCompletionGateDecision, NovelAutopilotCompletionGateError> {
    let expected_chapter_count = u32::try_from(run.total_chapters).unwrap_or_default();
    let consistency =
        load_book_completion_consistency(db, &run.project_id, user_id, expected_chapter_count)
            .await
            .map_err(NovelAutopilotCompletionGateError::Consistency)?;
    let review = load_book_review_summary(db, &run.project_id, user_id, expected_chapter_count)
        .await
        .map_err(NovelAutopilotCompletionGateError::BookReview)?;
    let pending_rewrites =
        serde_json::from_value::<Vec<BookReviewRewriteReference>>(run.pending_rewrites.clone())
            .map_err(|_| NovelAutopilotCompletionGateError::InvalidPendingRewrites)?;
    let workflow = novel_workflow_service::get_state(db, &run.project_id, user_id)
        .await
        .map_err(NovelAutopilotCompletionGateError::Workflow)?;

    let mut reasons = Vec::new();
    if config.execution_scope != NovelAutopilotExecutionScope::CompleteBook {
        reasons.push("execution_scope_not_complete_book".to_string());
    }
    if !consistency.ready {
        reasons.push("book_completion_consistency_not_ready".to_string());
    }
    if facts.target_chapter_count == 0
        || facts.target_chapter_count != expected_chapter_count
        || facts.completed_chapter_count != expected_chapter_count
        || consistency.completed_chapter_record_count != expected_chapter_count
    {
        reasons.push("chapter_completion_count_mismatch".to_string());
    }
    if facts.next_incomplete_chapter_id.is_some() {
        reasons.push("next_incomplete_chapter_present".to_string());
    }
    if facts.pending_analysis_chapter_id.is_some() {
        reasons.push("pending_analysis_present".to_string());
    }
    if facts.pending_repair_chapter_id.is_some() {
        reasons.push("pending_repair_present".to_string());
    }
    if !review.ready {
        reasons.push("book_review_summary_not_ready".to_string());
    }
    if config.run_book_review && !facts.book_review_completed {
        reasons.push("book_review_step_not_current".to_string());
    }
    if config.run_book_polish && (!pending_rewrites.is_empty() || !facts.book_polish_completed) {
        reasons.push("book_polish_not_completed".to_string());
    }

    let descriptor = parse_final_export_descriptor(run.final_export_ref.as_deref());
    let mut export_digest = descriptor
        .as_ref()
        .map(|descriptor| descriptor.content_digest.clone());
    match descriptor {
        Some(descriptor)
            if descriptor.schema_version == PROJECT_EXPORT_DESCRIPTOR_SCHEMA_VERSION
                && descriptor.project_id == run.project_id
                && descriptor.format == config.export_format =>
        {
            let artifact =
                build_project_export_artifact(db, &run.project_id, user_id, &config.export_format)
                    .await
                    .map_err(NovelAutopilotCompletionGateError::ProjectExport)?;
            export_digest = Some(artifact.descriptor.content_digest.clone());
            if artifact.descriptor != descriptor || !facts.export_completed {
                reasons.push("final_export_not_current".to_string());
            }
        }
        _ => reasons.push("final_export_ref_invalid".to_string()),
    }

    let mut report = NovelAutopilotCompletionGateReport {
        ready: false,
        reason_codes: reasons,
        consistency_digest: Some(consistency.result_digest),
        book_review_digest: Some(review.result_digest),
        export_digest,
        workflow_phase: workflow.phase.as_str().to_string(),
    };
    if !report.reason_codes.is_empty() {
        return Ok(NovelAutopilotCompletionGateDecision::Reroute(report));
    }

    if let Some(target) = next_completion_workflow_phase(workflow.phase) {
        return Ok(NovelAutopilotCompletionGateDecision::AdvanceWorkflow {
            report,
            expected: workflow.phase,
            target,
        });
    }

    report.ready = true;
    Ok(NovelAutopilotCompletionGateDecision::Ready(report))
}

pub(crate) async fn advance_complete_book_workflow_once(
    db: &DatabaseConnection,
    run: &novel_autopilot_run::Model,
    user_id: &str,
    expected: NovelWorkflowPhase,
    target: NovelWorkflowPhase,
) -> Result<NovelWorkflowTransitionReceipt, NovelAutopilotCompletionGateError> {
    if next_completion_workflow_phase(expected) != Some(target) {
        return Err(NovelAutopilotCompletionGateError::Workflow(
            NovelWorkflowError::IllegalTransition {
                from: expected,
                to: target,
            },
        ));
    }

    novel_workflow_service::transition(
        db,
        &run.project_id,
        user_id,
        expected,
        target,
        NovelWorkflowAuditContext {
            reason: Some(WORKFLOW_COMPLETION_REASON.to_string()),
            related_task_id: Some(run.id.clone()),
        },
    )
    .await
    .map_err(NovelAutopilotCompletionGateError::Workflow)
}

fn parse_final_export_descriptor(raw: Option<&str>) -> Option<ProjectExportArtifactDescriptorV1> {
    raw.filter(|value| !value.trim().is_empty())
        .and_then(|value| serde_json::from_str(value).ok())
}

fn next_completion_workflow_phase(current: NovelWorkflowPhase) -> Option<NovelWorkflowPhase> {
    match current {
        NovelWorkflowPhase::Inspiration => Some(NovelWorkflowPhase::Foundation),
        NovelWorkflowPhase::Foundation => Some(NovelWorkflowPhase::WorldBuilding),
        NovelWorkflowPhase::WorldBuilding => Some(NovelWorkflowPhase::CharacterDesign),
        NovelWorkflowPhase::CharacterDesign => Some(NovelWorkflowPhase::Outline),
        NovelWorkflowPhase::Outline => Some(NovelWorkflowPhase::Writing),
        NovelWorkflowPhase::Writing => Some(NovelWorkflowPhase::Reviewing),
        NovelWorkflowPhase::Reviewing => Some(NovelWorkflowPhase::Polishing),
        NovelWorkflowPhase::Polishing => Some(NovelWorkflowPhase::Completed),
        NovelWorkflowPhase::Completed => None,
    }
}

#[cfg(test)]
mod tests {
    use super::next_completion_workflow_phase;
    use crate::services::novel_workflow_service::NovelWorkflowPhase;

    #[test]
    fn completion_workflow_advances_one_legal_phase_per_tick() {
        let expected = [
            (
                NovelWorkflowPhase::Inspiration,
                NovelWorkflowPhase::Foundation,
            ),
            (
                NovelWorkflowPhase::Foundation,
                NovelWorkflowPhase::WorldBuilding,
            ),
            (
                NovelWorkflowPhase::WorldBuilding,
                NovelWorkflowPhase::CharacterDesign,
            ),
            (
                NovelWorkflowPhase::CharacterDesign,
                NovelWorkflowPhase::Outline,
            ),
            (NovelWorkflowPhase::Outline, NovelWorkflowPhase::Writing),
            (NovelWorkflowPhase::Writing, NovelWorkflowPhase::Reviewing),
            (NovelWorkflowPhase::Reviewing, NovelWorkflowPhase::Polishing),
            (NovelWorkflowPhase::Polishing, NovelWorkflowPhase::Completed),
        ];
        for (current, target) in expected {
            assert_eq!(next_completion_workflow_phase(current), Some(target));
            assert!(current.can_transition_to(target));
        }
        assert_eq!(
            next_completion_workflow_phase(NovelWorkflowPhase::Completed),
            None
        );
    }
}
