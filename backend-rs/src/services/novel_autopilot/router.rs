use super::types::{
    NovelAutopilotExecutionScope, NovelAutopilotPhase, NovelAutopilotRunConfig,
    NovelAutopilotRunStatus, NovelAutopilotStepType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NovelAutopilotBusinessFacts {
    pub foundation_ready: bool,
    pub world_ready: bool,
    pub careers_ready: bool,
    pub characters_ready: bool,
    pub organizations_ready: bool,
    pub outline_ready: bool,
    pub outline_mode: String,
    pub current_chapter_count: u32,
    pub next_unexpanded_outline_id: Option<String>,
    pub next_unexpanded_outline_order: Option<u32>,
    pub remaining_unexpanded_outline_count: u32,
    pub next_incomplete_chapter_id: Option<String>,
    pub next_incomplete_chapter_number: Option<u32>,
    pub target_chapter_count: u32,
    pub completed_chapter_count: u32,
    pub chapters_completed_in_run: u32,
    pub pending_analysis_chapter_id: Option<String>,
    pub pending_analysis_chapter_number: Option<u32>,
    pub pending_repair_chapter_id: Option<String>,
    pub pending_repair_chapter_number: Option<u32>,
    pub pending_polish_chapter_id: Option<String>,
    pub pending_polish_chapter_number: Option<u32>,
    pub book_review_completed: bool,
    pub book_polish_completed: bool,
    pub export_completed: bool,
}

impl Default for NovelAutopilotBusinessFacts {
    fn default() -> Self {
        Self {
            foundation_ready: false,
            world_ready: false,
            careers_ready: false,
            characters_ready: false,
            organizations_ready: false,
            outline_ready: false,
            outline_mode: String::new(),
            current_chapter_count: 0,
            next_unexpanded_outline_id: None,
            next_unexpanded_outline_order: None,
            remaining_unexpanded_outline_count: 0,
            next_incomplete_chapter_id: None,
            next_incomplete_chapter_number: None,
            target_chapter_count: 0,
            completed_chapter_count: 0,
            chapters_completed_in_run: 0,
            pending_analysis_chapter_id: None,
            pending_analysis_chapter_number: None,
            pending_repair_chapter_id: None,
            pending_repair_chapter_number: None,
            pending_polish_chapter_id: None,
            pending_polish_chapter_number: None,
            book_review_completed: false,
            book_polish_completed: false,
            export_completed: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NovelAutopilotRouteSnapshot {
    pub status: NovelAutopilotRunStatus,
    pub config: NovelAutopilotRunConfig,
    pub facts: NovelAutopilotBusinessFacts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutopilotStepPlan {
    pub step_key: String,
    pub step_type: NovelAutopilotStepType,
    pub phase: NovelAutopilotPhase,
    pub chapter_id: Option<String>,
    pub chapter_number: Option<u32>,
    pub outline_id: Option<String>,
    pub target_chapter_count: Option<u32>,
}

impl AutopilotStepPlan {
    fn planning(
        step_key: &'static str,
        step_type: NovelAutopilotStepType,
        phase: NovelAutopilotPhase,
    ) -> Self {
        Self {
            step_key: step_key.to_string(),
            step_type,
            phase,
            chapter_id: None,
            chapter_number: None,
            outline_id: None,
            target_chapter_count: None,
        }
    }

    fn outline_expand(outline_order: u32, outline_id: String, target_chapter_count: u32) -> Self {
        Self {
            step_key: format!("planning:outline_expand:{outline_order:04}:{outline_id}"),
            step_type: NovelAutopilotStepType::OutlineExpand,
            phase: NovelAutopilotPhase::Outline,
            chapter_id: None,
            chapter_number: None,
            outline_id: Some(outline_id),
            target_chapter_count: Some(target_chapter_count),
        }
    }

    pub(crate) fn chapter(
        chapter_number: u32,
        chapter_id: String,
        action: &'static str,
        step_type: NovelAutopilotStepType,
    ) -> Self {
        Self {
            step_key: format!("chapter:{chapter_number:04}:{action}"),
            step_type,
            phase: NovelAutopilotPhase::ChapterLoop,
            chapter_id: Some(chapter_id),
            chapter_number: Some(chapter_number),
            outline_id: None,
            target_chapter_count: None,
        }
    }

    fn completion(
        step_key: &'static str,
        step_type: NovelAutopilotStepType,
        phase: NovelAutopilotPhase,
    ) -> Self {
        Self::planning(step_key, step_type, phase)
    }

    fn completion_chapter(
        chapter_number: u32,
        chapter_id: String,
        action: &'static str,
        step_type: NovelAutopilotStepType,
        phase: NovelAutopilotPhase,
    ) -> Self {
        Self {
            step_key: format!("completion:{action}:chapter:{chapter_number:04}:{chapter_id}"),
            step_type,
            phase,
            chapter_id: Some(chapter_id),
            chapter_number: Some(chapter_number),
            outline_id: None,
            target_chapter_count: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NovelAutopilotRouteDecision {
    Execute(AutopilotStepPlan),
    Complete(NovelAutopilotPhase),
    Idle,
    InvalidFacts(&'static str),
}

pub(crate) fn route_next_step(
    snapshot: &NovelAutopilotRouteSnapshot,
) -> NovelAutopilotRouteDecision {
    if !snapshot.status.can_schedule() {
        return NovelAutopilotRouteDecision::Idle;
    }

    let facts = &snapshot.facts;
    if !facts.foundation_ready {
        return NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::planning(
            "planning:foundation",
            NovelAutopilotStepType::Foundation,
            NovelAutopilotPhase::Foundation,
        ));
    }
    if !facts.world_ready {
        return NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::planning(
            "planning:world_building",
            NovelAutopilotStepType::WorldBuilding,
            NovelAutopilotPhase::WorldBuilding,
        ));
    }
    if !facts.careers_ready {
        return NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::planning(
            "planning:career_design",
            NovelAutopilotStepType::CareerDesign,
            NovelAutopilotPhase::CareerDesign,
        ));
    }
    if !facts.characters_ready {
        return NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::planning(
            "planning:character_design",
            NovelAutopilotStepType::CharacterDesign,
            NovelAutopilotPhase::CharacterDesign,
        ));
    }
    if !facts.organizations_ready {
        return NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::planning(
            "planning:organization_design",
            NovelAutopilotStepType::OrganizationDesign,
            NovelAutopilotPhase::OrganizationDesign,
        ));
    }
    if !facts.outline_ready {
        return NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::planning(
            "planning:outline",
            NovelAutopilotStepType::Outline,
            NovelAutopilotPhase::Outline,
        ));
    }

    if let (Some(chapter_id), Some(chapter_number)) = (
        facts.pending_repair_chapter_id.clone(),
        facts.pending_repair_chapter_number,
    ) {
        return NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::chapter(
            chapter_number,
            chapter_id,
            "repair",
            NovelAutopilotStepType::ChapterRepair,
        ));
    }

    if let (Some(chapter_id), Some(chapter_number)) = (
        facts.pending_analysis_chapter_id.clone(),
        facts.pending_analysis_chapter_number,
    ) {
        return NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::chapter(
            chapter_number,
            chapter_id,
            "analyze",
            NovelAutopilotStepType::ChapterAnalyze,
        ));
    }

    if matches!(
        snapshot.config.execution_scope,
        NovelAutopilotExecutionScope::NextNChapters
    ) && facts.chapters_completed_in_run >= snapshot.config.next_chapter_count.unwrap_or(0)
    {
        return NovelAutopilotRouteDecision::Complete(NovelAutopilotPhase::Completed);
    }

    if facts.current_chapter_count > facts.target_chapter_count {
        return NovelAutopilotRouteDecision::InvalidFacts("chapter_count_exceeds_target");
    }

    if facts.outline_mode == "one-to-many"
        && facts.current_chapter_count < facts.target_chapter_count
    {
        if facts.remaining_unexpanded_outline_count == 0 {
            return NovelAutopilotRouteDecision::InvalidFacts("missing_expandable_outline");
        }
        let remaining_chapter_slots = facts
            .target_chapter_count
            .saturating_sub(facts.current_chapter_count);
        if remaining_chapter_slots < facts.remaining_unexpanded_outline_count {
            return NovelAutopilotRouteDecision::InvalidFacts("outline_expansion_target_too_small");
        }
        return match (
            facts.next_unexpanded_outline_id.clone(),
            facts.next_unexpanded_outline_order,
        ) {
            (Some(outline_id), Some(outline_order)) => {
                let target_chapter_count =
                    remaining_chapter_slots.div_ceil(facts.remaining_unexpanded_outline_count);
                NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::outline_expand(
                    outline_order,
                    outline_id,
                    target_chapter_count,
                ))
            }
            _ => NovelAutopilotRouteDecision::InvalidFacts("missing_expandable_outline"),
        };
    }

    if matches!(
        snapshot.config.execution_scope,
        NovelAutopilotExecutionScope::PlanningOnly
    ) {
        return NovelAutopilotRouteDecision::Complete(NovelAutopilotPhase::Completed);
    }

    if facts.completed_chapter_count > facts.target_chapter_count {
        return NovelAutopilotRouteDecision::InvalidFacts("completed_chapter_count_exceeds_target");
    }

    if facts.completed_chapter_count < facts.target_chapter_count {
        return match (
            facts.next_incomplete_chapter_id.clone(),
            facts.next_incomplete_chapter_number,
        ) {
            (Some(chapter_id), Some(chapter_number)) => {
                NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::chapter(
                    chapter_number,
                    chapter_id,
                    "generate",
                    NovelAutopilotStepType::ChapterGenerate,
                ))
            }
            _ => NovelAutopilotRouteDecision::InvalidFacts("missing_next_incomplete_chapter"),
        };
    }

    if !matches!(
        snapshot.config.execution_scope,
        NovelAutopilotExecutionScope::CompleteBook
    ) {
        return NovelAutopilotRouteDecision::Complete(NovelAutopilotPhase::Completed);
    }

    if snapshot.config.run_book_review && !facts.book_review_completed {
        return NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::completion(
            "completion:book_review",
            NovelAutopilotStepType::BookReview,
            NovelAutopilotPhase::BookReview,
        ));
    }
    if snapshot.config.run_book_polish && !facts.book_polish_completed {
        let (Some(chapter_id), Some(chapter_number)) = (
            facts.pending_polish_chapter_id.clone(),
            facts.pending_polish_chapter_number,
        ) else {
            return NovelAutopilotRouteDecision::InvalidFacts("book_polish_rewrite_missing");
        };
        return NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::completion_chapter(
            chapter_number,
            chapter_id,
            "book_polish",
            NovelAutopilotStepType::BookPolish,
            NovelAutopilotPhase::BookPolish,
        ));
    }
    if !facts.export_completed {
        return NovelAutopilotRouteDecision::Execute(AutopilotStepPlan::completion(
            "completion:export",
            NovelAutopilotStepType::Export,
            NovelAutopilotPhase::Export,
        ));
    }

    NovelAutopilotRouteDecision::Complete(NovelAutopilotPhase::Completed)
}
