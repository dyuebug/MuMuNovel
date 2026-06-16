use std::collections::{HashMap, HashSet};

use crate::models::project;
use serde_json::{Map, Value};

use super::PromptContextProviderPayload;

pub(crate) const QUALITY_RUNTIME_TRACKING_TAG: &str = "rule_v3_quality_block_20260307";
const MCP_CANON_PRIORITY_RULE: &str =
    "项目 canon（既有设定、角色关系、本章大纲）优先级高于一切外部参考。";
const MCP_SOURCE_DISCLOSURE_RULE: &str = "最终输出禁止暴露 MCP、工具名、检索过程或来源站点。";

const QUALITY_PREFERENCE_SPECS: [(&str, &str, &[&str]); 5] = [
    (
        "balanced",
        "均衡质感",
        &[
            "兼顾抓力、推进、情绪和信息密度，不让正文只剩单项发力。",
            "每章最好既有局势变化，也有读者能感到的回报与余味。",
        ],
    ),
    (
        "plot_drive",
        "强情节回报",
        &[
            "优先强化开头抓力、动作现场化、回报节点和章尾追读牵引。",
            "减少空转解释、慢热预热和没有反馈的过程性段落。",
        ],
    ),
    (
        "immersive",
        "沉浸场景感",
        &[
            "优先强化设定落地、视角纪律、场景密度和现场感。",
            "解释尽量嵌进动作、对白和环境反馈里，减少飘在空中的说明。",
        ],
    ),
    (
        "emotion_drama",
        "情绪关系向",
        &[
            "优先强化情绪触发、外显反应、对白张力和关系余波。",
            "让人物靠近、误伤、试探和迟来的理解都落在现场里。",
        ],
    ),
    (
        "clean_prose",
        "克制干净文风",
        &[
            "优先强化信息压缩、重复压缩、少盖章、少同义复述。",
            "减少油腻金句、过度解释和模板连接词，让正文更利落。",
        ],
    ),
];

const QUALITY_PREFERENCE_ALIASES: [(&str, &str); 15] = [
    ("balanced", "balanced"),
    ("均衡", "balanced"),
    ("均衡质感", "balanced"),
    ("plot_drive", "plot_drive"),
    ("强情节", "plot_drive"),
    ("强情节回报", "plot_drive"),
    ("immersive", "immersive"),
    ("沉浸", "immersive"),
    ("沉浸场景感", "immersive"),
    ("emotion_drama", "emotion_drama"),
    ("情绪关系", "emotion_drama"),
    ("情绪关系向", "emotion_drama"),
    ("clean_prose", "clean_prose"),
    ("克制文风", "clean_prose"),
    ("克制干净文风", "clean_prose"),
];

pub(crate) struct PromptInstructionSpec {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) chapter_bullets: &'static [&'static str],
}

const CREATIVE_MODE_SPECS: [PromptInstructionSpec; 6] = [
    PromptInstructionSpec {
        key: "balanced",
        label: "均衡推进",
        chapter_bullets: &[
            "兼顾推进效率、情绪余韵和章尾牵引，不让单一节拍统治全文。",
            "既要有动作落点，也要有关系或情绪反馈。",
        ],
    },
    PromptInstructionSpec {
        key: "hook",
        label: "钩子优先",
        chapter_bullets: &[
            "开场尽快抛出异常、任务或危险，章尾优先落在未解动作上。",
            "减少平铺解释，多用突发变化和信息缺口带动阅读。",
        ],
    },
    PromptInstructionSpec {
        key: "emotion",
        label: "情绪沉浸",
        chapter_bullets: &[
            "强化人物情绪的触发、压抑、外露和余震过程。",
            "多写反应、动作和潜台词，少写统一口径的抒情总结。",
        ],
    },
    PromptInstructionSpec {
        key: "suspense",
        label: "悬念拉满",
        chapter_bullets: &[
            "控制信息披露节奏，把真相拆成连续可追的碎片。",
            "对白和动作里埋认知偏差，让读者和角色都处在半知状态。",
        ],
    },
    PromptInstructionSpec {
        key: "relationship",
        label: "关系张力",
        chapter_bullets: &[
            "强化角色之间的试探、误解、压制、让步和反击。",
            "至少让一段关键互动同时推动剧情与关系变化。",
        ],
    },
    PromptInstructionSpec {
        key: "payoff",
        label: "爽点推进",
        chapter_bullets: &[
            "强化铺垫→爆发→反馈链条，让爽点有落地动作和后续影响。",
            "减少空转拉扯，关键节点尽量让角色主动出手换结果。",
        ],
    },
];

const CREATIVE_MODE_ALIASES: [(&str, &str); 18] = [
    ("balanced", "balanced"),
    ("均衡", "balanced"),
    ("均衡推进", "balanced"),
    ("hook", "hook"),
    ("钩子", "hook"),
    ("钩子优先", "hook"),
    ("emotion", "emotion"),
    ("情绪", "emotion"),
    ("情绪沉浸", "emotion"),
    ("suspense", "suspense"),
    ("悬念", "suspense"),
    ("悬念拉满", "suspense"),
    ("relationship", "relationship"),
    ("关系", "relationship"),
    ("关系张力", "relationship"),
    ("payoff", "payoff"),
    ("爽点", "payoff"),
    ("爽点推进", "payoff"),
];

const STORY_FOCUS_SPECS: [PromptInstructionSpec; 6] = [
    PromptInstructionSpec {
        key: "advance_plot",
        label: "主线推进",
        chapter_bullets: &[
            "优先写清角色做了什么、局势如何变化、下一步被逼向哪里。",
            "减少原地解释和重复抒情，让情节真正往前走。",
        ],
    },
    PromptInstructionSpec {
        key: "deepen_character",
        label: "人物塑形",
        chapter_bullets: &[
            "优先通过选择、反应、失误和坚持来立住人物。",
            "让角色的独特声音、习惯与价值判断真正显形。",
        ],
    },
    PromptInstructionSpec {
        key: "escalate_conflict",
        label: "冲突升级",
        chapter_bullets: &[
            "优先写出目标受阻、局面恶化、选择更难的过程。",
            "让冲突产生即时后果，不要只停留在嘴上对抗。",
        ],
    },
    PromptInstructionSpec {
        key: "reveal_mystery",
        label: "谜团揭示",
        chapter_bullets: &[
            "优先通过调查、对质、异常细节与证据变化推进认知。",
            "每章至少让读者比上一章多知道一点关键东西。",
        ],
    },
    PromptInstructionSpec {
        key: "relationship_shift",
        label: "关系转折",
        chapter_bullets: &[
            "优先写互动中的试探、让步、误判、亏欠或立场重排。",
            "对话和行动都要服务关系变化，不只写结果。",
        ],
    },
    PromptInstructionSpec {
        key: "foreshadow_payoff",
        label: "伏笔回收",
        chapter_bullets: &[
            "优先让前文埋下的悬念、承诺或能力产生可感的回报。",
            "回收不能只靠说明，要落在事件结果和人物反馈上。",
        ],
    },
];

const STORY_FOCUS_ALIASES: [(&str, &str); 24] = [
    ("advance_plot", "advance_plot"),
    ("主线", "advance_plot"),
    ("主线推进", "advance_plot"),
    ("推进剧情", "advance_plot"),
    ("deepen_character", "deepen_character"),
    ("人物", "deepen_character"),
    ("人物塑形", "deepen_character"),
    ("塑造人物", "deepen_character"),
    ("escalate_conflict", "escalate_conflict"),
    ("冲突", "escalate_conflict"),
    ("冲突升级", "escalate_conflict"),
    ("升级冲突", "escalate_conflict"),
    ("reveal_mystery", "reveal_mystery"),
    ("谜团", "reveal_mystery"),
    ("谜团揭示", "reveal_mystery"),
    ("揭示真相", "reveal_mystery"),
    ("relationship_shift", "relationship_shift"),
    ("关系", "relationship_shift"),
    ("关系转折", "relationship_shift"),
    ("关系变化", "relationship_shift"),
    ("foreshadow_payoff", "foreshadow_payoff"),
    ("伏笔", "foreshadow_payoff"),
    ("伏笔回收", "foreshadow_payoff"),
    ("回收伏笔", "foreshadow_payoff"),
];

const PLOT_STAGE_LABELS: [(&str, &str); 3] = [
    ("development", "发展阶段"),
    ("climax", "高潮阶段"),
    ("ending", "结局阶段"),
];

const PLOT_STAGE_ALIASES: [(&str, &str); 9] = [
    ("development", "development"),
    ("发展", "development"),
    ("发展阶段", "development"),
    ("climax", "climax"),
    ("高潮", "climax"),
    ("高潮阶段", "climax"),
    ("ending", "ending"),
    ("结局", "ending"),
    ("结局阶段", "ending"),
];

const QUALITY_CONTRACT_BLOCK_ORDER: [&str; 30] = [
    "quality_generation_block",
    "creative_mode_block",
    "story_focus_block",
    "narrative_blueprint_block",
    "story_creation_brief_block",
    "quality_preference_block",
    "story_objective_card_block",
    "story_result_card_block",
    "story_payoff_chain_card_block",
    "story_rule_grounding_card_block",
    "story_information_release_card_block",
    "story_emotion_landing_card_block",
    "story_action_rendering_card_block",
    "story_summary_tone_control_card_block",
    "story_repetition_control_card_block",
    "story_viewpoint_discipline_card_block",
    "story_dialogue_advancement_card_block",
    "story_opening_hook_card_block",
    "story_repair_target_block",
    "story_repair_diagnostic_block",
    "story_execution_checklist_block",
    "story_scene_anchor_card_block",
    "story_scene_density_card_block",
    "story_repetition_risk_block",
    "story_acceptance_card_block",
    "story_cliffhanger_card_block",
    "story_character_arc_card_block",
    "quality_generation_protocol_block",
    "quality_mcp_guard_block",
    "quality_external_assets_block",
];

pub(crate) fn resolve_prompt_preference(
    override_value: Option<&str>,
    project_default: Option<&str>,
) -> String {
    override_value
        .filter(|value| !value.trim().is_empty())
        .or(project_default.filter(|value| !value.trim().is_empty()))
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn build_optional_instruction_block(label: &str, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        String::new()
    } else {
        format!("【{}】\n{}\n", label, value)
    }
}

fn normalize_prompt_alias(
    value: &str,
    aliases: &[(&'static str, &'static str)],
) -> Option<&'static str> {
    let cleaned = value.trim();
    if cleaned.is_empty() {
        return None;
    }
    aliases
        .iter()
        .find(|(alias, _)| alias.eq_ignore_ascii_case(cleaned) || *alias == cleaned)
        .map(|(_, normalized)| *normalized)
}

pub(crate) fn normalize_creative_mode(value: &str) -> Option<&'static str> {
    normalize_prompt_alias(value, &CREATIVE_MODE_ALIASES)
}

pub(crate) fn normalize_story_focus(value: &str) -> Option<&'static str> {
    normalize_prompt_alias(value, &STORY_FOCUS_ALIASES)
}

pub(crate) fn normalize_plot_stage(value: &str) -> Option<&'static str> {
    normalize_prompt_alias(value, &PLOT_STAGE_ALIASES)
}

pub(crate) fn creative_mode_spec(normalized: &str) -> Option<&'static PromptInstructionSpec> {
    CREATIVE_MODE_SPECS
        .iter()
        .find(|spec| spec.key == normalized)
}

pub(crate) fn story_focus_spec(normalized: &str) -> Option<&'static PromptInstructionSpec> {
    STORY_FOCUS_SPECS.iter().find(|spec| spec.key == normalized)
}

pub(crate) fn plot_stage_label(normalized: &str) -> Option<&'static str> {
    PLOT_STAGE_LABELS
        .iter()
        .find(|(key, _)| *key == normalized)
        .map(|(_, label)| *label)
}

fn build_prompt_instruction_block(title: &str, lead: &str, spec: &PromptInstructionSpec) -> String {
    if spec.chapter_bullets.is_empty() {
        return String::new();
    }

    let mut lines = vec![format!("【{}】{}“{}”", title, lead, spec.label)];
    lines.extend(
        spec.chapter_bullets
            .iter()
            .map(|item| format!("- {}", item)),
    );
    format!("{}\n", lines.join("\n"))
}

pub(crate) fn build_creative_mode_block(mode: &str) -> String {
    normalize_creative_mode(mode)
        .and_then(creative_mode_spec)
        .map(|spec| build_prompt_instruction_block("创作模式", "当前采用", spec))
        .unwrap_or_default()
}

pub(crate) fn build_story_focus_block(value: &str) -> String {
    normalize_story_focus(value)
        .and_then(story_focus_spec)
        .map(|spec| build_prompt_instruction_block("结构侧重点", "当前优先", spec))
        .unwrap_or_default()
}

pub(crate) fn normalize_prompt_list(items: &[String]) -> Vec<String> {
    items
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

pub(crate) fn build_repair_target_block(targets: &[String], strengths: &[String]) -> String {
    let targets = normalize_prompt_list(targets);
    let strengths = normalize_prompt_list(strengths);

    if targets.is_empty() && strengths.is_empty() {
        return String::new();
    }

    let mut lines = vec!["【修复目标】".to_string()];
    if !targets.is_empty() {
        lines.push(format!("需要修复：{}", targets.join("；")));
    }
    if !strengths.is_empty() {
        lines.push(format!("必须保留：{}", strengths.join("；")));
    }

    format!("{}\n", lines.join("\n"))
}

pub(crate) fn build_repair_diagnostic_block(
    summary: &str,
    targets: &[String],
    strengths: &[String],
) -> String {
    let summary = summary.trim();
    let targets = normalize_prompt_list(targets);
    let strengths = normalize_prompt_list(strengths);

    if summary.is_empty() && targets.is_empty() && strengths.is_empty() {
        return String::new();
    }

    let mut lines = vec!["【修复诊断】".to_string()];
    if !summary.is_empty() {
        lines.push(summary.to_string());
    }
    if !targets.is_empty() {
        lines.push(format!("本章修复项：{}", targets.join("；")));
    }
    if !strengths.is_empty() {
        lines.push(format!("保留优势：{}", strengths.join("；")));
    }

    format!("{}\n", lines.join("\n"))
}

pub(crate) fn build_web_research_block(enabled: bool, query: Option<&str>) -> String {
    if !enabled {
        return String::new();
    }

    let note = query
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .map(|query| {
            format!(
                "已请求联网检索，优先吸收与以下问题直接相关的资料：{}",
                query
            )
        })
        .unwrap_or_else(|| {
            "已请求联网检索，可适度补充与本章设定、背景、职业、场景相关的外部事实参考。".to_string()
        });

    format!("【联网检索说明】\n{}\n", note)
}

pub(crate) fn build_external_assets_block(
    external_assets: &str,
    reference_assets: &str,
    mcp_references: &str,
) -> String {
    let external_assets = external_assets.trim();
    let reference_assets = reference_assets.trim();
    let mcp_references = mcp_references.trim();

    if (external_assets.is_empty() || external_assets == "[]")
        && (reference_assets.is_empty() || reference_assets == "[]")
        && mcp_references.is_empty()
    {
        return String::new();
    }

    let mut lines = vec!["【外部参考资产】".to_string()];
    if !external_assets.is_empty() && external_assets != "[]" {
        lines.push(format!("external_assets: {}", external_assets));
    }
    if !reference_assets.is_empty() && reference_assets != "[]" {
        lines.push(format!("reference_assets: {}", reference_assets));
    }
    if !mcp_references.is_empty() {
        lines.push(format!("mcp_references: {}", mcp_references));
    }

    format!("{}\n", lines.join("\n"))
}

fn normalize_quality_preset(value: &str) -> Option<&'static str> {
    let cleaned = value.trim();
    if cleaned.is_empty() {
        return None;
    }
    QUALITY_PREFERENCE_ALIASES
        .iter()
        .find(|(alias, _)| alias.eq_ignore_ascii_case(cleaned) || *alias == cleaned)
        .map(|(_, preset)| *preset)
}

fn split_quality_preference_note_items(value: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for raw in value
        .lines()
        .flat_map(|line| line.split(['；', ';']))
        .map(str::trim)
    {
        let normalized = raw
            .trim_start_matches(|ch: char| {
                ch.is_whitespace()
                    || matches!(ch, '-' | '*' | '•' | '·' | '.' | ')' | '(' | '、')
                    || ch.is_ascii_digit()
            })
            .trim();
        if normalized.is_empty() || !seen.insert(normalized.to_string()) {
            continue;
        }
        items.push(normalized.to_string());
        if items.len() >= 4 {
            break;
        }
    }
    items
}

pub(crate) fn build_quality_preference_block(quality_preset: &str, quality_notes: &str) -> String {
    let normalized_preset = normalize_quality_preset(quality_preset);
    let note_items = split_quality_preference_note_items(quality_notes);
    let spec = normalized_preset.and_then(|preset| {
        QUALITY_PREFERENCE_SPECS
            .iter()
            .find(|(key, _, _)| *key == preset)
    });

    if spec.is_none() && note_items.is_empty() {
        return String::new();
    }

    let mut lines = Vec::new();
    if let Some((_, label, bullets)) = spec {
        lines.push(format!("【质量预设】当前采用“{}”", label));
        lines.extend(bullets.iter().map(|item| format!("- {}", item)));
    } else {
        lines.push("【质量偏好补充】".to_string());
    }

    if note_items.len() == 1 {
        lines.push(format!("- 补充偏好：{}", note_items[0]));
    } else if !note_items.is_empty() {
        lines.push("- 补充偏好：".to_string());
        lines.extend(note_items.iter().map(|item| format!("  - {}", item)));
    }

    format!("{}\n", lines.join("\n"))
}

pub(crate) fn build_quality_generation_protocol_block() -> String {
    format!(
        "【统一协议护栏】\n- 质量块追踪标签：{}\n- 统一吸收第三版规则摘要，不在各链路重复手写散落逻辑。\n- runtime 质量块只补充规则来源，不覆盖用户模板主体与业务上下文。\n- {}\n- {}\n- 禁止输出流程化元文本、调度说明、自我评注与来源暴露。\n",
        QUALITY_RUNTIME_TRACKING_TAG, MCP_CANON_PRIORITY_RULE, MCP_SOURCE_DISCLOSURE_RULE
    )
}

pub(crate) fn build_quality_json_protocol_block() -> String {
    format!(
        "【统一JSON协议护栏】\n- 质量块追踪标签：{}\n- 维持纯 JSON 输出，不追加 markdown、解释说明、流程文本或来源披露。\n- {}\n- {}\n- 若证据不足，使用 null / 空数组 / 保守结论，不臆造事实。\n",
        QUALITY_RUNTIME_TRACKING_TAG, MCP_CANON_PRIORITY_RULE, MCP_SOURCE_DISCLOSURE_RULE
    )
}

pub(crate) fn build_quality_contract_block(params: &HashMap<String, String>) -> String {
    let mut body = Vec::new();
    for key in QUALITY_CONTRACT_BLOCK_ORDER {
        if let Some(value) = params.get(key).map(|item| item.trim()) {
            if !value.is_empty() {
                body.push(value.to_string());
            }
        }
    }
    if body.is_empty() {
        return String::new();
    }
    format!(
        "<quality_contract priority=\"P0\">\n{}\n</quality_contract>",
        body.join("\n")
    )
}

fn parse_quality_asset_payload(value: &str) -> Value {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "[]" {
        return Value::Array(Vec::new());
    }
    serde_json::from_str::<Value>(trimmed).unwrap_or_else(|_| {
        Value::Array(vec![Value::Object(Map::from_iter([(
            "raw_content".to_string(),
            Value::String(trimmed.to_string()),
        )]))])
    })
}

pub(crate) fn build_quality_profile_payload(
    project_model: &project::Model,
    quality_preset: &str,
    provider_payload: &PromptContextProviderPayload,
) -> Map<String, Value> {
    let external_assets = parse_quality_asset_payload(&provider_payload.external_assets);
    let reference_assets = parse_quality_asset_payload(&provider_payload.reference_assets);
    Map::from_iter([
        (
            "genre".to_string(),
            Value::String(project_model.genre.clone().unwrap_or_default()),
        ),
        ("style_name".to_string(), Value::String(String::new())),
        ("style_preset_id".to_string(), Value::String(String::new())),
        ("style_content".to_string(), Value::String(String::new())),
        (
            "quality_preset".to_string(),
            Value::String(quality_preset.trim().to_string()),
        ),
        ("external_assets".to_string(), external_assets),
        ("reference_assets".to_string(), reference_assets),
    ])
}
