use serde_json::{json, Value};

use crate::models::plot_analysis;
use crate::services::chapter_generation_execution_contract_service::{
    build_batch_request_runtime_state_owner_contract, BatchGenerationRequestRuntimeState,
};
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::{
    apply_batch_quality_runtime_context_to_payload,
    build_generation_quality_runtime_owner_contract,
    resolve_batch_quality_runtime_context_from_current_quality,
    resolve_batch_quality_runtime_context_preserving_existing_quality_state,
};
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::{
    build_story_repair_quality_context_owner_contract,
    resolve_active_story_repair_payload_with_quality_fallback,
};

pub(crate) fn build_batch_generation_quality_payload_owner_contract() -> Value {
    json!({
        "owner": "chapter_batch_generation_runtime_state_service::quality_payload_current_quality_projection",
        "scope": "plot_analysis_quality_summary_latest_metrics_and_runtime_state_payload_projection",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service/quality_payload_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/quality_runtime_context_owner.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs"
        ],
        "behavior_contract": {
            "plot_analysis_projection_entrypoints": [
                "normalized_quality_guidance_items",
                "build_current_chapter_quality_summary_from_plot_analysis",
                "build_current_chapter_latest_quality_metrics_from_plot_analysis"
            ],
            "runtime_payload_entrypoints": [
                "build_batch_generation_runtime_state_payload_preserving_quality_state",
                "build_batch_generation_runtime_state_payload_from_current_quality"
            ],
            "projection_contract": {
                "summary_owner": "plot analysis is normalized into batch quality summary + quality gate + quality runtime context in one owner",
                "latest_metrics_owner": "latest quality metrics projection stays aligned with summary projection and uses the same weakest-metric semantics",
                "runtime_payload_owner": "runtime payload projection keeps active story repair payload and quality runtime context assembly together for batch runtime state refresh"
            }
        },
        "active_consumers": [
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_runtime_state_service::follow_up_analysis_owner",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "quality_runtime_owner_contract": build_generation_quality_runtime_owner_contract(),
        "story_repair_quality_context_owner_contract": build_story_repair_quality_context_owner_contract(),
        "request_runtime_state_owner_contract": build_batch_request_runtime_state_owner_contract(),
        "validation_boundary": [
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test api::health",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "batch_generation_quality_payload_owner_is_rust_only_and_surviving_quality_runtime_surfaces_are_tracked_by_external_runtime_contracts",
            "runtime_state_keys": [
                "quality_metrics_summary",
                "quality_metrics_summary_state",
                "quality_metrics_history",
                "latest_quality_metrics",
                "active_story_repair_payload",
                "quality_history_context"
            ]
        }
    })
}

pub(crate) fn normalized_quality_guidance_items(
    value: Option<&Value>,
    limit: usize,
) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .take(limit)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn build_current_chapter_quality_summary_from_plot_analysis(
    analysis: &plot_analysis::Model,
) -> Option<Value> {
    let overall_score = analysis.overall_quality_score?;
    let pacing_score = analysis.pacing_score;
    let engagement_score = analysis.engagement_score;
    let coherence_score = analysis.coherence_score;
    let suggestions = normalized_quality_guidance_items(analysis.suggestions.as_ref(), 4);

    let metric_pairs = [
        ("pacing", "节奏", pacing_score),
        ("engagement", "吸引力", engagement_score),
        ("coherence", "连贯性", coherence_score),
    ];
    let weakest_metric = metric_pairs
        .into_iter()
        .filter_map(|(key, label, value)| value.map(|score| (key, label, score)))
        .min_by(|left, right| left.2.total_cmp(&right.2));
    let weakest_metric_key = weakest_metric.map(|item| item.0.to_string());
    let weakest_metric_label = weakest_metric.map(|item| item.1.to_string());
    let weakest_metric_value = weakest_metric.map(|item| item.2);

    let mut focus_areas = Vec::new();
    if pacing_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("节奏".to_string());
    }
    if engagement_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("吸引力".to_string());
    }
    if coherence_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("连贯性".to_string());
    }

    let mut preserve_strengths = Vec::new();
    if pacing_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("节奏稳定".to_string());
    }
    if engagement_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("追读牵引".to_string());
    }
    if coherence_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("逻辑连贯".to_string());
    }
    if preserve_strengths.is_empty() && analysis.hooks_count > 0 {
        preserve_strengths.push("钩子密度".to_string());
    }

    let repair_summary = suggestions
        .first()
        .cloned()
        .or_else(|| analysis.analysis_report.clone())
        .unwrap_or_else(|| "当前章节质量分析已完成，建议继续按分析结果微调正文。".to_string());

    let (quality_gate_status, quality_gate_decision, quality_gate_label) = if overall_score < 6.0 {
        ("failed", "manual_review", "需要人工复核")
    } else if overall_score < 8.0 {
        ("warning", "auto_repair", "建议继续修复")
    } else {
        ("passed", "passed", "当前章节通过")
    };

    let failed_metrics = weakest_metric_label
        .as_ref()
        .map(|label| vec![json!({"label": label})])
        .unwrap_or_default();

    Some(json!({
        "overall_score": overall_score,
        "chapter_count": 1,
        "repair_guidance": {
            "summary": repair_summary,
            "repair_targets": suggestions,
            "preserve_strengths": preserve_strengths,
            "focus_areas": focus_areas,
            "weakest_metric_key": weakest_metric_key,
            "weakest_metric_label": weakest_metric_label,
            "weakest_metric_value": weakest_metric_value,
        },
        "quality_gate": {
            "status": quality_gate_status,
            "decision": quality_gate_decision,
            "label": quality_gate_label,
            "summary": repair_summary,
            "failed_metrics": failed_metrics,
        },
        "quality_runtime_context": {
            "scope": "batch",
            "recent_metrics": [{
                "history_index": 0,
                "overall_score": overall_score,
                "repair_guidance": {
                    "summary": repair_summary,
                    "repair_targets": suggestions,
                    "preserve_strengths": preserve_strengths,
                    "focus_areas": focus_areas,
                },
                "quality_gate": {
                    "status": quality_gate_status,
                    "decision": quality_gate_decision,
                    "label": quality_gate_label,
                    "summary": repair_summary,
                    "failed_metrics": failed_metrics,
                }
            }]
        }
    }))
}

pub(crate) fn build_current_chapter_latest_quality_metrics_from_plot_analysis(
    analysis: &plot_analysis::Model,
) -> Option<Value> {
    let overall_score = analysis.overall_quality_score?;
    let pacing_score = analysis.pacing_score;
    let engagement_score = analysis.engagement_score;
    let coherence_score = analysis.coherence_score;
    let suggestions = normalized_quality_guidance_items(analysis.suggestions.as_ref(), 4);

    let metric_pairs = [
        ("pacing", "节奏", pacing_score),
        ("engagement", "吸引力", engagement_score),
        ("coherence", "连贯性", coherence_score),
    ];
    let weakest_metric = metric_pairs
        .into_iter()
        .filter_map(|(key, label, value)| value.map(|score| (key, label, score)))
        .min_by(|left, right| left.2.total_cmp(&right.2));
    let weakest_metric_key = weakest_metric.map(|item| item.0.to_string());
    let weakest_metric_label = weakest_metric.map(|item| item.1.to_string());
    let weakest_metric_value = weakest_metric.map(|item| item.2);

    let mut focus_areas = Vec::new();
    if pacing_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("节奏".to_string());
    }
    if engagement_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("吸引力".to_string());
    }
    if coherence_score.is_some_and(|score| score < 8.0) {
        focus_areas.push("连贯性".to_string());
    }

    let mut preserve_strengths = Vec::new();
    if pacing_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("节奏稳定".to_string());
    }
    if engagement_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("追读牵引".to_string());
    }
    if coherence_score.is_some_and(|score| score >= 8.5) {
        preserve_strengths.push("逻辑连贯".to_string());
    }
    if preserve_strengths.is_empty() && analysis.hooks_count > 0 {
        preserve_strengths.push("钩子密度".to_string());
    }

    let repair_summary = suggestions
        .first()
        .cloned()
        .or_else(|| analysis.analysis_report.clone())
        .unwrap_or_else(|| "当前章节质量分析已完成，建议继续按分析结果微调正文。".to_string());

    let (quality_gate_status, quality_gate_decision, quality_gate_label) = if overall_score < 6.0 {
        ("failed", "manual_review", "需要人工复核")
    } else if overall_score < 8.0 {
        ("warning", "auto_repair", "建议继续修复")
    } else {
        ("passed", "passed", "当前章节通过")
    };

    let failed_metrics = weakest_metric_label
        .as_ref()
        .map(|label| vec![json!({"label": label})])
        .unwrap_or_default();

    Some(json!({
        "overall_score": overall_score,
        "pacing_score": pacing_score,
        "engagement_score": engagement_score,
        "coherence_score": coherence_score,
        "repair_guidance": {
            "summary": repair_summary,
            "repair_targets": suggestions,
            "preserve_strengths": preserve_strengths,
            "focus_areas": focus_areas,
            "weakest_metric_key": weakest_metric_key,
            "weakest_metric_label": weakest_metric_label,
            "weakest_metric_value": weakest_metric_value,
        },
        "quality_gate": {
            "status": quality_gate_status,
            "decision": quality_gate_decision,
            "label": quality_gate_label,
            "summary": repair_summary,
            "failed_metrics": failed_metrics,
        },
        "quality_runtime_context": {
            "scope": "batch",
            "source": "plot_analysis",
        }
    }))
}

pub(crate) fn build_batch_generation_runtime_state_payload_preserving_quality_state(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    explicit_story_repair_payload: Option<&Value>,
    existing_quality_metrics_summary_state: Option<&Value>,
    existing_quality_metrics_history: Option<&Value>,
    existing_quality_summary: Option<&Value>,
    refreshed_quality_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Value {
    let resolved_quality_context =
        resolve_batch_quality_runtime_context_preserving_existing_quality_state(
            existing_quality_metrics_summary_state,
            existing_quality_metrics_history,
            existing_quality_summary,
            refreshed_quality_summary,
            latest_quality_metrics,
        );
    let resolved_quality_summary = resolved_quality_context
        .quality_metrics_summary
        .clone()
        .unwrap_or(Value::Null);
    let mut payload = serde_json::Map::from_iter([(
        "batch_request_runtime_state".to_string(),
        json!(request_runtime_state),
    )]);

    let active_story_repair_payload = resolve_active_story_repair_payload_with_quality_fallback(
        explicit_story_repair_payload,
        Some(&resolved_quality_summary),
        latest_quality_metrics,
        "batch",
        "current_quality_state_refresh",
        "Current quality state refresh",
    );

    if let Some(active_story_repair_payload) = active_story_repair_payload {
        payload.insert(
            "active_story_repair_payload".to_string(),
            active_story_repair_payload,
        );
    }
    apply_batch_quality_runtime_context_to_payload(
        &mut payload,
        resolved_quality_context,
        Some(resolved_quality_summary.clone()),
    );

    Value::Object(payload)
}

pub(crate) fn build_batch_generation_runtime_state_payload_from_current_quality(
    request_runtime_state: &BatchGenerationRequestRuntimeState,
    explicit_story_repair_payload: Option<&Value>,
    existing_quality_metrics_summary_state: Option<&Value>,
    existing_quality_metrics_history: Option<&Value>,
    quality_summary: &Value,
    latest_quality_metrics: Option<&Value>,
) -> Value {
    let resolved_quality_context = resolve_batch_quality_runtime_context_from_current_quality(
        existing_quality_metrics_summary_state,
        existing_quality_metrics_history,
        quality_summary,
        latest_quality_metrics,
    );
    let resolved_quality_summary = resolved_quality_context
        .quality_metrics_summary
        .clone()
        .unwrap_or_else(|| quality_summary.clone());
    let mut payload = serde_json::Map::from_iter([(
        "batch_request_runtime_state".to_string(),
        json!(request_runtime_state),
    )]);

    let active_story_repair_payload = resolve_active_story_repair_payload_with_quality_fallback(
        explicit_story_repair_payload,
        Some(&resolved_quality_summary),
        latest_quality_metrics,
        "batch",
        "current_chapter_quality",
        "Current chapter quality",
    );

    if let Some(active_story_repair_payload) = active_story_repair_payload {
        payload.insert(
            "active_story_repair_payload".to_string(),
            active_story_repair_payload,
        );
    }
    apply_batch_quality_runtime_context_to_payload(
        &mut payload,
        resolved_quality_context,
        Some(resolved_quality_summary.clone()),
    );

    Value::Object(payload)
}
