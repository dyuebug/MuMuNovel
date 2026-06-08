// Staged Rust owner for candidate quality hook assembly in Python
// chapter_generation/stream/candidate_service.py and batch_generation_candidate_service.py.
// It owns the adapter contract while the heavy quality rules remain injectable.
#![allow(dead_code)]

use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateQualityAdapterContext {
    pub(crate) story_packet: Value,
    pub(crate) project: Value,
    pub(crate) chapter: Value,
    pub(crate) chapter_context: Value,
    pub(crate) target_word_count: i64,
    pub(crate) generation_intent: Value,
    pub(crate) retry_count: i64,
    pub(crate) max_retries: i64,
    pub(crate) current_story_repair_payload: Option<Value>,
    pub(crate) scope: String,
    pub(crate) log_prefix: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CandidateQualityRuntimeContextBuildInput {
    pub(crate) story_packet: Value,
    pub(crate) project: Value,
    pub(crate) chapter: Value,
    pub(crate) chapter_context: Value,
    pub(crate) target_word_count: i64,
    pub(crate) generation_intent: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CandidateStoryQualityMetricsInput {
    pub(crate) content: String,
    pub(crate) chapter_outline: Value,
    pub(crate) world_rules: Value,
    pub(crate) quality_runtime_context: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CandidateQualityGatePlanInput {
    pub(crate) candidate_metrics: Option<Value>,
    pub(crate) retry_count: i64,
    pub(crate) max_retries: i64,
    pub(crate) current_story_repair_payload: Option<Value>,
    pub(crate) scope: String,
}

pub(crate) struct ChapterCandidateQualityAdapter<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
> {
    context: ChapterCandidateQualityAdapterContext,
    build_quality_runtime_context_fn: BuildQualityRuntimeContext,
    compute_story_quality_metrics_fn: ComputeStoryQualityMetrics,
    resolve_quality_gate_execution_plan_fn: ResolveQualityGatePlan,
}

pub(crate) fn build_chapter_candidate_quality_adapter<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
>(
    context: ChapterCandidateQualityAdapterContext,
    build_quality_runtime_context_fn: BuildQualityRuntimeContext,
    compute_story_quality_metrics_fn: ComputeStoryQualityMetrics,
    resolve_quality_gate_execution_plan_fn: ResolveQualityGatePlan,
) -> ChapterCandidateQualityAdapter<
    BuildQualityRuntimeContext,
    ComputeStoryQualityMetrics,
    ResolveQualityGatePlan,
>
where
    BuildQualityRuntimeContext: FnMut(CandidateQualityRuntimeContextBuildInput) -> Value,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value,
{
    ChapterCandidateQualityAdapter {
        context,
        build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn,
    }
}

impl<BuildQualityRuntimeContext, ComputeStoryQualityMetrics, ResolveQualityGatePlan>
    ChapterCandidateQualityAdapter<
        BuildQualityRuntimeContext,
        ComputeStoryQualityMetrics,
        ResolveQualityGatePlan,
    >
where
    BuildQualityRuntimeContext: FnMut(CandidateQualityRuntimeContextBuildInput) -> Value,
    ComputeStoryQualityMetrics: FnMut(CandidateStoryQualityMetricsInput) -> Value,
    ResolveQualityGatePlan: FnMut(CandidateQualityGatePlanInput) -> Value,
{
    pub(crate) fn evaluate_quality(&mut self, generated_content: &str) -> Value {
        let quality_runtime_context =
            (self.build_quality_runtime_context_fn)(CandidateQualityRuntimeContextBuildInput {
                story_packet: self.context.story_packet.clone(),
                project: self.context.project.clone(),
                chapter: self.context.chapter.clone(),
                chapter_context: self.context.chapter_context.clone(),
                target_word_count: self.context.target_word_count,
                generation_intent: self.context.generation_intent.clone(),
            });

        (self.compute_story_quality_metrics_fn)(CandidateStoryQualityMetricsInput {
            content: generated_content.to_string(),
            chapter_outline: object_field(&self.context.chapter_context, "chapter_outline"),
            world_rules: object_field(&self.context.project, "world_rules"),
            quality_runtime_context,
        })
    }

    pub(crate) fn build_quality_gate_plan(
        &mut self,
        candidate_metrics: Value,
        _attempt_offset: i64,
    ) -> Value {
        let candidate_metrics = candidate_metrics.is_object().then_some(candidate_metrics);
        (self.resolve_quality_gate_execution_plan_fn)(CandidateQualityGatePlanInput {
            candidate_metrics,
            retry_count: self.context.retry_count,
            max_retries: self.context.max_retries,
            current_story_repair_payload: self.context.current_story_repair_payload.clone(),
            scope: self.context.scope.clone(),
        })
    }

    pub(crate) fn log_prefix(&self) -> &str {
        &self.context.log_prefix
    }
}

fn object_field(value: &Value, key: &str) -> Value {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .cloned()
        .unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        build_chapter_candidate_quality_adapter, CandidateQualityGatePlanInput,
        CandidateQualityRuntimeContextBuildInput, CandidateStoryQualityMetricsInput,
        ChapterCandidateQualityAdapterContext,
    };

    #[test]
    fn should_evaluate_quality_with_runtime_context_outline_and_world_rules() {
        let context = adapter_context("chapter");
        let mut runtime_context_inputs = Vec::<CandidateQualityRuntimeContextBuildInput>::new();
        let mut metrics_inputs = Vec::<CandidateStoryQualityMetricsInput>::new();
        let mut adapter = build_chapter_candidate_quality_adapter(
            context,
            |input| {
                runtime_context_inputs.push(input);
                json!({"quality_runtime": "built"})
            },
            |input| {
                metrics_inputs.push(input);
                json!({
                    "overall_score": 86.0,
                    "quality_gate": {"decision": "allow_save"}
                })
            },
            |_input| json!({"quality_gate": {"decision": "allow_save"}}),
        );

        let metrics = adapter.evaluate_quality("generated chapter text");
        drop(adapter);

        assert_eq!(metrics["overall_score"], 86.0);
        assert_eq!(runtime_context_inputs.len(), 1);
        assert_eq!(runtime_context_inputs[0].target_word_count, 1200);
        assert_eq!(runtime_context_inputs[0].generation_intent["mode"], "draft");
        assert_eq!(metrics_inputs.len(), 1);
        assert_eq!(metrics_inputs[0].content, "generated chapter text");
        assert_eq!(metrics_inputs[0].chapter_outline, json!("outline A"));
        assert_eq!(metrics_inputs[0].world_rules, json!("rules A"));
        assert_eq!(
            metrics_inputs[0].quality_runtime_context,
            json!({"quality_runtime": "built"})
        );
    }

    #[test]
    fn should_build_gate_plan_with_retry_scope_and_story_repair_payload() {
        let context = adapter_context("batch");
        let mut gate_inputs = Vec::<CandidateQualityGatePlanInput>::new();
        let mut adapter = build_chapter_candidate_quality_adapter(
            context,
            |_input| Value::Null,
            |_input| Value::Null,
            |input| {
                gate_inputs.push(input);
                json!({"action": "retry", "quality_gate": {"decision": "auto_repair"}})
            },
        );

        let plan = adapter.build_quality_gate_plan(json!({"overall_score": 55.0}), 3);
        drop(adapter);

        assert_eq!(plan["action"], "retry");
        assert_eq!(gate_inputs.len(), 1);
        assert_eq!(
            gate_inputs[0].candidate_metrics.as_ref().unwrap()["overall_score"],
            55.0
        );
        assert_eq!(gate_inputs[0].retry_count, 1);
        assert_eq!(gate_inputs[0].max_retries, 2);
        assert_eq!(
            gate_inputs[0].current_story_repair_payload,
            Some(json!({"reason": "continuity"}))
        );
        assert_eq!(gate_inputs[0].scope, "batch");
    }

    #[test]
    fn should_drop_non_object_candidate_metrics_before_gate_plan() {
        let context = adapter_context("chapter");
        let mut gate_inputs = Vec::<CandidateQualityGatePlanInput>::new();
        let mut adapter = build_chapter_candidate_quality_adapter(
            context,
            |_input| Value::Null,
            |_input| Value::Null,
            |input| {
                gate_inputs.push(input);
                json!({"action": "continue"})
            },
        );

        adapter.build_quality_gate_plan(json!("invalid metrics"), 0);
        drop(adapter);

        assert_eq!(gate_inputs.len(), 1);
        assert!(gate_inputs[0].candidate_metrics.is_none());
    }

    fn adapter_context(scope: &str) -> ChapterCandidateQualityAdapterContext {
        ChapterCandidateQualityAdapterContext {
            story_packet: json!({"packet": true}),
            project: json!({"id": "project-1", "world_rules": "rules A"}),
            chapter: json!({"id": "chapter-1"}),
            chapter_context: json!({"chapter_outline": "outline A"}),
            target_word_count: 1200,
            generation_intent: json!({"mode": "draft"}),
            retry_count: 1,
            max_retries: 2,
            current_story_repair_payload: Some(json!({"reason": "continuity"})),
            scope: scope.to_string(),
            log_prefix: "Chapter".to_string(),
        }
    }
}
