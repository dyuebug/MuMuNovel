// Rust owner for candidate record construction originally mapped from Python
// chapter_candidate_record_service.py. Generation, repair, default dependency,
// and production adapter owners now consume this module for sanitized records,
// quality-gate normalization, and selection metadata.

use serde_json::{Map, Value};

use crate::services::chapter_narrative_cleaner_service::{
    contains_chapter_workflow_meta_text, sanitize_generated_narrative_text,
};

const QUALITY_GATE_ALLOW_SAVE_PRIORITY: i64 = 3;
const QUALITY_GATE_AUTO_REPAIR_PRIORITY: i64 = 2;
const QUALITY_GATE_MANUAL_REVIEW_PRIORITY: i64 = 1;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterCandidateRecordRequest {
    pub(crate) full_content: String,
    pub(crate) candidate_chunks: Vec<String>,
    pub(crate) target_word_count: i64,
    pub(crate) source: String,
    pub(crate) generation_label: String,
    pub(crate) candidate_index: i64,
    pub(crate) candidate_offset: i64,
    pub(crate) generation_path: String,
    pub(crate) attempt_kind: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ChapterCandidateRecordMetadataContext {
    word_count: i64,
    target_word_count: i64,
    candidate_index: i64,
    candidate_count: i64,
    source: String,
    generation_path: String,
    attempt_kind: String,
    rerank_used: bool,
    word_budget_repair_used: bool,
}

pub(crate) fn build_generation_candidate_record<QualityEvaluator, QualityGatePlanBuilder>(
    request: ChapterCandidateRecordRequest,
    quality_evaluator: &mut QualityEvaluator,
    quality_gate_plan_builder: &mut QualityGatePlanBuilder,
    mut log_warning: Option<&mut dyn FnMut(String)>,
) -> Result<Value, String>
where
    QualityEvaluator: FnMut(&str) -> Value,
    QualityGatePlanBuilder: FnMut(Value, i64) -> Value,
{
    let (full_content, removed_meta_lines) =
        sanitize_generated_narrative_text(&request.full_content);
    if removed_meta_lines > 0 {
        if let Some(log_warning_fn) = log_warning.as_mut() {
            log_warning_fn(format!(
                "Sanitized {removed_meta_lines} workflow/meta lines: {}, candidate={}",
                request.generation_label, request.candidate_index
            ));
        }
    }
    if full_content.trim().is_empty() {
        return Err(format!(
            "{} generated empty narrative after sanitization",
            request.generation_label
        ));
    }
    if contains_chapter_workflow_meta_text(&full_content) {
        return Err(format!(
            "{} generated workflow/meta text",
            request.generation_label
        ));
    }

    let candidate_word_count = full_content.chars().count() as i64;
    let metadata_context =
        build_generation_candidate_record_metadata_context(&request, candidate_word_count);

    let mut quality_metrics = object_from_value((quality_evaluator)(&full_content));
    let initial_quality_gate_plan = normalize_candidate_quality_gate_plan(
        (quality_gate_plan_builder)(
            Value::Object(quality_metrics.clone()),
            request.candidate_offset,
        ),
        candidate_word_count,
        request.target_word_count,
        Some(&quality_metrics),
    );
    copy_quality_gate_into_metrics(&initial_quality_gate_plan, &mut quality_metrics);

    let (_selection_metadata, mut enriched_quality_metrics) =
        build_attached_generation_candidate_selection_metadata(
            quality_metrics,
            &initial_quality_gate_plan,
            &metadata_context,
        );
    let enriched_plan = object_from_value((quality_gate_plan_builder)(
        Value::Object(enriched_quality_metrics.clone()),
        request.candidate_offset,
    ));
    let mut quality_gate_plan = if enriched_plan.is_empty() {
        initial_quality_gate_plan
    } else {
        enriched_plan
    };
    quality_gate_plan = normalize_candidate_quality_gate_plan(
        Value::Object(quality_gate_plan),
        candidate_word_count,
        request.target_word_count,
        Some(&enriched_quality_metrics),
    );
    copy_quality_gate_into_metrics(&quality_gate_plan, &mut enriched_quality_metrics);

    let (selection_metadata, enriched_quality_metrics) =
        build_attached_generation_candidate_selection_metadata(
            enriched_quality_metrics,
            &quality_gate_plan,
            &metadata_context,
        );

    let summary_preview = full_content
        .chars()
        .take(300)
        .collect::<String>()
        .replace('\n', " ");
    let mut record = Map::new();
    insert_i64(&mut record, "candidate_index", request.candidate_index);
    record.insert("full_content".to_string(), Value::String(full_content));
    insert_i64(&mut record, "word_count", candidate_word_count);
    record.insert(
        "summary_preview".to_string(),
        Value::String(summary_preview),
    );
    record.insert(
        "quality_metrics".to_string(),
        Value::Object(enriched_quality_metrics),
    );
    record.insert(
        "quality_gate_plan".to_string(),
        Value::Object(quality_gate_plan),
    );
    record.insert(
        "candidate_chunks".to_string(),
        Value::Array(
            request
                .candidate_chunks
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    for (key, value) in selection_metadata {
        record.insert(key, value);
    }
    Ok(Value::Object(record))
}

fn build_generation_candidate_record_metadata_context(
    request: &ChapterCandidateRecordRequest,
    word_count: i64,
) -> ChapterCandidateRecordMetadataContext {
    let attempt_kind = request.attempt_kind.trim().to_string();
    ChapterCandidateRecordMetadataContext {
        word_count,
        target_word_count: request.target_word_count,
        candidate_index: request.candidate_index,
        candidate_count: request.candidate_index,
        source: request.source.clone(),
        generation_path: request.generation_path.clone(),
        rerank_used: attempt_kind == "rerank_candidate",
        word_budget_repair_used: attempt_kind == "word_budget_repair",
        attempt_kind,
    }
}

fn build_attached_generation_candidate_selection_metadata(
    quality_metrics: Map<String, Value>,
    quality_gate_plan: &Map<String, Value>,
    metadata_context: &ChapterCandidateRecordMetadataContext,
) -> (Map<String, Value>, Map<String, Value>) {
    let selection_metadata = build_candidate_selection_metadata(
        &quality_metrics,
        metadata_context.word_count,
        metadata_context.target_word_count,
        metadata_context.candidate_index,
        metadata_context.candidate_count,
        &metadata_context.source,
        quality_gate_plan,
        &metadata_context.generation_path,
        &metadata_context.attempt_kind,
        metadata_context.rerank_used,
        metadata_context.word_budget_repair_used,
    );
    let mut metrics = quality_metrics;
    metrics.insert(
        "candidate_selection".to_string(),
        Value::Object(selection_metadata.clone()),
    );
    (selection_metadata, metrics)
}

fn normalize_candidate_quality_gate_plan(
    quality_gate_plan: Value,
    word_count: i64,
    target_word_count: i64,
    quality_metrics: Option<&Map<String, Value>>,
) -> Map<String, Value> {
    let mut plan = object_from_value(quality_gate_plan);
    let raw_quality_gate = plan
        .get("quality_gate")
        .and_then(Value::as_object)
        .cloned()
        .or_else(|| {
            quality_metrics
                .and_then(|metrics| metrics.get("quality_gate"))
                .and_then(Value::as_object)
                .cloned()
        });
    let normalized_quality_gate =
        normalize_candidate_quality_gate(raw_quality_gate, word_count, target_word_count);
    if !normalized_quality_gate.is_empty() {
        plan.insert(
            "quality_gate".to_string(),
            Value::Object(normalized_quality_gate),
        );
    }
    plan
}

fn normalize_candidate_quality_gate(
    quality_gate: Option<Map<String, Value>>,
    word_count: i64,
    target_word_count: i64,
) -> Map<String, Value> {
    let mut normalized = quality_gate.unwrap_or_default();
    let decision =
        safe_text(normalized.get("decision")).unwrap_or_else(|| "allow_save".to_string());
    let (has_severe_pressure, severe_reason) =
        resolve_severe_word_budget_pressure(word_count, target_word_count);
    if has_severe_pressure && decision == "allow_save" {
        normalized.insert(
            "decision".to_string(),
            Value::String("auto_repair".to_string()),
        );
        normalized.insert(
            "status".to_string(),
            Value::String("repairable".to_string()),
        );
        normalized.insert(
            "label".to_string(),
            Value::String(
                safe_text(normalized.get("label")).unwrap_or_else(|| "Needs repair".to_string()),
            ),
        );
        normalized.insert(
            "reason".to_string(),
            Value::String(safe_text(normalized.get("reason")).unwrap_or(severe_reason)),
        );
        normalized.insert(
            "summary".to_string(),
            Value::String(safe_text(normalized.get("summary")).unwrap_or_else(|| {
                "The draft still needs a targeted revision before it should be saved.".to_string()
            })),
        );
        normalized.insert("allow_save".to_string(), Value::Bool(false));
        normalized.insert("can_auto_repair".to_string(), Value::Bool(true));
        normalized.insert("requires_manual_review".to_string(), Value::Bool(false));
    }
    normalized
}

fn build_candidate_selection_metadata(
    quality_metrics: &Map<String, Value>,
    word_count: i64,
    target_word_count: i64,
    candidate_index: i64,
    candidate_count: i64,
    source: &str,
    quality_gate_plan: &Map<String, Value>,
    generation_path: &str,
    attempt_kind: &str,
    rerank_used: bool,
    word_budget_repair_used: bool,
) -> Map<String, Value> {
    let quality_gate = quality_gate_plan
        .get("quality_gate")
        .and_then(Value::as_object)
        .cloned()
        .or_else(|| {
            quality_metrics
                .get("quality_gate")
                .and_then(Value::as_object)
                .cloned()
        })
        .unwrap_or_default();

    let decision =
        safe_text(quality_gate.get("decision")).unwrap_or_else(|| "allow_save".to_string());
    let status = safe_text(quality_gate.get("status")).unwrap_or_else(|| "pass".to_string());
    let overall_score = safe_f64(quality_metrics.get("overall_score"));
    let pacing_score = safe_f64(quality_metrics.get("pacing_score"));
    let continuity_warning_count = quality_metrics
        .get("continuity_preflight")
        .and_then(Value::as_object)
        .and_then(|preflight| preflight.get("warning_count"))
        .and_then(value_to_i64)
        .unwrap_or(0);

    let normalized_target_word_count = target_word_count.max(1);
    let normalized_word_count = word_count.max(0);
    let word_count_delta = (normalized_word_count - normalized_target_word_count).abs();
    let word_count_fit_ratio =
        (1.0 - word_count_delta as f64 / normalized_target_word_count as f64).max(0.0);
    let word_count_fit_score = round_to(word_count_fit_ratio * 100.0, 1);
    let (target_lower_bound, target_upper_bound) =
        resolve_target_word_bounds(normalized_target_word_count);
    let out_of_window_chars = if normalized_word_count > target_upper_bound {
        normalized_word_count - target_upper_bound
    } else if normalized_word_count > 0 && normalized_word_count < target_lower_bound {
        target_lower_bound - normalized_word_count
    } else {
        0
    };
    let out_of_window_penalty = round_to(
        out_of_window_chars as f64 / normalized_target_word_count as f64 * 24.0,
        2,
    );

    let decision_priority = match decision.as_str() {
        "allow_save" => QUALITY_GATE_ALLOW_SAVE_PRIORITY,
        "auto_repair" => QUALITY_GATE_AUTO_REPAIR_PRIORITY,
        "manual_review" => QUALITY_GATE_MANUAL_REVIEW_PRIORITY,
        _ => 0,
    };
    let decision_bonus = match decision.as_str() {
        "allow_save" => 18.0,
        "auto_repair" => 4.0,
        "manual_review" => -18.0,
        _ => 0.0,
    };
    let selection_score = round_to(
        overall_score
            + decision_bonus
            + word_count_fit_score * 0.12
            + (pacing_score - 7.0).max(0.0) * 1.5
            - continuity_warning_count as f64 * 4.0
            - out_of_window_penalty,
        2,
    );

    let mut selection_metadata = Map::new();
    insert_i64(&mut selection_metadata, "candidate_index", candidate_index);
    insert_i64(&mut selection_metadata, "candidate_count", candidate_count);
    selection_metadata.insert("source".to_string(), Value::String(source.to_string()));
    insert_f64(&mut selection_metadata, "selection_score", selection_score);
    insert_f64(
        &mut selection_metadata,
        "overall_score",
        round_to(overall_score, 1),
    );
    selection_metadata.insert("quality_gate_decision".to_string(), Value::String(decision));
    selection_metadata.insert("quality_gate_status".to_string(), Value::String(status));
    insert_i64(
        &mut selection_metadata,
        "quality_gate_priority",
        decision_priority,
    );
    insert_i64(&mut selection_metadata, "word_count", normalized_word_count);
    insert_i64(
        &mut selection_metadata,
        "target_word_count",
        normalized_target_word_count,
    );
    insert_f64(
        &mut selection_metadata,
        "word_count_fit_score",
        word_count_fit_score,
    );
    insert_i64(
        &mut selection_metadata,
        "word_count_delta",
        word_count_delta,
    );
    insert_f64(
        &mut selection_metadata,
        "out_of_window_penalty",
        out_of_window_penalty,
    );
    insert_i64(
        &mut selection_metadata,
        "continuity_warning_count",
        continuity_warning_count,
    );
    if !generation_path.trim().is_empty() {
        selection_metadata.insert(
            "generation_path".to_string(),
            Value::String(generation_path.trim().to_string()),
        );
    }
    if !attempt_kind.trim().is_empty() {
        selection_metadata.insert(
            "attempt_kind".to_string(),
            Value::String(attempt_kind.trim().to_string()),
        );
    }
    selection_metadata.insert("rerank_used".to_string(), Value::Bool(rerank_used));
    selection_metadata.insert(
        "word_budget_repair_used".to_string(),
        Value::Bool(word_budget_repair_used),
    );
    selection_metadata
}

fn copy_quality_gate_into_metrics(
    quality_gate_plan: &Map<String, Value>,
    quality_metrics: &mut Map<String, Value>,
) {
    if let Some(quality_gate) = quality_gate_plan
        .get("quality_gate")
        .and_then(Value::as_object)
    {
        quality_metrics.insert(
            "quality_gate".to_string(),
            Value::Object(quality_gate.clone()),
        );
    }
}

fn object_from_value(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn safe_text(value: Option<&Value>) -> Option<String> {
    let text = match value {
        Some(Value::String(value)) => value.trim().to_string(),
        Some(Value::Null) | None => String::new(),
        Some(value) => value.to_string().trim_matches('"').trim().to_string(),
    };
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn safe_f64(value: Option<&Value>) -> f64 {
    value.and_then(Value::as_f64).unwrap_or_else(|| {
        value
            .and_then(Value::as_str)
            .and_then(|raw| raw.parse::<f64>().ok())
            .unwrap_or(0.0)
    })
}

fn value_to_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_str()
            .and_then(|raw| raw.trim().parse::<i64>().ok())
    })
}

fn resolve_target_word_bounds(target_word_count: i64) -> (i64, i64) {
    let safe_target_word_count = target_word_count.max(200);
    let target_lower_bound = 200.max(
        (safe_target_word_count - 120).min((safe_target_word_count as f64 * 0.9).trunc() as i64),
    );
    let target_upper_bound = (target_lower_bound + 80).max(
        (safe_target_word_count + 150).min((safe_target_word_count as f64 * 1.15).trunc() as i64),
    );
    (target_lower_bound, target_upper_bound)
}

fn resolve_severe_word_budget_pressure(word_count: i64, target_word_count: i64) -> (bool, String) {
    let normalized_target_word_count = target_word_count.max(0);
    let normalized_word_count = word_count.max(0);
    if normalized_target_word_count <= 0 || normalized_word_count <= 0 {
        return (false, String::new());
    }

    let (target_lower_bound, target_upper_bound) =
        resolve_target_word_bounds(normalized_target_word_count);
    let severe_upper_bound =
        (target_upper_bound + 120).max((target_upper_bound as f64 * 1.1).trunc() as i64);
    let severe_lower_bound =
        200.max((target_lower_bound - 120).min((target_lower_bound as f64 * 0.9).trunc() as i64));
    let severe_pressure = normalized_word_count > severe_upper_bound
        || (normalized_word_count > 0 && normalized_word_count < severe_lower_bound);
    if !severe_pressure {
        return (false, String::new());
    }
    (
        true,
        format!(
            "Word count deviates too far from the target window (current {normalized_word_count}, target {normalized_target_word_count}, ideal range {target_lower_bound}-{target_upper_bound})."
        ),
    )
}

fn round_to(value: f64, decimals: i32) -> f64 {
    let factor = 10_f64.powi(decimals);
    (value * factor).round() / factor
}

fn insert_i64(map: &mut Map<String, Value>, key: &str, value: i64) {
    map.insert(key.to_string(), Value::Number(value.into()));
}

fn insert_f64(map: &mut Map<String, Value>, key: &str, value: f64) {
    if let Some(number) = serde_json::Number::from_f64(value) {
        map.insert(key.to_string(), Value::Number(number));
    }
}

pub(crate) fn build_chapter_candidate_record_owner_contract() -> Value {
    serde_json::json!({
        "owner": "chapter_candidate_record_service",
        "scope": "candidate_record_sanitization_quality_gate_selection_owner",
        "python_source_map": [
            "backend/app/services/chapter_candidate_record_service.py",
            "backend/app/services/chapter_candidate_generation_service.py",
            "backend/app/services/chapter_candidate_finalize_service.py",
            "backend/app/services/chapter_candidate_executor_service.py",
            "backend/tests/test_services/test_chapter_candidate_record_service.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_candidate_record_service.rs",
            "backend-rs/src/services/chapter_candidate_generation_service.rs",
            "backend-rs/src/services/chapter_candidate_finalize_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_default_dependency_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_narrative_cleaner_service.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_generation_candidate_record"
            ],
            "request_fields": [
                "full_content",
                "candidate_chunks",
                "target_word_count",
                "source",
                "generation_label",
                "candidate_index",
                "candidate_offset",
                "generation_path",
                "attempt_kind"
            ],
            "record_fields": [
                "candidate_index",
                "full_content",
                "word_count",
                "summary_preview",
                "quality_metrics",
                "quality_gate_plan",
                "candidate_chunks",
                "candidate_selection metadata fields"
            ],
            "record_policy": [
                "sanitize generated narrative and log removed workflow/meta lines",
                "reject empty narrative after sanitization",
                "reject remaining workflow/meta text",
                "evaluate quality metrics on sanitized content",
                "build quality gate plan before and after candidate_selection attachment",
                "fallback to initial quality gate plan when enriched plan is empty",
                "copy normalized quality gate into quality_metrics",
                "attach selection metadata to both top-level record and quality_metrics.candidate_selection"
            ],
            "quality_gate_policy": [
                "allow_save has priority 3",
                "auto_repair has priority 2",
                "manual_review has priority 1",
                "severe word budget pressure converts allow_save into auto_repair",
                "quality gate defaults to allow_save when decision is absent"
            ],
            "selection_metadata_policy": [
                "word_count and target_word_count are projected",
                "candidate_index and candidate_count are projected",
                "source, generation_path, and attempt_kind are projected",
                "rerank_used is inferred from rerank_candidate attempt kind",
                "word_budget_repair_used is inferred from word_budget_repair attempt kind"
            ],
            "error_contract": [
                "{generation_label} generated empty narrative after sanitization",
                "{generation_label} generated workflow/meta text"
            ]
        },
        "validation_boundary": [
            "cargo test services::chapter_candidate_record_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "active_consumers": [
            "chapter_candidate_generation_service",
            "chapter_candidate_finalize_service",
            "chapter_candidate_executor_default_dependency_service",
            "chapter_candidate_executor_production_adapter_service",
            "chapter_candidate_route_gateway_service"
        ],
        "rollback_boundary": {
            "python_source_map": "chapter_candidate_record_python_source_map",
            "python_fallback_removal_ready": false,
            "approval_required": "explicit source-map freeze/delete/repoint approval"
        },
        "service_runtime_closeout_status": {
            "owner_profiles": [
                "phase5-single-generation-owner",
                "phase5-batch-generation-owner"
            ],
            "single_generation_manifest_probe_count": 6,
            "batch_generation_manifest_probe_count": 11,
            "rust_manifest_probe_count": 17,
            "python_fallback_probe_count": 0,
            "record_builder_owner": "build_generation_candidate_record",
            "sanitization_owner": "sanitize_generated_narrative_text",
            "quality_gate_normalization_owner": "normalize_candidate_quality_gate_plan",
            "selection_metadata_owner": "build_attached_generation_candidate_selection_metadata",
            "source_map_closeout_ready": true,
            "physical_python_closeout_completed": false,
            "remaining_cutover_gate": "explicit source-map freeze/delete/repoint approval with same-round rollback policy",
            "status": "rust_chapter_candidate_record_owner_ready_for_source_map_closeout_review"
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        build_chapter_candidate_record_owner_contract, build_generation_candidate_record,
        ChapterCandidateRecordRequest,
    };

    fn base_request() -> ChapterCandidateRecordRequest {
        ChapterCandidateRecordRequest {
            full_content: "First paragraph.\nSecond paragraph.".to_string(),
            candidate_chunks: vec![
                "First paragraph.".to_string(),
                "Second paragraph.".to_string(),
            ],
            target_word_count: 1200,
            source: "chapter".to_string(),
            generation_label: "test-candidate".to_string(),
            candidate_index: 2,
            candidate_offset: 1,
            generation_path: "rerank_retry".to_string(),
            attempt_kind: "rerank_candidate".to_string(),
        }
    }

    #[test]
    fn should_build_generation_candidate_record_with_enriched_selection_metadata() {
        let mut builder_calls = Vec::<Value>::new();
        let mut quality_evaluator = |_content: &str| {
            json!({
                "overall_score": 82.5,
                "quality_gate": {
                    "decision": "manual_review",
                    "status": "blocked"
                }
            })
        };
        let mut quality_gate_plan_builder = |metrics: Value, attempt_offset: i64| {
            let has_selection = metrics
                .get("candidate_selection")
                .map(|value| !value.is_null())
                .unwrap_or(false);
            builder_calls.push(json!({
                "attempt_offset": attempt_offset,
                "has_selection": has_selection
            }));
            json!({
                "action": "retry",
                "quality_gate": {
                    "decision": "manual_review",
                    "status": "blocked"
                },
                "saw_selection": has_selection
            })
        };

        let result = build_generation_candidate_record(
            base_request(),
            &mut quality_evaluator,
            &mut quality_gate_plan_builder,
            None,
        )
        .expect("candidate record");

        assert_eq!(builder_calls.len(), 2);
        assert_eq!(builder_calls[0]["has_selection"], false);
        assert_eq!(builder_calls[1]["has_selection"], true);
        assert_eq!(result["candidate_index"], 2);
        assert_eq!(result["candidate_count"], 2);
        assert_eq!(result["generation_path"], "rerank_retry");
        assert_eq!(result["attempt_kind"], "rerank_candidate");
        assert_eq!(result["quality_gate_plan"]["saw_selection"], true);
        assert_eq!(
            result["quality_metrics"]["candidate_selection"]["candidate_index"],
            2
        );
        assert_eq!(
            result["quality_metrics"]["candidate_selection"]["rerank_used"],
            true
        );
    }

    #[test]
    fn should_fallback_to_initial_quality_gate_plan_when_enriched_plan_is_empty() {
        let request = ChapterCandidateRecordRequest {
            full_content: "Valid chapter content.".to_string(),
            candidate_chunks: vec!["Valid chapter content.".to_string()],
            target_word_count: 1200,
            source: "chapter".to_string(),
            generation_label: "test-fallback-plan".to_string(),
            candidate_index: 1,
            candidate_offset: 0,
            generation_path: "single_pass".to_string(),
            attempt_kind: "initial_candidate".to_string(),
        };
        let mut quality_evaluator = |_content: &str| {
            json!({
                "overall_score": 91.0,
                "quality_gate": {
                    "decision": "allow_save",
                    "status": "pass"
                }
            })
        };
        let mut quality_gate_plan_builder = |metrics: Value, _attempt_offset: i64| {
            if metrics.get("candidate_selection").is_some() {
                json!({})
            } else {
                json!({
                    "action": "continue",
                    "quality_gate": {
                        "decision": "allow_save",
                        "status": "pass"
                    }
                })
            }
        };

        let result = build_generation_candidate_record(
            request,
            &mut quality_evaluator,
            &mut quality_gate_plan_builder,
            None,
        )
        .expect("candidate record");

        assert_eq!(result["quality_gate_plan"]["action"], "continue");
        assert!(result["quality_metrics"]["quality_gate"].is_object());
        assert_eq!(
            result["quality_metrics"]["quality_gate"],
            result["quality_gate_plan"]["quality_gate"]
        );
    }

    #[test]
    fn should_raise_when_sanitized_generation_is_empty_and_log_removed_meta_lines() {
        let request = ChapterCandidateRecordRequest {
            full_content: "step 1\nstep 2".to_string(),
            candidate_chunks: vec!["step 1".to_string(), "step 2".to_string()],
            target_word_count: 1200,
            source: "chapter".to_string(),
            generation_label: "test-empty-after-sanitize".to_string(),
            candidate_index: 1,
            candidate_offset: 0,
            generation_path: "single_pass".to_string(),
            attempt_kind: "initial_candidate".to_string(),
        };
        let mut warnings = Vec::<String>::new();
        let mut quality_evaluator = |_content: &str| json!({"overall_score": 50.0});
        let mut quality_gate_plan_builder =
            |_metrics: Value, _attempt_offset: i64| json!({"action": "retry"});

        let error = build_generation_candidate_record(
            request,
            &mut quality_evaluator,
            &mut quality_gate_plan_builder,
            Some(&mut |message| warnings.push(message)),
        )
        .expect_err("empty sanitized candidate should fail");

        assert!(error.contains("generated empty narrative after sanitization"));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Sanitized 2 workflow/meta lines"));
    }

    #[test]
    fn should_publish_chapter_candidate_record_owner_contract() {
        let contract = build_chapter_candidate_record_owner_contract();

        assert_eq!(contract["owner"], "chapter_candidate_record_service");
        assert_eq!(
            contract["scope"],
            "candidate_record_sanitization_quality_gate_selection_owner"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/app/services/chapter_candidate_record_service.py"
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_candidate_record_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][0],
            "build_generation_candidate_record"
        );
        assert_eq!(
            contract["behavior_contract"]["request_fields"][8],
            "attempt_kind"
        );
        assert_eq!(
            contract["behavior_contract"]["record_policy"][5],
            "fallback to initial quality gate plan when enriched plan is empty"
        );
        assert_eq!(
            contract["behavior_contract"]["quality_gate_policy"][3],
            "severe word budget pressure converts allow_save into auto_repair"
        );
        assert_eq!(
            contract["active_consumers"][0],
            "chapter_candidate_generation_service"
        );
        assert_eq!(
            contract["rollback_boundary"]["python_fallback_removal_ready"],
            false
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][0],
            "phase5-single-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["owner_profiles"][1],
            "phase5-batch-generation-owner"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["single_generation_manifest_probe_count"],
            6
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["batch_generation_manifest_probe_count"],
            11
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["rust_manifest_probe_count"],
            17
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["python_fallback_probe_count"],
            0
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["record_builder_owner"],
            "build_generation_candidate_record"
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["source_map_closeout_ready"],
            true
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["physical_python_closeout_completed"],
            false
        );
        assert_eq!(
            contract["service_runtime_closeout_status"]["status"],
            "rust_chapter_candidate_record_owner_ready_for_source_map_closeout_review"
        );
    }
}
