use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};
use std::collections::HashSet;

use crate::models::{chapter, generation_history};
use crate::services::chapter_generation_execution_contract_service::SingleChapterGenerationCompatOptions;
use crate::services::chapter_generation_prompt_service::{
    resolve_adaptive_quality_gate_profile, resolve_metric_threshold_adjustments,
    resolve_quality_weight_profile,
};
use crate::services::chapter_generation_runtime_service::quality_runtime_context_owner::manual_review_label_from_quality_context;
use crate::services::chapter_quality_metrics_query_service::build_chapter_analysis_quality_fragments;

const MANUAL_REQUEST_SOURCE: &str = "manual_request";
const MANUAL_REQUEST_SOURCE_LABEL: &str = "Manual request";
const CURRENT_CHAPTER_QUALITY_SOURCE: &str = "current_chapter_quality";
const MANUAL_PLUS_CURRENT_CHAPTER_QUALITY_SOURCE: &str = "manual_plus_current_chapter_quality";
const MANUAL_PLUS_CURRENT_CHAPTER_QUALITY_SOURCE_LABEL: &str = "Manual + current chapter quality";
const MANUAL_PLUS_RECENT_HISTORY_SUMMARY_SOURCE: &str = "manual_plus_recent_history_summary";
const MANUAL_PLUS_RECENT_HISTORY_SUMMARY_SOURCE_LABEL: &str = "Manual + recent history summary";

pub(crate) fn has_explicit_story_repair_input(
    compat_options: &SingleChapterGenerationCompatOptions,
) -> bool {
    !compat_options.story_repair_summary().trim().is_empty()
        || !compat_options.story_repair_targets().is_empty()
        || !compat_options.story_preserve_strengths().is_empty()
}

pub(crate) fn restore_story_repair_compat_options_from_active_snapshot(
    compat_options: &SingleChapterGenerationCompatOptions,
    active_story_repair_payload: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> SingleChapterGenerationCompatOptions {
    if has_explicit_story_repair_input(compat_options) {
        return compat_options.clone();
    }

    let snapshot_payload = active_story_repair_payload
        .and_then(Value::as_object)
        .cloned()
        .or_else(|| {
            restore_story_repair_payload_from_quality_context(
                quality_metrics_summary,
                latest_quality_metrics,
            )
        });

    let Some(snapshot) = snapshot_payload else {
        return compat_options.clone();
    };

    let summary = snapshot
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let repair_targets = snapshot
        .get("repair_targets")
        .and_then(Value::as_array)
        .map(|items| normalize_guidance_items(items, 4))
        .unwrap_or_default();
    let preserve_strengths = snapshot
        .get("preserve_strengths")
        .and_then(Value::as_array)
        .map(|items| normalize_guidance_items(items, 2))
        .unwrap_or_default();

    let mut restored = compat_options.clone();
    restored.story_repair_summary = summary;
    restored.story_repair_targets = repair_targets;
    restored.story_preserve_strengths = preserve_strengths;
    restored
}

pub(crate) fn restore_story_repair_payload_from_quality_context(
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Option<serde_json::Map<String, Value>> {
    let guidance = quality_repair_guidance_from_quality_context(
        quality_metrics_summary,
        latest_quality_metrics,
    )?;
    let merged_guidance = merged_story_repair_guidance_from_quality_context(
        quality_metrics_summary,
        latest_quality_metrics,
    );

    let mut payload = serde_json::Map::new();
    if let Some(summary) = guidance.get("summary").cloned() {
        payload.insert("summary".to_string(), summary);
    }
    if let Some(repair_targets) = merged_guidance
        .as_ref()
        .and_then(|guidance| guidance.get("repair_targets").cloned())
    {
        payload.insert("repair_targets".to_string(), repair_targets);
    }
    if let Some(preserve_strengths) = merged_guidance
        .as_ref()
        .and_then(|guidance| guidance.get("preserve_strengths").cloned())
    {
        payload.insert("preserve_strengths".to_string(), preserve_strengths);
    }

    (!payload.is_empty()).then_some(payload)
}

pub(crate) async fn load_recent_batch_story_repair_quality_summary(
    db: &DatabaseConnection,
    project_id: &str,
    before_chapter_number: i32,
) -> Result<Option<Value>, String> {
    if before_chapter_number <= 1 {
        return Ok(None);
    }

    let previous_chapters = chapter::Entity::find()
        .filter(chapter::Column::ProjectId.eq(project_id))
        .filter(chapter::Column::ChapterNumber.lt(before_chapter_number))
        .order_by_desc(chapter::Column::ChapterNumber)
        .limit(3)
        .all(db)
        .await
        .map_err(|error| {
            format!("load previous chapters for batch story repair failed: {error}")
        })?;

    if previous_chapters.is_empty() {
        return Ok(None);
    }

    let mut summaries = Vec::new();
    for previous_chapter in previous_chapters {
        let histories = generation_history::Entity::find()
            .filter(generation_history::Column::ChapterId.eq(Some(previous_chapter.id.clone())))
            .order_by_desc(generation_history::Column::CreatedAt)
            .limit(30)
            .all(db)
            .await
            .map_err(|error| {
                format!("load generation histories for batch story repair failed: {error}")
            })?;
        let quality_fragments = build_chapter_analysis_quality_fragments(&histories, None);
        if let Some(summary) = quality_fragments.quality_metrics_summary {
            summaries.push(summary);
        }
    }

    Ok(aggregate_story_repair_quality_summaries(
        &summaries, "batch",
    ))
}

pub(crate) fn restore_active_story_repair_payload_from_quality_context(
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    scope: &str,
    source: &str,
    source_label: &str,
) -> Option<Value> {
    let guidance = quality_repair_guidance_from_quality_context(
        quality_metrics_summary,
        latest_quality_metrics,
    )?;
    let merged_guidance = merged_story_repair_guidance_from_quality_context(
        quality_metrics_summary,
        latest_quality_metrics,
    );
    let payload = restore_story_repair_payload_from_quality_context(
        quality_metrics_summary,
        latest_quality_metrics,
    )?;
    let quality_gate = reconciled_quality_gate_from_quality_context(
        quality_metrics_summary,
        latest_quality_metrics,
    );

    let summary = payload.get("summary").cloned().unwrap_or(Value::Null);
    let repair_targets = payload
        .get("repair_targets")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let preserve_strengths = payload
        .get("preserve_strengths")
        .cloned()
        .unwrap_or_else(|| json!([]));

    let focus_areas = merged_guidance
        .as_ref()
        .and_then(|guidance| guidance.get("focus_areas"))
        .or_else(|| guidance.get("focus_areas"))
        .and_then(Value::as_array)
        .map(|items| normalize_guidance_items(items, 4))
        .unwrap_or_default();
    let weakest_metric_key = guidance
        .get("weakest_metric_key")
        .cloned()
        .filter(|value| value.is_string());
    let weakest_metric_label = guidance
        .get("weakest_metric_label")
        .cloned()
        .filter(|value| value.is_string());
    let weakest_metric_value = guidance
        .get("weakest_metric_value")
        .cloned()
        .filter(|value| value.is_number());
    let quality_gate_status = quality_gate
        .as_ref()
        .and_then(|gate| gate.get("status"))
        .cloned()
        .filter(|value| value.is_string());
    let quality_gate_decision = quality_gate
        .as_ref()
        .and_then(|gate| gate.get("decision"))
        .cloned()
        .filter(|value| value.is_string());
    let quality_gate_label = quality_gate
        .as_ref()
        .and_then(|gate| gate.get("label"))
        .cloned()
        .filter(|value| value.is_string());
    let quality_gate_summary = quality_gate
        .as_ref()
        .and_then(|gate| gate.get("summary"))
        .cloned()
        .filter(|value| value.is_string());
    let quality_gate_failed_metrics = quality_gate
        .as_ref()
        .and_then(|gate| gate.get("failed_metrics"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str().map(str::to_string).or_else(|| {
                        item.as_object()
                            .and_then(|entry| entry.get("label"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                })
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .map(|items| {
            let mut seen = HashSet::new();
            items
                .into_iter()
                .filter(|value| seen.insert(value.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(json!({
        "summary": summary,
        "repair_targets": repair_targets,
        "preserve_strengths": preserve_strengths,
        "focus_areas": focus_areas,
        "weakest_metric_key": weakest_metric_key.unwrap_or(Value::Null),
        "weakest_metric_label": weakest_metric_label.unwrap_or(Value::Null),
        "weakest_metric_value": weakest_metric_value.unwrap_or(Value::Null),
        "quality_gate": quality_gate.unwrap_or(Value::Null),
        "quality_gate_status": quality_gate_status.unwrap_or(Value::Null),
        "quality_gate_decision": quality_gate_decision.unwrap_or(Value::Null),
        "quality_gate_label": quality_gate_label.unwrap_or(Value::Null),
        "quality_gate_summary": quality_gate_summary.unwrap_or(Value::Null),
        "quality_gate_failed_metrics": quality_gate_failed_metrics,
        "source": source,
        "source_label": source_label,
        "scope": scope,
        "updated_at": Value::Null,
    }))
}

pub(crate) fn resolve_active_story_repair_payload_with_quality_fallback(
    explicit_payload: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    scope: &str,
    quality_source: &str,
    quality_source_label: &str,
) -> Option<Value> {
    let derived_payload = restore_active_story_repair_payload_from_quality_context(
        quality_metrics_summary,
        latest_quality_metrics,
        scope,
        quality_source,
        quality_source_label,
    );

    merge_active_story_repair_payloads(
        explicit_payload,
        derived_payload.as_ref(),
        scope,
        quality_source,
        quality_source_label,
    )
}

pub(crate) fn resolve_resumed_active_story_repair_payload(
    runtime_payload: Option<&Value>,
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
    request_payload: Option<&Value>,
    scope: &str,
    quality_source: &str,
    quality_source_label: &str,
) -> Option<Value> {
    normalize_active_story_repair_payload_value(runtime_payload)
        .or_else(|| {
            restore_active_story_repair_payload_from_quality_context(
                quality_metrics_summary,
                latest_quality_metrics,
                scope,
                quality_source,
                quality_source_label,
            )
        })
        .or_else(|| {
            merge_active_story_repair_payloads(
                request_payload,
                None,
                scope,
                quality_source,
                quality_source_label,
            )
        })
}

pub(crate) fn merge_active_story_repair_payloads(
    explicit_payload: Option<&Value>,
    derived_payload: Option<&Value>,
    scope: &str,
    derived_source: &str,
    derived_source_label: &str,
) -> Option<Value> {
    let explicit_payload = explicit_payload.and_then(normalize_active_story_repair_payload);
    let derived_payload = derived_payload.and_then(normalize_active_story_repair_payload);

    match (explicit_payload, derived_payload) {
        (None, None) => None,
        (Some(explicit), None) => Some(stamp_story_repair_payload(
            explicit,
            MANUAL_REQUEST_SOURCE,
            MANUAL_REQUEST_SOURCE_LABEL,
            scope,
        )),
        (None, Some(derived)) => Some(stamp_story_repair_payload(
            derived,
            derived_source,
            derived_source_label,
            scope,
        )),
        (Some(explicit), Some(derived)) => {
            let (merged_source, merged_source_label) =
                merged_story_repair_source_label(derived_source);
            let summary = value_or_fallback(explicit.get("summary"), derived.get("summary"));
            let repair_targets = merge_guidance_value_lists(
                explicit.get("repair_targets"),
                derived.get("repair_targets"),
                4,
            );
            let preserve_strengths = merge_guidance_value_lists(
                explicit.get("preserve_strengths"),
                derived.get("preserve_strengths"),
                2,
            );
            let focus_areas = merge_guidance_value_lists(
                explicit.get("focus_areas"),
                derived.get("focus_areas"),
                4,
            );

            let mut merged = derived;
            merged.insert("summary".to_string(), summary.unwrap_or(Value::Null));
            merged.insert("repair_targets".to_string(), json!(repair_targets));
            merged.insert("preserve_strengths".to_string(), json!(preserve_strengths));
            merged.insert("focus_areas".to_string(), json!(focus_areas));

            Some(stamp_story_repair_payload(
                merged,
                merged_source,
                merged_source_label,
                scope,
            ))
        }
    }
}

pub(crate) fn extract_quality_history_context(
    quality_metrics_summary: Option<&Value>,
) -> Option<Value> {
    quality_metrics_summary
        .and_then(|summary| summary.get("quality_runtime_context"))
        .filter(|context| context.is_object())
        .and_then(|context| {
            context
                .as_object()
                .filter(|value| !value.is_empty())
                .cloned()
                .map(Value::Object)
        })
}

#[derive(Clone, Copy)]
struct QualityMetricDescriptor {
    metric_key: &'static str,
    detail_key: Option<&'static str>,
    focus_area: &'static str,
    label: &'static str,
    weak_threshold: f64,
    repair_target: &'static str,
    preserve_hint: &'static str,
}

#[derive(Clone, Copy)]
struct QualityMetricSignal {
    descriptor: QualityMetricDescriptor,
    value: f64,
    normalized_value: f64,
    weak_threshold: f64,
}

const QUALITY_SUMMARY_METRIC_FIELDS: [(&str, &str); 10] = [
    ("overall_score", "avg_overall_score"),
    ("engagement_score", "avg_engagement_score"),
    ("coherence_score", "avg_coherence_score"),
    ("conflict_chain_hit_rate", "avg_conflict_chain_hit_rate"),
    ("rule_grounding_hit_rate", "avg_rule_grounding_hit_rate"),
    ("outline_alignment_rate", "avg_outline_alignment_rate"),
    ("dialogue_naturalness_rate", "avg_dialogue_naturalness_rate"),
    ("opening_hook_rate", "avg_opening_hook_rate"),
    ("payoff_chain_rate", "avg_payoff_chain_rate"),
    ("cliffhanger_rate", "avg_cliffhanger_rate"),
];

const QUALITY_METRIC_DESCRIPTORS: [QualityMetricDescriptor; 8] = [
    QualityMetricDescriptor {
        metric_key: "conflict_chain_hit_rate",
        detail_key: Some("conflict_chain"),
        focus_area: "conflict",
        label: "冲突推进",
        weak_threshold: 72.0,
        repair_target: "本章至少推进 1 个主线矛盾，并明确新的阻力或代价。",
        preserve_hint: "主线冲突推进",
    },
    QualityMetricDescriptor {
        metric_key: "rule_grounding_hit_rate",
        detail_key: Some("rule_grounding"),
        focus_area: "rule_grounding",
        label: "规则落地",
        weak_threshold: 72.0,
        repair_target: "把世界规则、代价或限制写进具体动作与结果，避免只停留在说明层。",
        preserve_hint: "世界规则落地",
    },
    QualityMetricDescriptor {
        metric_key: "outline_alignment_rate",
        detail_key: Some("outline_alignment"),
        focus_area: "outline",
        label: "大纲贴合",
        weak_threshold: 72.0,
        repair_target: "把本章大纲任务拆成可见动作与结果，不要只做解释性铺陈。",
        preserve_hint: "大纲任务清晰",
    },
    QualityMetricDescriptor {
        metric_key: "dialogue_naturalness_rate",
        detail_key: Some("dialogue"),
        focus_area: "dialogue",
        label: "对白自然度",
        weak_threshold: 74.0,
        repair_target: "收紧对白解释，改成更符合角色立场与情绪的对抗式表达。",
        preserve_hint: "对白辨识度",
    },
    QualityMetricDescriptor {
        metric_key: "opening_hook_rate",
        detail_key: Some("opening_hook"),
        focus_area: "opening",
        label: "开场钩子",
        weak_threshold: 72.0,
        repair_target: "开篇更早抛出异常、目标或风险，避免长铺垫后才进入主事件。",
        preserve_hint: "开场牵引",
    },
    QualityMetricDescriptor {
        metric_key: "payoff_chain_rate",
        detail_key: Some("payoff_chain"),
        focus_area: "payoff",
        label: "回报兑现",
        weak_threshold: 72.0,
        repair_target: "优先回收至少一个既有伏笔、承诺或情绪账，形成阶段性结果。",
        preserve_hint: "阶段兑现",
    },
    QualityMetricDescriptor {
        metric_key: "cliffhanger_rate",
        detail_key: Some("cliffhanger"),
        focus_area: "cliffhanger",
        label: "章尾牵引",
        weak_threshold: 74.0,
        repair_target: "在章尾保留明确未决问题、危险或选择压力，增强追读拉力。",
        preserve_hint: "章尾牵引",
    },
    QualityMetricDescriptor {
        metric_key: "pacing_score",
        detail_key: None,
        focus_area: "pacing",
        label: "节奏稳定度",
        weak_threshold: 7.2,
        repair_target: "压缩解释段并前置冲突触发，让节拍保持目标—受阻—反制推进。",
        preserve_hint: "节奏稳定",
    },
];

fn quality_stage_label(stage: &str) -> &'static str {
    match stage {
        "opening" => "开篇",
        "development" => "发展",
        "ending" => "收束",
        _ => "",
    }
}

fn focus_area_label(focus_area: &str) -> String {
    match focus_area {
        "conflict" => "冲突推进".to_string(),
        "outline" => "大纲贴合".to_string(),
        "pacing" => "节奏稳定度".to_string(),
        "payoff" => "回报兑现".to_string(),
        "cliffhanger" => "章尾牵引".to_string(),
        "dialogue" => "对白自然度".to_string(),
        "rule_grounding" => "规则落地".to_string(),
        "opening" => "开场钩子".to_string(),
        "foreshadow_continuity" => "伏笔连续性".to_string(),
        "relationship_continuity" => "关系连续性".to_string(),
        "character_continuity" => "角色连续性".to_string(),
        "organization_continuity" => "组织连续性".to_string(),
        "career_continuity" => "职业连续性".to_string(),
        _ => focus_area.to_string(),
    }
}

fn repair_effectiveness_metric_spec(focus_area: &str) -> Option<(&'static str, f64, f64)> {
    match focus_area {
        "conflict" => Some(("conflict_chain_hit_rate", 72.0, 3.0)),
        "outline" => Some(("outline_alignment_rate", 72.0, 3.0)),
        "pacing" => Some(("pacing_score", 7.2, 0.4)),
        "payoff" => Some(("payoff_chain_rate", 72.0, 3.0)),
        "cliffhanger" => Some(("cliffhanger_rate", 74.0, 3.0)),
        "dialogue" => Some(("dialogue_naturalness_rate", 74.0, 3.0)),
        "rule_grounding" => Some(("rule_grounding_hit_rate", 72.0, 3.0)),
        "opening" => Some(("opening_hook_rate", 72.0, 3.0)),
        "foreshadow_continuity" => Some(("payoff_chain_rate", 72.0, 3.0)),
        "relationship_continuity" => Some(("dialogue_naturalness_rate", 74.0, 3.0)),
        "character_continuity" => Some(("dialogue_naturalness_rate", 74.0, 3.0)),
        "organization_continuity" => Some(("rule_grounding_hit_rate", 72.0, 3.0)),
        "career_continuity" => Some(("rule_grounding_hit_rate", 72.0, 3.0)),
        _ => None,
    }
}

fn metric_value_as_f64(metrics: &serde_json::Map<String, Value>, metric_key: &str) -> Option<f64> {
    metrics.get(metric_key).and_then(Value::as_f64)
}

fn metric_is_applicable(
    metrics: &serde_json::Map<String, Value>,
    descriptor: QualityMetricDescriptor,
) -> bool {
    let Some(detail_key) = descriptor.detail_key else {
        return true;
    };
    let Some(details) = metrics.get("details").and_then(Value::as_object) else {
        return true;
    };
    let Some(detail) = details.get(detail_key).and_then(Value::as_object) else {
        return true;
    };
    match detail.get("applicable") {
        Some(Value::Bool(false)) => false,
        Some(Value::Bool(_)) => true,
        Some(_) | None => true,
    }
}

fn normalized_metric_value(metric_key: &str, value: f64) -> f64 {
    if metric_key == "pacing_score" {
        value * 10.0
    } else {
        value
    }
}

fn normalize_runtime_context_item_texts(value: Option<&Value>, limit: usize) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };

    let items = match value {
        Value::Array(items) => items.iter().collect::<Vec<_>>(),
        Value::Null => Vec::new(),
        _ => vec![value],
    };
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for item in items {
        let text = if let Some(text) = item.as_str() {
            text.trim().to_string()
        } else if let Some(object) = item.as_object() {
            let keys = [
                "setup",
                "payoff",
                "summary",
                "trigger",
                "resolution",
                "content",
                "item",
                "value",
                "title",
                "name",
                "label",
                "status",
            ];
            keys.iter()
                .filter_map(|key| object.get(*key))
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            item.to_string()
        };
        if text.is_empty() || !seen.insert(text.clone()) {
            continue;
        }
        normalized.push(text);
        if normalized.len() >= limit {
            break;
        }
    }

    normalized
}

fn extract_quality_runtime_context_object(
    value: Option<&Value>,
) -> Option<serde_json::Map<String, Value>> {
    value
        .and_then(|metrics| metrics.get("quality_runtime_context"))
        .and_then(Value::as_object)
        .filter(|context| !context.is_empty())
        .cloned()
}

fn resolve_quality_stage(
    runtime_context: Option<&serde_json::Map<String, Value>>,
) -> Option<String> {
    let runtime_context = runtime_context?;
    if let Some(stage) = runtime_context
        .get("plot_stage")
        .or_else(|| runtime_context.get("quality_stage"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| matches!(*value, "opening" | "development" | "ending"))
    {
        return Some(stage.to_string());
    }

    let current = runtime_context
        .get("current_chapter_number")
        .and_then(Value::as_f64);
    let total = runtime_context.get("chapter_count").and_then(Value::as_f64);
    match (current, total) {
        (Some(current), Some(total)) if total > 0.0 => {
            let progress = current / total;
            if progress <= 0.22 {
                Some("opening".to_string())
            } else if progress >= 0.78 {
                Some("ending".to_string())
            } else {
                Some("development".to_string())
            }
        }
        _ => None,
    }
}

fn build_quality_runtime_pressure(
    runtime_context: Option<&serde_json::Map<String, Value>>,
) -> Value {
    let Some(runtime_context) = runtime_context else {
        return json!({
            "character_state_count": 0,
            "relationship_state_count": 0,
            "foreshadow_plan_count": 0,
            "foreshadow_state_count": 0,
            "organization_state_count": 0,
            "career_state_count": 0,
            "chapter_progress_ratio": Value::Null,
            "character_state_items": [],
            "relationship_state_items": [],
            "foreshadow_state_items": [],
            "organization_state_items": [],
            "career_state_items": [],
        });
    };

    let character_state_items =
        normalize_runtime_context_item_texts(runtime_context.get("character_state_ledger"), 6);
    let relationship_state_items =
        normalize_runtime_context_item_texts(runtime_context.get("relationship_state_ledger"), 6);
    let foreshadow_plan_count = normalize_runtime_context_item_texts(
        runtime_context.get("foreshadow_payoff_plan"),
        8,
    )
    .len() as i64;
    let foreshadow_state_items =
        normalize_runtime_context_item_texts(runtime_context.get("foreshadow_state_ledger"), 8);
    let organization_state_items =
        normalize_runtime_context_item_texts(runtime_context.get("organization_state_ledger"), 6);
    let career_state_items =
        normalize_runtime_context_item_texts(runtime_context.get("career_state_ledger"), 6);
    let chapter_progress_ratio = match (
        runtime_context
            .get("current_chapter_number")
            .and_then(Value::as_f64),
        runtime_context.get("chapter_count").and_then(Value::as_f64),
    ) {
        (Some(current), Some(total)) if total > 0.0 => {
            Some(round_quality_metric(current / total * 100.0) / 100.0)
        }
        _ => None,
    };

    json!({
        "character_state_count": character_state_items.len() as i64,
        "relationship_state_count": relationship_state_items.len() as i64,
        "foreshadow_plan_count": foreshadow_plan_count,
        "foreshadow_state_count": foreshadow_state_items.len() as i64,
        "organization_state_count": organization_state_items.len() as i64,
        "career_state_count": career_state_items.len() as i64,
        "chapter_progress_ratio": chapter_progress_ratio.map(Value::from).unwrap_or(Value::Null),
        "character_state_items": character_state_items.into_iter().take(3).collect::<Vec<_>>(),
        "relationship_state_items": relationship_state_items.into_iter().take(3).collect::<Vec<_>>(),
        "foreshadow_state_items": foreshadow_state_items.into_iter().take(3).collect::<Vec<_>>(),
        "organization_state_items": organization_state_items.into_iter().take(3).collect::<Vec<_>>(),
        "career_state_items": career_state_items.into_iter().take(3).collect::<Vec<_>>(),
    })
}

fn collect_quality_metric_signals(
    metrics: &serde_json::Map<String, Value>,
    runtime_context: Option<&serde_json::Map<String, Value>>,
) -> Vec<QualityMetricSignal> {
    QUALITY_METRIC_DESCRIPTORS
        .iter()
        .filter_map(|descriptor| {
            if !metric_is_applicable(metrics, *descriptor) {
                return None;
            }
            let value = metric_value_as_f64(metrics, descriptor.metric_key)?;
            let weak_threshold =
                adjusted_quality_metric_weak_threshold(*descriptor, runtime_context);
            Some(QualityMetricSignal {
                descriptor: *descriptor,
                value,
                normalized_value: normalized_metric_value(descriptor.metric_key, value),
                weak_threshold,
            })
        })
        .collect()
}

fn adjusted_quality_metric_weak_threshold(
    descriptor: QualityMetricDescriptor,
    runtime_context: Option<&serde_json::Map<String, Value>>,
) -> f64 {
    let Some(runtime_context) = runtime_context else {
        return descriptor.weak_threshold;
    };
    let stage = resolve_quality_stage(Some(runtime_context));
    let profile_adjustments =
        resolve_metric_threshold_adjustments(Some(runtime_context), stage.as_deref());
    let pressure = build_quality_runtime_pressure(Some(runtime_context));
    let pressure_count = |key: &str| {
        pressure
            .get(key)
            .and_then(Value::as_i64)
            .unwrap_or_default()
    };

    let mut adjustment = 0.0;
    if descriptor.metric_key == "payoff_chain_rate" && pressure_count("foreshadow_state_count") >= 3
    {
        adjustment += 2.0;
    }
    if descriptor.metric_key == "dialogue_naturalness_rate"
        && pressure_count("relationship_state_count") >= 2
    {
        adjustment += 1.0;
    }
    if descriptor.metric_key == "conflict_chain_hit_rate"
        && pressure_count("relationship_state_count") >= 2
    {
        adjustment += 1.0;
    }
    if descriptor.metric_key == "outline_alignment_rate"
        && pressure_count("character_state_count") >= 3
    {
        adjustment += 1.0;
    }
    if descriptor.metric_key == "rule_grounding_hit_rate"
        && pressure_count("organization_state_count") >= 2
    {
        adjustment += 1.0;
    }
    if descriptor.metric_key == "conflict_chain_hit_rate"
        && pressure_count("organization_state_count") >= 2
    {
        adjustment += 1.0;
    }
    if descriptor.metric_key == "outline_alignment_rate"
        && pressure_count("career_state_count") >= 2
    {
        adjustment += 1.0;
    }
    if descriptor.metric_key == "payoff_chain_rate" && pressure_count("career_state_count") >= 2 {
        adjustment += 1.0;
    }

    let profile_adjustment = profile_adjustments
        .get(descriptor.metric_key)
        .copied()
        .unwrap_or_default();
    round_quality_metric((descriptor.weak_threshold + profile_adjustment + adjustment).max(0.0))
}

fn has_quality_metric_signal(metrics: &serde_json::Map<String, Value>) -> bool {
    QUALITY_SUMMARY_METRIC_FIELDS
        .iter()
        .any(|(metric_key, _)| metrics.contains_key(*metric_key))
        || metrics
            .get("quality_runtime_context")
            .is_some_and(Value::is_object)
        || metrics
            .get("story_runtime_contract")
            .is_some_and(Value::is_object)
}

fn derive_story_repair_guidance_from_metrics_object(
    metrics: &serde_json::Map<String, Value>,
    _scope: &str,
) -> Value {
    let runtime_context =
        extract_quality_runtime_context_object(Some(&Value::Object(metrics.clone())));
    let stage = resolve_quality_stage(runtime_context.as_ref());
    let stage_label = stage
        .as_deref()
        .map(quality_stage_label)
        .unwrap_or_default()
        .to_string();
    let adaptive_quality_profile =
        resolve_adaptive_quality_gate_profile(runtime_context.as_ref(), stage.as_deref());
    let quality_runtime_pressure = build_quality_runtime_pressure(runtime_context.as_ref());
    let metric_signals = collect_quality_metric_signals(metrics, runtime_context.as_ref());

    if metric_signals.is_empty() {
        return json!({
            "summary": "当前质量指标不足，暂时无法生成修复指引。",
            "repair_targets": [],
            "preserve_strengths": [],
            "focus_areas": [],
            "weakest_metric_key": Value::Null,
            "weakest_metric_label": Value::Null,
            "weakest_metric_value": Value::Null,
            "quality_stage": stage.unwrap_or_default(),
            "quality_stage_label": stage_label,
            "adaptive_quality_profile": adaptive_quality_profile,
            "quality_runtime_pressure": quality_runtime_pressure,
        });
    }

    let weakest_metric = metric_signals
        .iter()
        .min_by(|left, right| left.normalized_value.total_cmp(&right.normalized_value));
    let low_items = metric_signals
        .iter()
        .filter(|item| item.value < item.weak_threshold)
        .copied()
        .collect::<Vec<_>>();
    let strength_items = metric_signals
        .iter()
        .filter(|item| {
            item.value
                >= if item.descriptor.metric_key == "pacing_score" {
                    8.3
                } else {
                    item.weak_threshold + 8.0
                }
        })
        .copied()
        .collect::<Vec<_>>();

    let weakest_metric = weakest_metric.expect("metric_signals should not be empty");
    let mut repair_targets = low_items
        .iter()
        .map(|item| item.descriptor.repair_target.to_string())
        .collect::<Vec<_>>();
    if repair_targets.is_empty() {
        repair_targets.push("当前章节质量走势基本稳定，继续保持既有推进与兑现节拍。".to_string());
    }

    let mut preserve_strengths = strength_items
        .iter()
        .map(|item| item.descriptor.preserve_hint.to_string())
        .collect::<Vec<_>>();
    if preserve_strengths.is_empty() {
        preserve_strengths.push("保留当前已成立的章节优势与角色辨识度。".to_string());
    }

    let mut focus_areas = low_items
        .iter()
        .map(|item| item.descriptor.focus_area.to_string())
        .collect::<Vec<_>>();
    if focus_areas.is_empty() {
        focus_areas.push(weakest_metric.descriptor.focus_area.to_string());
    }

    let weakest_metric_label = weakest_metric.descriptor.label;
    let summary = if low_items.is_empty() {
        if stage_label.is_empty() {
            "当前章节质量走势基本稳定，可继续保持既有长板。".to_string()
        } else {
            format!("当前章节在{stage_label}阶段整体稳定，可继续保持既有长板。")
        }
    } else if stage_label.is_empty() {
        format!("当前章节需优先补强{weakest_metric_label}，并同步修复近期暴露的短板。")
    } else {
        format!("当前章节在{stage_label}阶段需优先补强{weakest_metric_label}，并同步修复近期暴露的短板。")
    };

    json!({
        "summary": summary,
        "repair_targets": repair_targets,
        "preserve_strengths": preserve_strengths,
        "focus_areas": focus_areas,
        "weakest_metric_key": weakest_metric.descriptor.focus_area,
        "weakest_metric_label": weakest_metric.descriptor.label,
        "weakest_metric_value": round_quality_metric(weakest_metric.value),
        "quality_stage": stage.unwrap_or_default(),
        "quality_stage_label": stage_label,
        "adaptive_quality_profile": adaptive_quality_profile,
        "quality_runtime_pressure": quality_runtime_pressure,
    })
}

fn derive_quality_gate_from_metrics_object(
    metrics: &serde_json::Map<String, Value>,
    scope: &str,
) -> Value {
    let runtime_context =
        extract_quality_runtime_context_object(Some(&Value::Object(metrics.clone())));
    let stage = resolve_quality_stage(runtime_context.as_ref()).unwrap_or_default();
    let stage_label = quality_stage_label(&stage).to_string();
    let adaptive_profile =
        resolve_adaptive_quality_gate_profile(runtime_context.as_ref(), Some(stage.as_str()));
    let pressure = build_quality_runtime_pressure(runtime_context.as_ref());
    let overall_score = metric_value_as_f64(metrics, "overall_score");
    let metric_signals = collect_quality_metric_signals(metrics, runtime_context.as_ref());
    let low_items = metric_signals
        .iter()
        .filter(|item| item.value < item.weak_threshold)
        .copied()
        .collect::<Vec<_>>();
    let weakest_metric = low_items
        .iter()
        .min_by(|left, right| left.normalized_value.total_cmp(&right.normalized_value))
        .copied()
        .or_else(|| {
            metric_signals
                .iter()
                .min_by(|left, right| left.normalized_value.total_cmp(&right.normalized_value))
                .copied()
        });
    let weak_metric_count = low_items.len() as i64;
    let failed_metrics = low_items
        .iter()
        .map(|item| {
            json!({
                "key": item.descriptor.metric_key,
                "label": item.descriptor.label,
                "value": round_quality_metric(item.value),
                "threshold": item.weak_threshold,
                "gap": round_quality_metric((item.weak_threshold - item.value).max(0.0)),
                "focus_area": item.descriptor.focus_area,
                "repair_target": item.descriptor.repair_target,
            })
        })
        .collect::<Vec<_>>();
    let focus_areas = low_items
        .iter()
        .map(|item| item.descriptor.focus_area.to_string())
        .collect::<Vec<_>>();
    let repair_targets = low_items
        .iter()
        .map(|item| item.descriptor.repair_target.to_string())
        .collect::<Vec<_>>();
    let scope_label = if scope == "batch" {
        "最近章节"
    } else {
        "当前章节"
    };

    let (status, decision, label, reason, summary) = match (overall_score, weak_metric_count) {
        (Some(score), count) if score < 68.0 || count >= 4 => {
            let summary = if stage_label.is_empty() {
                format!("{scope_label}质量短板较明显，建议按修复指引补强后继续。")
            } else {
                format!(
                    "{scope_label}在{stage_label}阶段质量短板较明显，建议按修复指引补强后继续。"
                )
            };
            (
                "repairable",
                "auto_repair",
                "需修复",
                format!("总分 {:.1} 或弱项数量已触发强化修复阈值", score),
                summary,
            )
        }
        (Some(score), count) if score < 80.0 || count > 1 => {
            let summary = if stage_label.is_empty() {
                format!("{scope_label}仍有明显短板，建议先按修复指引补强后再保存。")
            } else {
                format!(
                    "{scope_label}在{stage_label}阶段仍有明显短板，建议先按修复指引补强后再保存。"
                )
            };
            (
                "repairable",
                "auto_repair",
                "可修复",
                if count > 0 {
                    format!("存在 {count} 个待修复弱项")
                } else {
                    "综合分未达直接保存阈值".to_string()
                },
                summary,
            )
        }
        _ => {
            let summary = if stage_label.is_empty() {
                format!("{scope_label}已通过质量闸门，可继续保存或进入下一步。")
            } else {
                format!("{scope_label}在{stage_label}阶段通过质量闸门，可继续保存或进入下一步。")
            };
            (
                "pass",
                "allow_save",
                "可保存",
                "质量指标达到保存要求".to_string(),
                summary,
            )
        }
    };

    json!({
        "status": status,
        "decision": decision,
        "label": label,
        "summary": summary,
        "reason": reason,
        "overall_score": overall_score.map(Value::from).unwrap_or(Value::Null),
        "weak_metric_count": weak_metric_count,
        "failed_metrics": failed_metrics,
        "focus_areas": focus_areas,
        "repair_targets": repair_targets,
        "allow_save": status == "pass",
        "can_auto_repair": status == "repairable",
        "requires_manual_review": false,
        "weakest_metric_key": weakest_metric
            .map(|item| Value::String(item.descriptor.focus_area.to_string()))
            .unwrap_or(Value::Null),
        "weakest_metric_label": weakest_metric
            .map(|item| Value::String(item.descriptor.label.to_string()))
            .unwrap_or(Value::Null),
        "weakest_metric_value": weakest_metric
            .map(|item| Value::from(round_quality_metric(item.value)))
            .unwrap_or(Value::Null),
        "quality_stage": stage,
        "quality_stage_label": stage_label,
        "adaptive_quality_profile": adaptive_profile,
        "quality_runtime_pressure": pressure,
    })
}

pub(crate) fn normalize_quality_metrics_history_item(value: &Value, scope: &str) -> Option<Value> {
    let mut normalized = value.as_object()?.clone();
    if !has_quality_metric_signal(&normalized) {
        return Some(Value::Object(normalized));
    }

    if normalized
        .get("repair_guidance")
        .is_none_or(|entry| !entry.is_object())
    {
        normalized.insert(
            "repair_guidance".to_string(),
            derive_story_repair_guidance_from_metrics_object(&normalized, scope),
        );
    }

    if normalized
        .get("quality_gate")
        .is_none_or(|entry| !entry.is_object())
    {
        normalized.insert(
            "quality_gate".to_string(),
            derive_quality_gate_from_metrics_object(&normalized, scope),
        );
    }

    Some(Value::Object(normalized))
}

fn average_quality_values(values: &[f64]) -> Option<f64> {
    (!values.is_empty())
        .then(|| round_quality_metric(values.iter().sum::<f64>() / values.len() as f64))
}

fn extract_recent_metric_average(history: &[Value], metric_keys: &[&str]) -> Option<f64> {
    let values = history
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|item| {
            let current_values = metric_keys
                .iter()
                .filter_map(|metric_key| item.get(*metric_key).and_then(Value::as_f64))
                .collect::<Vec<_>>();
            (!current_values.is_empty())
                .then_some(current_values.iter().sum::<f64>() / current_values.len() as f64)
        })
        .collect::<Vec<_>>();
    average_quality_values(&values)
}

fn build_pacing_imbalance_summary(history: &[Value]) -> Option<Value> {
    let recent_history = history
        .iter()
        .filter_map(Value::as_object)
        .cloned()
        .collect::<Vec<_>>();
    if recent_history.len() < 2 {
        return None;
    }

    let recent_history_values = recent_history
        .iter()
        .cloned()
        .map(Value::Object)
        .collect::<Vec<_>>();
    let recent_progression_density = extract_recent_metric_average(
        &recent_history_values,
        &[
            "conflict_chain_hit_rate",
            "outline_alignment_rate",
            "payoff_chain_rate",
        ],
    );
    let recent_payoff_momentum = extract_recent_metric_average(
        &recent_history_values,
        &["payoff_chain_rate", "cliffhanger_rate"],
    );
    let recent_payoff_rate = average_quality_values(
        &recent_history
            .iter()
            .filter_map(|item| item.get("payoff_chain_rate").and_then(Value::as_f64))
            .collect::<Vec<_>>(),
    );
    let recent_cliffhanger_pull = average_quality_values(
        &recent_history
            .iter()
            .filter_map(|item| item.get("cliffhanger_rate").and_then(Value::as_f64))
            .collect::<Vec<_>>(),
    );

    let mut tension_variation_samples: Vec<f64> = Vec::new();
    let mut previous_overall_score: Option<f64> = None;
    let mut previous_cliffhanger_rate: Option<f64> = None;
    for item in &recent_history {
        let overall_score = item.get("overall_score").and_then(Value::as_f64);
        let cliffhanger_rate = item.get("cliffhanger_rate").and_then(Value::as_f64);
        if let (Some(previous), Some(current)) = (previous_overall_score, overall_score) {
            tension_variation_samples.push((current - previous).abs());
        }
        if let (Some(previous), Some(current)) = (previous_cliffhanger_rate, cliffhanger_rate) {
            tension_variation_samples.push((current - previous).abs());
        }
        if overall_score.is_some() {
            previous_overall_score = overall_score;
        }
        if cliffhanger_rate.is_some() {
            previous_cliffhanger_rate = cliffhanger_rate;
        }
    }
    let recent_tension_variation = average_quality_values(&tension_variation_samples);

    if recent_progression_density.is_none()
        && recent_payoff_momentum.is_none()
        && recent_tension_variation.is_none()
    {
        return None;
    }

    let mut signals = Vec::new();
    let mut focus_areas = Vec::new();
    let mut repair_targets = Vec::new();
    let mut status = "stable";

    let mut push_signal = |key: &str,
                           label: &str,
                           severity: &str,
                           summary: &str,
                           metric: Option<f64>,
                           current_focus_areas: &[&str],
                           current_repair_targets: &[&str]| {
        signals.push(json!({
            "key": key,
            "label": label,
            "severity": severity,
            "summary": summary,
            "metric": metric.map(Value::from).unwrap_or(Value::Null),
        }));
        focus_areas.extend(current_focus_areas.iter().map(|value| value.to_string()));
        repair_targets.extend(current_repair_targets.iter().map(|value| value.to_string()));
        if severity == "warning" {
            status = "warning";
        } else if severity == "watch" && status == "stable" {
            status = "watch";
        }
    };

    if recent_progression_density.is_some_and(|value| value < 68.0)
        && recent_tension_variation.is_some_and(|value| value < 6.5)
    {
        push_signal(
            "middle_drag",
            "中段拖滞",
            if recent_progression_density.is_some_and(|value| value < 64.0) {
                "warning"
            } else {
                "watch"
            },
            "最近数章推进密度与张力波动都偏低，容易出现连续铺陈但有效事件不足。",
            recent_progression_density,
            &["conflict", "outline", "pacing"],
            &[
                "本章至少推进 1 个主线矛盾，并写出新的代价、反制或局势变化。",
                "把当前章节的大纲任务拆成可见动作，不要只做解释性铺陈。",
            ],
        );
    }

    if recent_cliffhanger_pull.is_some_and(|value| value >= 80.0)
        && recent_payoff_rate.is_some_and(|value| value < 70.0)
    {
        push_signal(
            "overstretched_suspense",
            "悬念透支",
            if recent_payoff_rate.is_some_and(|value| value < 66.0) {
                "warning"
            } else {
                "watch"
            },
            "章尾牵引持续偏强，但兑现率偏低，容易形成只吊胃口、不回收承诺的拖尾。",
            recent_payoff_rate,
            &["payoff", "cliffhanger"],
            &[
                "本章必须回收至少 1 个既有伏笔、承诺或情绪账。",
                "新增悬念前，先让已有悬念落地成结果、损失或关系变化。",
            ],
        );
    }

    if recent_payoff_rate.is_some_and(|value| value < 66.0) {
        push_signal(
            "payoff_fatigue",
            "回报疲劳",
            if recent_payoff_rate.is_some_and(|value| value < 62.0) {
                "warning"
            } else {
                "watch"
            },
            "最近几章兑现动作持续偏弱，读者获得感和阶段闭环不足。",
            recent_payoff_rate,
            &["payoff", "pacing"],
            &["让本章出现一个阶段性结果、关系改写或资源转移，形成明确小闭环。"],
        );
    }

    if recent_tension_variation.is_some_and(|value| value > 16.0) {
        push_signal(
            "rhythm_whiplash",
            "节奏摆荡",
            if recent_tension_variation.is_some_and(|value| value > 20.0) {
                "warning"
            } else {
                "watch"
            },
            "最近张力波动过大，容易出现忽强忽弱、节拍断裂的阅读体验。",
            recent_tension_variation,
            &["pacing"],
            &["把本章张力曲线收束为“目标—受阻—反制—余波”，避免无序跳档。"],
        );
    }

    let leading_labels = signals
        .iter()
        .take(2)
        .filter_map(|signal| signal.get("label"))
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    let summary = if leading_labels.is_empty() {
        "最近数章推进密度、兑现节拍与张力波动整体可控，可继续维持当前节奏并放大优势。".to_string()
    } else {
        format!(
            "最近 {} 章出现{}风险，需优先修复推进密度、兑现节拍与张力接力。",
            recent_history.len(),
            leading_labels.join("、")
        )
    };

    Some(json!({
        "status": status,
        "window_size": recent_history.len(),
        "signal_count": signals.len(),
        "recent_progression_density": recent_progression_density.map(Value::from).unwrap_or(Value::Null),
        "recent_payoff_momentum": recent_payoff_momentum.map(Value::from).unwrap_or(Value::Null),
        "recent_payoff_rate": recent_payoff_rate.map(Value::from).unwrap_or(Value::Null),
        "recent_cliffhanger_pull": recent_cliffhanger_pull.map(Value::from).unwrap_or(Value::Null),
        "recent_tension_variation": recent_tension_variation.map(Value::from).unwrap_or(Value::Null),
        "signals": signals.into_iter().take(4).collect::<Vec<_>>(),
        "focus_areas": normalize_guidance_items(
            &focus_areas.into_iter().map(Value::String).collect::<Vec<_>>(),
            4,
        ),
        "repair_targets": normalize_guidance_items(
            &repair_targets.into_iter().map(Value::String).collect::<Vec<_>>(),
            4,
        ),
        "summary": summary,
    }))
}

fn aggregate_quality_runtime_context(history: &[Value], scope: &str) -> Value {
    let latest_context = history
        .iter()
        .rev()
        .find_map(|item| extract_quality_runtime_context_object(Some(item)))
        .unwrap_or_default();
    let recent_metrics = history
        .iter()
        .enumerate()
        .rev()
        .map(|(index, item)| {
            json!({
                "history_index": index,
                "overall_score": item.get("overall_score").cloned().unwrap_or(Value::Null),
                "repair_guidance": item.get("repair_guidance").cloned().unwrap_or(Value::Null),
                "quality_gate": item.get("quality_gate").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();

    let mut context = latest_context;
    context.insert("scope".to_string(), json!(scope));
    context.insert("recent_metrics".to_string(), Value::Array(recent_metrics));
    Value::Object(context)
}

fn build_volume_goal_completion_summary(summary: &Value) -> Option<Value> {
    let summary_object = summary.as_object()?;
    let runtime_context = summary_object
        .get("quality_runtime_context")
        .and_then(Value::as_object)?;
    let expected_stage = resolve_quality_stage(Some(runtime_context))?;
    let weight_profile =
        resolve_quality_weight_profile(Some(runtime_context), Some(expected_stage.as_str()));
    let current_stage = runtime_context
        .get("plot_stage")
        .or_else(|| runtime_context.get("quality_stage"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| matches!(*value, "opening" | "development" | "ending"))
        .unwrap_or(expected_stage.as_str())
        .to_string();

    let (metric_keys, stage_goal, default_targets, profile_summary) = match expected_stage.as_str()
    {
        "opening" => (
            vec![
                "avg_opening_hook_rate",
                "avg_outline_alignment_rate",
                "avg_conflict_chain_hit_rate",
            ],
            "开篇阶段需要把主目标、异常与初始阻力快速立起来。",
            vec![
                "尽快抛出主线目标或异常，不要用整章解释背景。",
                "让主角在本章就遭遇第一次明确受阻或代价。",
            ],
            "当前按开篇阶段权重评估卷级目标完成度。",
        ),
        "ending" => (
            vec![
                "avg_payoff_chain_rate",
                "avg_outline_alignment_rate",
                "avg_cliffhanger_rate",
                "avg_conflict_chain_hit_rate",
            ],
            "收束阶段需要完成阶段兑现、冲突回收与下一步牵引。",
            vec![
                "优先回收已经承诺的结果、伏笔或关系变化，不要继续横向开新坑。",
                "让阶段冲突形成结果、损失或站队变化，并保留下一步牵引。",
            ],
            "当前按收束阶段权重评估卷级目标完成度。",
        ),
        _ => (
            vec![
                "avg_conflict_chain_hit_rate",
                "avg_outline_alignment_rate",
                "avg_pacing_score",
                "avg_payoff_chain_rate",
            ],
            "发展阶段需要把卷内任务拆成可见动作、反制和局势位移。",
            vec![
                "把当前卷的阶段目标拆成可见动作，不要只做解释性铺陈。",
                "至少推进一条主线矛盾，并让角色因此付出新代价。",
            ],
            "当前按发展阶段权重评估卷级目标完成度。",
        ),
    };

    let metric_values = metric_keys
        .iter()
        .filter_map(|metric_key| {
            let value = summary_object.get(*metric_key).and_then(Value::as_f64)?;
            Some(if *metric_key == "avg_pacing_score" {
                value * 10.0
            } else {
                value
            })
        })
        .collect::<Vec<_>>();
    if metric_values.is_empty() {
        return None;
    }

    let completion_rate =
        round_quality_metric(metric_values.iter().sum::<f64>() / metric_values.len() as f64);
    let current_label = quality_stage_label(&current_stage);
    let expected_label = quality_stage_label(&expected_stage);
    let stage_alignment = if current_stage == expected_stage {
        100.0
    } else {
        65.0
    };
    let status = if completion_rate < 68.0 {
        "warning"
    } else if completion_rate < 78.0 {
        "watch"
    } else {
        "stable"
    };
    let summary_text = if current_stage == expected_stage {
        format!("卷级目标达成率约 {completion_rate:.1}%，{stage_goal}")
    } else {
        format!(
            "卷级目标达成率约 {completion_rate:.1}%，按章节进度应处于{expected_label}，但当前质量信号更接近{current_label}。"
        )
    };

    Some(json!({
        "status": status,
        "completion_rate": completion_rate,
        "expected_stage": expected_stage,
        "expected_stage_label": expected_label,
        "current_stage": current_stage,
        "current_stage_label": current_label,
        "stage_alignment": stage_alignment,
        "summary": summary_text,
        "focus_areas": [],
        "repair_targets": default_targets,
        "profile_summary": profile_summary,
        "profile_focuses": weight_profile
            .get("focus_areas")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "style_profile": weight_profile
            .get("style_profile")
            .cloned()
            .unwrap_or_else(|| json!("")),
        "genre_profiles": weight_profile
            .get("genre_profiles")
            .cloned()
            .unwrap_or_else(|| json!([])),
        "quality_preset": weight_profile
            .get("quality_preset")
            .cloned()
            .unwrap_or_else(|| json!("")),
        "quality_weight_profile": weight_profile,
    }))
}

fn build_foreshadow_payoff_delay_summary(summary: &Value) -> Option<Value> {
    let summary_object = summary.as_object()?;
    let runtime_context = summary_object
        .get("quality_runtime_context")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let foreshadow_payoff_plan =
        normalize_runtime_context_item_texts(runtime_context.get("foreshadow_payoff_plan"), 6);
    let foreshadow_state_ledger =
        normalize_runtime_context_item_texts(runtime_context.get("foreshadow_state_ledger"), 6);
    let recent_payoff_rate = summary_object
        .get("avg_payoff_chain_rate")
        .and_then(Value::as_f64);
    let recent_payoff_momentum = summary_object
        .get("pacing_imbalance")
        .and_then(|payload| payload.get("recent_payoff_momentum"))
        .and_then(Value::as_f64);

    if foreshadow_payoff_plan.is_empty()
        && foreshadow_state_ledger.is_empty()
        && recent_payoff_rate.is_none()
    {
        return None;
    }

    let outstanding_count = foreshadow_payoff_plan
        .len()
        .max(foreshadow_state_ledger.len()) as f64;
    let progress_ratio = match (
        runtime_context
            .get("current_chapter_number")
            .and_then(Value::as_f64),
        runtime_context.get("chapter_count").and_then(Value::as_f64),
    ) {
        (Some(current), Some(total)) if total > 0.0 => Some(current / total),
        _ => None,
    };
    let backlog_pressure = (outstanding_count * 18.0).min(100.0);
    let payoff_gap = recent_payoff_rate
        .map(|value| (78.0 - value).max(0.0))
        .unwrap_or_else(|| if outstanding_count > 0.0 { 18.0 } else { 0.0 });
    let momentum_gap = recent_payoff_momentum
        .map(|value| (76.0 - value).max(0.0))
        .unwrap_or_else(|| if outstanding_count > 1.0 { 10.0 } else { 0.0 });
    let progress_multiplier = if progress_ratio.is_some_and(|value| value >= 0.75) {
        1.15
    } else if progress_ratio.is_some_and(|value| value >= 0.55) {
        1.05
    } else {
        1.0
    };
    let delay_index = round_quality_metric(
        (backlog_pressure * 0.45 + payoff_gap * 0.35 + momentum_gap * 0.20) * progress_multiplier,
    )
    .min(100.0);

    let status = if delay_index >= 55.0
        || (progress_ratio.unwrap_or_default() >= 0.7 && outstanding_count >= 3.0)
    {
        "warning"
    } else if delay_index >= 35.0 || outstanding_count >= 2.0 {
        "watch"
    } else {
        "stable"
    };

    let mut repair_targets = Vec::new();
    if !foreshadow_payoff_plan.is_empty() {
        repair_targets.push(format!(
            "优先兑现伏笔计划中的至少 1 条：{}。",
            foreshadow_payoff_plan
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join(" / ")
        ));
    }
    if outstanding_count >= 3.0 {
        repair_targets.push("减少新增悬念，把已有伏笔写成结果、损失或信息揭示。".to_string());
    }
    if progress_ratio.unwrap_or_default() >= 0.72 {
        repair_targets
            .push("临近收束阶段，未兑现伏笔必须与主线结果绑定，避免尾部堆积。".to_string());
    }
    if repair_targets.is_empty() && recent_payoff_rate.is_some_and(|value| value < 72.0) {
        repair_targets
            .push("本章至少回收一个既有伏笔、承诺或情绪账，避免继续透支悬念。".to_string());
    }

    let summary_text = if foreshadow_state_ledger.is_empty() {
        format!(
            "伏笔兑现延迟指数 {delay_index:.1}，当前仍有 {} 项伏笔/承诺需要清偿。",
            outstanding_count as i64
        )
    } else {
        format!(
            "伏笔兑现延迟指数 {delay_index:.1}，待清偿重点包括 {}。",
            foreshadow_state_ledger
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join(" / ")
        )
    };

    Some(json!({
        "status": status,
        "delay_index": delay_index,
        "plan_count": foreshadow_payoff_plan.len(),
        "backlog_count": foreshadow_state_ledger.len(),
        "recent_payoff_rate": recent_payoff_rate.map(Value::from).unwrap_or(Value::Null),
        "recent_payoff_momentum": recent_payoff_momentum.map(Value::from).unwrap_or(Value::Null),
        "summary": summary_text,
        "focus_areas": if progress_ratio.unwrap_or_default() >= 0.72 {
            json!(["payoff", "cliffhanger", "outline"])
        } else {
            json!(["payoff", "cliffhanger"])
        },
        "repair_targets": repair_targets,
    }))
}

fn build_repair_effectiveness_summary(history: &[Value], scope: &str) -> Option<Value> {
    if history.len() < 2 {
        return None;
    }

    let normalized_history = history
        .iter()
        .filter_map(|item| normalize_quality_metrics_history_item(item, scope))
        .collect::<Vec<_>>();
    if normalized_history.len() < 2 {
        return None;
    }

    let mut evaluated_pairs = 0_i64;
    let mut successful_pairs = 0_i64;
    let mut focus_area_state = serde_json::Map::new();

    for window in normalized_history.windows(2) {
        let current_item = window[0]
            .as_object()
            .expect("normalized history should be objects");
        let next_item = window[1]
            .as_object()
            .expect("normalized history should be objects");
        let focus_areas = current_item
            .get("repair_guidance")
            .and_then(Value::as_object)
            .and_then(|guidance| guidance.get("focus_areas"))
            .and_then(Value::as_array)
            .map(|areas| normalize_guidance_items(areas, 4))
            .unwrap_or_default();

        let mut pair_evaluations = Vec::new();
        for focus_area in focus_areas {
            let Some((metric_key, safe_threshold, improvement_threshold)) =
                repair_effectiveness_metric_spec(&focus_area)
            else {
                continue;
            };
            let Some(current_value) = current_item.get(metric_key).and_then(Value::as_f64) else {
                continue;
            };
            let Some(next_value) = next_item.get(metric_key).and_then(Value::as_f64) else {
                continue;
            };

            let delta = round_quality_metric(next_value - current_value);
            let success = next_value >= current_value + improvement_threshold
                || (current_value < safe_threshold && next_value >= safe_threshold);
            pair_evaluations.push((focus_area.clone(), metric_key.to_string(), delta, success));

            let entry = focus_area_state
                .entry(focus_area.clone())
                .or_insert_with(|| {
                    json!({
                        "focus_area": focus_area,
                        "label": focus_area_label(&focus_area),
                        "metric_key": metric_key,
                        "evaluated_pairs": 0,
                        "successful_pairs": 0,
                        "delta_total": 0.0,
                    })
                });
            if let Some(entry_object) = entry.as_object_mut() {
                let current_pairs = entry_object
                    .get("evaluated_pairs")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let current_successful_pairs = entry_object
                    .get("successful_pairs")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let current_delta_total = entry_object
                    .get("delta_total")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0);
                entry_object.insert("evaluated_pairs".to_string(), json!(current_pairs + 1));
                entry_object.insert(
                    "successful_pairs".to_string(),
                    json!(current_successful_pairs + i64::from(success)),
                );
                entry_object.insert(
                    "delta_total".to_string(),
                    json!(round_quality_metric(current_delta_total + delta)),
                );
            }
        }

        if pair_evaluations.is_empty() {
            continue;
        }

        evaluated_pairs += 1;
        let pair_success_count = pair_evaluations
            .iter()
            .filter(|(_, _, _, success)| *success)
            .count() as i64;
        if pair_success_count >= ((pair_evaluations.len() as i64 + 1) / 2).max(1) {
            successful_pairs += 1;
        }
    }

    if evaluated_pairs <= 0 {
        return None;
    }

    let success_rate =
        round_quality_metric(successful_pairs as f64 / evaluated_pairs as f64 * 100.0);
    let mut focus_area_stats = focus_area_state
        .into_values()
        .filter_map(|state| {
            let state = state.as_object()?;
            let evaluated_pairs = state.get("evaluated_pairs").and_then(Value::as_i64).unwrap_or(0);
            if evaluated_pairs <= 0 {
                return None;
            }
            let successful_pairs = state
                .get("successful_pairs")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let delta_total = state.get("delta_total").and_then(Value::as_f64).unwrap_or(0.0);
            Some(json!({
                "focus_area": state.get("focus_area").cloned().unwrap_or(Value::Null),
                "label": state.get("label").cloned().unwrap_or(Value::Null),
                "metric_key": state.get("metric_key").cloned().unwrap_or(Value::Null),
                "evaluated_pairs": evaluated_pairs,
                "successful_pairs": successful_pairs,
                "success_rate": round_quality_metric(successful_pairs as f64 / evaluated_pairs as f64 * 100.0),
                "avg_delta": round_quality_metric(delta_total / evaluated_pairs as f64),
            }))
        })
        .collect::<Vec<_>>();
    focus_area_stats.sort_by(|left, right| {
        let left_success_rate = left
            .get("success_rate")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let right_success_rate = right
            .get("success_rate")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        left_success_rate
            .total_cmp(&right_success_rate)
            .then_with(|| {
                right
                    .get("evaluated_pairs")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
                    .cmp(
                        &left
                            .get("evaluated_pairs")
                            .and_then(Value::as_i64)
                            .unwrap_or(0),
                    )
            })
            .then_with(|| {
                left.get("label")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .cmp(
                        right
                            .get("label")
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    )
            })
    });

    let recovered_focus_areas = focus_area_stats
        .iter()
        .filter_map(|item| {
            let success_rate = item
                .get("success_rate")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let avg_delta = item.get("avg_delta").and_then(Value::as_f64).unwrap_or(0.0);
            (success_rate >= 60.0 && avg_delta > 0.0).then(|| {
                item.get("label")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            })
        })
        .take(3)
        .collect::<Vec<_>>();
    let unresolved_focus_areas = focus_area_stats
        .iter()
        .filter_map(|item| {
            let success_rate = item
                .get("success_rate")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            (success_rate < 50.0).then(|| {
                item.get("label")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            })
        })
        .take(3)
        .collect::<Vec<_>>();
    let summary_text = format!(
        "最近 {evaluated_pairs} 组相邻章节中，修复成效率约 {success_rate:.1}%。{}{}",
        if recovered_focus_areas.is_empty() {
            String::new()
        } else {
            format!(
                " 已开始回收：{}。",
                recovered_focus_areas
                    .iter()
                    .take(2)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" / ")
            )
        },
        if unresolved_focus_areas.is_empty() {
            String::new()
        } else {
            format!(
                " 仍需盯住：{}。",
                unresolved_focus_areas
                    .iter()
                    .take(2)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" / ")
            )
        }
    );
    let status = if success_rate < 40.0 {
        "warning"
    } else if success_rate < 65.0 {
        "watch"
    } else {
        "stable"
    };

    Some(json!({
        "status": status,
        "success_rate": success_rate,
        "evaluated_pairs": evaluated_pairs,
        "successful_pairs": successful_pairs,
        "recovered_focus_areas": recovered_focus_areas,
        "unresolved_focus_areas": unresolved_focus_areas,
        "focus_area_stats": focus_area_stats,
        "summary": summary_text,
    }))
}

fn insert_quality_summary_advanced_fields(
    payload: &mut serde_json::Map<String, Value>,
    history: &[Value],
    scope: &str,
) {
    let normalized_history = history
        .iter()
        .filter_map(|item| normalize_quality_metrics_history_item(item, scope))
        .collect::<Vec<_>>();
    if normalized_history.is_empty() {
        return;
    }

    payload.insert(
        "quality_runtime_context".to_string(),
        aggregate_quality_runtime_context(&normalized_history, scope),
    );

    if let Some(pacing_imbalance) = build_pacing_imbalance_summary(&normalized_history) {
        payload.insert("pacing_imbalance".to_string(), pacing_imbalance);
    }

    let summary_snapshot = Value::Object(payload.clone());
    if let Some(volume_goal_completion) = build_volume_goal_completion_summary(&summary_snapshot) {
        payload.insert("volume_goal_completion".to_string(), volume_goal_completion);
    }

    let summary_snapshot = Value::Object(payload.clone());
    if let Some(foreshadow_payoff_delay) = build_foreshadow_payoff_delay_summary(&summary_snapshot)
    {
        payload.insert(
            "foreshadow_payoff_delay".to_string(),
            foreshadow_payoff_delay,
        );
    }

    if let Some(repair_effectiveness) =
        build_repair_effectiveness_summary(&normalized_history, scope)
    {
        payload.insert("repair_effectiveness".to_string(), repair_effectiveness);
    }
}

pub(crate) fn build_quality_metrics_summary_state_from_history(
    history: &[Value],
    scope: &str,
) -> Option<Value> {
    let normalized_history = history
        .iter()
        .filter_map(|item| normalize_quality_metrics_history_item(item, scope))
        .collect::<Vec<_>>();
    if normalized_history.is_empty() {
        return None;
    }

    let first_overall_score = normalized_history
        .first()
        .and_then(|item| item.get("overall_score"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let last_overall_score = normalized_history
        .last()
        .and_then(|item| item.get("overall_score"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let recent_history = normalized_history
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    let mut state = serde_json::Map::from_iter([
        ("scope".to_string(), json!(scope)),
        (
            "chapter_count".to_string(),
            json!(normalized_history.len() as i64),
        ),
        (
            "first_overall_score".to_string(),
            json!(first_overall_score),
        ),
        ("last_overall_score".to_string(), json!(last_overall_score)),
        ("recent_history".to_string(), Value::Array(recent_history)),
        ("pacing_score_total".to_string(), json!(0.0)),
        ("pacing_score_count".to_string(), json!(0_i64)),
    ]);

    for (metric_key, _avg_key) in QUALITY_SUMMARY_METRIC_FIELDS {
        let total = normalized_history
            .iter()
            .filter_map(|item| item.get(metric_key).and_then(Value::as_f64))
            .sum::<f64>();
        state.insert(
            format!("{metric_key}_total"),
            json!(round_quality_metric(total)),
        );
    }

    let pacing_values = normalized_history
        .iter()
        .filter_map(|item| item.get("pacing_score").and_then(Value::as_f64))
        .collect::<Vec<_>>();
    if !pacing_values.is_empty() {
        state.insert(
            "pacing_score_total".to_string(),
            json!(round_quality_metric(pacing_values.iter().sum::<f64>())),
        );
        state.insert(
            "pacing_score_count".to_string(),
            json!(pacing_values.len() as i64),
        );
    }

    Some(Value::Object(state))
}

pub(crate) fn advance_quality_metrics_summary_state(
    summary_state: Option<&Value>,
    appended_event: &Value,
    current_history: &[Value],
    dropped_event: Option<&Value>,
    scope: &str,
) -> Option<Value> {
    if !appended_event.is_object() || current_history.is_empty() {
        return None;
    }
    let Some(existing_state) = summary_state.and_then(Value::as_object) else {
        return build_quality_metrics_summary_state_from_history(current_history, scope);
    };

    let chapter_count = current_history.len() as i64;
    let first_history_event = current_history
        .first()
        .cloned()
        .unwrap_or_else(|| appended_event.clone());
    let recent_history = current_history
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();

    let mut state = existing_state.clone();
    state.insert("scope".to_string(), json!(scope));
    state.insert("chapter_count".to_string(), json!(chapter_count));
    state.insert(
        "first_overall_score".to_string(),
        json!(first_history_event
            .get("overall_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)),
    );
    state.insert(
        "last_overall_score".to_string(),
        json!(appended_event
            .get("overall_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.0)),
    );
    state.insert("recent_history".to_string(), Value::Array(recent_history));

    for (metric_key, _avg_key) in QUALITY_SUMMARY_METRIC_FIELDS {
        let total_key = format!("{metric_key}_total");
        let appended = appended_event
            .get(metric_key)
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let dropped = dropped_event
            .and_then(|item| item.get(metric_key))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let current_total = state.get(&total_key).and_then(Value::as_f64).unwrap_or(0.0);
        state.insert(
            total_key,
            json!(round_quality_metric(current_total + appended - dropped)),
        );
    }

    let appended_pacing = appended_event.get("pacing_score").and_then(Value::as_f64);
    let dropped_pacing = dropped_event
        .and_then(|item| item.get("pacing_score"))
        .and_then(Value::as_f64);
    let current_pacing_total = state
        .get("pacing_score_total")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let current_pacing_count = state
        .get("pacing_score_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let next_pacing_total =
        current_pacing_total + appended_pacing.unwrap_or(0.0) - dropped_pacing.unwrap_or(0.0);
    let next_pacing_count = current_pacing_count + i64::from(appended_pacing.is_some())
        - i64::from(dropped_pacing.is_some());
    state.insert(
        "pacing_score_total".to_string(),
        json!(round_quality_metric(next_pacing_total)),
    );
    state.insert(
        "pacing_score_count".to_string(),
        json!(next_pacing_count.max(0)),
    );

    Some(Value::Object(state))
}

pub(crate) fn build_quality_metrics_summary_from_state(
    summary_state: Option<&Value>,
    fallback_history: &[Value],
    scope: &str,
) -> Option<Value> {
    let Some(state) = summary_state.and_then(Value::as_object) else {
        return aggregate_story_repair_quality_summaries(
            &fallback_history.iter().rev().cloned().collect::<Vec<_>>(),
            scope,
        );
    };

    let chapter_count = state
        .get("chapter_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if chapter_count <= 0 {
        return None;
    }

    let recent_history = state
        .get("recent_history")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| normalize_quality_metrics_history_item(item, scope))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut fallback_summary = aggregate_story_repair_quality_summaries(
        &fallback_history.iter().rev().cloned().collect::<Vec<_>>(),
        scope,
    )?;
    let fallback_object = fallback_summary.as_object_mut()?;

    fallback_object.insert("chapter_count".to_string(), json!(chapter_count));
    fallback_object.insert(
        "overall_score".to_string(),
        state
            .get("last_overall_score")
            .cloned()
            .unwrap_or(Value::Null),
    );

    let first_overall_score = state
        .get("first_overall_score")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let last_overall_score = state
        .get("last_overall_score")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let delta = if chapter_count > 1 {
        round_quality_metric(last_overall_score - first_overall_score)
    } else {
        0.0
    };
    fallback_object.insert("overall_score_delta".to_string(), json!(delta));
    fallback_object.insert(
        "overall_score_trend".to_string(),
        json!(if delta >= 2.0 {
            "rising"
        } else if delta <= -2.0 {
            "falling"
        } else {
            "stable"
        }),
    );

    for (metric_key, avg_key) in QUALITY_SUMMARY_METRIC_FIELDS {
        let avg = state
            .get(&format!("{metric_key}_total"))
            .and_then(Value::as_f64)
            .map(|total| round_quality_metric(total / chapter_count as f64))
            .unwrap_or(0.0);
        fallback_object.insert(avg_key.to_string(), json!(avg));
    }

    let avg_pacing_score = match (
        state.get("pacing_score_total").and_then(Value::as_f64),
        state.get("pacing_score_count").and_then(Value::as_i64),
    ) {
        (Some(total), Some(count)) if count > 0 => Some(round_quality_metric(total / count as f64)),
        _ => None,
    };
    fallback_object.insert(
        "avg_pacing_score".to_string(),
        avg_pacing_score.map(Value::from).unwrap_or(Value::Null),
    );
    insert_quality_summary_advanced_fields(fallback_object, &recent_history, scope);

    Some(Value::Object(fallback_object.clone()))
}

pub(crate) fn aggregate_story_repair_quality_summaries(
    summaries: &[Value],
    scope: &str,
) -> Option<Value> {
    let normalized_summaries = summaries
        .iter()
        .filter_map(|summary| normalize_quality_metrics_history_item(summary, scope))
        .collect::<Vec<_>>();
    if normalized_summaries.is_empty() {
        return None;
    }

    let chapter_count = normalized_summaries.len() as i64;
    let overall_scores = normalized_summaries
        .iter()
        .filter_map(|summary| summary.get("overall_score").and_then(Value::as_f64))
        .collect::<Vec<_>>();
    let latest_overall_score = overall_scores.first().copied();
    let oldest_overall_score = overall_scores.last().copied();
    let overall_score_delta = match (latest_overall_score, oldest_overall_score) {
        (Some(latest), Some(oldest)) if overall_scores.len() > 1 => {
            Some(round_quality_metric(latest - oldest))
        }
        _ => None,
    };
    let overall_score_trend = overall_score_delta.map(|delta| {
        if delta >= 2.0 {
            "rising"
        } else if delta <= -2.0 {
            "falling"
        } else {
            "stable"
        }
    });

    let merged_repair_targets = normalized_summaries
        .iter()
        .filter_map(|summary| extract_repair_guidance_object(Some(summary)))
        .filter_map(|guidance| guidance.get("repair_targets").cloned())
        .fold(Vec::new(), |merged, value| {
            merge_guidance_lists_with_limit(merged, Some(&value), 4)
        });
    let merged_preserve_strengths = normalized_summaries
        .iter()
        .filter_map(|summary| extract_repair_guidance_object(Some(summary)))
        .filter_map(|guidance| guidance.get("preserve_strengths").cloned())
        .fold(Vec::new(), |merged, value| {
            merge_guidance_lists_with_limit(merged, Some(&value), 2)
        });
    let merged_focus_areas = normalized_summaries
        .iter()
        .filter_map(|summary| extract_repair_guidance_object(Some(summary)))
        .filter_map(|guidance| guidance.get("focus_areas").cloned())
        .fold(Vec::new(), |merged, value| {
            merge_guidance_lists_with_limit(merged, Some(&value), 4)
        });
    let recent_focus_areas = merged_focus_areas.clone();

    let latest_guidance = normalized_summaries
        .iter()
        .find_map(|summary| extract_repair_guidance_object(Some(summary)));
    let latest_quality_gate = normalized_summaries
        .iter()
        .find_map(|summary| extract_quality_gate_object(Some(summary)));

    let repair_guidance = latest_guidance.map(|guidance| {
        let mut payload = serde_json::Map::new();
        payload.insert(
            "summary".to_string(),
            guidance.get("summary").cloned().unwrap_or(Value::Null),
        );
        payload.insert("repair_targets".to_string(), json!(merged_repair_targets));
        payload.insert(
            "preserve_strengths".to_string(),
            json!(merged_preserve_strengths),
        );
        payload.insert("focus_areas".to_string(), json!(merged_focus_areas));
        if let Some(value) = guidance.get("weakest_metric_key").cloned() {
            payload.insert("weakest_metric_key".to_string(), value);
        }
        if let Some(value) = guidance.get("weakest_metric_label").cloned() {
            payload.insert("weakest_metric_label".to_string(), value);
        }
        if let Some(value) = guidance.get("weakest_metric_value").cloned() {
            payload.insert("weakest_metric_value".to_string(), value);
        }
        if let Some(value) = guidance.get("quality_stage").cloned() {
            payload.insert("quality_stage".to_string(), value);
        }
        if let Some(value) = guidance.get("quality_stage_label").cloned() {
            payload.insert("quality_stage_label".to_string(), value);
        }
        if let Some(value) = guidance.get("quality_runtime_pressure").cloned() {
            payload.insert("quality_runtime_pressure".to_string(), value);
        }
        Value::Object(payload)
    });

    let mut failed_metric_counts = serde_json::Map::new();
    let mut quality_gate_counts = serde_json::Map::new();
    let mut recent_manual_review_count = 0_i64;
    let mut recent_auto_repair_count = 0_i64;

    for summary in &normalized_summaries {
        if let Some(quality_gate) = extract_quality_gate_object(Some(summary)) {
            if let Some(decision) = quality_gate
                .get("decision")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                increment_object_count(&mut quality_gate_counts, decision);
                match decision {
                    "manual_review" => recent_manual_review_count += 1,
                    "auto_repair" | "repair" => recent_auto_repair_count += 1,
                    _ => {}
                }
            }

            if let Some(items) = quality_gate.get("failed_metrics").and_then(Value::as_array) {
                for label in items
                    .iter()
                    .filter_map(Value::as_object)
                    .filter_map(|item| item.get("label"))
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    increment_object_count(&mut failed_metric_counts, label);
                }
            }
        }
    }

    let mut payload = serde_json::Map::new();
    payload.insert("chapter_count".to_string(), json!(chapter_count));
    payload.insert(
        "overall_score".to_string(),
        latest_overall_score.map(Value::from).unwrap_or(Value::Null),
    );
    if let Some(delta) = overall_score_delta {
        payload.insert("overall_score_delta".to_string(), json!(delta));
    }
    if let Some(trend) = overall_score_trend {
        payload.insert("overall_score_trend".to_string(), json!(trend));
    }
    payload.insert("recent_focus_areas".to_string(), json!(recent_focus_areas));
    payload.insert(
        "recent_failed_metric_counts".to_string(),
        Value::Object(failed_metric_counts),
    );
    payload.insert(
        "quality_gate_counts".to_string(),
        Value::Object(quality_gate_counts),
    );
    payload.insert(
        "recent_manual_review_count".to_string(),
        json!(recent_manual_review_count),
    );
    payload.insert(
        "recent_auto_repair_count".to_string(),
        json!(recent_auto_repair_count),
    );
    let chronological_history = normalized_summaries
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    insert_quality_summary_advanced_fields(&mut payload, &chronological_history, scope);
    if let Some(repair_guidance) = repair_guidance {
        payload.insert("repair_guidance".to_string(), repair_guidance);
    }
    if let Some(quality_gate) = latest_quality_gate {
        payload.insert("quality_gate".to_string(), quality_gate);
    }

    Some(Value::Object(payload))
}

pub(crate) fn quality_repair_guidance_from_quality_context(
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Option<serde_json::Map<String, Value>> {
    extract_repair_guidance_object(latest_quality_metrics)
        .or_else(|| extract_repair_guidance_object(quality_metrics_summary))
}

pub(crate) fn merged_story_repair_guidance_from_quality_context(
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Option<serde_json::Map<String, Value>> {
    let latest_guidance = extract_repair_guidance_object(latest_quality_metrics);
    let summary_guidance = extract_repair_guidance_object(quality_metrics_summary);

    match (latest_guidance, summary_guidance) {
        (None, None) => None,
        (Some(guidance), None) | (None, Some(guidance)) => Some(guidance),
        (Some(mut latest_guidance), Some(summary_guidance)) => {
            let repair_targets = merge_guidance_value_lists(
                latest_guidance.get("repair_targets"),
                summary_guidance.get("repair_targets"),
                4,
            );
            let preserve_strengths = merge_guidance_value_lists(
                latest_guidance.get("preserve_strengths"),
                summary_guidance.get("preserve_strengths"),
                2,
            );
            let focus_areas = merge_guidance_value_lists(
                latest_guidance.get("focus_areas"),
                summary_guidance.get("focus_areas"),
                4,
            );

            if !repair_targets.is_empty() {
                latest_guidance.insert("repair_targets".to_string(), json!(repair_targets));
            }
            if !preserve_strengths.is_empty() {
                latest_guidance.insert("preserve_strengths".to_string(), json!(preserve_strengths));
            }
            if !focus_areas.is_empty() {
                latest_guidance.insert("focus_areas".to_string(), json!(focus_areas));
            }

            Some(latest_guidance)
        }
    }
}

pub(crate) fn merged_quality_gate_from_quality_context(
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Option<Value> {
    let latest_quality_gate = extract_quality_gate_object(latest_quality_metrics);
    let summary_quality_gate = extract_quality_gate_object(quality_metrics_summary);

    match (latest_quality_gate, summary_quality_gate) {
        (None, None) => None,
        (Some(gate), None) | (None, Some(gate)) => Some(gate),
        (Some(mut latest_gate), Some(summary_gate)) => {
            let Some(latest_object) = latest_gate.as_object_mut() else {
                return Some(latest_gate);
            };
            let summary_object = summary_gate.as_object();

            for key in ["status", "decision", "label", "summary"] {
                if !latest_object.contains_key(key) {
                    if let Some(value) = summary_object.and_then(|gate| gate.get(key)).cloned() {
                        latest_object.insert(key.to_string(), value);
                    }
                }
            }

            let failed_metrics = merge_failed_quality_gate_metrics(
                latest_object.get("failed_metrics"),
                summary_object.and_then(|gate| gate.get("failed_metrics")),
            );
            if !failed_metrics.is_empty() {
                latest_object.insert("failed_metrics".to_string(), json!(failed_metrics));
            }

            Some(latest_gate)
        }
    }
}

pub(crate) fn reconciled_quality_gate_from_quality_context(
    quality_metrics_summary: Option<&Value>,
    latest_quality_metrics: Option<&Value>,
) -> Option<Value> {
    let mut quality_gate =
        merged_quality_gate_from_quality_context(quality_metrics_summary, latest_quality_metrics)?;
    let Some(_manual_review_label) = manual_review_label_from_quality_context(
        None,
        quality_metrics_summary,
        latest_quality_metrics,
    ) else {
        return Some(quality_gate);
    };

    let Some(gate_object) = quality_gate.as_object_mut() else {
        return Some(quality_gate);
    };

    gate_object.insert("status".to_string(), json!("warning"));
    gate_object.insert("decision".to_string(), json!("auto_repair"));
    gate_object.insert("label".to_string(), json!("建议继续修复"));

    Some(quality_gate)
}

pub(crate) fn extract_repair_guidance_object(
    value: Option<&Value>,
) -> Option<serde_json::Map<String, Value>> {
    let value = value?;
    if let Some(repair_guidance) = value.get("repair_guidance").and_then(Value::as_object) {
        return Some(repair_guidance.clone());
    }

    value
        .get("raw")
        .and_then(|raw| raw.get("repair_guidance"))
        .and_then(Value::as_object)
        .cloned()
}

pub(crate) fn extract_quality_gate_object(value: Option<&Value>) -> Option<Value> {
    let value = value?;
    if let Some(quality_gate) = value.get("quality_gate") {
        return Some(quality_gate.clone());
    }

    value
        .get("raw")
        .and_then(|raw| raw.get("quality_gate"))
        .cloned()
}

pub(crate) fn normalize_guidance_items(values: &[Value], limit: usize) -> Vec<String> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    for value in values {
        let Some(text) = value.as_str().map(str::trim) else {
            continue;
        };
        if text.is_empty() || !seen.insert(text.to_string()) {
            continue;
        }
        items.push(text.to_string());
        if items.len() >= limit {
            break;
        }
    }
    items
}

fn normalize_active_story_repair_payload(
    payload: &Value,
) -> Option<serde_json::Map<String, Value>> {
    let payload = payload.as_object()?;
    let summary = payload
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| json!(value));
    let repair_targets = payload
        .get("repair_targets")
        .and_then(Value::as_array)
        .map(|values| normalize_guidance_items(values, 4))
        .unwrap_or_default();
    let preserve_strengths = payload
        .get("preserve_strengths")
        .and_then(Value::as_array)
        .map(|values| normalize_guidance_items(values, 2))
        .unwrap_or_default();

    if summary.is_none() && repair_targets.is_empty() && preserve_strengths.is_empty() {
        return None;
    }

    let mut normalized = payload.clone();
    normalized.insert("summary".to_string(), summary.unwrap_or(Value::Null));
    normalized.insert("repair_targets".to_string(), json!(repair_targets));
    normalized.insert("preserve_strengths".to_string(), json!(preserve_strengths));
    Some(normalized)
}

fn normalize_active_story_repair_payload_value(payload: Option<&Value>) -> Option<Value> {
    payload
        .and_then(normalize_active_story_repair_payload)
        .map(Value::Object)
}

fn merge_guidance_value_lists(
    primary: Option<&Value>,
    fallback: Option<&Value>,
    limit: usize,
) -> Vec<String> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();
    for value in [primary, fallback].into_iter().flatten() {
        let Some(items) = value.as_array() else {
            continue;
        };
        for item in items {
            let Some(text) = item.as_str().map(str::trim) else {
                continue;
            };
            if text.is_empty() || !seen.insert(text.to_string()) {
                continue;
            }
            merged.push(text.to_string());
            if merged.len() >= limit {
                return merged;
            }
        }
    }
    merged
}

fn merge_failed_quality_gate_metrics(
    primary: Option<&Value>,
    fallback: Option<&Value>,
) -> Vec<String> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();

    for value in [primary, fallback].into_iter().flatten() {
        let Some(items) = value.as_array() else {
            continue;
        };
        for item in items {
            let label = item
                .as_object()
                .and_then(|entry| entry.get("label"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let Some(label) = label else {
                continue;
            };
            if seen.insert(label.to_string()) {
                merged.push(label.to_string());
            }
        }
    }

    merged
}

fn merge_guidance_lists_with_limit(
    mut existing: Vec<String>,
    additional: Option<&Value>,
    limit: usize,
) -> Vec<String> {
    if existing.len() >= limit {
        existing.truncate(limit);
        return existing;
    }

    let mut seen = existing.iter().cloned().collect::<HashSet<_>>();
    let Some(additional_items) = additional.and_then(Value::as_array) else {
        return existing;
    };

    for item in additional_items {
        let Some(text) = item.as_str().map(str::trim) else {
            continue;
        };
        if text.is_empty() || !seen.insert(text.to_string()) {
            continue;
        }
        existing.push(text.to_string());
        if existing.len() >= limit {
            break;
        }
    }

    existing
}

fn increment_object_count(target: &mut serde_json::Map<String, Value>, key: &str) {
    let current = target.get(key).and_then(Value::as_i64).unwrap_or(0);
    target.insert(key.to_string(), json!(current + 1));
}

fn round_quality_metric(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn value_or_fallback(primary: Option<&Value>, fallback: Option<&Value>) -> Option<Value> {
    primary
        .filter(|value| match value {
            Value::Null => false,
            Value::String(text) => !text.trim().is_empty(),
            Value::Array(items) => !items.is_empty(),
            Value::Object(items) => !items.is_empty(),
            _ => true,
        })
        .cloned()
        .or_else(|| fallback.cloned())
}

fn stamp_story_repair_payload(
    mut payload: serde_json::Map<String, Value>,
    source: &str,
    source_label: &str,
    scope: &str,
) -> Value {
    payload.insert("source".to_string(), json!(source));
    payload.insert("source_label".to_string(), json!(source_label));
    payload.insert("scope".to_string(), json!(scope));
    payload.insert("updated_at".to_string(), Value::Null);
    Value::Object(payload)
}

fn merged_story_repair_source_label(derived_source: &str) -> (&'static str, &'static str) {
    match derived_source {
        CURRENT_CHAPTER_QUALITY_SOURCE => (
            MANUAL_PLUS_CURRENT_CHAPTER_QUALITY_SOURCE,
            MANUAL_PLUS_CURRENT_CHAPTER_QUALITY_SOURCE_LABEL,
        ),
        _ => (
            MANUAL_PLUS_RECENT_HISTORY_SUMMARY_SOURCE,
            MANUAL_PLUS_RECENT_HISTORY_SUMMARY_SOURCE_LABEL,
        ),
    }
}

pub(crate) fn build_story_repair_quality_context_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_runtime_service::story_repair_quality_context_owner",
        "scope": "story_repair_payload_quality_context_resume_and_recent_history",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs",
            "backend-rs/src/services/chapter_generation_prompt_service/quality_profile_owner.rs",
            "backend-rs/src/services/chapter_single_generation_stream_workflow_service.rs",
            "backend-rs/src/services/chapter_single_generation_runtime_restore_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_write_workflow_service.rs",
            "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs",
            "backend-rs/src/services/chapter_batch_generation_resume_task_command_service.rs",
            "backend-rs/src/services/chapter_batch_generation_task_payload_base_service.rs",
            "backend-rs/src/services/chapter_query_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "explicit_request_precedence": "explicit story repair input keeps compat options unchanged",
            "active_snapshot_restore_fields": [
                "summary",
                "repair_targets",
                "preserve_strengths"
            ],
            "quality_context_restore_fields": [
                "repair_guidance",
                "quality_gate",
                "quality_runtime_context",
                "recent_history",
                "overall_score_trend"
            ],
            "active_payload_fields": [
                "summary",
                "repair_targets",
                "preserve_strengths",
                "focus_areas",
                "quality_gate_status",
                "quality_gate_decision",
                "quality_gate_label",
                "quality_gate_summary",
                "quality_gate_failed_metrics",
                "source",
                "source_label",
                "scope",
                "updated_at"
            ],
            "merge_limits": {
                "repair_targets": 4,
                "preserve_strengths": 2,
                "focus_areas": 4
            },
            "manual_plus_quality_source_labels": [
                "Manual + current chapter quality",
                "Manual + recent history summary"
            ],
            "resume_precedence": [
                "runtime_active_story_repair_payload",
                "quality_context_payload",
                "request_payload"
            ],
            "quality_gate_reconciliation": "summary terminal manual_review overrides latest auto_repair while preserving merged failed metrics",
            "recent_history_summary": "aggregate_story_repair_quality_summaries normalizes latest-first quality metrics into repair guidance, gate counts, trend, and runtime pressure"
            ,
            "shared_quality_profile_owner": [
                "resolve_quality_weight_profile",
                "resolve_adaptive_quality_gate_profile",
                "resolve_metric_threshold_adjustments"
            ]
        },
        "active_consumers": [
            "chapter_single_generation_stream_workflow_service",
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_batch_generation_write_workflow_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_batch_generation_resume_task_command_service",
            "chapter_batch_generation_task_payload_base_service",
            "chapter_query_service",
            "chapter-single-generation-active-gateway-smoke-rust",
            "chapter-batch-generation-active-gateway-smoke-rust"
        ],
        "validation_boundary": [
            "cargo test chapter_generation_runtime_service::story_repair_quality_context_owner",
            "cargo test chapter_single_generation_stream_workflow_service",
            "cargo test chapter_single_generation_runtime_restore_workflow_service",
            "cargo test chapter_batch_generation_write_workflow_service",
            "cargo test chapter_batch_generation_runtime_state_service",
            "cargo test chapter_batch_generation_resume_task_command_service",
            "cargo test api::health",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only",
            "cargo check"
        ],
        "rollback_boundary": {
            "source_map_policy": "production_python_quality_context_file_deleted_test_support_and_reference_contracts_only",
            "runtime_state_keys": [
                "active_story_repair_payload",
                "quality_metrics_summary",
                "latest_quality_metrics",
                "quality_runtime_context"
            ],
            "delete_or_freeze_requires": "same_round_rollback_policy_and_active_route_smoke"
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        aggregate_story_repair_quality_summaries,
        build_story_repair_quality_context_owner_contract, extract_quality_history_context,
        merge_active_story_repair_payloads, merged_quality_gate_from_quality_context,
        merged_story_repair_guidance_from_quality_context, normalize_quality_metrics_history_item,
        reconciled_quality_gate_from_quality_context,
        restore_active_story_repair_payload_from_quality_context,
    };

    #[test]
    fn should_merge_manual_and_recent_history_story_repair_payloads() {
        let explicit_payload = json!({
            "summary": "手工摘要",
            "repair_targets": ["手工目标", "共同目标"],
            "preserve_strengths": ["手工优点"],
            "focus_areas": ["手工焦点"]
        });
        let derived_payload = json!({
            "summary": "历史摘要",
            "repair_targets": ["共同目标", "历史目标"],
            "preserve_strengths": ["历史优点"],
            "focus_areas": ["历史焦点", "手工焦点"],
            "quality_gate_status": "warning",
            "quality_gate_summary": "近期质量波动",
            "source": "recent_history_summary",
            "source_label": "Recent history summary",
            "scope": "batch"
        });

        let merged = merge_active_story_repair_payloads(
            Some(&explicit_payload),
            Some(&derived_payload),
            "batch",
            "recent_history_summary",
            "Recent history summary",
        )
        .expect("merged payload");

        assert_eq!(merged["summary"], "手工摘要");
        assert_eq!(
            merged["repair_targets"],
            json!(["手工目标", "共同目标", "历史目标"])
        );
        assert_eq!(
            merged["preserve_strengths"],
            json!(["手工优点", "历史优点"])
        );
        assert_eq!(merged["focus_areas"], json!(["手工焦点", "历史焦点"]));
        assert_eq!(merged["quality_gate_status"], "warning");
        assert_eq!(merged["source"], "manual_plus_recent_history_summary");
        assert_eq!(merged["source_label"], "Manual + recent history summary");
        assert_eq!(merged["scope"], "batch");
    }

    #[test]
    fn should_merge_latest_and_summary_quality_guidance_with_latest_priority() {
        let latest = json!({
            "repair_guidance": {
                "summary": "来自 latest",
                "repair_targets": ["latest target"],
                "preserve_strengths": ["latest strength"],
                "focus_areas": ["latest focus"]
            }
        });
        let summary = json!({
            "repair_guidance": {
                "summary": "来自 summary",
                "repair_targets": ["summary target"],
                "preserve_strengths": ["summary strength"],
                "focus_areas": ["summary focus"]
            }
        });

        let guidance =
            merged_story_repair_guidance_from_quality_context(Some(&summary), Some(&latest))
                .expect("merged guidance");

        assert_eq!(guidance.get("summary"), Some(&json!("来自 latest")));
        assert_eq!(
            guidance.get("repair_targets"),
            Some(&json!(["latest target", "summary target"]))
        );
        assert_eq!(
            guidance.get("preserve_strengths"),
            Some(&json!(["latest strength", "summary strength"]))
        );
        assert_eq!(
            guidance.get("focus_areas"),
            Some(&json!(["latest focus", "summary focus"]))
        );
    }

    #[test]
    fn should_merge_latest_and_summary_quality_gate_with_latest_priority() {
        let latest = json!({
            "quality_gate": {
                "decision": "auto_repair",
                "label": "来自 latest",
                "failed_metrics": [{"label": "节奏"}]
            }
        });
        let summary = json!({
            "quality_gate": {
                "status": "failed",
                "decision": "manual_review",
                "label": "来自 summary",
                "summary": "建议继续修复",
                "failed_metrics": [{"label": "节奏"}, {"label": "信息密度"}]
            }
        });

        let gate = merged_quality_gate_from_quality_context(Some(&summary), Some(&latest))
            .expect("merged quality gate");

        assert_eq!(gate["status"], "failed");
        assert_eq!(gate["decision"], "auto_repair");
        assert_eq!(gate["label"], "来自 latest");
        assert_eq!(gate["summary"], "建议继续修复");
        assert_eq!(gate["failed_metrics"], json!(["节奏", "信息密度"]));
    }

    #[test]
    fn should_reconcile_quality_gate_to_auto_repair_when_summary_is_terminal() {
        let latest = json!({
            "quality_gate": {
                "decision": "auto_repair",
                "label": "来自 latest",
                "failed_metrics": [{"label": "节奏"}]
            }
        });
        let summary = json!({
            "quality_gate": {
                "status": "failed",
                "decision": "manual_review",
                "label": "来自 summary",
                "summary": "建议继续修复",
                "failed_metrics": [{"label": "信息密度"}]
            }
        });

        let gate = reconciled_quality_gate_from_quality_context(Some(&summary), Some(&latest))
            .expect("reconciled quality gate");

        assert_eq!(gate["status"], "warning");
        assert_eq!(gate["decision"], "auto_repair");
        assert_eq!(gate["label"], "建议继续修复");
        assert_eq!(gate["summary"], "建议继续修复");
        assert_eq!(gate["failed_metrics"], json!(["节奏", "信息密度"]));
    }

    #[test]
    fn should_restore_active_payload_with_reconciled_auto_repair_gate() {
        let latest = json!({
            "repair_guidance": {
                "summary": "先修正文节奏",
                "repair_targets": ["压缩说明段"],
                "preserve_strengths": ["人物口吻"]
            },
            "quality_gate": {
                "decision": "auto_repair",
                "label": "继续自动修复",
                "failed_metrics": [{"label": "节奏"}]
            }
        });
        let summary = json!({
            "quality_gate": {
                "status": "failed",
                "decision": "manual_review",
                "label": "自动修复预算已耗尽",
                "summary": "建议继续修复",
                "failed_metrics": [{"label": "信息密度"}]
            }
        });

        let payload = restore_active_story_repair_payload_from_quality_context(
            Some(&summary),
            Some(&latest),
            "batch",
            "recent_history_summary",
            "Recent history summary",
        )
        .expect("active story repair payload");

        assert_eq!(payload["quality_gate_status"], "warning");
        assert_eq!(payload["quality_gate_decision"], "auto_repair");
        assert_eq!(payload["quality_gate_label"], "建议继续修复");
        assert_eq!(
            payload["quality_gate_failed_metrics"],
            json!(["节奏", "信息密度"])
        );
    }

    #[test]
    fn should_extract_quality_history_context_from_summary() {
        let summary = json!({
            "summary": "ok",
            "quality_runtime_context": {
                "recent_metrics": [{"score": 91}],
                "trend": "up"
            }
        });

        let context = extract_quality_history_context(Some(&summary)).expect("history context");

        assert_eq!(
            context,
            json!({
                "recent_metrics": [{"score": 91}],
                "trend": "up"
            })
        );
    }

    #[test]
    fn should_surface_extended_runtime_pressure_ledgers_in_repair_guidance() {
        let metrics = json!({
            "overall_score": 86,
            "rule_grounding_hit_rate": 82,
            "outline_alignment_rate": 81,
            "dialogue_naturalness_rate": 80,
            "quality_runtime_context": {
                "character_state_ledger": [
                    {"label": "主角", "summary": "压制副作用"},
                    {"label": "盟友", "summary": "信任摇摆"},
                    {"label": "反派", "summary": "掌握证据"}
                ],
                "relationship_state_ledger": [
                    {"label": "主角/盟友", "summary": "暂时结盟"},
                    {"label": "主角/导师", "summary": "理念冲突"}
                ],
                "organization_state_ledger": [
                    {"label": "商会", "summary": "断供"},
                    {"label": "学院", "summary": "追责"}
                ],
                "career_state_ledger": [
                    {"label": "炼器师", "summary": "瓶颈"},
                    {"label": "调查员", "summary": "权限代价"}
                ]
            }
        });

        let normalized = normalize_quality_metrics_history_item(&metrics, "chapter")
            .expect("normalized metrics");

        assert_eq!(
            normalized["repair_guidance"]["quality_runtime_pressure"]["character_state_count"],
            3
        );
        assert_eq!(
            normalized["repair_guidance"]["quality_runtime_pressure"]["relationship_state_count"],
            2
        );
        assert_eq!(
            normalized["repair_guidance"]["quality_runtime_pressure"]["organization_state_count"],
            2
        );
        assert_eq!(
            normalized["repair_guidance"]["quality_runtime_pressure"]["career_state_count"],
            2
        );
        assert_eq!(
            normalized["repair_guidance"]["quality_runtime_pressure"]["organization_state_items"],
            json!(["断供 商会", "追责 学院"])
        );
    }

    #[test]
    fn should_apply_runtime_pressure_to_quality_gate_thresholds() {
        let metrics = json!({
            "overall_score": 86,
            "rule_grounding_hit_rate": 72.5,
            "conflict_chain_hit_rate": 72.5,
            "outline_alignment_rate": 72.5,
            "payoff_chain_rate": 72.5,
            "quality_runtime_context": {
                "organization_state_ledger": [
                    {"label": "商会", "summary": "断供"},
                    {"label": "学院", "summary": "追责"}
                ],
                "career_state_ledger": [
                    {"label": "炼器师", "summary": "瓶颈"},
                    {"label": "调查员", "summary": "权限代价"}
                ]
            }
        });

        let normalized = normalize_quality_metrics_history_item(&metrics, "chapter")
            .expect("normalized metrics");
        let failed_metrics = normalized["quality_gate"]["failed_metrics"]
            .as_array()
            .expect("failed metrics");

        assert!(failed_metrics.iter().any(|metric| {
            metric["key"] == "rule_grounding_hit_rate" && metric["threshold"] == 73.0
        }));
        assert!(failed_metrics
            .iter()
            .any(|metric| metric["key"] == "payoff_chain_rate" && metric["threshold"] == 73.0));
        assert_eq!(
            normalized["quality_gate"]["quality_runtime_pressure"]["career_state_count"],
            2
        );
    }

    #[test]
    fn should_ignore_inapplicable_quality_metrics_in_gate_and_guidance() {
        let metrics = json!({
            "overall_score": 66.1,
            "conflict_chain_hit_rate": 86.0,
            "rule_grounding_hit_rate": 84.0,
            "outline_alignment_rate": 0.0,
            "dialogue_naturalness_rate": 79.0,
            "opening_hook_rate": 75.0,
            "payoff_chain_rate": 74.0,
            "cliffhanger_rate": 72.0,
            "pacing_score": 7.1,
            "details": {
                "outline_alignment": {
                    "applicable": false
                }
            }
        });

        let normalized = normalize_quality_metrics_history_item(&metrics, "chapter")
            .expect("normalized metrics");
        let gate = normalized["quality_gate"]
            .as_object()
            .expect("quality gate");
        let guidance = normalized["repair_guidance"]
            .as_object()
            .expect("repair guidance");

        let failed_metrics = gate["failed_metrics"].as_array().expect("failed metrics");
        assert!(failed_metrics
            .iter()
            .all(|metric| metric["key"] != "outline_alignment_rate"));
        assert_ne!(gate["weakest_metric_key"], json!("outline"));
        assert_ne!(guidance["weakest_metric_key"], json!("outline"));
        assert!(!gate["focus_areas"]
            .as_array()
            .expect("focus areas")
            .iter()
            .any(|item| item == "outline"));
        assert!(!guidance["focus_areas"]
            .as_array()
            .expect("guidance focus areas")
            .iter()
            .any(|item| item == "outline"));
        assert!(!gate["repair_targets"]
            .as_array()
            .expect("repair targets")
            .iter()
            .any(|item| item.as_str().is_some_and(|text| text.contains("大纲"))));
    }

    #[test]
    fn should_keep_flat_metrics_when_details_are_missing() {
        let metrics = json!({
            "overall_score": 66.1,
            "conflict_chain_hit_rate": 71.5,
            "rule_grounding_hit_rate": 84.0,
            "outline_alignment_rate": 68.0,
            "dialogue_naturalness_rate": 79.0,
            "opening_hook_rate": 75.0,
            "payoff_chain_rate": 74.0,
            "cliffhanger_rate": 72.0,
            "pacing_score": 7.1
        });

        let normalized = normalize_quality_metrics_history_item(&metrics, "chapter")
            .expect("normalized metrics");
        let gate = normalized["quality_gate"]
            .as_object()
            .expect("quality gate");

        let failed_metrics = gate["failed_metrics"].as_array().expect("failed metrics");
        assert!(failed_metrics
            .iter()
            .any(|metric| metric["key"] == "outline_alignment_rate"));
        assert_eq!(gate["weakest_metric_key"], json!("outline"));
    }

    #[test]
    fn should_aggregate_recent_story_repair_quality_summaries() {
        let latest = json!({
            "overall_score": 88,
            "repair_guidance": {
                "summary": "优先压缩说明段落",
                "repair_targets": ["压缩说明", "提前冲突"],
                "preserve_strengths": ["尾章钩子"],
                "focus_areas": ["pacing", "conflict"],
                "weakest_metric_key": "pacing",
                "weakest_metric_label": "节奏",
                "weakest_metric_value": 0.61
            },
            "quality_gate": {
                "decision": "repair",
                "failed_metrics": [{"label": "Pacing"}]
            }
        });
        let previous = json!({
            "overall_score": 82,
            "repair_guidance": {
                "summary": "补强角色动机",
                "repair_targets": ["强化动机", "提前冲突"],
                "preserve_strengths": ["人物口吻"],
                "focus_areas": ["character", "pacing"]
            },
            "quality_gate": {
                "decision": "manual_review",
                "failed_metrics": [{"label": "Character"}]
            }
        });

        let aggregated =
            aggregate_story_repair_quality_summaries(&[latest.clone(), previous.clone()], "batch")
                .expect("aggregated summary");

        assert_eq!(aggregated["chapter_count"], 2);
        assert_eq!(aggregated["overall_score"], 88.0);
        assert_eq!(aggregated["overall_score_delta"], 6.0);
        assert_eq!(aggregated["overall_score_trend"], "rising");
        assert_eq!(
            aggregated["repair_guidance"]["repair_targets"],
            json!(["压缩说明", "提前冲突", "强化动机"])
        );
        assert_eq!(
            aggregated["repair_guidance"]["preserve_strengths"],
            json!(["尾章钩子", "人物口吻"])
        );
        assert_eq!(
            aggregated["repair_guidance"]["focus_areas"],
            json!(["pacing", "conflict", "character"])
        );
        assert_eq!(aggregated["quality_gate_counts"]["repair"], 1);
        assert_eq!(aggregated["quality_gate_counts"]["manual_review"], 1);
        assert_eq!(aggregated["recent_manual_review_count"], 1);
        assert_eq!(aggregated["recent_auto_repair_count"], 1);
        assert_eq!(aggregated["recent_failed_metric_counts"]["Pacing"], 1);
        assert_eq!(aggregated["recent_failed_metric_counts"]["Character"], 1);
        assert_eq!(
            aggregated["quality_runtime_context"]["recent_metrics"]
                .as_array()
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(
            aggregated["quality_runtime_context"]["recent_metrics"][0]["overall_score"],
            88
        );
        assert_eq!(
            aggregated["quality_runtime_context"]["recent_metrics"][1]["overall_score"],
            82
        );
    }

    #[test]
    fn should_describe_story_repair_quality_context_owner_boundary() {
        let contract = build_story_repair_quality_context_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_generation_runtime_service::story_repair_quality_context_owner"
        );
        assert_eq!(
            contract["scope"],
            "story_repair_payload_quality_context_resume_and_recent_history"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .map(|items| items.len()),
            Some(0)
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["active_payload_fields"][8],
            "quality_gate_failed_metrics"
        );
        assert_eq!(
            contract["behavior_contract"]["merge_limits"]["repair_targets"],
            4
        );
        assert_eq!(
            contract["behavior_contract"]["resume_precedence"][0],
            "runtime_active_story_repair_payload"
        );
        assert_eq!(
            contract["rust_owner_map"][9],
            "backend-rs/src/api/health.rs"
        );
        assert_eq!(
            contract["active_consumers"][7],
            "chapter-single-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["active_consumers"][8],
            "chapter-batch-generation-active-gateway-smoke-rust"
        );
        assert_eq!(
            contract["rollback_boundary"]["runtime_state_keys"][0],
            "active_story_repair_payload"
        );
        assert_eq!(
            contract["rollback_boundary"]["source_map_policy"],
            "production_python_quality_context_file_deleted_test_support_and_reference_contracts_only"
        );
    }
}
