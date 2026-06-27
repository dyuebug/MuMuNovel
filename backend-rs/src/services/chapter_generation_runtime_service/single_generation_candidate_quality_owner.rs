use serde_json::{json, Map, Value};

use crate::services::chapter_candidate_executor_production_adapter_service::{
    CandidateQualityGatePlanInput, CandidateQualityRuntimeContextBuildInput,
    CandidateStoryQualityMetricsInput,
};
use crate::services::chapter_generation_runtime_service::story_repair_quality_context_owner::normalize_quality_metrics_history_item;

const CONFLICT_WEIGHT: f64 = 0.26;
const RULE_GROUNDING_WEIGHT: f64 = 0.22;
const OUTLINE_ALIGNMENT_WEIGHT: f64 = 0.18;
const DIALOGUE_WEIGHT: f64 = 0.12;
const OPENING_HOOK_WEIGHT: f64 = 0.10;
const PAYOFF_CHAIN_WEIGHT: f64 = 0.07;
const CLIFFHANGER_WEIGHT: f64 = 0.05;

#[derive(Clone, Copy)]
struct ContinuityLedgerSpec {
    ledger_key: &'static str,
    focus_area: &'static str,
    ledger_label: &'static str,
    repair_template_prefix: &'static str,
}

const CONTINUITY_LEDGER_SPECS: [ContinuityLedgerSpec; 5] = [
ContinuityLedgerSpec {
    ledger_key: "character_state_ledger",
    focus_area: "character_continuity",
    ledger_label: "Character continuity ledger",
    repair_template_prefix: "Carry forward the character continuity ledger",
},
ContinuityLedgerSpec {
    ledger_key: "relationship_state_ledger",
    focus_area: "relationship_continuity",
    ledger_label: "Relationship continuity ledger",
    repair_template_prefix: "Express the relationship ledger through dialogue, alignment, or exchange",
},
ContinuityLedgerSpec {
    ledger_key: "foreshadow_state_ledger",
    focus_area: "foreshadow_continuity",
    ledger_label: "Foreshadow continuity ledger",
    repair_template_prefix: "Advance the foreshadow ledger toward payoff",
},
ContinuityLedgerSpec {
    ledger_key: "organization_state_ledger",
    focus_area: "organization_continuity",
    ledger_label: "Organization continuity ledger",
    repair_template_prefix: "Carry forward the organization continuity ledger through command, resource, or territory change",
},
ContinuityLedgerSpec {
    ledger_key: "career_state_ledger",
    focus_area: "career_continuity",
    ledger_label: "Career continuity ledger",
    repair_template_prefix: "Carry forward the career growth ledger through skill use, bottleneck, or cost",
},
];

#[derive(Debug, Clone)]
struct QualityMetricRate {
    hit_rate: f64,
    payload: Value,
}

impl QualityMetricRate {
    fn applicable(&self) -> bool {
        self.payload
            .get("applicable")
            .and_then(Value::as_bool)
            .unwrap_or(true)
    }
}

pub(crate) fn build_single_generation_quality_runtime_context(
    input: CandidateQualityRuntimeContextBuildInput,
) -> Value {
    let story_packet = input.story_packet;
    let mut context = Map::new();
    if let Some(packet) = story_packet.as_object() {
        copy_story_packet_runtime_value(&mut context, packet, "story_long_term_goal");
        copy_story_packet_runtime_value(&mut context, packet, "character_focus");
        copy_story_packet_runtime_value(&mut context, packet, "foreshadow_payoff_plan");
        copy_story_packet_runtime_value(&mut context, packet, "character_state_ledger");
        copy_story_packet_runtime_value(&mut context, packet, "relationship_state_ledger");
        copy_story_packet_runtime_value(&mut context, packet, "foreshadow_state_ledger");
        copy_story_packet_runtime_value(&mut context, packet, "organization_state_ledger");
        copy_story_packet_runtime_value(&mut context, packet, "career_state_ledger");
        copy_story_packet_runtime_value(&mut context, packet, "target_word_count");
        copy_story_packet_runtime_value(&mut context, packet, "chapter_count");
        copy_story_packet_runtime_value(&mut context, packet, "current_chapter_number");
    }
    context.insert("story_packet".to_string(), story_packet);
    context.insert("project".to_string(), input.project.clone());
    context.insert("chapter".to_string(), input.chapter.clone());
    context.insert("chapter_context".to_string(), input.chapter_context.clone());
    context.insert(
        "target_word_count".to_string(),
        json!(input.target_word_count),
    );
    context.insert("generation_intent".to_string(), input.generation_intent);
    insert_non_empty_string(&mut context, "creative_mode", input.creative_mode.trim());
    insert_non_empty_string(&mut context, "story_focus", input.story_focus.trim());
    insert_non_empty_string(&mut context, "plot_stage", input.plot_stage.trim());
    insert_non_empty_string(
        &mut context,
        "story_creation_brief",
        input.story_creation_brief.trim(),
    );
    insert_non_empty_string(&mut context, "quality_preset", input.quality_preset.trim());
    insert_non_empty_string(&mut context, "quality_notes", input.quality_notes.trim());
    insert_non_empty_string(
        &mut context,
        "story_repair_summary",
        input.story_repair_summary.trim(),
    );
    if let Some(chapter_count) = input.chapter_count {
        context.insert("chapter_count".to_string(), json!(chapter_count));
    }
    if let Some(current_chapter_number) = input.current_chapter_number {
        context.insert(
            "current_chapter_number".to_string(),
            json!(current_chapter_number),
        );
    }
    if !input.story_repair_targets.is_empty() {
        context.insert(
            "story_repair_targets".to_string(),
            json!(input.story_repair_targets),
        );
    }
    if !input.story_preserve_strengths.is_empty() {
        context.insert(
            "story_preserve_strengths".to_string(),
            json!(input.story_preserve_strengths),
        );
    }
    if let Some(payload) = input.current_story_repair_payload {
        context.insert("current_story_repair_payload".to_string(), payload);
    }

    copy_object_field(&mut context, &input.project, "world_rules");
    copy_object_field(&mut context, &input.chapter, "chapter_number");
    copy_object_field(&mut context, &input.chapter, "title");
    copy_object_field(
        &mut context,
        &input.chapter_context,
        "previous_chapter_continuation_point",
    );
    copy_object_field(
        &mut context,
        &input.chapter_context,
        "previous_chapter_content",
    );

    Value::Object(context)
}

fn copy_story_packet_runtime_value(
    context: &mut Map<String, Value>,
    packet: &Map<String, Value>,
    field_name: &str,
) {
    if let Some(value) = packet
        .get(field_name)
        .cloned()
        .filter(|value| !value.is_null())
    {
        context.insert(field_name.to_string(), value);
    }
}

pub(crate) fn compute_single_generation_story_quality_metrics(
    input: CandidateStoryQualityMetricsInput,
) -> Value {
    let chapter_outline = value_to_text(&input.chapter_outline);
    let world_rules = resolve_rule_grounding_source_text(
        value_to_text(&input.world_rules),
        &chapter_outline,
        &input.quality_runtime_context,
    );
    let conflict = calc_conflict_chain_rate(&input.content);
    let rule_grounding = calc_rule_grounding_rate(&input.content, &world_rules);
    let outline_alignment = calc_outline_alignment_rate(&input.content, &chapter_outline);
    let dialogue = calc_dialogue_naturalness_rate(&input.content);
    let opening_hook = calc_opening_hook_rate(&input.content);
    let payoff_chain = calc_payoff_chain_rate(
        &input.content,
        &chapter_outline,
        &input.quality_runtime_context,
    );
    let cliffhanger = calc_cliffhanger_rate(&input.content);
    let continuity_preflight =
        build_story_continuity_preflight(&input.content, &input.quality_runtime_context);
    let overall = applicable_quality_overall(&[
        (&conflict, CONFLICT_WEIGHT),
        (&rule_grounding, RULE_GROUNDING_WEIGHT),
        (&outline_alignment, OUTLINE_ALIGNMENT_WEIGHT),
        (&dialogue, DIALOGUE_WEIGHT),
        (&opening_hook, OPENING_HOOK_WEIGHT),
        (&payoff_chain, PAYOFF_CHAIN_WEIGHT),
        (&cliffhanger, CLIFFHANGER_WEIGHT),
    ]);

    let mut metrics = Map::new();
    metrics.insert(
        "overall_score".to_string(),
        json!(round_metric(overall * 100.0)),
    );
    metrics.insert(
        "conflict_chain_hit_rate".to_string(),
        json!(round_metric(conflict.hit_rate * 100.0)),
    );
    metrics.insert(
        "rule_grounding_hit_rate".to_string(),
        json!(round_metric(rule_grounding.hit_rate * 100.0)),
    );
    metrics.insert(
        "outline_alignment_rate".to_string(),
        json!(round_metric(outline_alignment.hit_rate * 100.0)),
    );
    metrics.insert(
        "dialogue_naturalness_rate".to_string(),
        json!(round_metric(dialogue.hit_rate * 100.0)),
    );
    metrics.insert(
        "opening_hook_rate".to_string(),
        json!(round_metric(opening_hook.hit_rate * 100.0)),
    );
    metrics.insert(
        "payoff_chain_rate".to_string(),
        json!(round_metric(payoff_chain.hit_rate * 100.0)),
    );
    metrics.insert(
        "cliffhanger_rate".to_string(),
        json!(round_metric(cliffhanger.hit_rate * 100.0)),
    );
    metrics.insert(
        "word_count".to_string(),
        json!(input.content.chars().count() as i64),
    );
    metrics.insert(
        "details".to_string(),
        json!({
            "conflict_chain": conflict.payload,
            "rule_grounding": rule_grounding.payload,
            "outline_alignment": outline_alignment.payload,
            "dialogue": dialogue.payload,
            "opening_hook": opening_hook.payload,
            "payoff_chain": payoff_chain.payload,
            "cliffhanger": cliffhanger.payload,
        }),
    );
    if input
        .quality_runtime_context
        .as_object()
        .is_some_and(|context| !context.is_empty())
    {
        metrics.insert(
            "quality_runtime_context".to_string(),
            input.quality_runtime_context.clone(),
        );
    }
    if let Some(continuity_preflight) = continuity_preflight {
        metrics.insert("continuity_preflight".to_string(), continuity_preflight);
    }

    let normalized_metrics =
        normalize_quality_metrics_history_item(&Value::Object(metrics.clone()), "chapter")
            .unwrap_or(Value::Object(metrics));
    enrich_quality_gate_with_continuity_preflight(normalized_metrics)
}

pub(crate) fn resolve_single_generation_quality_gate_plan(
    input: CandidateQualityGatePlanInput,
) -> Value {
    let metrics = input
        .candidate_metrics
        .and_then(|metrics| normalize_quality_metrics_history_item(&metrics, &input.scope))
        .unwrap_or_else(|| json!({}));
    let quality_gate = metrics
        .get("quality_gate")
        .filter(|gate| gate.is_object())
        .cloned()
        .unwrap_or_else(|| {
            json!({
                "decision": "allow_save",
                "status": "pass",
                "allow_save": true,
                "can_auto_repair": false,
                "requires_manual_review": false,
            })
        });
    let decision = quality_gate
        .get("decision")
        .and_then(Value::as_str)
        .unwrap_or("allow_save");
    let retry_available = input.retry_count < input.max_retries;
    let action = match decision {
        "auto_repair" | "repair" if retry_available => "retry",
        "manual_review" => "manual_review",
        _ => "continue",
    };

    json!({
        "action": action,
        "quality_gate": quality_gate,
        "quality_metrics": metrics,
        "attempt_offset": input.attempt_offset,
        "retry_count": input.retry_count,
        "max_retries": input.max_retries,
        "scope": input.scope,
        "current_story_repair_payload": input.current_story_repair_payload,
    })
}

fn copy_object_field(target: &mut Map<String, Value>, source: &Value, key: &str) {
    if let Some(value) = source
        .as_object()
        .and_then(|object| object.get(key))
        .cloned()
    {
        target.insert(key.to_string(), value);
    }
}

fn insert_non_empty_string(target: &mut Map<String, Value>, key: &str, value: &str) {
    if !value.is_empty() {
        target.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn value_to_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.trim().to_string(),
        Value::Null => String::new(),
        Value::Array(items) => items
            .iter()
            .map(value_to_text)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(object) => object
            .iter()
            .filter_map(|(key, value)| {
                let text = value_to_text(value);
                (!text.is_empty()).then(|| format!("{key}: {text}"))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.to_string(),
    }
}

fn normalize_world_rules_text(value: String) -> String {
    let text = value.trim();
    let placeholders = [
        "未设置",
        "未设定",
        "暂无",
        "暂无设定",
        "未设置世界规则",
        "未设定世界规则",
        "未提供",
        "无世界规则",
        "暂无世界规则",
        "待补充",
    ];
    if text.is_empty() || placeholders.contains(&text) {
        String::new()
    } else {
        text.to_string()
    }
}

fn resolve_rule_grounding_source_text(
    world_rules: String,
    chapter_outline: &str,
    quality_runtime_context: &Value,
) -> String {
    let explicit_rules = normalize_world_rules_text(world_rules);
    if !explicit_rules.is_empty() {
        return explicit_rules;
    }

    if let Some(context) = quality_runtime_context.as_object() {
        for key in [
            "world_rules",
            "world_rule_hints",
            "rule_impact",
            "world_rule_trigger",
        ] {
            let Some(value) = context.get(key) else {
                continue;
            };
            let text = normalize_world_rules_text(value_to_text(value));
            if !text.is_empty() {
                return text;
            }
        }
    }

    extract_outline_rule_hints(chapter_outline, 4).join("\n")
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '。' | '！' | '？' | '!' | '?' | '\n' | '；' | ';') {
            let sentence = current.trim();
            if !sentence.is_empty() {
                sentences.push(sentence.to_string());
            }
            current.clear();
        }
    }
    let tail = current.trim();
    if !tail.is_empty() {
        sentences.push(tail.to_string());
    }
    sentences
}

fn compact_text(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn contains_any(text: &str, words: &[&str]) -> bool {
    words.iter().any(|word| text.contains(word))
}

fn calc_conflict_chain_rate(text: &str) -> QualityMetricRate {
    let sentences = split_sentences(text);
    if sentences.is_empty() {
        return rate_payload(
            0.0,
            json!({"hit_rate": 0.0, "hit_count": 0, "expected_count": 1, "applicable": true}),
        );
    }

    let obstacle_words = [
        "受阻",
        "拦住",
        "失败",
        "危机",
        "危险",
        "封锁",
        "卡住",
        "失控",
        "逼近",
        "困住",
        "锁死",
        "逼迫",
        "不行",
        "终止",
        "追责",
        "复核",
        "底稿追索",
        "热度",
        "认主",
    ];
    let choice_words = [
        "选择",
        "决定",
        "只能",
        "必须",
        "打算",
        "转而",
        "赌一把",
        "咬牙",
        "开播",
        "接听",
        "推门",
        "按下",
        "反锁",
        "继续",
        "输入",
        "前往",
        "跑",
        "断流",
        "交出",
        "不交",
    ];
    let cost_words = [
        "代价",
        "损失",
        "牺牲",
        "受伤",
        "暴露",
        "后果",
        "风险",
        "失去",
        "封号",
        "扣走",
        "死",
        "出事",
        "反噬",
        "锁定",
        "实名",
        "手机号",
        "伤人",
        "流血",
    ];
    let expected_count = (text.chars().count() / 900).max(1);
    let mut hit_count = 0usize;
    for index in 0..sentences.len() {
        let obstacle_window = join_window(&sentences, index, index + 4);
        if !contains_any(&obstacle_window, &obstacle_words) {
            continue;
        }
        let choice_window = join_window(&sentences, index, index + 6);
        let cost_window = join_window(&sentences, index, index + 10);
        if contains_any(&choice_window, &choice_words) && contains_any(&cost_window, &cost_words) {
            hit_count += 1;
        }
    }
    if hit_count < expected_count {
        let obstacle_window = join_window(&sentences, 0, (sentences.len() * 6 / 10).max(4));
        let choice_window = join_window(
            &sentences,
            sentences.len() * 25 / 100,
            (sentences.len() * 85 / 100).max(1),
        );
        let cost_window = join_window(&sentences, sentences.len() * 45 / 100, sentences.len());
        if contains_any(&obstacle_window, &obstacle_words)
            && contains_any(&choice_window, &choice_words)
            && contains_any(&cost_window, &cost_words)
        {
            hit_count = (hit_count + 1).min(expected_count);
        }
    }
    let hit_rate = (hit_count as f64 / expected_count as f64).min(1.0);
    rate_payload(
        hit_rate,
        json!({
            "hit_rate": round_rate(hit_rate),
            "hit_count": hit_count,
            "expected_count": expected_count,
            "applicable": true,
        }),
    )
}

fn calc_rule_grounding_rate(text: &str, world_rules: &str) -> QualityMetricRate {
    let keywords = extract_keywords(world_rules, 8);
    if keywords.is_empty() {
        return rate_payload(
            0.0,
            json!({
                "hit_rate": 0.0,
                "hit_count": 0,
                "expected_count": 0,
                "matched_keywords": [],
                "applicable": false,
                "skipped_reason": "no_world_rules",
            }),
        );
    }

    let sentences = split_sentences(text);
    let expected_count = (text.chars().count() / 1100).max(1);
    let causal_words = [
        "导致",
        "所以",
        "因此",
        "结果",
        "触发",
        "引发",
        "迫使",
        "只能",
        "不得不",
        "于是",
        "只要",
        "一旦",
        "否则",
        "才会",
        "才能",
        "过不了",
    ];
    let cue_words = [
        "规则",
        "限制",
        "边界",
        "代价",
        "触发",
        "改写",
        "污染",
        "登记",
        "校对",
        "反噬",
        "见证人",
        "复核",
        "直播",
        "样本",
    ];
    let mut matched_keywords = Vec::new();
    let mut grounded_events = 0usize;
    for sentence in &sentences {
        let sentence_keywords = keywords
            .iter()
            .filter(|keyword| sentence.contains(keyword.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let has_rule_cue = contains_any(sentence, &cue_words);
        if sentence_keywords.is_empty() && !has_rule_cue {
            continue;
        }
        for keyword in sentence_keywords {
            if !matched_keywords.contains(&keyword) {
                matched_keywords.push(keyword);
            }
        }
        if has_rule_cue
            && !matched_keywords
                .iter()
                .any(|keyword| keyword == "__implicit_rule_cue__")
        {
            matched_keywords.push("__implicit_rule_cue__".to_string());
        }
        if contains_any(sentence, &causal_words)
            || (sentence.contains("只要") && sentence.contains("就"))
            || (sentence.contains("一旦") && sentence.contains("就"))
        {
            grounded_events += 1;
        }
    }
    let keyword_coverage = matched_keywords.len() as f64 / keywords.len().min(4).max(1) as f64;
    let event_rate = grounded_events as f64 / expected_count as f64;
    let hit_rate = (0.5 * keyword_coverage + 0.5 * event_rate).min(1.0);
    rate_payload(
        hit_rate,
        json!({
            "hit_rate": round_rate(hit_rate),
            "hit_count": grounded_events,
            "expected_count": expected_count,
            "matched_keywords": matched_keywords.into_iter().take(6).collect::<Vec<_>>(),
            "applicable": true,
        }),
    )
}

fn calc_outline_alignment_rate(text: &str, chapter_outline: &str) -> QualityMetricRate {
    let anchors = extract_outline_anchor_lines(chapter_outline, 8);
    if anchors.is_empty() {
        return rate_payload(
            0.0,
            json!({
                "hit_rate": 0.0,
                "hit_count": 0,
                "expected_count": 0,
                "matched_anchors": [],
                "applicable": false,
                "skipped_reason": "no_outline_anchors",
            }),
        );
    }

    let compact = compact_text(text);
    let mut relevant_anchors = 0usize;
    let mut hit_count = 0usize;
    let mut matched_anchors = Vec::new();
    for anchor in anchors {
        let tokens = expand_anchor_match_tokens(extract_keywords(&anchor, 12), 24);
        if tokens.is_empty() {
            continue;
        }
        relevant_anchors += 1;
        let matched = tokens
            .iter()
            .filter(|token| compact.contains(token.as_str()))
            .collect::<Vec<_>>();
        let long_match = tokens
            .iter()
            .any(|token| token.chars().count() >= 4 && compact.contains(token.as_str()));
        let strong_match = matched.iter().any(|token| token.chars().count() >= 3);
        if long_match || (matched.len() >= 2 && strong_match) || matched.len() >= 3 {
            hit_count += 1;
            matched_anchors.push(take_chars(&anchor, 120));
        }
    }
    if relevant_anchors == 0 {
        return rate_payload(
            0.0,
            json!({
                "hit_rate": 0.0,
                "hit_count": 0,
                "expected_count": 0,
                "matched_anchors": [],
                "applicable": false,
                "skipped_reason": "no_anchor_tokens",
            }),
        );
    }

    let expected_count = relevant_anchors.min(5).max(1);
    let effective_hits = hit_count.min(expected_count);
    let hit_rate = (effective_hits as f64 / expected_count as f64).min(1.0);
    rate_payload(
        hit_rate,
        json!({
            "hit_rate": round_rate(hit_rate),
            "hit_count": hit_count,
            "expected_count": expected_count,
            "matched_anchors": matched_anchors.into_iter().take(6).collect::<Vec<_>>(),
            "applicable": true,
        }),
    )
}

fn calc_dialogue_naturalness_rate(text: &str) -> QualityMetricRate {
    let dialogues = extract_dialogue_segments(text);
    if dialogues.is_empty() {
        return rate_payload(
            0.0,
            json!({
                "hit_rate": 0.0,
                "total_dialogues": 0,
                "short_ratio": 0.0,
                "interrupt_ratio": 0.0,
                "pressure_ratio": 0.0,
                "applicable": true,
            }),
        );
    }

    let interrupt_markers = ["…", "——", "？", "！", "嗯", "啊"];
    let pressure_markers = [
        "？",
        "！",
        "别",
        "快",
        "马上",
        "立刻",
        "谁",
        "为什么",
        "怎么",
        "什么",
        "撤",
        "申诉",
        "下去",
    ];
    let total = dialogues.len() as f64;
    let short_count = dialogues
        .iter()
        .filter(|dialogue| dialogue.trim().chars().count() <= 28)
        .count() as f64;
    let interrupt_count = dialogues
        .iter()
        .filter(|dialogue| contains_any(dialogue, &interrupt_markers))
        .count() as f64;
    let pressure_count = dialogues
        .iter()
        .filter(|dialogue| contains_any(dialogue, &pressure_markers))
        .count() as f64;
    let short_ratio = short_count / total;
    let interrupt_ratio = interrupt_count / total;
    let pressure_ratio = pressure_count / total;
    let hit_rate = (0.7 * short_ratio + 0.3 * interrupt_ratio + 0.05 * pressure_ratio).min(1.0);
    rate_payload(
        hit_rate,
        json!({
            "hit_rate": round_rate(hit_rate),
            "total_dialogues": dialogues.len(),
            "short_ratio": round_rate(short_ratio),
            "interrupt_ratio": round_rate(interrupt_ratio),
            "pressure_ratio": round_rate(pressure_ratio),
            "applicable": true,
        }),
    )
}

fn calc_opening_hook_rate(text: &str) -> QualityMetricRate {
    let opening = take_chars(text, 300);
    if opening.trim().is_empty() {
        return rate_payload(
            0.0,
            json!({"hit_rate": 0.0, "matched_markers": [], "window_length": 0, "applicable": true}),
        );
    }

    let marker_groups = [
        (
            "异常",
            &[
                "忽然",
                "突然",
                "竟然",
                "不对劲",
                "异样",
                "通缉",
                "失控",
                "红字",
                "弹窗",
                "热榜",
                "现场复核",
            ][..],
        ),
        (
            "危险",
            &[
                "危险", "杀", "追", "爆炸", "血", "死", "失火", "崩塌", "警报", "污染", "锁定",
                "尖叫",
            ][..],
        ),
        (
            "任务",
            &[
                "必须", "限时", "任务", "命令", "今晚", "立刻", "马上", "确认", "报警",
            ][..],
        ),
        (
            "冲突",
            &[
                "质问", "拦住", "对峙", "打断", "冲突", "反驳", "拍桌", "别", "只能",
            ][..],
        ),
    ];
    let matched = marker_groups
        .iter()
        .filter_map(|(label, words)| contains_any(&opening, words).then_some((*label).to_string()))
        .collect::<Vec<_>>();
    let hit_rate = if matched.is_empty() {
        0.0
    } else {
        (matched.len() as f64 / 2.0).min(1.0)
    };
    rate_payload(
        hit_rate,
        json!({
            "hit_rate": round_rate(hit_rate),
            "matched_markers": matched,
            "window_length": opening.chars().count(),
            "applicable": true,
        }),
    )
}

fn calc_payoff_chain_rate(
    text: &str,
    chapter_outline: &str,
    quality_runtime_context: &Value,
) -> QualityMetricRate {
    let sentences = split_sentences(text);
    if sentences.is_empty() {
        return rate_payload(
            0.0,
            json!({"hit_rate": 0.0, "hit_count": 0, "expected_count": 1, "applicable": true}),
        );
    }

    let setup_words = [
        "原本",
        "本来",
        "一直",
        "眼看",
        "刚要",
        "正要",
        "谁知",
        "没想到",
        "偏偏",
        "被逼",
        "认定",
        "校验失败",
        "底稿偏差",
    ];
    let burst_words = [
        "突然", "当场", "直接", "瞬间", "反手", "竟然", "立刻", "翻盘", "突破", "触发", "炸开",
        "终于", "找到",
    ];
    let feedback_words = [
        "愣住",
        "哗然",
        "脸色",
        "看傻",
        "松了口气",
        "发麻",
        "欢呼",
        "炸开了锅",
        "弹幕",
        "热度",
        "认主",
        "呼吸停",
    ];
    let expected_count = (text.chars().count() / 1800).max(1);
    let mut hit_count = 0usize;
    for index in 0..sentences.len().saturating_sub(2) {
        let setup_window = join_window(&sentences, index.saturating_sub(1), index + 1);
        let burst_window = join_window(&sentences, index, index + 3);
        let feedback_window = join_window(&sentences, index, index + 5);
        if contains_any(&setup_window, &setup_words)
            && contains_any(&burst_window, &burst_words)
            && contains_any(&feedback_window, &feedback_words)
        {
            hit_count += 1;
        }
    }
    if hit_count == 0 {
        let compact = compact_text(text);
        for hint in extract_payoff_chain_hints(chapter_outline, quality_runtime_context, 4) {
            let tokens = expand_anchor_match_tokens(extract_keywords(&hint, 8), 24);
            let matched_count = tokens
                .iter()
                .filter(|token| compact.contains(token.as_str()))
                .count();
            let long_match = tokens
                .iter()
                .any(|token| token.chars().count() >= 4 && compact.contains(token.as_str()));
            if matched_count >= 2 || long_match {
                hit_count = 1;
                break;
            }
        }
    }
    let hit_rate = (hit_count as f64 / expected_count as f64).min(1.0);
    rate_payload(
        hit_rate,
        json!({
            "hit_rate": round_rate(hit_rate),
            "hit_count": hit_count,
            "expected_count": expected_count,
            "applicable": true,
        }),
    )
}

fn calc_cliffhanger_rate(text: &str) -> QualityMetricRate {
    let ending = take_last_chars(text, 360);
    if ending.trim().is_empty() {
        return rate_payload(
            0.0,
            json!({"hit_rate": 0.0, "matched_markers": [], "window_length": 0, "applicable": true}),
        );
    }

    let compact = compact_text(&ending);
    let weak_endings = [
        "总之",
        "他明白了",
        "命运将会",
        "一切都会好起来",
        "故事还在继续",
    ];
    if contains_any(&compact, &weak_endings) {
        return rate_payload(
            0.0,
            json!({"hit_rate": 0.0, "matched_markers": [], "window_length": ending.chars().count(), "applicable": true}),
        );
    }
    let marker_groups = [
        (
            "info_gap",
            &[
                "怎么会",
                "原来",
                "却发现",
                "门后",
                "那个人",
                "竟是",
                "真相",
                "秘密",
                "待复核",
                "只有一行字",
                "另一个自己",
            ][..],
        ),
        (
            "danger",
            &[
                "脚步声",
                "逼近",
                "枪口",
                "刀",
                "下一秒",
                "扑来",
                "要出事",
                "拍门声",
                "倒计时",
                "锁定",
                "胸口一片血",
            ][..],
        ),
        (
            "identity_twist",
            &[
                "竟然是你",
                "身份",
                "卧底",
                "叛徒",
                "冒名",
                "伪装",
                "认出来",
                "认主",
            ][..],
        ),
        (
            "choice_pending",
            &[
                "该不该",
                "要不要",
                "只能",
                "必须选",
                "下一步",
                "还没决定",
                "二十分钟内",
            ][..],
        ),
        (
            "escalation",
            &[
                "破万",
                "全网同步",
                "升级为",
                "开始校对",
                "新的任务",
                "第二轮复核",
            ][..],
        ),
    ];
    let matched = marker_groups
        .iter()
        .filter_map(|(label, words)| contains_any(&compact, words).then_some((*label).to_string()))
        .collect::<Vec<_>>();
    let hit_rate = if matched.is_empty() {
        0.0
    } else {
        (matched.len() as f64 / 2.0).min(1.0)
    };
    rate_payload(
        hit_rate,
        json!({
            "hit_rate": round_rate(hit_rate),
            "matched_markers": matched,
            "window_length": ending.chars().count(),
            "applicable": true,
        }),
    )
}

fn build_story_continuity_preflight(content: &str, runtime_context: &Value) -> Option<Value> {
    let runtime_context = runtime_context.as_object()?;
    let normalized_content = compact_text(content).to_lowercase();
    if normalized_content.is_empty() {
        return None;
    }

    let mut warnings = Vec::<Value>::new();
    let mut focus_areas = Vec::<String>::new();
    let mut repair_targets = Vec::<String>::new();
    let mut checked_item_count = 0_i64;
    let mut missing_item_count = 0_i64;

    for spec in CONTINUITY_LEDGER_SPECS {
        for item_text in normalize_runtime_context_items(runtime_context.get(spec.ledger_key), 3) {
            checked_item_count += 1;
            let anchors = extract_continuity_anchor_candidates(&item_text);
            let matched_anchor_count = anchors
                .iter()
                .filter(|anchor| anchor.chars().count() >= 2)
                .filter(|anchor| normalized_content.contains(anchor.as_str()))
                .collect::<std::collections::HashSet<_>>()
                .len() as i64;
            let required_match_count = if matches!(
                spec.ledger_key,
                "relationship_state_ledger" | "career_state_ledger"
            ) && anchors.len() >= 2
            {
                2
            } else {
                1
            };
            if matched_anchor_count >= required_match_count {
                continue;
            }

            missing_item_count += 1;
            push_unique(&mut focus_areas, spec.focus_area.to_string());
            let repair_target = format!("{}: {}", spec.repair_template_prefix, item_text);
            push_unique(&mut repair_targets, repair_target);
            warnings.push(json!({
                "ledger_key": spec.ledger_key,
                "ledger_label": spec.ledger_label,
                "focus_area": spec.focus_area,
                "item": item_text,
                "anchors": anchors,
                "matched_anchor_count": matched_anchor_count,
                "required_match_count": required_match_count,
            }));
            if warnings.len() >= 4 {
                break;
            }
        }
        if warnings.len() >= 4 {
            break;
        }
    }

    if warnings.is_empty() {
        return Some(json!({
            "status": "ok",
            "checked_item_count": checked_item_count,
            "warning_count": 0,
            "warnings": [],
            "focus_areas": [],
            "repair_targets": [],
            "summary": "",
        }));
    }

    let labels = warnings
        .iter()
        .filter_map(|warning| warning.get("ledger_label"))
        .filter_map(Value::as_str)
        .fold(Vec::<String>::new(), |mut labels, label| {
            push_unique(&mut labels, label.to_string());
            labels
        })
        .join(", ");
    let mut summary = format!(
        "Current chapter misses explicit handoff for {missing_item_count} continuity ledger items."
    );
    if !labels.is_empty() {
        summary = format!("{summary} Prioritize {labels}.");
    }

    Some(json!({
        "status": "warning",
        "checked_item_count": checked_item_count,
        "warning_count": warnings.len() as i64,
        "missing_item_count": missing_item_count,
        "warnings": warnings,
        "focus_areas": focus_areas,
        "repair_targets": repair_targets.into_iter().take(4).collect::<Vec<_>>(),
        "summary": summary,
    }))
}

fn enrich_quality_gate_with_continuity_preflight(mut metrics: Value) -> Value {
    let Some(metrics_object) = metrics.as_object_mut() else {
        return metrics;
    };
    let Some(continuity_preflight) = metrics_object
        .get("continuity_preflight")
        .filter(|value| value.is_object())
        .cloned()
    else {
        return metrics;
    };
    let warning_count = continuity_preflight
        .get("warning_count")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let Some(quality_gate) = metrics_object
        .get_mut("quality_gate")
        .and_then(Value::as_object_mut)
    else {
        return metrics;
    };

    quality_gate.insert("continuity_warning_count".to_string(), json!(warning_count));
    quality_gate.insert(
        "continuity_preflight".to_string(),
        continuity_preflight.clone(),
    );
    merge_string_array_field(
        quality_gate,
        "focus_areas",
        continuity_preflight.get("focus_areas"),
    );
    merge_string_array_field(
        quality_gate,
        "repair_targets",
        continuity_preflight.get("repair_targets"),
    );
    metrics
}

fn merge_string_array_field(target: &mut Map<String, Value>, key: &str, values: Option<&Value>) {
    let mut merged = target
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(values) = values.and_then(Value::as_array) {
        for value in values {
            if let Some(text) = value
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
            {
                push_unique(&mut merged, text.to_string());
            }
        }
    }
    target.insert(
        key.to_string(),
        Value::Array(merged.into_iter().map(Value::String).collect()),
    );
}

fn normalize_runtime_context_items(value: Option<&Value>, limit: usize) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    let mut items = Vec::new();
    match value {
        Value::Array(raw_items) => {
            for item in raw_items {
                let text = stringify_runtime_context_item(item);
                if !text.is_empty() {
                    push_unique(&mut items, text);
                }
                if items.len() >= limit {
                    break;
                }
            }
        }
        Value::Null => {}
        other => {
            let text = stringify_runtime_context_item(other);
            if !text.is_empty() {
                items.push(text);
            }
        }
    }
    items
}

fn stringify_runtime_context_item(value: &Value) -> String {
    match value {
        Value::String(text) => text.trim().to_string(),
        Value::Object(object) => {
            let keys = [
                "name",
                "title",
                "label",
                "summary",
                "content",
                "setup",
                "payoff",
                "trigger",
                "resolution",
                "state",
                "status",
                "location",
                "stage",
                "bottleneck",
                "cost",
            ];
            keys.iter()
                .filter_map(|key| object.get(*key))
                .map(value_to_text)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        }
        Value::Null => String::new(),
        other => value_to_text(other),
    }
}

fn extract_continuity_anchor_candidates(item: &str) -> Vec<String> {
    let text = item.trim();
    if text.is_empty() {
        return Vec::new();
    }
    let head = text
        .split_once(':')
        .map(|(head, _)| head)
        .or_else(|| text.split_once('：').map(|(head, _)| head))
        .map(str::trim)
        .filter(|head| !head.is_empty())
        .unwrap_or(text);
    let segments = split_continuity_anchor_segments(head);
    let mut tokens = Vec::new();
    for segment in segments.into_iter().take(3) {
        for token in extract_continuity_segment_tokens(&segment) {
            push_unique(&mut tokens, token);
            if tokens.len() >= 3 {
                return tokens;
            }
        }
    }
    if !tokens.is_empty() {
        return tokens;
    }

    let fallback = compact_text(head).to_lowercase();
    (fallback.chars().count() >= 2)
        .then_some(fallback)
        .into_iter()
        .collect()
}

fn split_continuity_anchor_segments(head: &str) -> Vec<String> {
    let separators = ['、', ',', '/', '|', '&', '＆', '和', '与', '+', '·', '•'];
    let mut segments = Vec::new();
    let mut current = String::new();
    for ch in head.chars() {
        if separators.contains(&ch) {
            let segment = current.trim();
            if !segment.is_empty() {
                segments.push(segment.to_string());
            }
            current.clear();
        } else {
            current.push(ch);
        }
    }
    let tail = current.trim();
    if !tail.is_empty() {
        segments.push(tail.to_string());
    }
    if segments.is_empty() {
        segments.push(head.to_string());
    }
    segments
}

fn extract_continuity_segment_tokens(segment: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in segment.chars() {
        let is_token_char = ('\u{4e00}'..='\u{9fff}').contains(&ch)
            || ch.is_ascii_alphanumeric()
            || ch == '_'
            || ch == '-';
        if is_token_char {
            current.push(ch.to_ascii_lowercase());
            continue;
        }
        push_continuity_token(&mut tokens, &current);
        current.clear();
    }
    push_continuity_token(&mut tokens, &current);
    tokens
}

fn push_continuity_token(tokens: &mut Vec<String>, raw: &str) {
    let token = raw.trim().to_lowercase();
    if token.chars().count() >= 2 {
        push_unique(tokens, token);
    }
}

fn applicable_quality_overall(entries: &[(&QualityMetricRate, f64)]) -> f64 {
    let mut weighted_sum = 0.0;
    let mut total_weight = 0.0;
    for (metric, weight) in entries {
        if !metric.applicable() {
            continue;
        }
        weighted_sum += metric.hit_rate * weight;
        total_weight += weight;
    }
    if total_weight <= 0.0 {
        0.0
    } else {
        weighted_sum / total_weight
    }
}

fn extract_outline_rule_hints(chapter_outline: &str, limit: usize) -> Vec<String> {
    let mut hints = Vec::new();
    let cue_tokens = [
        "规则", "边界", "限制", "触发", "反噬", "登记", "改写", "污染", "校对", "纠错",
    ];
    let constraint_tokens = [
        "不能", "不得", "否则", "代价", "伤害", "说破", "公开", "只要", "一旦", "才会", "就会",
    ];
    for line in chapter_outline.lines() {
        let normalized = line.trim().trim_start_matches(['-', '*', ' ']).trim();
        if normalized.is_empty() {
            continue;
        }
        let in_rule_section = normalized.contains("规则")
            || normalized.contains("边界")
            || normalized.contains("限制");
        let has_rule_cue = contains_any(normalized, &cue_tokens);
        let has_constraint = contains_any(normalized, &constraint_tokens);
        if in_rule_section || (has_rule_cue && has_constraint) {
            push_unique(&mut hints, take_chars(normalized, 120));
            if hints.len() >= limit {
                break;
            }
        }
    }
    hints
}

fn extract_outline_anchor_lines(chapter_outline: &str, max_lines: usize) -> Vec<String> {
    let mut anchors = Vec::new();
    for line in chapter_outline.lines() {
        let normalized = line.trim().trim_start_matches(['-', '*', ' ']).trim();
        if normalized.is_empty() || (normalized.starts_with('【') && normalized.ends_with('】')) {
            continue;
        }
        push_unique(&mut anchors, normalized.to_string());
        if anchors.len() >= max_lines {
            break;
        }
    }
    if anchors.is_empty() && !chapter_outline.trim().is_empty() {
        anchors.extend(
            split_sentences(chapter_outline)
                .into_iter()
                .filter(|sentence| sentence.chars().count() >= 8)
                .take(max_lines),
        );
    }
    anchors
}

fn extract_payoff_chain_hints(
    chapter_outline: &str,
    quality_runtime_context: &Value,
    limit: usize,
) -> Vec<String> {
    let mut hints = Vec::new();
    if let Some(items) = quality_runtime_context
        .get("foreshadow_payoff_plan")
        .and_then(Value::as_array)
    {
        for item in items {
            let text = value_to_text(item);
            if !text.is_empty() {
                push_unique(&mut hints, take_chars(&text, 120));
                if hints.len() >= limit {
                    return hints;
                }
            }
        }
    }

    let tokens = [
        "小爽点",
        "章尾",
        "钩子",
        "折返",
        "救人",
        "尸体",
        "锚点",
        "反馈",
        "回收",
        "兑现",
        "悬念",
        "翻盘",
    ];
    for line in chapter_outline.lines() {
        let normalized = line.trim().trim_start_matches(['-', '*', ' ']).trim();
        if normalized.is_empty() {
            continue;
        }
        if contains_any(normalized, &tokens) {
            push_unique(&mut hints, take_chars(normalized, 120));
            if hints.len() >= limit {
                return hints;
            }
        }
    }
    hints
}

fn extract_keywords(text: &str, limit: usize) -> Vec<String> {
    let stop_tokens = [
        "章节概要",
        "剧情摘要",
        "关键事件",
        "情节要点",
        "叙事目标",
        "规则影响",
        "角色选择",
        "人物转折",
        "对话钩子",
        "角色焦点",
        "小爽点",
        "本章",
        "这一章",
        "这里",
        "继续",
    ];
    let mut candidates = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ('\u{4e00}'..='\u{9fff}').contains(&ch) || ch.is_ascii_alphanumeric() {
            current.push(ch);
            continue;
        }
        push_keyword_candidates(&mut candidates, &current, &stop_tokens);
        current.clear();
    }
    push_keyword_candidates(&mut candidates, &current, &stop_tokens);
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.chars().count()));
    candidates.dedup();
    candidates.into_iter().take(limit).collect()
}

fn push_keyword_candidates(candidates: &mut Vec<String>, raw: &str, stop_tokens: &[&str]) {
    let token = raw.trim();
    if token.chars().count() < 2 || stop_tokens.contains(&token) {
        return;
    }
    let split_chars =
        "的了着过把将让给在向对跟与和并而或但却被因于从到往里上下前后再还又先就都也仍会要想";
    let mut pieces = vec![token.to_string()];
    for split_char in split_chars.chars() {
        let next = pieces
            .into_iter()
            .flat_map(|piece| {
                piece
                    .split(split_char)
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        pieces = if next.is_empty() {
            vec![token.to_string()]
        } else {
            next
        };
    }
    for piece in pieces {
        if piece.chars().count() < 2 || stop_tokens.contains(&piece.as_str()) {
            continue;
        }
        push_unique(candidates, piece.clone());
        let len = piece.chars().count();
        if len > 6 {
            push_unique(candidates, take_chars(&piece, 4));
            push_unique(candidates, take_last_chars(&piece, 4));
        } else if len > 4 {
            push_unique(candidates, take_chars(&piece, 3));
            push_unique(candidates, take_last_chars(&piece, 3));
        }
    }
}

fn expand_anchor_match_tokens(tokens: Vec<String>, limit: usize) -> Vec<String> {
    let mut expanded = Vec::new();
    for token in tokens {
        push_unique(&mut expanded, token.clone());
        let chars = token.chars().collect::<Vec<_>>();
        if chars.len() >= 4 {
            for width in 2..=4.min(chars.len()) {
                for start in 0..=chars.len() - width {
                    push_unique(&mut expanded, chars[start..start + width].iter().collect());
                    if expanded.len() >= limit {
                        return expanded;
                    }
                }
            }
        }
        if expanded.len() >= limit {
            break;
        }
    }
    expanded
}

fn extract_dialogue_segments(text: &str) -> Vec<String> {
    let pairs = [
        ('“', '”'),
        ('‘', '’'),
        ('「', '」'),
        ('『', '』'),
        ('"', '"'),
    ];
    let mut segments = Vec::new();
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        let Some((_, closing)) = pairs.iter().find(|(opening, _)| chars[index] == *opening) else {
            index += 1;
            continue;
        };
        let start = index + 1;
        let mut end = start;
        while end < chars.len() && chars[end] != *closing && chars[end] != '\n' {
            end += 1;
        }
        if end < chars.len() && chars[end] == *closing {
            let segment = chars[start..end].iter().collect::<String>();
            let len = segment.chars().count();
            if (1..=120).contains(&len) {
                segments.push(segment.trim().to_string());
            }
            index = end + 1;
        } else {
            index = start;
        }
    }
    segments
}

fn join_window(sentences: &[String], start: usize, end: usize) -> String {
    sentences
        .iter()
        .skip(start.min(sentences.len()))
        .take(end.saturating_sub(start))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if value.trim().is_empty() || values.contains(&value) {
        return;
    }
    values.push(value);
}

fn take_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn take_last_chars(text: &str, limit: usize) -> String {
    let chars = text.chars().collect::<Vec<_>>();
    chars
        .iter()
        .skip(chars.len().saturating_sub(limit))
        .collect()
}

fn rate_payload(hit_rate: f64, payload: Value) -> QualityMetricRate {
    QualityMetricRate { hit_rate, payload }
}

fn round_rate(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn round_metric(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

pub(crate) fn build_chapter_single_generation_candidate_quality_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_runtime_service::single_generation_candidate_quality_owner",
        "scope": "single_generation_candidate_quality_metrics_and_gate_owner",
        "python_source_map": [
            "backend/tests/test_support/schemas/quality.py"
        ],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_generation_runtime_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_executor_production_adapter_service.rs",
            "backend-rs/src/services/chapter_candidate_generation_service.rs",
            "backend-rs/src/services/chapter_generation_runtime_service/story_repair_quality_context_owner.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_single_generation_quality_runtime_context",
                "compute_single_generation_story_quality_metrics",
                "resolve_single_generation_quality_gate_plan"
            ],
            "metric_fields": [
                "overall_score",
                "conflict_chain_hit_rate",
                "rule_grounding_hit_rate",
                "outline_alignment_rate",
                "dialogue_naturalness_rate",
                "opening_hook_rate",
                "payoff_chain_rate",
                "cliffhanger_rate",
                "word_count",
                "details",
                "repair_guidance",
                "quality_gate",
                "continuity_preflight"
            ],
            "runtime_context_fields": [
                "story_packet",
                "project",
                "chapter",
                "chapter_context",
                "target_word_count",
                "generation_intent",
                "creative_mode",
                "story_focus",
                "plot_stage",
                "story_creation_brief",
                "quality_preset",
                "quality_notes",
                "chapter_count",
                "current_chapter_number",
                "story_repair_summary",
                "story_repair_targets",
                "story_preserve_strengths",
                "current_story_repair_payload",
                "world_rules",
                "chapter_number",
                "title",
                "previous_chapter_continuation_point",
                "previous_chapter_content"
            ],
            "quality_gate_policy": [
                "missing_metrics -> allow_save",
                "auto_repair_with_remaining_retry_budget -> retry",
                "auto_repair_with_exhausted_retry_budget -> continue",
                "non_repair_decision -> continue"
            ],
            "continuity_policy": "runtime continuity ledgers produce preflight warnings and gate continuity_warning_count when anchors are missing",
            "normalization_policy": "metrics are normalized through chapter_generation_runtime_service::story_repair_quality_context_owner before gate enrichment"
        },
        "validation_boundary": [
            "cargo test chapter_generation_runtime_service",
            "cargo check --manifest-path backend-rs/Cargo.toml",
            "python backend/tools/run_strangler_gateway_smoke.py --validate-manifest-only"
        ],
        "rollback_boundary": "single_generation_candidate_quality_python_source_map"
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_chapter_single_generation_candidate_quality_owner_contract,
        build_single_generation_quality_runtime_context,
        compute_single_generation_story_quality_metrics,
        resolve_single_generation_quality_gate_plan,
    };
    use crate::services::chapter_candidate_executor_production_adapter_service::{
        CandidateQualityGatePlanInput, CandidateQualityRuntimeContextBuildInput,
        CandidateStoryQualityMetricsInput,
    };

    #[test]
    fn should_publish_chapter_single_generation_candidate_quality_owner_contract() {
        let contract = build_chapter_single_generation_candidate_quality_owner_contract();

        assert_eq!(
            contract["owner"],
            "chapter_generation_runtime_service::single_generation_candidate_quality_owner"
        );
        assert_eq!(
            contract["scope"],
            "single_generation_candidate_quality_metrics_and_gate_owner"
        );
        assert_eq!(
            contract["python_source_map"][0],
            "backend/tests/test_support/schemas/quality.py"
        );
        assert_eq!(
            contract["python_source_map"]
                .as_array()
                .map(|items| items.len()),
            Some(1)
        );
        assert_eq!(
            contract["rust_owner_map"][0],
            "backend-rs/src/services/chapter_generation_runtime_service.rs"
        );
        assert_eq!(
            contract["behavior_contract"]["entrypoints"][1],
            "compute_single_generation_story_quality_metrics"
        );
        assert_eq!(
            contract["behavior_contract"]["metric_fields"][11],
            "quality_gate"
        );
        assert_eq!(
            contract["behavior_contract"]["quality_gate_policy"][1],
            "auto_repair_with_remaining_retry_budget -> retry"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_context_fields"][6],
            "creative_mode"
        );
        assert_eq!(
            contract["behavior_contract"]["runtime_context_fields"][17],
            "current_story_repair_payload"
        );
        assert_eq!(
            contract["rollback_boundary"],
            "single_generation_candidate_quality_python_source_map"
        );
    }

    #[test]
    fn should_compute_real_single_generation_quality_metrics_and_gate() {
        let metrics = compute_single_generation_story_quality_metrics(
        CandidateStoryQualityMetricsInput {
            content: "忽然红字弹窗触发复核。主角只能开播继续校对，否则会封号流血。原本以为线索断了，突然找到旧页，弹幕炸开了锅。“别停，马上确认！”章尾门后传来脚步声，另一个自己出现。".to_string(),
            chapter_outline: json!("- 红字弹窗触发复核\n- 主角开播校对并承担代价\n- 章尾留下另一个自己"),
            world_rules: json!("公开复核会触发见证人记录，否则样本会污染。"),
            quality_runtime_context: json!({
                "current_chapter_number": 2,
                "chapter_count": 10,
                "foreshadow_payoff_plan": [{"setup": "旧页", "payoff": "找到旧页"}],
            }),
        },
    );

        assert!(metrics["overall_score"].as_f64().unwrap_or_default() > 50.0);
        assert!(metrics["details"]["conflict_chain"].is_object());
        assert!(metrics["repair_guidance"].is_object());
        assert!(metrics["quality_gate"].is_object());
        assert_eq!(
            metrics["quality_runtime_context"]["current_chapter_number"],
            2
        );
    }

    #[test]
    fn should_lower_score_for_empty_or_unanchored_content() {
        let metrics =
            compute_single_generation_story_quality_metrics(CandidateStoryQualityMetricsInput {
                content: "天气很好。他想了很多。故事还在继续。".to_string(),
                chapter_outline: json!("- 主角必须完成现场复核\n- 章尾出现危险选择"),
                world_rules: json!("公开复核会触发代价。"),
                quality_runtime_context: json!({}),
            });

        assert!(metrics["overall_score"].as_f64().unwrap_or_default() < 30.0);
        assert_ne!(metrics["quality_gate"]["decision"], "allow_save");
    }

    #[test]
    fn should_add_continuity_preflight_warning_for_missing_runtime_ledgers() {
        let metrics =
            compute_single_generation_story_quality_metrics(CandidateStoryQualityMetricsInput {
                content: "主角推门进入废楼，红字弹窗突然亮起。".to_string(),
                chapter_outline: json!(""),
                world_rules: json!(""),
                quality_runtime_context: json!({
                    "organization_state_ledger": [
                        "ShadowGuild: power=72; location=North Dock"
                    ],
                    "career_state_ledger": [
                        "Lin/Strategist: stage 3; promotion blocked by council"
                    ],
                }),
            });

        let preflight = &metrics["continuity_preflight"];
        assert_eq!(preflight["status"], "warning");
        assert_eq!(preflight["warning_count"], 2);
        assert_eq!(
            preflight["warnings"][0]["ledger_key"],
            "organization_state_ledger"
        );
        assert!(preflight["focus_areas"]
            .as_array()
            .unwrap()
            .contains(&json!("organization_continuity")));
        assert!(preflight["repair_targets"][0]
            .as_str()
            .unwrap()
            .contains("ShadowGuild"));
        assert_eq!(metrics["quality_gate"]["continuity_warning_count"], 2);
    }

    #[test]
    fn should_keep_continuity_preflight_ok_when_runtime_anchors_are_present() {
        let metrics =
        compute_single_generation_story_quality_metrics(CandidateStoryQualityMetricsInput {
            content: "ShadowGuild在North Dock调动资源。Lin作为Strategist突破stage 3瓶颈，继续处理council阻拦。".to_string(),
            chapter_outline: json!(""),
            world_rules: json!(""),
            quality_runtime_context: json!({
                "organization_state_ledger": [
                    "ShadowGuild: power=72; location=North Dock"
                ],
                "career_state_ledger": [
                    "Lin/Strategist: stage 3; promotion blocked by council"
                ],
            }),
        });

        let preflight = &metrics["continuity_preflight"];
        assert_eq!(preflight["status"], "ok");
        assert_eq!(preflight["warning_count"], 0);
        assert_eq!(preflight["checked_item_count"], 2);
    }

    #[test]
    fn should_build_runtime_context_from_adapter_input() {
        let context = build_single_generation_quality_runtime_context(
            CandidateQualityRuntimeContextBuildInput {
                story_packet: json!({
                    "story": true,
                    "story_long_term_goal": "追回主线伏笔",
                    "character_focus": ["沈砚"],
                    "foreshadow_payoff_plan": ["回收旧约定"],
                    "character_state_ledger": [{"label": "沈砚", "summary": "情绪收紧"}],
                    "organization_state_ledger": [{"label": "夜巡司", "summary": "开始施压"}]
                }),
                project: json!({"id": "p1", "world_rules": "rules"}),
                chapter: json!({"id": "c1", "chapter_number": 4, "title": "第四章"}),
                chapter_context: json!({
                    "chapter_outline": "outline",
                    "previous_chapter_continuation_point": "door opened",
                }),
                target_word_count: 1800,
                generation_intent: json!({"mode": "single_generation_active_route"}),
                creative_mode: "hook".to_string(),
                story_focus: "advance_plot".to_string(),
                plot_stage: "climax".to_string(),
                story_creation_brief: "保持直播事故压迫感".to_string(),
                quality_preset: "plot_drive".to_string(),
                quality_notes: "减少解释".to_string(),
                chapter_count: Some(12),
                current_chapter_number: Some(4),
                story_repair_summary: "冲突升级不够".to_string(),
                story_repair_targets: vec!["补强冲突".to_string()],
                story_preserve_strengths: vec!["保留直播张力".to_string()],
                current_story_repair_payload: Some(json!({"source": "manual_request"})),
            },
        );

        assert_eq!(context["target_word_count"], 1800);
        assert_eq!(context["chapter_number"], 4);
        assert_eq!(context["world_rules"], "rules");
        assert_eq!(context["creative_mode"], "hook");
        assert_eq!(context["story_focus"], "advance_plot");
        assert_eq!(context["plot_stage"], "climax");
        assert_eq!(context["quality_preset"], "plot_drive");
        assert_eq!(context["chapter_count"], 12);
        assert_eq!(context["current_chapter_number"], 4);
        assert_eq!(context["story_long_term_goal"], "追回主线伏笔");
        assert_eq!(context["character_focus"][0], "沈砚");
        assert_eq!(context["foreshadow_payoff_plan"][0], "回收旧约定");
        assert_eq!(context["organization_state_ledger"][0]["label"], "夜巡司");
        assert_eq!(context["story_repair_summary"], "冲突升级不够");
        assert_eq!(context["story_repair_targets"][0], "补强冲突");
        assert_eq!(context["story_preserve_strengths"][0], "保留直播张力");
        assert_eq!(
            context["current_story_repair_payload"]["source"],
            "manual_request"
        );
        assert_eq!(
            context["previous_chapter_continuation_point"],
            "door opened"
        );
    }

    #[test]
    fn should_resolve_retry_gate_plan_when_auto_repair_and_budget_remains() {
        let plan = resolve_single_generation_quality_gate_plan(CandidateQualityGatePlanInput {
            candidate_metrics: Some(json!({
                "overall_score": 70.0,
                "conflict_chain_hit_rate": 20.0,
                "quality_gate": {
                    "decision": "auto_repair",
                    "status": "repairable",
                    "allow_save": false,
                    "can_auto_repair": true,
                    "requires_manual_review": false,
                }
            })),
            attempt_offset: 2,
            retry_count: 0,
            max_retries: 1,
            current_story_repair_payload: Some(json!({"reason": "conflict"})),
            scope: "chapter".to_string(),
        });

        assert_eq!(plan["action"], "retry");
        assert_eq!(plan["attempt_offset"], 2);
        assert_eq!(plan["quality_gate"]["decision"], "auto_repair");
        assert_eq!(plan["current_story_repair_payload"]["reason"], "conflict");
    }

    #[test]
    fn should_keep_continue_when_retry_budget_is_exhausted() {
        let plan = resolve_single_generation_quality_gate_plan(CandidateQualityGatePlanInput {
            candidate_metrics: Some(json!({
                "overall_score": 70.0,
                "conflict_chain_hit_rate": 20.0,
                "quality_gate": {"decision": "auto_repair", "status": "repairable"}
            })),
            attempt_offset: 0,
            retry_count: 1,
            max_retries: 1,
            current_story_repair_payload: None,
            scope: "chapter".to_string(),
        });

        assert_eq!(plan["action"], "continue");
        assert_eq!(plan["quality_gate"]["decision"], "auto_repair");
    }
}
