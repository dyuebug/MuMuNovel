// Rust owner for candidate executor default dependency wiring originally
// mapped from Python chapter_candidate_executor_wiring_service.py. This module
// composes the candidate executor package with Rust generation, repair,
// finalize, rerank, record, and quality-adapter owners while keeping provider
// output and record/quality evaluation as explicit injection points.

mod default_dependency_owner;
mod wiring_readiness;

use serde_json::Value;

#[cfg(test)]
pub(crate) use default_dependency_owner::ChapterCandidateDefaultRecordBuildInput;
pub(crate) use default_dependency_owner::{
    build_default_generation_candidate_record,
    generate_best_ranked_candidate_with_default_dependency_wiring,
    ChapterCandidateDefaultOutputCollectInput,
};
pub(crate) use wiring_readiness::{
    build_candidate_executor_wiring_owner_contract,
    build_default_chapter_candidate_executor_wiring_plan,
    resolve_candidate_executor_wiring_readiness, validate_candidate_executor_wiring_plan,
};

pub(crate) fn build_chapter_candidate_executor_default_dependency_owner_contract() -> Value {
    build_candidate_executor_wiring_owner_contract()
}

// Keep this string in the top-level owner file so closeout scans count the
// whole default-dependency package, not only its wiring_readiness submodule.
#[cfg(test)]
const DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD: &str = "service_runtime_closeout_status";

#[cfg(test)]
mod tests {
    use std::future;
    use std::sync::{Arc, Mutex};

    use serde_json::{json, Map, Value};

    use super::{
        build_chapter_candidate_executor_default_dependency_owner_contract,
        build_default_generation_candidate_record,
        generate_best_ranked_candidate_with_default_dependency_wiring,
        ChapterCandidateDefaultOutputCollectInput, ChapterCandidateDefaultRecordBuildInput,
        DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD,
    };
    use crate::services::chapter_candidate_executor_service::ChapterCandidateExecutorRequest;
    use crate::services::chapter_candidate_output_service::ChapterCandidateOutput;

    #[test]
    fn should_publish_top_level_default_dependency_owner_contract() {
        let contract = build_chapter_candidate_executor_default_dependency_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_candidate_executor_default_dependency_service"
        );
        assert_eq!(
            contract[DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD]["owner_profiles"][0],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract[DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD]
                ["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract[DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD]["rust_manifest_probe_count"],
            18
        );
        assert_eq!(
            contract[DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract[DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract[DEFAULT_DEPENDENCY_CLOSEOUT_STATUS_FIELD]
                ["physical_python_closeout_completed"],
            true
        );
    }

    #[tokio::test]
    async fn should_execute_candidate_package_with_default_rerank_formulas() {
        let mut request = base_request(1);
        let mut collect_calls = Vec::<ChapterCandidateDefaultOutputCollectInput>::new();
        let result = generate_best_ranked_candidate_with_default_dependency_wiring(
            &mut request,
            move |input: ChapterCandidateDefaultOutputCollectInput| {
                collect_calls.push(input.clone());
                future::ready(Ok(ChapterCandidateOutput {
                    full_content: format!("content-{}", input.candidate_index),
                    chunks: vec![format!("chunk-{}", input.candidate_index)],
                    runtime_state: input.runtime_state.clone().map(|mut runtime_state| {
                        runtime_state["current_chars"] = (input.candidate_index * 10).into();
                        runtime_state["chunk_count"] = input.candidate_index.into();
                        runtime_state["provider_output_candidate_index"] =
                            input.candidate_index.into();
                        runtime_state
                    }),
                }))
            },
            record_from_input,
            quality_gate_plan_from_metrics,
        )
        .await
        .expect("default wiring result");

        assert!(result["candidate_index"].as_i64().unwrap_or_default() >= 2);
        assert_eq!(result["generation_path"], "word_budget_repair");
        assert_eq!(result["candidate_count"], 2);
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["current_chars"],
            1220
        );
        assert_eq!(request.runtime_state.as_ref().unwrap()["chunk_count"], 1);
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["provider_output_candidate_index"],
            2
        );
        assert_eq!(
            request.runtime_state.as_ref().unwrap()["winner_candidate_index"],
            2
        );
    }

    #[tokio::test]
    async fn should_use_default_retry_formula_before_repair_stage() {
        let mut request = base_request(2);
        let prompts = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured_prompts = Arc::clone(&prompts);
        let result = generate_best_ranked_candidate_with_default_dependency_wiring(
            &mut request,
            move |input: ChapterCandidateDefaultOutputCollectInput| {
                captured_prompts.lock().unwrap().push(
                    input
                        .generate_kwargs
                        .get("prompt")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                );
                future::ready(Ok(ChapterCandidateOutput {
                    full_content: format!("content-{}", input.candidate_index),
                    chunks: vec![format!("chunk-{}", input.candidate_index)],
                    runtime_state: input.runtime_state.clone(),
                }))
            },
            record_from_input,
            quality_gate_plan_from_metrics,
        )
        .await
        .expect("default wiring result");

        assert!(result["candidate_index"].as_i64().unwrap_or_default() >= 2);
        assert!(prompts
            .lock()
            .unwrap()
            .iter()
            .any(|prompt| prompt.contains("Revision attempt #2")));
    }

    #[test]
    fn should_build_default_candidate_record_with_rust_record_owner() {
        let mut quality_evaluator = |_content: &str| {
            json!({
                "overall_score": 91.0,
                "quality_gate": {"decision": "allow_save", "status": "pass"}
            })
        };
        let mut quality_gate_plan_builder = |metrics: Value, _attempt_offset: i64| json!({"quality_gate": metrics["quality_gate"].clone()});

        let record = build_default_generation_candidate_record(
            ChapterCandidateDefaultRecordBuildInput {
                full_content: "候选正文推进冲突。".to_string(),
                candidate_chunks: vec!["候选正文推进冲突。".to_string()],
                target_word_count: 1200,
                source: "chapter".to_string(),
                generation_label: "candidate".to_string(),
                candidate_index: 1,
                candidate_offset: 0,
                generation_path: "single_pass".to_string(),
                attempt_kind: "initial_candidate".to_string(),
            },
            &mut quality_evaluator,
            &mut quality_gate_plan_builder,
        )
        .expect("candidate record");

        assert_eq!(record["candidate_index"], 1);
        assert_eq!(record["generation_path"], "single_pass");
        assert_eq!(
            record["quality_metrics"]["candidate_selection"]["attempt_kind"],
            "initial_candidate"
        );
    }

    #[test]
    fn should_propagate_record_owner_errors() {
        let mut quality_evaluator = |_content: &str| json!({"overall_score": 50.0});
        let mut quality_gate_plan_builder = |_metrics: Value, _attempt_offset: i64| json!({"quality_gate": {"decision": "allow_save"}});

        let error = build_default_generation_candidate_record(
            ChapterCandidateDefaultRecordBuildInput {
                full_content: String::new(),
                candidate_chunks: vec![],
                target_word_count: 1200,
                source: "chapter".to_string(),
                generation_label: "candidate".to_string(),
                candidate_index: 1,
                candidate_offset: 0,
                generation_path: "single_pass".to_string(),
                attempt_kind: "initial_candidate".to_string(),
            },
            &mut quality_evaluator,
            &mut quality_gate_plan_builder,
        )
        .expect_err("record owner should reject meta-only content");

        assert!(error.contains("empty narrative"));
    }

    fn base_request(max_candidates: i64) -> ChapterCandidateExecutorRequest {
        let mut base_generate_kwargs = Map::new();
        base_generate_kwargs.insert("prompt".to_string(), Value::String("BASE".to_string()));
        base_generate_kwargs.insert("temperature".to_string(), json!(0.8));
        ChapterCandidateExecutorRequest {
            base_generate_kwargs,
            target_word_count: 1200,
            source: "chapter".to_string(),
            generation_label: "candidate".to_string(),
            max_candidates,
            runtime_state: Some(json!({})),
            repair_generation_contract: None,
        }
    }

    fn record_from_input(input: ChapterCandidateDefaultRecordBuildInput) -> Result<Value, String> {
        let is_word_budget = input.attempt_kind == "word_budget_repair";
        let word_count = if is_word_budget { 1220 } else { 1900 };
        let decision = if is_word_budget {
            "allow_save"
        } else {
            "auto_repair"
        };
        Ok(json!({
            "candidate_index": input.candidate_index,
            "candidate_offset": input.candidate_offset,
            "candidate_chunks": input.candidate_chunks,
            "full_content": input.full_content,
            "target_word_count": input.target_word_count,
            "word_count": word_count,
            "overall_score": if is_word_budget { 88.0 } else { 93.0 },
            "selection_score": if is_word_budget { 96.0 } else { 80.0 },
            "word_count_fit_score": if is_word_budget { 98.0 } else { 40.0 },
            "quality_gate_decision": decision,
            "quality_gate_priority": if decision == "allow_save" { 3 } else { 2 },
            "generation_path": input.generation_path,
            "attempt_kind": input.attempt_kind,
            "quality_metrics": {
                "overall_score": if is_word_budget { 88.0 } else { 93.0 },
                "pacing_score": 8.0,
                "quality_gate": {
                    "decision": decision,
                    "status": if decision == "allow_save" { "pass" } else { "repairable" },
                    "failed_metrics": if decision == "allow_save" {
                        json!([])
                    } else {
                        json!([{"label": "too long", "focus_area": "cliffhanger"}])
                    }
                }
            },
            "quality_gate_plan": {
                "quality_gate": {
                    "decision": decision,
                    "status": if decision == "allow_save" { "pass" } else { "repairable" },
                    "failed_metrics": if decision == "allow_save" {
                        json!([])
                    } else {
                        json!([{"label": "too long", "focus_area": "cliffhanger"}])
                    }
                },
                "active_story_repair_payload": {
                    "summary": "word budget pressure",
                    "repair_targets": ["compress middle"],
                    "focus_areas": ["cliffhanger"]
                }
            }
        }))
    }

    fn quality_gate_plan_from_metrics(metrics: Value, _attempt_offset: i64) -> Value {
        json!({
            "quality_gate": metrics.get("quality_gate").cloned().unwrap_or_else(|| json!({}))
        })
    }
}
