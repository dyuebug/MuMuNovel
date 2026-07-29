use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    services::{
        chapter_candidate_route_gateway_service::ChapterCandidateRouteGatewayConfig,
        cooperative_cancellation_service::CooperativeCancellationToken,
    },
    tasks::types::TaskRecord,
};

use super::{
    book_polish_adapter::{
        execute_book_polish_step, BookPolishAdapterError, BookPolishAdapterOutcome,
    },
    book_review_adapter::{
        execute_book_review_step, BookReviewAdapterError, BookReviewAdapterOutcome,
    },
    budget_guard::{evaluate_postflight, evaluate_preflight, NovelAutopilotBudgetViolation},
    career_adapter::{execute_career_design_step, CareerAdapterError, CareerAdapterOutcome},
    chapter_adapter::{execute_chapter_generate_step, ChapterAdapterError, ChapterAdapterOutcome},
    chapter_analysis_adapter::{
        execute_chapter_analysis_step, ChapterAnalysisAdapterError, ChapterAnalysisAdapterOutcome,
    },
    chapter_repair_adapter::{
        execute_chapter_repair_step, ChapterRepairAdapterError, ChapterRepairAdapterOutcome,
    },
    character_adapter::{
        execute_character_design_step, CharacterAdapterError, CharacterAdapterOutcome,
    },
    completion_gate_service::{
        advance_complete_book_workflow_once, evaluate_complete_book_completion_gate,
        NovelAutopilotCompletionGateDecision, NovelAutopilotCompletionGateError,
    },
    export_adapter::{execute_export_step, ExportAdapterError, ExportAdapterOutcome},
    facts::{
        enrich_novel_autopilot_completion_facts, load_novel_autopilot_business_facts,
        NovelAutopilotFactsError, NovelAutopilotQualityFactScope,
    },
    foundation_adapter::{
        execute_foundation_step, FoundationAdapterError, FoundationAdapterOutcome,
    },
    organization_adapter::{
        execute_organization_design_step, OrganizationAdapterError, OrganizationAdapterOutcome,
    },
    outline_adapter::{execute_outline_design_step, OutlineAdapterError, OutlineAdapterOutcome},
    outline_expansion_adapter::{
        execute_outline_expansion_step, OutlineExpansionAdapterError,
        OutlineExpansionAdapterOutcome,
    },
    output_observer::NovelAutopilotOutputObserver,
    repository::{
        CreateNovelAutopilotStepAttempt, NovelAutopilotRepository, NovelAutopilotRepositoryError,
        NovelAutopilotStepTerminalPatch, PrepareAndClaimNovelAutopilotStep,
    },
    router::{
        route_next_step, AutopilotStepPlan, NovelAutopilotRouteDecision,
        NovelAutopilotRouteSnapshot,
    },
    types::{
        NovelAutopilotExecutionScope, NovelAutopilotPrivateSnapshot, NovelAutopilotRunConfig,
        NovelAutopilotRunStatus, NovelAutopilotStepStatus, NovelAutopilotStepType,
    },
    world_adapter::{execute_world_building_step, WorldAdapterError, WorldAdapterOutcome},
};

const EXECUTION_FAILED: &str = "小说自动创作编排步骤执行失败";
const EXECUTION_CANCELLED: &str = "小说自动创作编排步骤已取消";
const PLANNING_ADAPTER_UNAVAILABLE: &str = "planning_adapter_unavailable";
const ROUTER_IDLE: &str = "router_idle";
const INVALID_ROUTER_FACTS: &str = "invalid_router_facts";
const HUMAN_DECISION_CANDIDATE_UNAVAILABLE: &str = "human_decision_candidate_unavailable";
const HUMAN_DECISION_CANDIDATE_STALE: &str = "human_decision_candidate_stale";
const HUMAN_DECISION_RETRY_ROUTE_MISMATCH: &str = "human_decision_retry_route_mismatch";
const HUMAN_DECISION_REPAIR_NOT_SUPPORTED: &str = "human_decision_repair_not_supported";
const HUMAN_DECISION_INVALID: &str = "human_decision_invalid";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotNextTickLease {
    pub run_id: String,
    pub project_id: String,
    pub user_id: String,
    pub epoch: i64,
    pub version: i64,
    pub current_phase: String,
}

#[derive(Debug)]
pub(crate) enum NovelAutopilotTickOutcome {
    Completed {
        task_result: Value,
    },
    AwaitingHuman {
        task_result: Value,
    },
    ScheduleNext {
        task_result: Value,
        lease: NovelAutopilotNextTickLease,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TickTaskBinding {
    AlreadyBound,
    NeedsBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AppliedHumanDecision {
    Route(NovelAutopilotRouteDecision),
    WaitForHuman(&'static str),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NovelBookAutopilotTaskPayload {
    run_id: String,
    run_epoch: i64,
    run_version: i64,
    #[serde(default)]
    decision: Option<String>,
    project_id: String,
    user_id: String,
}

/// Executes exactly one durable orchestration tick.
///
/// This coordinator deliberately has no loop.  The Run/Step records are the source of
/// truth; a Background Task only carries the currently scheduled tick and may be lost
/// or cancelled independently of the durable workflow.
pub(crate) async fn execute_novel_book_autopilot_tick(
    db: &DatabaseConnection,
    record: &TaskRecord,
    payload: Value,
    candidate_gateway_config: &ChapterCandidateRouteGatewayConfig,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: CooperativeCancellationToken,
) -> Result<NovelAutopilotTickOutcome, String> {
    let payload: NovelBookAutopilotTaskPayload =
        serde_json::from_value(payload).map_err(|_| EXECUTION_FAILED.to_string())?;
    validate_payload(&payload, record)?;
    ensure_not_cancelled(&cancellation_token)?;

    let mut run = NovelAutopilotRepository::find_owned(db, &payload.run_id, &record.user_id)
        .await
        .map_err(map_repository_error)?;
    if run.project_id != record.project_id || run.epoch != payload.run_epoch {
        return Err(EXECUTION_FAILED.to_string());
    }

    match classify_tick_task_binding(
        run.version,
        payload.run_version,
        run.active_background_task_id.as_deref(),
        &record.task_id,
    )? {
        TickTaskBinding::AlreadyBound => {}
        TickTaskBinding::NeedsBinding => {
            run = NovelAutopilotRepository::set_active_background_task_owned(
                db,
                &run.id,
                &record.user_id,
                run.version,
                run.epoch,
                Some(&record.task_id),
            )
            .await
            .map_err(map_repository_error)?;
        }
    }
    ensure_not_cancelled(&cancellation_token)?;

    let status = run
        .status
        .parse::<NovelAutopilotRunStatus>()
        .map_err(|_| EXECUTION_FAILED.to_string())?;
    if status == NovelAutopilotRunStatus::Queued {
        run = NovelAutopilotRepository::transition_owned(
            db,
            &run.id,
            &record.user_id,
            run.version,
            NovelAutopilotRunStatus::Running,
        )
        .await
        .map_err(map_repository_error)?;
    } else if status != NovelAutopilotRunStatus::Running {
        return Err(EXECUTION_CANCELLED.to_string());
    }
    ensure_not_cancelled(&cancellation_token)?;

    let NovelAutopilotPrivateSnapshot { config, guidance } =
        NovelAutopilotPrivateSnapshot::decode(&run.config_snapshot)
            .map_err(|_| EXECUTION_FAILED.to_string())?;
    let latest_step = if payload.decision.is_some() {
        NovelAutopilotRepository::list_steps_owned(db, &run.id, &record.user_id)
            .await
            .map_err(map_repository_error)?
            .pop()
    } else {
        None
    };
    if payload.decision.as_deref() == Some("accept")
        && latest_step
            .as_ref()
            .is_some_and(is_manual_review_candidate_step)
    {
        let latest_step = latest_step
            .as_ref()
            .expect("manual-review candidate predicate requires a latest step");
        match NovelAutopilotRepository::accept_chapter_manual_review_candidate(
            db,
            &run.id,
            &latest_step.id,
            &record.user_id,
            run.version,
            run.epoch,
            Some(&record.task_id),
        )
        .await
        {
            Ok(accepted) => {
                return Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: json!({
                        "run_id": accepted.run.id,
                        "run_status": accepted.run.status,
                        "run_epoch": accepted.run.epoch,
                        "run_version": accepted.run.version,
                        "dispatch_status": "candidate_accepted",
                        "candidate_id": accepted.candidate_id,
                        "step_id": accepted.step.id,
                        "step_type": accepted.step.step_type,
                        "chapter_id": accepted.chapter_id,
                        "chapter_number": accepted.chapter_number,
                        "word_count": accepted.word_count,
                        "quality_decision": "accept",
                    }),
                    lease: next_tick_lease(&accepted.run),
                });
            }
            Err(error @ NovelAutopilotRepositoryError::Database(_)) => {
                return Err(map_repository_error(error));
            }
            Err(error) => {
                let reason_code = match error {
                    NovelAutopilotRepositoryError::NotFoundOrAccessDenied
                    | NovelAutopilotRepositoryError::InvalidConfig { .. } => {
                        HUMAN_DECISION_CANDIDATE_UNAVAILABLE
                    }
                    NovelAutopilotRepositoryError::InvalidTransition
                    | NovelAutopilotRepositoryError::StaleVersion
                    | NovelAutopilotRepositoryError::StaleEpoch
                    | NovelAutopilotRepositoryError::BusinessDataChanged => {
                        HUMAN_DECISION_CANDIDATE_STALE
                    }
                    NovelAutopilotRepositoryError::Database(_) => unreachable!(),
                };
                let current = NovelAutopilotRepository::find_owned(db, &run.id, &record.user_id)
                    .await
                    .map_err(map_repository_error)?;
                let task_result =
                    wait_for_human(db, &current, &record.user_id, reason_code, None).await?;
                return Ok(NovelAutopilotTickOutcome::AwaitingHuman { task_result });
            }
        }
    }
    let quality_scope = match config.execution_scope {
        NovelAutopilotExecutionScope::CompleteBook => NovelAutopilotQualityFactScope::AllChapters,
        NovelAutopilotExecutionScope::PlanningOnly
        | NovelAutopilotExecutionScope::NextNChapters
        | NovelAutopilotExecutionScope::ContinueFromCurrent => {
            NovelAutopilotQualityFactScope::CurrentChapter(run.current_chapter_id.as_deref())
        }
    };
    let mut facts = load_novel_autopilot_business_facts(
        db,
        &run.project_id,
        &record.user_id,
        u32::try_from(run.total_chapters).unwrap_or_default(),
        u32::try_from(run.completed_chapters).unwrap_or_default(),
        quality_scope,
    )
    .await
    .map_err(map_facts_error)?;
    enrich_novel_autopilot_completion_facts(db, &run, &record.user_id, &config, &mut facts)
        .await
        .map_err(map_facts_error)?;
    // Facts are read from mutable business tables. Re-check cancellation immediately before a
    // Router decision can become a claimed durable Step.
    ensure_not_cancelled(&cancellation_token)?;
    let routed_decision = route_next_step(&NovelAutopilotRouteSnapshot {
        status: NovelAutopilotRunStatus::Running,
        config: config.clone(),
        facts: facts.clone(),
    });
    let decision = match apply_human_decision(
        payload.decision.as_deref(),
        routed_decision,
        latest_step.as_ref(),
    ) {
        AppliedHumanDecision::Route(decision) => decision,
        AppliedHumanDecision::WaitForHuman(reason_code) => {
            let task_result = wait_for_human(db, &run, &record.user_id, reason_code, None).await?;
            return Ok(NovelAutopilotTickOutcome::AwaitingHuman { task_result });
        }
    };

    match decision {
        NovelAutopilotRouteDecision::Execute(step) => {
            let latest_attempt = NovelAutopilotRepository::latest_step_attempt_owned(
                db,
                &run.id,
                &record.user_id,
                &step.step_key,
            )
            .await
            .map_err(map_repository_error)?;
            if let Some(violation) =
                evaluate_preflight(&run, &config, &step, latest_attempt, Utc::now().naive_utc())
            {
                return wait_for_budget(db, record, &run, violation, Some(&step)).await;
            }
            execute_deferred_step(
                db,
                record,
                run,
                step,
                &config,
                guidance.as_deref(),
                candidate_gateway_config,
                output_observer,
                &cancellation_token,
            )
            .await
        }
        NovelAutopilotRouteDecision::Complete(phase) => {
            if config.execution_scope != NovelAutopilotExecutionScope::CompleteBook {
                let completed = NovelAutopilotRepository::transition_owned(
                    db,
                    &run.id,
                    &record.user_id,
                    run.version,
                    NovelAutopilotRunStatus::Completed,
                )
                .await
                .map_err(map_repository_error)?;
                return Ok(NovelAutopilotTickOutcome::Completed {
                    task_result: json!({
                        "run_id": completed.id,
                        "run_status": completed.status,
                        "run_epoch": completed.epoch,
                        "run_version": completed.version,
                        "dispatch_status": "completed",
                        "next_step": route_decision_view(NovelAutopilotRouteDecision::Complete(phase)),
                    }),
                });
            }

            ensure_not_cancelled(&cancellation_token)?;
            match evaluate_complete_book_completion_gate(db, &run, &record.user_id, &config, &facts)
                .await
                .map_err(map_completion_gate_error)?
            {
                NovelAutopilotCompletionGateDecision::Reroute(report) => {
                    let released = NovelAutopilotRepository::set_active_background_task_owned(
                        db,
                        &run.id,
                        &record.user_id,
                        run.version,
                        run.epoch,
                        None,
                    )
                    .await
                    .map_err(map_repository_error)?;
                    Ok(NovelAutopilotTickOutcome::ScheduleNext {
                        task_result: json!({
                            "run_id": released.id,
                            "run_status": released.status,
                            "run_epoch": released.epoch,
                            "run_version": released.version,
                            "dispatch_status": "completion_gate_reroute",
                            "completion_gate": report,
                        }),
                        lease: next_tick_lease(&released),
                    })
                }
                NovelAutopilotCompletionGateDecision::AdvanceWorkflow {
                    report,
                    expected,
                    target,
                } => {
                    ensure_not_cancelled(&cancellation_token)?;
                    let receipt = advance_complete_book_workflow_once(
                        db,
                        &run,
                        &record.user_id,
                        expected,
                        target,
                    )
                    .await
                    .map_err(map_completion_gate_error)?;
                    let released = NovelAutopilotRepository::set_active_background_task_owned(
                        db,
                        &run.id,
                        &record.user_id,
                        run.version,
                        run.epoch,
                        None,
                    )
                    .await
                    .map_err(map_repository_error)?;
                    Ok(NovelAutopilotTickOutcome::ScheduleNext {
                        task_result: json!({
                            "run_id": released.id,
                            "run_status": released.status,
                            "run_epoch": released.epoch,
                            "run_version": released.version,
                            "dispatch_status": "workflow_advanced",
                            "workflow_previous_phase": receipt.previous_phase,
                            "workflow_phase": receipt.state.phase,
                            "completion_gate": report,
                        }),
                        lease: next_tick_lease(&released),
                    })
                }
                NovelAutopilotCompletionGateDecision::Ready(report) => {
                    ensure_not_cancelled(&cancellation_token)?;
                    let completed = NovelAutopilotRepository::transition_owned(
                        db,
                        &run.id,
                        &record.user_id,
                        run.version,
                        NovelAutopilotRunStatus::Completed,
                    )
                    .await
                    .map_err(map_repository_error)?;
                    Ok(NovelAutopilotTickOutcome::Completed {
                        task_result: json!({
                            "run_id": completed.id,
                            "run_status": completed.status,
                            "run_epoch": completed.epoch,
                            "run_version": completed.version,
                            "dispatch_status": "completed",
                            "next_step": route_decision_view(NovelAutopilotRouteDecision::Complete(phase)),
                            "completion_gate": report,
                        }),
                    })
                }
            }
        }
        NovelAutopilotRouteDecision::Idle => {
            wait_for_human(db, &run, &record.user_id, ROUTER_IDLE, None)
                .await
                .map(|task_result| NovelAutopilotTickOutcome::AwaitingHuman { task_result })
        }
        NovelAutopilotRouteDecision::InvalidFacts(code) => {
            wait_for_human(db, &run, &record.user_id, INVALID_ROUTER_FACTS, Some(code))
                .await
                .map(|task_result| NovelAutopilotTickOutcome::AwaitingHuman { task_result })
        }
    }
}

async fn execute_deferred_step(
    db: &DatabaseConnection,
    record: &TaskRecord,
    run: crate::models::novel_autopilot_run::Model,
    step: AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    additional_guidance: Option<&str>,
    candidate_gateway_config: &ChapterCandidateRouteGatewayConfig,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<NovelAutopilotTickOutcome, String> {
    let run_id = run.id.clone();
    let run_epoch = run.epoch;
    output_observer.reset_estimated_tokens();
    let outcome = execute_deferred_step_unmetered(
        db,
        record,
        run,
        step,
        config,
        additional_guidance,
        candidate_gateway_config,
        output_observer,
        cancellation_token,
    )
    .await?;
    persist_observed_usage(
        db,
        record,
        &run_id,
        run_epoch,
        config,
        output_observer,
        outcome,
    )
    .await
}

async fn execute_deferred_step_unmetered(
    db: &DatabaseConnection,
    record: &TaskRecord,
    run: crate::models::novel_autopilot_run::Model,
    step: AutopilotStepPlan,
    config: &NovelAutopilotRunConfig,
    additional_guidance: Option<&str>,
    candidate_gateway_config: &ChapterCandidateRouteGatewayConfig,
    output_observer: &NovelAutopilotOutputObserver,
    cancellation_token: &CooperativeCancellationToken,
) -> Result<NovelAutopilotTickOutcome, String> {
    let input_digest = step_input_digest(&run, &step);
    let claimed = NovelAutopilotRepository::prepare_and_claim_step(
        db,
        PrepareAndClaimNovelAutopilotStep {
            attempt: CreateNovelAutopilotStepAttempt {
                run_id: run.id.clone(),
                user_id: record.user_id.clone(),
                step_key: step.step_key.clone(),
                step_type: step.step_type,
                phase: step.phase,
                chapter_id: step.chapter_id.clone(),
                chapter_number: step.chapter_number,
                run_epoch: run.epoch,
                input_digest,
            },
            expected_run_version: run.version,
            background_task_id: Some(record.task_id.clone()),
        },
    )
    .await
    .map_err(map_repository_error)?;

    ensure_not_cancelled(cancellation_token)?;
    if step.step_type == NovelAutopilotStepType::Foundation {
        return match execute_foundation_step(
            db,
            record,
            claimed,
            &step,
            additional_guidance,
            output_observer,
            cancellation_token,
        )
        .await
        .map_err(map_foundation_adapter_error)?
        {
            FoundationAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            FoundationAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }
    if step.step_type == NovelAutopilotStepType::WorldBuilding {
        return match execute_world_building_step(
            db,
            record,
            claimed,
            &step,
            additional_guidance,
            output_observer,
            cancellation_token,
        )
        .await
        .map_err(map_world_adapter_error)?
        {
            WorldAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            WorldAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }
    if step.step_type == NovelAutopilotStepType::CareerDesign {
        return match execute_career_design_step(
            db,
            record,
            claimed,
            &step,
            additional_guidance,
            output_observer,
            cancellation_token,
        )
        .await
        .map_err(map_career_adapter_error)?
        {
            CareerAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            CareerAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }
    if step.step_type == NovelAutopilotStepType::CharacterDesign {
        return match execute_character_design_step(
            db,
            record,
            claimed,
            &step,
            additional_guidance,
            output_observer,
            cancellation_token,
        )
        .await
        .map_err(map_character_adapter_error)?
        {
            CharacterAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            CharacterAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }
    if step.step_type == NovelAutopilotStepType::OrganizationDesign {
        return match execute_organization_design_step(
            db,
            record,
            claimed,
            &step,
            additional_guidance,
            output_observer,
            cancellation_token,
        )
        .await
        .map_err(map_organization_adapter_error)?
        {
            OrganizationAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            OrganizationAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }

    if step.step_type == NovelAutopilotStepType::Outline {
        return match execute_outline_design_step(
            db,
            record,
            claimed,
            &step,
            additional_guidance,
            output_observer,
            cancellation_token,
        )
        .await
        .map_err(map_outline_adapter_error)?
        {
            OutlineAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            OutlineAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }
    if step.step_type == NovelAutopilotStepType::OutlineExpand {
        return match execute_outline_expansion_step(
            db,
            record,
            claimed,
            &step,
            additional_guidance,
            output_observer,
            cancellation_token,
        )
        .await
        .map_err(map_outline_expansion_adapter_error)?
        {
            OutlineExpansionAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            OutlineExpansionAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }
    if step.step_type == NovelAutopilotStepType::ChapterGenerate {
        return match execute_chapter_generate_step(
            db,
            record,
            claimed,
            &step,
            config,
            additional_guidance,
            candidate_gateway_config,
            output_observer,
            cancellation_token,
        )
        .await
        .map_err(map_chapter_adapter_error)?
        {
            ChapterAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            ChapterAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }
    if step.step_type == NovelAutopilotStepType::ChapterAnalyze {
        return match execute_chapter_analysis_step(
            db,
            record,
            claimed,
            &step,
            config,
            additional_guidance,
            output_observer,
            cancellation_token,
        )
        .await
        .map_err(map_chapter_analysis_adapter_error)?
        {
            ChapterAnalysisAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            ChapterAnalysisAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }
    if step.step_type == NovelAutopilotStepType::ChapterRepair {
        return match execute_chapter_repair_step(
            db,
            record,
            claimed,
            &step,
            config,
            additional_guidance,
            candidate_gateway_config,
            output_observer,
            cancellation_token,
        )
        .await
        .map_err(map_chapter_repair_adapter_error)?
        {
            ChapterRepairAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            ChapterRepairAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }

    if step.step_type == NovelAutopilotStepType::BookReview {
        return match execute_book_review_step(db, record, claimed, &step, cancellation_token)
            .await
            .map_err(map_book_review_adapter_error)?
        {
            BookReviewAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            BookReviewAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }

    if step.step_type == NovelAutopilotStepType::BookPolish {
        return match execute_book_polish_step(
            db,
            record,
            claimed,
            &step,
            config,
            additional_guidance,
            output_observer,
            cancellation_token,
        )
        .await
        .map_err(map_book_polish_adapter_error)?
        {
            BookPolishAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            BookPolishAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }

    if step.step_type == NovelAutopilotStepType::Export {
        return match execute_export_step(db, record, claimed, &step, config, cancellation_token)
            .await
            .map_err(map_export_adapter_error)?
        {
            ExportAdapterOutcome::StepCompleted { result, run } => {
                Ok(NovelAutopilotTickOutcome::ScheduleNext {
                    task_result: result,
                    lease: next_tick_lease(&run),
                })
            }
            ExportAdapterOutcome::WaitingHuman { result } => {
                Ok(NovelAutopilotTickOutcome::AwaitingHuman {
                    task_result: result,
                })
            }
        };
    }

    // The remaining planning/book-completion adapters are not connected yet. Do not claim that a
    // model call, business write, or export succeeded. Close the attempt and expose a
    // human gate until each typed adapter is implemented.
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
            result_digest: Some(deferred_step_result_digest(&claimed.step.id, &step)),
            quality_decision: None,
            error_code: Some(PLANNING_ADAPTER_UNAVAILABLE.to_string()),
        },
    )
    .await
    .map_err(map_repository_error)?;

    let waiting = NovelAutopilotRepository::transition_owned(
        db,
        &terminal.run.id,
        &record.user_id,
        terminal.run.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    .map_err(map_repository_error)?;

    Ok(NovelAutopilotTickOutcome::AwaitingHuman {
        task_result: json!({
            "run_id": waiting.id,
            "run_status": waiting.status,
            "run_epoch": waiting.epoch,
            "run_version": waiting.version,
            "dispatch_status": "waiting_human",
            "reason_code": PLANNING_ADAPTER_UNAVAILABLE,
            "next_step": route_decision_view(NovelAutopilotRouteDecision::Execute(step)),
        }),
    })
}

async fn persist_observed_usage(
    db: &DatabaseConnection,
    record: &TaskRecord,
    run_id: &str,
    run_epoch: i64,
    config: &NovelAutopilotRunConfig,
    output_observer: &NovelAutopilotOutputObserver,
    outcome: NovelAutopilotTickOutcome,
) -> Result<NovelAutopilotTickOutcome, String> {
    let estimated_tokens = output_observer.take_estimated_tokens();
    if estimated_tokens == 0 {
        return Ok(outcome);
    }

    let current = NovelAutopilotRepository::find_owned(db, run_id, &record.user_id)
        .await
        .map_err(map_repository_error)?;
    if current.epoch != run_epoch {
        return Err(EXECUTION_CANCELLED.to_string());
    }
    let updated = NovelAutopilotRepository::increment_estimated_usage_owned(
        db,
        run_id,
        &record.user_id,
        current.version,
        run_epoch,
        current.active_background_task_id.as_deref(),
        estimated_tokens,
    )
    .await
    .map_err(map_repository_error)?;

    match outcome {
        NovelAutopilotTickOutcome::ScheduleNext {
            mut task_result, ..
        } => {
            update_usage_result(&mut task_result, &updated);
            if let Some(violation) = evaluate_postflight(&updated, config, Utc::now().naive_utc()) {
                return wait_for_budget(db, record, &updated, violation, None).await;
            }
            Ok(NovelAutopilotTickOutcome::ScheduleNext {
                task_result,
                lease: next_tick_lease(&updated),
            })
        }
        NovelAutopilotTickOutcome::AwaitingHuman { mut task_result } => {
            update_usage_result(&mut task_result, &updated);
            Ok(NovelAutopilotTickOutcome::AwaitingHuman { task_result })
        }
        NovelAutopilotTickOutcome::Completed { mut task_result } => {
            update_usage_result(&mut task_result, &updated);
            Ok(NovelAutopilotTickOutcome::Completed { task_result })
        }
    }
}

async fn wait_for_budget(
    db: &DatabaseConnection,
    record: &TaskRecord,
    run: &crate::models::novel_autopilot_run::Model,
    violation: NovelAutopilotBudgetViolation,
    step: Option<&AutopilotStepPlan>,
) -> Result<NovelAutopilotTickOutcome, String> {
    let waiting = NovelAutopilotRepository::wait_for_budget_owned(
        db,
        &run.id,
        &record.user_id,
        run.version,
        run.epoch,
        run.active_background_task_id.as_deref(),
        violation.code(),
    )
    .await
    .map_err(map_repository_error)?;
    tracing::warn!(
        event = "novel_book_autopilot_budget_waiting_human",
        error_code = violation.code(),
        run_id = %waiting.id,
        run_epoch = waiting.epoch,
        run_version = waiting.version,
        "durable novel autopilot stopped before another model call because a budget guard fired"
    );
    Ok(NovelAutopilotTickOutcome::AwaitingHuman {
        task_result: json!({
            "run_id": waiting.id,
            "run_status": waiting.status,
            "run_epoch": waiting.epoch,
            "run_version": waiting.version,
            "dispatch_status": "waiting_human",
            "reason_code": violation.code(),
            "used_tokens": waiting.used_tokens,
            "estimated_cost": waiting.estimated_cost,
            "next_step": step.map(|step| route_decision_view(
                NovelAutopilotRouteDecision::Execute(step.clone())
            )),
        }),
    })
}

fn update_usage_result(task_result: &mut Value, run: &crate::models::novel_autopilot_run::Model) {
    let Value::Object(result) = task_result else {
        return;
    };
    result.insert("run_version".to_string(), json!(run.version));
    result.insert("used_tokens".to_string(), json!(run.used_tokens));
    result.insert("estimated_cost".to_string(), json!(run.estimated_cost));
}

async fn wait_for_human(
    db: &DatabaseConnection,
    run: &crate::models::novel_autopilot_run::Model,
    user_id: &str,
    reason_code: &str,
    router_code: Option<&str>,
) -> Result<Value, String> {
    let waiting = NovelAutopilotRepository::transition_owned(
        db,
        &run.id,
        user_id,
        run.version,
        NovelAutopilotRunStatus::WaitingHuman,
    )
    .await
    .map_err(map_repository_error)?;
    Ok(json!({
        "run_id": waiting.id,
        "run_status": waiting.status,
        "run_epoch": waiting.epoch,
        "run_version": waiting.version,
        "dispatch_status": "waiting_human",
        "reason_code": reason_code,
        "router_code": router_code,
    }))
}

fn next_tick_lease(run: &crate::models::novel_autopilot_run::Model) -> NovelAutopilotNextTickLease {
    NovelAutopilotNextTickLease {
        run_id: run.id.clone(),
        project_id: run.project_id.clone(),
        user_id: run.user_id.clone(),
        epoch: run.epoch,
        version: run.version,
        current_phase: run.current_phase.clone(),
    }
}

fn is_manual_review_candidate_step(step: &crate::models::novel_autopilot_step_run::Model) -> bool {
    matches!(
        step.error_code.as_deref(),
        Some("chapter_quality_manual_review" | "chapter_repair_manual_review")
    )
}

fn apply_human_decision(
    decision: Option<&str>,
    routed: NovelAutopilotRouteDecision,
    latest_step: Option<&crate::models::novel_autopilot_step_run::Model>,
) -> AppliedHumanDecision {
    let Some(decision) = decision else {
        return AppliedHumanDecision::Route(routed);
    };
    let Some(latest_step) = latest_step else {
        return AppliedHumanDecision::WaitForHuman(HUMAN_DECISION_INVALID);
    };

    match decision {
        "accept" => {
            if matches!(
                latest_step.error_code.as_deref(),
                Some("chapter_quality_manual_review" | "chapter_repair_manual_review")
            ) {
                AppliedHumanDecision::WaitForHuman(HUMAN_DECISION_CANDIDATE_UNAVAILABLE)
            } else {
                AppliedHumanDecision::Route(routed)
            }
        }
        "retry" => match &routed {
            NovelAutopilotRouteDecision::Execute(step) if step.step_key == latest_step.step_key => {
                AppliedHumanDecision::Route(routed)
            }
            _ => AppliedHumanDecision::WaitForHuman(HUMAN_DECISION_RETRY_ROUTE_MISMATCH),
        },
        "repair" => {
            let step_type = latest_step.step_type.parse::<NovelAutopilotStepType>().ok();
            let chapter_number = latest_step
                .chapter_number
                .and_then(|value| u32::try_from(value).ok());
            match (step_type, latest_step.chapter_id.clone(), chapter_number) {
                (
                    Some(
                        NovelAutopilotStepType::ChapterAnalyze
                        | NovelAutopilotStepType::ChapterRepair,
                    ),
                    Some(chapter_id),
                    Some(chapter_number),
                ) => AppliedHumanDecision::Route(NovelAutopilotRouteDecision::Execute(
                    AutopilotStepPlan::chapter(
                        chapter_number,
                        chapter_id,
                        "repair",
                        NovelAutopilotStepType::ChapterRepair,
                    ),
                )),
                _ => AppliedHumanDecision::WaitForHuman(HUMAN_DECISION_REPAIR_NOT_SUPPORTED),
            }
        }
        _ => AppliedHumanDecision::WaitForHuman(HUMAN_DECISION_INVALID),
    }
}

fn classify_tick_task_binding(
    run_version: i64,
    payload_version: i64,
    active_task_id: Option<&str>,
    current_task_id: &str,
) -> Result<TickTaskBinding, String> {
    match active_task_id {
        Some(task_id) if task_id == current_task_id => {
            let bound_version = payload_version
                .checked_add(1)
                .ok_or_else(|| EXECUTION_FAILED.to_string())?;
            if run_version == bound_version || run_version == payload_version {
                Ok(TickTaskBinding::AlreadyBound)
            } else {
                Err(EXECUTION_CANCELLED.to_string())
            }
        }
        Some(_) => Err(EXECUTION_CANCELLED.to_string()),
        None if run_version == payload_version => Ok(TickTaskBinding::NeedsBinding),
        None => Err(EXECUTION_CANCELLED.to_string()),
    }
}

fn validate_payload(
    payload: &NovelBookAutopilotTaskPayload,
    record: &TaskRecord,
) -> Result<(), String> {
    if payload.run_id.trim().is_empty()
        || payload.project_id != record.project_id
        || payload.user_id != record.user_id
    {
        return Err(EXECUTION_FAILED.to_string());
    }
    if let Some(decision) = payload.decision.as_deref() {
        if !matches!(decision, "accept" | "retry" | "repair" | "stop") {
            return Err(EXECUTION_FAILED.to_string());
        }
    }
    Ok(())
}

fn step_input_digest(
    run: &crate::models::novel_autopilot_run::Model,
    step: &AutopilotStepPlan,
) -> String {
    let stable_input = format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}",
        run.id,
        run.epoch,
        run.guidance_digest.as_deref().unwrap_or_default(),
        step.step_key,
        step.step_type.as_str(),
        step.chapter_id.as_deref().unwrap_or_default(),
        step.chapter_number.unwrap_or_default(),
        step.outline_id.as_deref().unwrap_or_default(),
        step.target_chapter_count.unwrap_or_default(),
    );
    format!("{:x}", md5::compute(stable_input.as_bytes()))
}

fn deferred_step_result_digest(step_id: &str, step: &AutopilotStepPlan) -> String {
    let stable_result = format!(
        "deferred:{}:{}:{}",
        step_id,
        step.step_key,
        step.phase.as_str(),
    );
    format!("{:x}", md5::compute(stable_result.as_bytes()))
}

fn route_decision_view(decision: NovelAutopilotRouteDecision) -> Value {
    match decision {
        NovelAutopilotRouteDecision::Execute(step) => json!({
            "decision": "execute",
            "step_key": step.step_key,
            "step_type": step.step_type.as_str(),
            "phase": step.phase.as_str(),
            "chapter_id": step.chapter_id,
            "chapter_number": step.chapter_number,
            "outline_id": step.outline_id,
            "target_chapter_count": step.target_chapter_count,
        }),
        NovelAutopilotRouteDecision::Complete(phase) => json!({
            "decision": "complete",
            "phase": phase.as_str(),
        }),
        NovelAutopilotRouteDecision::Idle => json!({"decision": "idle"}),
        NovelAutopilotRouteDecision::InvalidFacts(code) => json!({
            "decision": "invalid_facts",
            "code": code,
        }),
    }
}

fn ensure_not_cancelled(token: &CooperativeCancellationToken) -> Result<(), String> {
    if token.is_cancelled() {
        Err(EXECUTION_CANCELLED.to_string())
    } else {
        Ok(())
    }
}

fn map_completion_gate_error(error: NovelAutopilotCompletionGateError) -> String {
    tracing::error!(
        event = "novel_book_autopilot_completion_gate_failed",
        error_code = error.code(),
        "durable novel autopilot final completion gate failed"
    );
    EXECUTION_FAILED.to_string()
}

fn map_facts_error(error: NovelAutopilotFactsError) -> String {
    tracing::error!(
        event = "novel_book_autopilot_facts_load_failed",
        error_code = error.code(),
        "durable novel autopilot could not load its project facts"
    );
    EXECUTION_FAILED.to_string()
}

fn map_foundation_adapter_error(error: FoundationAdapterError) -> String {
    match error {
        FoundationAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        FoundationAdapterError::Repository(error) => map_repository_error(error),
        FoundationAdapterError::ProjectRead => EXECUTION_FAILED.to_string(),
    }
}

fn map_career_adapter_error(error: CareerAdapterError) -> String {
    let error_code = error.code();
    match error {
        CareerAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        CareerAdapterError::Repository(repository_error) => {
            tracing::error!(
                event = "novel_book_autopilot_career_adapter_failed",
                error_code,
                "durable career adapter repository operation failed"
            );
            map_repository_error(repository_error)
        }
    }
}
fn map_character_adapter_error(error: CharacterAdapterError) -> String {
    let error_code = error.code();
    match error {
        CharacterAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        CharacterAdapterError::Repository(repository_error) => {
            tracing::error!(
                event = "novel_book_autopilot_character_adapter_failed",
                error_code,
                "durable character adapter repository operation failed"
            );
            map_repository_error(repository_error)
        }
    }
}

fn map_organization_adapter_error(error: OrganizationAdapterError) -> String {
    let error_code = error.code();
    match error {
        OrganizationAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        OrganizationAdapterError::Repository(repository_error) => {
            tracing::error!(
                event = "novel_book_autopilot_organization_adapter_failed",
                error_code,
                "durable organization adapter repository operation failed"
            );
            map_repository_error(repository_error)
        }
    }
}

fn map_world_adapter_error(error: WorldAdapterError) -> String {
    let error_code = error.code();
    match error {
        WorldAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        WorldAdapterError::Repository(repository_error) => map_repository_error(repository_error),
        WorldAdapterError::ProjectRead => {
            tracing::error!(
                event = "novel_book_autopilot_world_adapter_failed",
                error_code,
                "durable world adapter could not load its project"
            );
            EXECUTION_FAILED.to_string()
        }
    }
}

fn map_chapter_adapter_error(error: ChapterAdapterError) -> String {
    match error {
        ChapterAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        ChapterAdapterError::Repository(error) => map_repository_error(error),
    }
}

fn map_chapter_analysis_adapter_error(error: ChapterAnalysisAdapterError) -> String {
    let error_code = error.code();
    match error {
        ChapterAnalysisAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        ChapterAnalysisAdapterError::Repository(repository_error) => {
            tracing::error!(
                event = "novel_book_autopilot_chapter_analysis_adapter_failed",
                error_code,
                "durable chapter analysis adapter repository operation failed"
            );
            map_repository_error(repository_error)
        }
    }
}

fn map_chapter_repair_adapter_error(error: ChapterRepairAdapterError) -> String {
    let error_code = error.code();
    match error {
        ChapterRepairAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        ChapterRepairAdapterError::Repository(repository_error) => {
            tracing::error!(
                event = "novel_book_autopilot_chapter_repair_adapter_failed",
                error_code,
                "durable chapter repair adapter repository operation failed"
            );
            map_repository_error(repository_error)
        }
    }
}

fn map_book_polish_adapter_error(error: BookPolishAdapterError) -> String {
    let error_code = error.code();
    match error {
        BookPolishAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        BookPolishAdapterError::Repository(repository_error) => {
            tracing::error!(
                event = "novel_book_autopilot_book_polish_adapter_failed",
                error_code,
                "durable book polish adapter repository operation failed"
            );
            map_repository_error(repository_error)
        }
    }
}

fn map_export_adapter_error(error: ExportAdapterError) -> String {
    let error_code = error.code();
    match error {
        ExportAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        ExportAdapterError::Repository(error) => map_repository_error(error),
        ExportAdapterError::Service(_) => {
            tracing::error!(
                event = "novel_book_autopilot_export_failed",
                error_code,
                "durable project export execution failed"
            );
            EXECUTION_FAILED.to_string()
        }
    }
}

fn map_book_review_adapter_error(error: BookReviewAdapterError) -> String {
    let error_code = error.code();
    match error {
        BookReviewAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        BookReviewAdapterError::Repository(error) => map_repository_error(error),
        BookReviewAdapterError::Service(_) => {
            tracing::error!(
                event = "novel_book_autopilot_book_review_failed",
                error_code,
                "durable book review execution failed"
            );
            EXECUTION_FAILED.to_string()
        }
    }
}

fn map_outline_expansion_adapter_error(error: OutlineExpansionAdapterError) -> String {
    let error_code = error.code();
    match error {
        OutlineExpansionAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        OutlineExpansionAdapterError::Repository(repository_error) => {
            tracing::error!(
                event = "novel_book_autopilot_outline_expansion_adapter_failed",
                error_code,
                "durable outline expansion adapter repository operation failed"
            );
            map_repository_error(repository_error)
        }
    }
}

fn map_outline_adapter_error(error: OutlineAdapterError) -> String {
    let error_code = error.code();
    match error {
        OutlineAdapterError::Cancelled => EXECUTION_CANCELLED.to_string(),
        OutlineAdapterError::ProjectRead => EXECUTION_FAILED.to_string(),
        OutlineAdapterError::Repository(repository_error) => {
            tracing::error!(
                event = "novel_book_autopilot_outline_adapter_failed",
                error_code,
                "durable outline adapter repository operation failed"
            );
            map_repository_error(repository_error)
        }
    }
}

fn map_repository_error(error: NovelAutopilotRepositoryError) -> String {
    tracing::error!(
        event = "novel_book_autopilot_tick_repository_failed",
        error_code = error.code(),
        "durable novel autopilot tick could not update its run"
    );
    EXECUTION_FAILED.to_string()
}

#[cfg(test)]
mod tests {
    use super::{route_decision_view, NovelAutopilotRouteDecision};
    use crate::services::novel_autopilot::{
        router::AutopilotStepPlan,
        types::{NovelAutopilotPhase, NovelAutopilotStepType},
    };

    #[test]
    fn route_decision_view_exposes_only_safe_step_metadata() {
        let value = route_decision_view(NovelAutopilotRouteDecision::Execute(AutopilotStepPlan {
            step_key: "planning:foundation".to_string(),
            step_type: NovelAutopilotStepType::Foundation,
            phase: NovelAutopilotPhase::Foundation,
            chapter_id: None,
            chapter_number: None,
            outline_id: None,
            target_chapter_count: None,
        }));

        assert_eq!(value["decision"], "execute");
        assert_eq!(value["step_type"], "foundation");
        assert!(value.get("prompt").is_none());
        assert!(value.get("reasoning").is_none());
        assert!(value.get("provider_response").is_none());
    }
}

#[cfg(test)]
mod human_decision_tests {
    use chrono::Utc;

    use super::{
        apply_human_decision, AppliedHumanDecision, NovelAutopilotRouteDecision,
        HUMAN_DECISION_CANDIDATE_UNAVAILABLE, HUMAN_DECISION_INVALID,
        HUMAN_DECISION_REPAIR_NOT_SUPPORTED, HUMAN_DECISION_RETRY_ROUTE_MISMATCH,
    };
    use crate::{
        models::novel_autopilot_step_run,
        services::novel_autopilot::{
            router::AutopilotStepPlan,
            types::{
                NovelAutopilotPhase, NovelAutopilotQualityDecision, NovelAutopilotStepStatus,
                NovelAutopilotStepType,
            },
        },
    };

    fn chapter_plan(
        chapter_number: u32,
        chapter_id: &str,
        action: &'static str,
        step_type: NovelAutopilotStepType,
    ) -> AutopilotStepPlan {
        AutopilotStepPlan {
            step_key: format!("chapter:{chapter_number:04}:{action}"),
            step_type,
            phase: NovelAutopilotPhase::ChapterLoop,
            chapter_id: Some(chapter_id.to_string()),
            chapter_number: Some(chapter_number),
            outline_id: None,
            target_chapter_count: None,
        }
    }

    fn latest_step(
        step_key: &str,
        step_type: NovelAutopilotStepType,
        error_code: Option<&str>,
    ) -> novel_autopilot_step_run::Model {
        let now = Utc::now().naive_utc();
        novel_autopilot_step_run::Model {
            id: "step-1".to_string(),
            run_id: "run-1".to_string(),
            step_key: step_key.to_string(),
            step_type: step_type.as_str().to_string(),
            phase: NovelAutopilotPhase::ChapterLoop.as_str().to_string(),
            chapter_id: Some("chapter-1".to_string()),
            chapter_number: Some(1),
            attempt: 1,
            run_epoch: 1,
            status: NovelAutopilotStepStatus::Completed.as_str().to_string(),
            background_task_id: None,
            input_digest: "input-digest".to_string(),
            result_digest: Some("result-digest".to_string()),
            quality_decision: Some(
                NovelAutopilotQualityDecision::ManualReview
                    .as_str()
                    .to_string(),
            ),
            error_code: error_code.map(str::to_string),
            started_at: Some(now),
            completed_at: Some(now),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn accept_periodic_gate_uses_current_facts_route() {
        let routed = NovelAutopilotRouteDecision::Execute(chapter_plan(
            2,
            "chapter-2",
            "generate",
            NovelAutopilotStepType::ChapterGenerate,
        ));
        let latest = latest_step(
            "chapter:0001:generate",
            NovelAutopilotStepType::ChapterGenerate,
            None,
        );

        assert_eq!(
            apply_human_decision(Some("accept"), routed.clone(), Some(&latest)),
            AppliedHumanDecision::Route(routed)
        );
    }

    #[test]
    fn accept_without_persisted_candidate_stays_waiting_human() {
        for error_code in [
            "chapter_quality_manual_review",
            "chapter_repair_manual_review",
        ] {
            let latest = latest_step(
                "chapter:0001:generate",
                NovelAutopilotStepType::ChapterGenerate,
                Some(error_code),
            );
            assert_eq!(
                apply_human_decision(
                    Some("accept"),
                    NovelAutopilotRouteDecision::Execute(chapter_plan(
                        1,
                        "chapter-1",
                        "generate",
                        NovelAutopilotStepType::ChapterGenerate,
                    )),
                    Some(&latest),
                ),
                AppliedHumanDecision::WaitForHuman(HUMAN_DECISION_CANDIDATE_UNAVAILABLE)
            );
        }
    }

    #[test]
    fn retry_requires_the_facts_router_to_select_the_same_step() {
        let latest = latest_step(
            "chapter:0001:generate",
            NovelAutopilotStepType::ChapterGenerate,
            Some("chapter_generation_failed"),
        );
        let same = NovelAutopilotRouteDecision::Execute(chapter_plan(
            1,
            "chapter-1",
            "generate",
            NovelAutopilotStepType::ChapterGenerate,
        ));
        assert_eq!(
            apply_human_decision(Some("retry"), same.clone(), Some(&latest)),
            AppliedHumanDecision::Route(same)
        );

        let different = NovelAutopilotRouteDecision::Execute(chapter_plan(
            1,
            "chapter-1",
            "repair",
            NovelAutopilotStepType::ChapterRepair,
        ));
        assert_eq!(
            apply_human_decision(Some("retry"), different, Some(&latest)),
            AppliedHumanDecision::WaitForHuman(HUMAN_DECISION_RETRY_ROUTE_MISMATCH)
        );
    }

    #[test]
    fn repair_routes_analyzed_or_repaired_chapter_to_chapter_repair() {
        for step_type in [
            NovelAutopilotStepType::ChapterAnalyze,
            NovelAutopilotStepType::ChapterRepair,
        ] {
            let latest = latest_step("chapter:0001:analyze", step_type, None);
            let applied = apply_human_decision(
                Some("repair"),
                NovelAutopilotRouteDecision::Idle,
                Some(&latest),
            );
            let AppliedHumanDecision::Route(NovelAutopilotRouteDecision::Execute(step)) = applied
            else {
                panic!("repair must produce an executable chapter repair plan");
            };
            assert_eq!(step.step_key, "chapter:0001:repair");
            assert_eq!(step.step_type, NovelAutopilotStepType::ChapterRepair);
            assert_eq!(step.chapter_id.as_deref(), Some("chapter-1"));
        }
    }

    #[test]
    fn repair_rejects_generation_candidate_and_missing_decision_context() {
        let latest = latest_step(
            "chapter:0001:generate",
            NovelAutopilotStepType::ChapterGenerate,
            Some("chapter_quality_manual_review"),
        );
        assert_eq!(
            apply_human_decision(
                Some("repair"),
                NovelAutopilotRouteDecision::Idle,
                Some(&latest),
            ),
            AppliedHumanDecision::WaitForHuman(HUMAN_DECISION_REPAIR_NOT_SUPPORTED)
        );
        assert_eq!(
            apply_human_decision(Some("accept"), NovelAutopilotRouteDecision::Idle, None,),
            AppliedHumanDecision::WaitForHuman(HUMAN_DECISION_INVALID)
        );
    }
}

#[cfg(test)]
mod tick_binding_tests {
    use super::{
        classify_tick_task_binding, TickTaskBinding, EXECUTION_CANCELLED, EXECUTION_FAILED,
    };

    #[test]
    fn unbound_tick_requires_exact_lease_version() {
        assert_eq!(
            classify_tick_task_binding(7, 7, None, "task-1"),
            Ok(TickTaskBinding::NeedsBinding)
        );
        assert_eq!(
            classify_tick_task_binding(8, 7, None, "task-1"),
            Err(EXECUTION_CANCELLED.to_string())
        );
    }

    #[test]
    fn bound_tick_accepts_pre_bind_and_post_bind_versions() {
        assert_eq!(
            classify_tick_task_binding(7, 7, Some("task-1"), "task-1"),
            Ok(TickTaskBinding::AlreadyBound)
        );
        assert_eq!(
            classify_tick_task_binding(8, 7, Some("task-1"), "task-1"),
            Ok(TickTaskBinding::AlreadyBound)
        );
    }

    #[test]
    fn bound_tick_rejects_stale_or_different_task() {
        assert_eq!(
            classify_tick_task_binding(9, 7, Some("task-1"), "task-1"),
            Err(EXECUTION_CANCELLED.to_string())
        );
        assert_eq!(
            classify_tick_task_binding(8, 7, Some("task-2"), "task-1"),
            Err(EXECUTION_CANCELLED.to_string())
        );
    }

    #[test]
    fn bound_tick_rejects_version_overflow() {
        assert_eq!(
            classify_tick_task_binding(i64::MAX, i64::MAX, Some("task-1"), "task-1"),
            Err(EXECUTION_FAILED.to_string())
        );
    }
}
