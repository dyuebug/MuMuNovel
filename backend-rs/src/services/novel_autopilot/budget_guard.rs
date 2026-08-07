use chrono::NaiveDateTime;

use crate::models::novel_autopilot_run;

use super::{
    router::AutopilotStepPlan,
    types::{NovelAutopilotRunConfig, NovelAutopilotStepType},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NovelAutopilotBudgetViolation {
    ChapterLimit,
    TokenLimit,
    EstimatedCostLimit,
    RuntimeLimit,
    StepAttemptLimit,
    ProviderFailureLimit,
    QualityFailureLimit,
    CostEstimationUnavailable,
}

impl NovelAutopilotBudgetViolation {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::ChapterLimit => "novel_autopilot_budget_chapters_exhausted",
            Self::TokenLimit => "novel_autopilot_budget_tokens_exhausted",
            Self::EstimatedCostLimit => "novel_autopilot_budget_cost_exhausted",
            Self::RuntimeLimit => "novel_autopilot_budget_runtime_exhausted",
            Self::StepAttemptLimit => "novel_autopilot_step_attempts_exhausted",
            Self::ProviderFailureLimit => "novel_autopilot_provider_failures_exhausted",
            Self::QualityFailureLimit => "novel_autopilot_quality_failures_exhausted",
            Self::CostEstimationUnavailable => "novel_autopilot_cost_estimation_unavailable",
        }
    }
}

pub(crate) fn evaluate_preflight(
    run: &novel_autopilot_run::Model,
    config: &NovelAutopilotRunConfig,
    step: &AutopilotStepPlan,
    latest_attempt: Option<i32>,
    now: NaiveDateTime,
) -> Option<NovelAutopilotBudgetViolation> {
    if runtime_exhausted(run.started_at, config.max_runtime_seconds, now) {
        return Some(NovelAutopilotBudgetViolation::RuntimeLimit);
    }

    if run.total_chapters > i32_from_u32(config.max_chapters)
        || step
            .chapter_number
            .is_some_and(|chapter| chapter > config.max_chapters)
    {
        return Some(NovelAutopilotBudgetViolation::ChapterLimit);
    }

    if latest_attempt.is_some_and(|attempt| attempt >= i32_from_u32(config.max_step_attempts)) {
        return Some(NovelAutopilotBudgetViolation::StepAttemptLimit);
    }

    if !step_consumes_model(step.step_type) {
        return None;
    }

    if run.used_tokens >= i64_from_u64(config.max_tokens) {
        return Some(NovelAutopilotBudgetViolation::TokenLimit);
    }
    if run.consecutive_provider_failures >= i32_from_u32(config.max_consecutive_provider_failures) {
        return Some(NovelAutopilotBudgetViolation::ProviderFailureLimit);
    }
    if run.consecutive_quality_failures >= i32_from_u32(config.max_consecutive_quality_failures) {
        return Some(NovelAutopilotBudgetViolation::QualityFailureLimit);
    }
    if let Some(limit) = config.max_estimated_cost {
        if run.estimated_cost >= limit {
            return Some(NovelAutopilotBudgetViolation::EstimatedCostLimit);
        }
        // The current provider abstraction exposes neither authoritative token usage nor a
        // provider/model pricing table. Refuse to pretend that zero is a reliable cost estimate.
        return Some(NovelAutopilotBudgetViolation::CostEstimationUnavailable);
    }

    None
}

pub(crate) fn evaluate_postflight(
    run: &novel_autopilot_run::Model,
    config: &NovelAutopilotRunConfig,
    now: NaiveDateTime,
) -> Option<NovelAutopilotBudgetViolation> {
    if runtime_exhausted(run.started_at, config.max_runtime_seconds, now) {
        return Some(NovelAutopilotBudgetViolation::RuntimeLimit);
    }
    if run.total_chapters > i32_from_u32(config.max_chapters) {
        return Some(NovelAutopilotBudgetViolation::ChapterLimit);
    }
    if run.used_tokens >= i64_from_u64(config.max_tokens) {
        return Some(NovelAutopilotBudgetViolation::TokenLimit);
    }
    if run.consecutive_provider_failures >= i32_from_u32(config.max_consecutive_provider_failures) {
        return Some(NovelAutopilotBudgetViolation::ProviderFailureLimit);
    }
    if run.consecutive_quality_failures >= i32_from_u32(config.max_consecutive_quality_failures) {
        return Some(NovelAutopilotBudgetViolation::QualityFailureLimit);
    }
    if config
        .max_estimated_cost
        .is_some_and(|limit| run.estimated_cost >= limit)
    {
        return Some(NovelAutopilotBudgetViolation::EstimatedCostLimit);
    }
    None
}

fn runtime_exhausted(
    started_at: Option<NaiveDateTime>,
    max_runtime_seconds: u64,
    now: NaiveDateTime,
) -> bool {
    let Some(started_at) = started_at else {
        return false;
    };
    now.signed_duration_since(started_at).num_seconds() >= i64_from_u64(max_runtime_seconds)
}

fn step_consumes_model(step_type: NovelAutopilotStepType) -> bool {
    !matches!(
        step_type,
        NovelAutopilotStepType::Validate
            | NovelAutopilotStepType::BookReview
            | NovelAutopilotStepType::Export
    )
}

const fn i32_from_u32(value: u32) -> i32 {
    if value > i32::MAX as u32 {
        i32::MAX
    } else {
        value as i32
    }
}

const fn i64_from_u64(value: u64) -> i64 {
    if value > i64::MAX as u64 {
        i64::MAX
    } else {
        value as i64
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, NaiveDate};
    use serde_json::json;

    use super::*;
    use crate::services::novel_autopilot::{
        router::AutopilotStepPlan,
        types::{NovelAutopilotPhase, NovelAutopilotRunConfig},
    };

    fn now() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 7, 19)
            .expect("valid date")
            .and_hms_opt(12, 0, 0)
            .expect("valid time")
    }

    fn run() -> novel_autopilot_run::Model {
        novel_autopilot_run::Model {
            id: "run-1".to_string(),
            project_id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            schema_version: "v1".to_string(),
            status: "running".to_string(),
            current_phase: "chapter_loop".to_string(),
            current_step: None,
            active_scope_key: Some("project-1".to_string()),
            current_chapter_id: None,
            current_chapter_number: None,
            total_chapters: 3,
            completed_chapters: 0,
            failed_chapters: json!([]),
            pending_rewrites: json!([]),
            total_word_count: 0,
            execution_scope: "complete_book".to_string(),
            human_gate_mode: "fully_automatic".to_string(),
            gate_interval: Some(1),
            config_snapshot: json!({}),
            max_chapters: Some(3),
            max_tokens: Some(1000),
            max_estimated_cost: None,
            max_runtime_seconds: Some(3600),
            used_tokens: 0,
            estimated_cost: 0.0,
            epoch: 0,
            version: 0,
            consecutive_provider_failures: 0,
            consecutive_quality_failures: 0,
            last_error_code: None,
            next_attempt_at: None,
            guidance_digest: None,
            active_background_task_id: Some("task-1".to_string()),
            final_export_ref: None,
            created_at: now(),
            updated_at: now(),
            started_at: Some(now()),
            paused_at: None,
            completed_at: None,
        }
    }

    fn chapter_step() -> AutopilotStepPlan {
        AutopilotStepPlan {
            step_key: "chapter:1:generate".to_string(),
            step_type: NovelAutopilotStepType::ChapterGenerate,
            phase: NovelAutopilotPhase::ChapterLoop,
            chapter_id: Some("chapter-1".to_string()),
            chapter_number: Some(1),
            outline_id: None,
            target_chapter_count: None,
        }
    }

    #[test]
    fn preflight_rejects_before_next_step_attempt_is_claimed() {
        let config = NovelAutopilotRunConfig {
            max_step_attempts: 2,
            ..NovelAutopilotRunConfig::default()
        };
        assert_eq!(
            evaluate_preflight(&run(), &config, &chapter_step(), Some(2), now()),
            Some(NovelAutopilotBudgetViolation::StepAttemptLimit)
        );
    }

    #[test]
    fn configured_cost_budget_fails_closed_without_pricing_source() {
        let config = NovelAutopilotRunConfig {
            max_estimated_cost: Some(1.0),
            ..NovelAutopilotRunConfig::default()
        };
        assert_eq!(
            evaluate_preflight(&run(), &config, &chapter_step(), None, now()),
            Some(NovelAutopilotBudgetViolation::CostEstimationUnavailable)
        );
    }

    #[test]
    fn deterministic_export_can_finish_after_token_budget_is_reached() {
        let mut run = run();
        run.used_tokens = 1000;
        let config = NovelAutopilotRunConfig {
            max_tokens: 1000,
            ..NovelAutopilotRunConfig::default()
        };
        let step = AutopilotStepPlan {
            step_key: "completion:export".to_string(),
            step_type: NovelAutopilotStepType::Export,
            phase: NovelAutopilotPhase::Export,
            chapter_id: None,
            chapter_number: None,
            outline_id: None,
            target_chapter_count: None,
        };
        assert_eq!(evaluate_preflight(&run, &config, &step, None, now()), None);
    }

    #[test]
    fn runtime_limit_is_checked_for_every_step_type() {
        let mut run = run();
        run.started_at = Some(now() - Duration::seconds(60));
        let config = NovelAutopilotRunConfig {
            max_runtime_seconds: 60,
            ..NovelAutopilotRunConfig::default()
        };
        assert_eq!(
            evaluate_preflight(&run, &config, &chapter_step(), None, now()),
            Some(NovelAutopilotBudgetViolation::RuntimeLimit)
        );
    }
}
