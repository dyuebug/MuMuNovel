use std::collections::{HashMap, HashSet};

use crate::models::{chapter, project};
use crate::services::chapter_generation_prompt_context_provider_service::PromptContextProviderPayload;
use crate::services::novel_quality_profile_service::build_novel_quality_prompt_blocks;
use crate::services::prompt_template_service::PromptTemplateService;
use serde_json::{Map, Value};

const QUALITY_RUNTIME_TRACKING_TAG: &str = "rule_v3_quality_block_20260307";
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

struct PromptInstructionSpec {
    key: &'static str,
    label: &'static str,
    chapter_bullets: &'static [&'static str],
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChapterGenerationPromptOverrides {
    pub narrative_perspective: Option<String>,
    pub creative_mode: Option<String>,
    pub story_focus: Option<String>,
    pub plot_stage: Option<String>,
    pub story_creation_brief: Option<String>,
    pub quality_preset: Option<String>,
    pub quality_notes: Option<String>,
    pub web_research_enabled: bool,
    pub web_research_query: Option<String>,
    pub story_repair_summary: Option<String>,
    pub story_repair_targets: Vec<String>,
    pub story_preserve_strengths: Vec<String>,
}

fn continuation_point(previous_chapter: Option<&chapter::Model>) -> String {
    previous_chapter
        .and_then(|item| item.content.clone())
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
        .chars()
        .rev()
        .take(500)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn previous_chapter_content(previous_chapter: Option<&chapter::Model>) -> String {
    previous_chapter
        .and_then(|item| item.content.clone())
        .map(|value| value.trim().to_string())
        .unwrap_or_default()
        .chars()
        .rev()
        .take(500)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PreviousChapterPromptContext {
    pub(crate) continuation_point: String,
    pub(crate) previous_chapter_content: String,
}

pub(crate) fn build_previous_chapter_prompt_context(
    previous_chapter: Option<&chapter::Model>,
) -> PreviousChapterPromptContext {
    PreviousChapterPromptContext {
        continuation_point: continuation_point(previous_chapter),
        previous_chapter_content: previous_chapter_content(previous_chapter),
    }
}

fn resolve_prompt_preference(
    override_value: Option<&str>,
    project_default: Option<&str>,
) -> String {
    override_value
        .filter(|value| !value.trim().is_empty())
        .or(project_default.filter(|value| !value.trim().is_empty()))
        .unwrap_or_default()
        .to_string()
}

fn build_optional_instruction_block(label: &str, value: &str) -> String {
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

fn normalize_creative_mode(value: &str) -> Option<&'static str> {
    normalize_prompt_alias(value, &CREATIVE_MODE_ALIASES)
}

fn normalize_story_focus(value: &str) -> Option<&'static str> {
    normalize_prompt_alias(value, &STORY_FOCUS_ALIASES)
}

fn normalize_plot_stage(value: &str) -> Option<&'static str> {
    normalize_prompt_alias(value, &PLOT_STAGE_ALIASES)
}

fn creative_mode_spec(normalized: &str) -> Option<&'static PromptInstructionSpec> {
    CREATIVE_MODE_SPECS
        .iter()
        .find(|spec| spec.key == normalized)
}

fn story_focus_spec(normalized: &str) -> Option<&'static PromptInstructionSpec> {
    STORY_FOCUS_SPECS.iter().find(|spec| spec.key == normalized)
}

fn plot_stage_label(normalized: &str) -> Option<&'static str> {
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

fn build_creative_mode_block(mode: &str) -> String {
    normalize_creative_mode(mode)
        .and_then(creative_mode_spec)
        .map(|spec| build_prompt_instruction_block("创作模式", "当前采用", spec))
        .unwrap_or_default()
}

fn build_story_focus_block(value: &str) -> String {
    normalize_story_focus(value)
        .and_then(story_focus_spec)
        .map(|spec| build_prompt_instruction_block("结构侧重点", "当前优先", spec))
        .unwrap_or_default()
}

fn dedupe_static_prompt_items(items: Vec<&'static str>) -> Vec<&'static str> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for item in items {
        let text = item.trim();
        if text.is_empty() || !seen.insert(text) {
            continue;
        }
        result.push(text);
    }
    result
}

fn build_chapter_combo_text(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
    fallback: &str,
) -> String {
    let mut labels = Vec::new();
    if let Some(label) = normalize_creative_mode(creative_mode)
        .and_then(creative_mode_spec)
        .map(|spec| spec.label)
    {
        labels.push(label);
    }
    if let Some(label) = normalize_story_focus(story_focus)
        .and_then(story_focus_spec)
        .map(|spec| spec.label)
    {
        labels.push(label);
    }
    if let Some(label) = normalize_plot_stage(plot_stage).and_then(plot_stage_label) {
        labels.push(label);
    }
    if labels.is_empty() {
        fallback.to_string()
    } else {
        labels.join(" / ")
    }
}

fn build_narrative_blueprint_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);

    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        return String::new();
    }

    let mut priority_beats: Vec<&'static str> = Vec::new();
    let mut priority_risks: Vec<&'static str> = Vec::new();

    match normalized_mode {
        Some("hook") => {
            priority_beats.extend([
                "开场更早抛出异常、危险或未完成目标，先抓住读者注意力。",
                "尾段优先保留信息缺口、危险临门或选择未决，不要平收。",
            ]);
            priority_risks.push("不要只堆钩子和异常，却缺少实质推进。");
        }
        Some("emotion") => {
            priority_beats.extend([
                "关键转折后要写出人物情绪余震和关系反应，不只交代结果。",
                "让动作、停顿和对白共同承载情绪，而不是全靠抒情说明。",
            ]);
            priority_risks.push("不要让情绪独自悬空，必须落回选择与后果。");
        }
        Some("suspense") => {
            priority_beats.extend([
                "中前段持续制造信息差、误判或证据变化，让压力逐步抬升。",
                "每个阶段都给出一点新认知，但不要一次讲透底牌。",
            ]);
            priority_risks.push("避免把悬念写成纯遮掩，读者需要看到有效推进。");
        }
        Some("relationship") => {
            priority_beats.extend([
                "把关键冲突尽量落在人与人之间的立场差、亏欠感或试探上。",
                "安排一次关系位移，让后续行动因为关系变化而改道。",
            ]);
            priority_risks.push("不要只有关系情绪，没有行动层面的后续影响。");
        }
        Some("payoff") => {
            priority_beats.extend([
                "优先安排前文铺垫兑现、收获反馈或阶段性反转，给读者明确回报。",
                "兑现后顺手打开下一轮更大的目标或麻烦，不把气口写死。",
            ]);
            priority_risks.push("不要只顾爽点回收，忽略代价与后续空间。");
        }
        Some("balanced") => {
            priority_beats.push("推进、情绪、信息释放和回报要彼此穿插，不让单一节拍统治全文。");
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            priority_beats.push("每个关键段都要写出行动结果和局势变化，避免原地解释。");
            priority_risks.push("避免设定说明和情绪回旋挤压主线推进。");
        }
        Some("deepen_character") => {
            priority_beats.push("至少安排一次能暴露人物弱点、执念或价值判断的选择。");
            priority_risks.push("不要把人物塑形写成静态介绍，必须落到行为上。");
        }
        Some("escalate_conflict") => {
            priority_beats.push("让阻力、代价和对立面逐段变强，形成持续抬压链条。");
            priority_risks.push("避免重复同级冲突，读者会觉得原地踏步。");
        }
        Some("reveal_mystery") => {
            priority_beats.push("优先安排线索出现、误导修正和认知刷新，至少推进一点真相。");
            priority_risks.push("不要把揭示写成解释堆叠，尽量通过事件和证据推进。");
        }
        Some("relationship_shift") => {
            priority_beats.push("对话、动作和站队变化都要服务关系转折，而不只是口头表态。");
            priority_risks.push("不要让关系变化只停留在情绪层，没有后续选择代价。");
        }
        Some("foreshadow_payoff") => {
            priority_beats.push("回收时既要兑现前文承诺，也要带出新的悬念或任务。");
            priority_risks.push("避免只用说明句回收伏笔，最好落在事件结果上。");
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            priority_beats.push("当前阶段优先扩张局势、铺开变量，并把选择成本逐章抬高。");
            priority_risks.push("避免太早交底或提前透支高潮。");
        }
        Some("climax") => {
            priority_beats.push("当前阶段要让核心矛盾正面碰撞，把选择逼到无法拖延的节点。");
            priority_risks.push("避免高潮只有声量，没有清晰结果与代价。");
        }
        Some("ending") => {
            priority_beats.push("当前阶段要优先收束主承诺、主悬念和关键关系线，再留余味。");
            priority_risks.push("避免只顾收尾，忘了兑现前文最重要的铺垫。");
        }
        _ => {}
    }

    let mut beat_candidates = priority_beats;
    beat_candidates.extend([
        "开场尽快抛出异常、目标或受阻点，不做平铺导入。",
        "中段用连续动作推进局势，并让阻力或代价升级。",
        "后段安排一次局势改写、信息刷新或关系位移。",
        "结尾保留明确追读牵引，不要平收。",
    ]);
    let beats = dedupe_static_prompt_items(beat_candidates);

    let mut risk_candidates = priority_risks;
    risk_candidates.push("不要把节拍写成说明书，关键节点都要有动作和即时结果。");
    let risks = dedupe_static_prompt_items(risk_candidates);

    let mut lines = vec![format!(
        "【结构蓝图】本轮按“{}”组织章节节拍",
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认结构")
    )];
    lines.extend(beats.into_iter().take(4).map(|item| format!("- {}", item)));
    if let Some(risk) = risks.first() {
        lines.push(format!("- 重点避免：{}", risk));
    }
    format!("{}\n", lines.join("\n"))
}

fn normalized_story_runtime_inputs(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> Option<(
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
)> {
    let normalized_mode = normalize_creative_mode(creative_mode);
    let normalized_focus = normalize_story_focus(story_focus);
    let normalized_stage = normalize_plot_stage(plot_stage);
    if normalized_mode.is_none() && normalized_focus.is_none() && normalized_stage.is_none() {
        None
    } else {
        Some((normalized_mode, normalized_focus, normalized_stage))
    }
}

fn build_story_objective_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut objective = "让本章推动一个看得见的目标，不写空转段落。";
    let mut obstacle = "安排一次明确受阻、代价上升或信息错位。";
    let mut turn = "在中后段安排一次认知或局面改写。";
    let mut hook = "章尾留下追读牵引，不平收。";

    match normalized_mode {
        Some("hook") => {
            hook = "把钩子放在异常、危险或未决选择上，尽量做到前段抓人、尾段牵引。";
            turn = "转折优先用信息缺口扩大、危险临门或局势突然偏转来触发。";
        }
        Some("emotion") => {
            objective = "让本章既推进事件，也逼出人物情绪与关系反应。";
            turn = "转折优先落在情绪反噬、误伤、和解受阻或认知偏移上。";
            hook = "钩子留在情绪未落地、关系未说破或选择仍有余震处。";
        }
        Some("suspense") => {
            obstacle = "阻力优先来自信息差、误判、证据反噬或真相未全。";
            turn = "转折通过线索翻面、认知刷新、身份异动或危险升级完成。";
            hook = "钩子留在新疑点、半揭开的答案或更近一步的危险上。";
        }
        Some("relationship") => {
            objective = "让本章推动一次明确的关系位移，而不只是情绪点缀。";
            obstacle = "阻力来自立场差、亏欠、信任裂缝或试探失手。";
            turn = "转折优先用关系破裂、突然靠近、站队变化或误会反转来完成。";
            hook = "钩子留在关系未定、话没说透、立场悬空的地方。";
        }
        Some("payoff") => {
            objective = "让本章承担一次明确兑现，让读者感到回报落地。";
            turn = "转折优先让兑现带出更大代价、更高目标或新的麻烦。";
            hook = "钩子放在回报之后的新失衡上，而不是只停在爽点本身。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            objective = "核心目标是把局势往前推一格，至少形成新的行动结果。";
        }
        Some("deepen_character") => {
            objective = "核心目标是让角色在选择里显形，暴露弱点、执念或价值判断。";
        }
        Some("escalate_conflict") => {
            obstacle = "阻力必须逐层变强，让代价和对立面都更具体。";
        }
        Some("reveal_mystery") => {
            turn = "转折优先通过线索出现、误导修正和认知刷新来完成。";
        }
        Some("relationship_shift") => {
            turn = "转折必须带来关系位移、立场重排或信任结构变化。";
        }
        Some("foreshadow_payoff") => {
            objective = "核心目标是兑现前文埋设，并顺手打开新的后续空间。";
            hook = "钩子留在兑现后的新承诺、新麻烦或更大代价上。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            objective = "当前阶段先把局势和眼前目标推到更难的位置。";
        }
        Some("climax") => {
            obstacle = "阻力要逼近正面碰撞，选择代价必须明显抬高。";
            turn = "转折要接近核心碰撞点，不能只是小波动。";
        }
        Some("ending") => {
            objective = "当前阶段让本章承担主承诺或关键关系线的回收职责。";
            hook = "钩子更适合留余味、次级悬念或收束后的新失衡，不能抢走主收束。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认任务");
    format!(
        "【章节目标卡】本轮按“{}”优先落实以下叙事任务\n- 目标：{}\n- 阻力：{}\n- 转折：{}\n- 钩子：{}\n",
        combo_text, objective, obstacle, turn, hook
    )
}

fn build_story_result_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut progress = "这一章结束后，局势应明确前移，人物不能还停在原地。";
    let mut reveal = "至少交付一个新认知、新线索或一次有效兑现。";
    let mut relationship = "至少有一条人物关系线出现可见变化，而不是只说情绪。";
    let mut fallout = "章尾要留下一个会逼出下章动作的余波，而不是平稳收住。";

    match normalized_mode {
        Some("hook") => {
            progress = "本章结束后，局势必须被推到一个不继续看就会难受的节点。";
            fallout = "余波优先落在未决选择、临门危险或刚被挑开的异常上。";
        }
        Some("emotion") => {
            reveal = "结果里要能看到情绪代价、误伤、和解受阻或内心认知变化。";
            relationship = "关系结果要落到互动后果上，让人物之后的做法因此改变。";
        }
        Some("suspense") => {
            reveal = "至少留下一个更接近真相的新证据，同时制造新的误判空间。";
            fallout = "余波留在新疑点、身份异动或危险升级上，不能只剩空白遮掩。";
        }
        Some("relationship") => {
            relationship = "结果里必须出现一次明确的关系位移、立场变化或信任重排。";
            fallout = "余波最好落在关系未定、话未说透或站队未稳上。";
        }
        Some("payoff") => {
            reveal = "结果要让读者看到铺垫兑现、回报落地，并感到不是白等。";
            progress = "兑现之后，局势要被顺势推向更高目标或更大麻烦。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            progress = "推进结果必须清晰可见：行动产生了后果，局势换了位置。";
        }
        Some("deepen_character") => {
            reveal = "结果要让人物的弱点、执念或价值判断真正显形，而非停在说明。";
            relationship = "人物变化要影响他与他人的互动方式或后续选择。";
        }
        Some("escalate_conflict") => {
            progress = "推进结果不是前进一步，而是把人推入更高代价的冲突区。";
            fallout = "余波要把冲突继续抬高，让下一轮没有轻松退路。";
        }
        Some("reveal_mystery") => {
            reveal = "揭示结果必须真实推进谜团，不只是制造更多模糊表述。";
        }
        Some("relationship_shift") => {
            relationship = "关系结果必须足够明确，能改变两人之后的说话方式、站位或合作条件。";
        }
        Some("foreshadow_payoff") => {
            reveal = "结果要让前文埋设获得兑现，同时打开新的后续空间。";
            fallout = "余波放在兑现后的新承诺、新代价或更大失衡上。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            progress = "这一章结束后，故事要进入一个更难但更清晰的推进区。";
            fallout = "余波要把后续任务钉住，让读者知道下一章不是重复上一章。";
        }
        Some("climax") => {
            progress = "推进结果要逼近或触发正面碰撞，不能只是外围晃动。";
            reveal = "揭示结果要掀开关键底牌、核心真相或决定性误判。";
        }
        Some("ending") => {
            reveal = "揭示结果优先服务主承诺、主悬念与关键伏笔的回收。";
            relationship = "关系结果要体现收束、定局或带余温的最终位移。";
            fallout = "余波更适合留余味、后效和新失衡，不能抢走主收束。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认结果");
    format!(
        "【章节结果卡】本轮写完后，至少让读者感知到以下结果变化（{}）\n- 推进：{}\n- 揭示：{}\n- 关系：{}\n- 余波：{}\n",
        combo_text, progress, reveal, relationship, fallout
    )
}

fn build_story_payoff_chain_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut seed_point = "本章最好承接一个前文钩点，或提前挂出一个本章内/近章可回收的小铺垫。";
    let mut payoff_point =
        "给读者一个看得见的兑现瞬间：动作打中、关系变位、计划起效、真相掀半层、承诺终于落地。";
    let mut feedback_chain = "兑现后立刻写反馈和余波，不只报结果，要让人物和局面都跟着变。";
    let mut reader_reward = "让追更读者在本章拿到一个明确回报，而不是一直被要求耐心等待。";
    let mut stage_line = "";
    let mut avoid_line = "不要只铺不收，也不要把兑现写成一句轻飘飘的结果播报。";

    match normalized_mode {
        Some("hook") => {
            payoff_point = "钩子型兑现最好来得更快，让读者早一点尝到“这章真的有事发生”的回报。";
        }
        Some("emotion") => {
            payoff_point =
                "情绪型兑现可以落在一句没说出口的话被说出、一次误解被捅破，或一次安慰彻底失败。";
            feedback_chain = "兑现后的余波优先写关系温差、情绪后坐力和人物自我认知变化。";
        }
        Some("suspense") => {
            payoff_point = "悬念型兑现更适合“揭半层真相 + 打开更危险缺口”，既满足又继续勾人。";
        }
        Some("relationship") => {
            payoff_point = "关系型兑现优先落在站位变化、信任转移、边界突破或彻底决裂。";
        }
        Some("payoff") => {
            seed_point = "优先锁定前文明确埋过的承诺、伏笔或能力点，不要再临时找替身回收。";
            reader_reward = "兑现时让读者清楚感到“前面那些铺垫没有白等”。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            feedback_chain = "兑现后的反馈必须推动主线进入下一格，别回收完又回到原地。";
        }
        Some("deepen_character") => {
            payoff_point = "兑现瞬间最好顺便照出人物的底线、成长、执念或迟来的代价感。";
        }
        Some("escalate_conflict") => {
            feedback_chain = "回收后不要泄压，最好把人物推进更难的冲突层级。";
        }
        Some("reveal_mystery") => {
            payoff_point = "优先给一个有效答案，但同时暴露更关键的缺口或更大的反常。";
        }
        Some("relationship_shift") => {
            reader_reward = "读者要能明显看见关系不一样了，而不是只在心理旁白里说“其实变了”。";
        }
        Some("foreshadow_payoff") => {
            seed_point = "尽量指定哪条旧伏笔要回收，不要泛泛地说“注意前后呼应”。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line = "发展阶段也要给小回收，让读者持续获得推进感，别把所有满足感都压后。";
        }
        Some("climax") => {
            stage_line = "高潮阶段优先回收最值钱的承诺和冲突，不要只继续预热更大的后面。";
            avoid_line = "不要在高潮里还只会继续铺垫和预告，却不给真正爆发与反馈。";
        }
        Some("ending") => {
            stage_line = "结局阶段优先回收主承诺、主关系和主谜面，再保留必要余波。";
            avoid_line = "不要在结局阶段把核心伏笔继续往后拖，削弱收束满足感。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认回收");
    let mut lines = vec![format!(
        "【章节爽点回收卡】本轮请形成可感知的“铺垫→兑现→反馈”链条（{}）",
        combo_text
    )];
    lines.push(format!("- 预埋点：{}", seed_point));
    lines.push(format!("- 兑现点：{}", payoff_point));
    lines.push(format!("- 反馈链：{}", feedback_chain));
    lines.push(format!("- 读者回报：{}", reader_reward));
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

fn build_story_rule_grounding_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut rule_landing =
        "本章至少让一个设定规则通过人物动作、现场反馈和后果被看见，而不是靠讲解出现。";
    let mut trigger_condition =
        "写清这条规则是怎么被触发的，谁触发、在什么条件下触发、为什么现在触发。";
    let mut cost_limit = "规则生效后要有边界、耗损、反噬、误差或现实牵连，避免像开挂按钮。";
    let mut scene_manifestation =
        "规则必须改写当下场面：让人受限、受益、失手、暴露、受伤或改变判断。";
    let mut stage_line = "";
    let mut avoid_line =
        "不要把设定写成只存在于说明文字里的背景板，也不要一触发就万能解决所有问题。";

    match normalized_mode {
        Some("hook") => {
            rule_landing = "设定最好一上来就制造麻烦、压力或危险，让规则本身成为抓手。";
        }
        Some("emotion") => {
            scene_manifestation =
                "规则落地最好能压到情绪与关系，让人物因为规则约束、代价或失手而受伤。";
        }
        Some("suspense") => {
            trigger_condition =
                "规则触发最好带出异常征兆、反常反馈或认知缺口，让读者感觉哪里不对。";
            cost_limit = "边界与代价不要一次讲完，先给足够可感的反常，再逐步揭开机制。";
        }
        Some("relationship") => {
            rule_landing = "设定最好落在身份、契约、门第、组织纪律或社会规则上，直接影响人物站位。";
        }
        Some("payoff") => {
            scene_manifestation = "优先让前文埋过的规则真正兑现，展示它终于生效时的爽点与后效。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            scene_manifestation = "规则生效后必须推动主线，不要只是展示世界观却不改局势。";
        }
        Some("deepen_character") => {
            trigger_condition =
                "最好通过人物主动触发、拒绝触发或误用规则，暴露他的价值判断与软肋。";
        }
        Some("escalate_conflict") => {
            cost_limit = "规则的代价、限制或反噬要把冲突抬高，而不是轻松替角色解围。";
        }
        Some("reveal_mystery") => {
            rule_landing = "规则落地应顺带暴露机制缺口、异常样本或隐藏条件，让谜团推进。";
        }
        Some("relationship_shift") => {
            scene_manifestation = "设定效果最好改写人与人之间的信任、合作权限或站队关系。";
        }
        Some("foreshadow_payoff") => {
            rule_landing = "优先回收前文提过的规则伏笔，让读者感到“原来之前那句设定现在真有用”。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line = "发展阶段先把最常用、最会咬人的规则边界立清楚，后面推进才有稳定抓手。";
        }
        Some("climax") => {
            stage_line = "高潮阶段让规则真正咬人或兑现，不要临近决战才重新解释一整套世界观。";
            avoid_line = "不要在高潮段落里突然停下来长讲机制说明，优先让规则直接在碰撞中显形。";
        }
        Some("ending") => {
            stage_line = "结局阶段优先回收最核心的规则承诺与代价，不要再抛全新体系。";
            avoid_line = "不要在结局阶段新增大块设定补丁，把收束重心冲散。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认设定落地");
    let mut lines = vec![format!(
        "【章节设定落地卡】本轮请让规则与设定真正进场（{}）",
        combo_text
    )];
    lines.push(format!("- 规则着陆：{}", rule_landing));
    lines.push(format!("- 触发条件：{}", trigger_condition));
    lines.push(format!("- 代价/限制：{}", cost_limit));
    lines.push(format!("- 场景表现：{}", scene_manifestation));
    lines.push(
        "- 硬指标：至少完成一条“触发条件→规则生效→限制/代价→局势变化”的完整链，禁止只讲设定不让设定出手。"
            .to_string(),
    );
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

fn build_story_information_release_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut new_info = "本章新信息尽量只命中一层：让读者明白当前最关键的规则、背景或动机即可。";
    let mut carrier = "把信息拆进动作、观察、对白和即时后果里，尽量让读者边看事边懂事。";
    let mut explanation_limit = "解释到能支撑当前冲突和理解即可，剩下的留给后续场景继续补。";
    let mut reader_handle = "新词、新职业、新力量或新关系出现时，尽快补一句读者能立刻听懂的人话。";
    let mut stage_line = "";
    let mut avoid_line = "不要在高潮动作中间突然插整段背景介绍，也不要连着三段都在解释。";

    match normalized_mode {
        Some("hook") => {
            carrier = "先抓事件，再补信息；解释要贴着异常、危险或选择出现，别抢在钩子前面。";
        }
        Some("emotion") => {
            carrier = "信息最好从争执、试探、隐瞒、误解或安慰失败里漏出来，而不是平铺直叙。";
        }
        Some("suspense") => {
            new_info = "悬念型信息优先只揭半层：给可追踪的新线索，不把底牌一口气翻完。";
            explanation_limit = "解释要刚好够读者继续猜，不要把所有反常都立刻讲穿。";
        }
        Some("relationship") => {
            carrier = "信息最好挂在关系互动里，用谁敢说、谁不肯说、谁故意隐瞒来制造张力。";
        }
        Some("payoff") => {
            new_info = "优先释放与兑现直接相关的信息，让读者知道这次回收了什么、又打开了什么后效。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            new_info = "只放能推动主线前进的信息，和当前推进无关的设定先别急着补。";
        }
        Some("deepen_character") => {
            carrier = "信息最好通过人物选择、口误、回避和偏见露出来，而不是作者代说。";
        }
        Some("escalate_conflict") => {
            reader_handle = "让读者迅速明白这条信息为什么会让局势更糟、更难、更贵。";
        }
        Some("reveal_mystery") => {
            new_info = "优先放能推进谜团的一小块有效信息，而不是旁枝背景。";
            explanation_limit = "每次只多揭一层，不要直接把谜底和世界观补课一起打包端上来。";
        }
        Some("relationship_shift") => {
            carrier = "信息最好通过立场变化、试探问答、隐瞒失效或关系破口流出来。";
        }
        Some("foreshadow_payoff") => {
            new_info =
                "信息释放要服务于伏笔回收，让读者在“原来如此”和“接下来怎么办”之间获得连锁反馈。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line = "发展阶段重点是把任务所需的最小信息量说清，别一开始就把整套世界全摊开。";
        }
        Some("climax") => {
            stage_line = "高潮阶段压缩说明比例，优先用已建立的信息打架，让新增解释只服务当下决断。";
            avoid_line = "不要在高潮关键碰撞前后连续长讲设定，把情绪和动作气口掐断。";
        }
        Some("ending") => {
            stage_line = "结局阶段优先投放回收性信息和结果性信息，不要突然补大量新设定。";
            avoid_line = "不要在结局处开启新的百科讲解，避免把收束拉回说明书。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认信息投放");
    let mut lines = vec![format!(
        "【章节信息投放卡】本轮请控制信息释放方式与密度（{}）",
        combo_text
    )];
    lines.push(format!("- 本轮信息：{}", new_info));
    lines.push(format!("- 承载方式：{}", carrier));
    lines.push(format!("- 解释上限：{}", explanation_limit));
    lines.push(format!("- 读者抓手：{}", reader_handle));
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

fn build_story_emotion_landing_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut trigger_point = "本章关键情绪先落在明确触发事件上，别让情绪像凭空冒出来。";
    let mut outer_reaction =
        "优先写呼吸、停顿、动作错位、措辞变化、沉默和失控边缘，而不是直接给标签。";
    let mut relationship_wave = "让情绪改变人与人之间的距离、说话方式、信任程度或之后的选择。";
    let mut layered_shift = "情绪推进尽量分层：先忍、再裂、再回避/反击/崩掉，不要一步到顶。";
    let mut stage_line = "";
    let mut avoid_line = "不要连续几句旁白都在盖章人物心情，也不要把复杂情绪一句话写死。";

    match normalized_mode {
        Some("hook") => {
            trigger_point = "开场情绪最好直接绑定险情、麻烦或打断，让压力先压到人物身上。";
        }
        Some("emotion") => {
            outer_reaction =
                "情绪型段落更要靠停顿、改口、嘴硬、回避和细小动作发声，而不是抒情盖章。";
            layered_shift = "情绪最好出现误伤、自我压抑、短暂失控和余波回流的层次。";
        }
        Some("suspense") => {
            trigger_point = "悬念型情绪优先来自异常、误判、恐惧和答案缺口，而不是纯抒情。";
        }
        Some("relationship") => {
            relationship_wave =
                "关系戏里的情绪重点是靠近失败、信任松动、边界被碰、迟到的理解或不肯承认。";
        }
        Some("payoff") => {
            layered_shift =
                "兑现后的情绪别只停在爽或痛，要继续写余震、亏欠、松一口气后的空心或新责任。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            outer_reaction = "情绪反应之后最好立刻影响下一步行动，不让情绪段和主线脱节。";
        }
        Some("deepen_character") => {
            layered_shift =
                "人物塑形时优先写他怎么忍、怎么装、怎么解释自己，而不是作者替他总结性格。";
        }
        Some("escalate_conflict") => {
            relationship_wave =
                "冲突升级时让情绪带来误伤、顶撞、失控或撤回援手，而不是只提高音量。";
        }
        Some("reveal_mystery") => {
            trigger_point = "谜团推进时把情绪绑定到“看懂了一半”和“更不安了”这种认知落差上。";
        }
        Some("relationship_shift") => {
            relationship_wave =
                "关系变化重点写温差、试探落空、迟疑和态度微偏，不只写一句“关系变了”。";
        }
        Some("foreshadow_payoff") => {
            trigger_point = "伏笔兑现时优先写人物对旧承诺、旧创伤、旧误解被碰到时的即时反应。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line = "发展阶段先把情绪触发与余波立住，让后续人物线有持续发酵空间。";
        }
        Some("climax") => {
            stage_line = "高潮阶段情绪要跟着碰撞一起爆，不要躲回长段抒情和解释。";
            avoid_line = "不要在高潮情绪点后立刻用旁白把人物全部解释完，冲掉现场余震。";
        }
        Some("ending") => {
            stage_line = "结局阶段的情绪更适合落在余波、代价、和解未尽或迟来的理解上。";
            avoid_line = "不要在结尾把所有情绪做成统一口号式总结，留一点人味和回声。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认情绪落点");
    let mut lines = vec![format!(
        "【章节情绪落点卡】本轮请把情绪压回现场与关系里（{}）",
        combo_text
    )];
    lines.push(format!("- 触发点：{}", trigger_point));
    lines.push(format!("- 外显反应：{}", outer_reaction));
    lines.push(format!("- 关系余波：{}", relationship_wave));
    lines.push(format!("- 层次推进：{}", layered_shift));
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

fn build_story_action_rendering_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut action_start = "本章关键桥段先写动作发起：谁出手、谁试探、谁先失手、谁先顶上去。";
    let mut collision_feedback = "动作里要有碰撞反馈：被挡住、打偏、误判、迟疑、反咬、变招或代价。";
    let mut visible_change = "动作之后必须带来可见变化，不只报结果，要看见场面怎么被改写。";
    let mut lens_priority = "最值钱的冲突、破局、兑现和危险临门尽量给现场镜头，不要躲去摘要句。";
    let mut stage_line = "";
    let mut avoid_line = "不要把整场关键动作压成“他们打了一阵”“事情很快解决了”这种概述。";

    match normalized_mode {
        Some("hook") => {
            action_start = "钩子段优先让动作先响，先让事情发生，再补解释。";
        }
        Some("emotion") => {
            collision_feedback =
                "情绪戏里的动作也要显形：推开、停住、没接住、想碰又收回，比抽象形容更有劲。";
        }
        Some("suspense") => {
            visible_change = "悬念型动作优先留下新反常、新危险或新证据，不要动作做完什么都没变。";
        }
        Some("relationship") => {
            action_start = "关系戏里的关键动作可以是靠近、退开、挡住、递回去、没接、转身或越界。";
        }
        Some("payoff") => {
            lens_priority = "兑现型桥段更要现场化，把最值钱的那一下真正写在台前。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            visible_change = "动作结束后主线最好明确前进一格，而不是热闹完还在原地。";
        }
        Some("deepen_character") => {
            collision_feedback = "动作反馈要顺手照出人物习惯、底线、软肋和犹豫，不只看热闹。";
        }
        Some("escalate_conflict") => {
            action_start = "冲突升级时优先写更难的现场碰撞，不靠旁白宣布“局势更严重了”。";
        }
        Some("reveal_mystery") => {
            visible_change = "动作之后最好掉出线索、破绽、证据或更大的缺口。";
        }
        Some("relationship_shift") => {
            collision_feedback = "关系变化尽量通过动作错位、接与不接、站位变化和边界碰撞来显形。";
        }
        Some("foreshadow_payoff") => {
            lens_priority = "伏笔兑现时优先写兑现发生的那一刻，不要只在事后回顾“原来如此”。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line = "发展阶段先把关键动作链写清，别让中段长期停在说明和准备态。";
        }
        Some("climax") => {
            stage_line = "高潮阶段的动作要更现场、更具体、更有反馈，不要只剩结果播报。";
            avoid_line = "不要在高潮关键桥段里大量省略动作过程，让最该爆的地方直接哑火。";
        }
        Some("ending") => {
            stage_line = "结局阶段优先现场化最重要的兑现、告别、冲突终局和代价落地。";
            avoid_line = "不要在收尾阶段把关键回收全写成叙述总结，削弱满足感。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认动作显影");
    let mut lines = vec![format!(
        "【章节动作显影卡】本轮请把关键桥段写成可见动作链（{}）",
        combo_text
    )];
    lines.push(format!("- 起手动作：{}", action_start));
    lines.push(format!("- 碰撞反馈：{}", collision_feedback));
    lines.push(format!("- 局面变化：{}", visible_change));
    lines.push(format!("- 镜头优先：{}", lens_priority));
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

fn build_story_summary_tone_control_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut conclusion_hold = "本章少直接宣布人物心境、关系定性和主题意义，优先把判断埋进现场。";
    let mut replacement_path =
        "该写结论时，尽量换成动作停顿、没说出口的话、被看见的物件和局面变化。";
    let mut blank_space = "给读者留一点自己体会的空间，不要刚发生完就立刻替他总结感受。";
    let mut sentence_control =
        "少用抽象总结句和命运句，尤其别用旁白把人物成长、爱情或主题一次性说穿。";
    let mut stage_line = "";
    let mut avoid_line = "不要连续用“他终于明白”“她忽然懂得”“这意味着一切都变了”收段。";

    match normalized_mode {
        Some("hook") => {
            conclusion_hold = "钩子段更要少总结，优先把问题留在事件和动作上。";
        }
        Some("emotion") => {
            replacement_path = "情绪结论尽量改成呼吸、目光、错开的动作、答非所问和沉默。";
            blank_space = "情绪戏别刚掀起就旁白总结，给余波一点扩散空间。";
        }
        Some("suspense") => {
            sentence_control = "悬念段更要克制解释性总结，别一边卖疑一边把答案和意义都旁白清楚。";
        }
        Some("relationship") => {
            replacement_path =
                "关系变化尽量通过称呼、距离、口气、是否接话和是否站到一起表现，不靠盖章。";
        }
        Some("payoff") => {
            conclusion_hold = "兑现后少讲大道理，优先让反馈和代价证明这次回收值不值。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            replacement_path = "主线推进时用“发生了什么变化”代替“这意味着什么”，让局势自己发声。";
        }
        Some("deepen_character") => {
            blank_space = "人物塑形时少替人物写人物小结，保留一些矛盾和自欺让读者自己品。";
        }
        Some("escalate_conflict") => {
            sentence_control = "冲突升级时少复盘和评点，让更贵的动作和后果承担说服力。";
        }
        Some("reveal_mystery") => {
            conclusion_hold = "揭谜时只给必要答案，不顺手把主题点评和全部意义打包讲完。";
        }
        Some("relationship_shift") => {
            replacement_path =
                "关系变化更适合落在没接住的话、退后的半步、迟疑和让步上，而不是口头定性。";
        }
        Some("foreshadow_payoff") => {
            blank_space = "回收伏笔时让“原来如此”的快感由前后呼应产生，不用旁白替读者喊出来。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line = "发展阶段先克制解释欲，让读者跟着事件自己建立判断。";
        }
        Some("climax") => {
            stage_line = "高潮阶段尤其要少讲道理，让碰撞、代价和沉默承担重量。";
            avoid_line = "不要在高潮关键段突然插长句评语，把现场冲击改写成作者感悟。";
        }
        Some("ending") => {
            stage_line = "结局阶段允许有余味，但不等于大段讲主题总结，优先让结尾意象和余波说话。";
            avoid_line = "不要在收尾用旁白把所有主题、成长和命运一次性解释完。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认总结克制");
    let mut lines = vec![format!(
        "【章节总结腔抑制卡】本轮请减少作者盖章式结论（{}）",
        combo_text
    )];
    lines.push(format!("- 结论克制：{}", conclusion_hold));
    lines.push(format!("- 替代表现：{}", replacement_path));
    lines.push(format!("- 留白位置：{}", blank_space));
    lines.push(format!("- 句式控制：{}", sentence_control));
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

fn build_story_repetition_control_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut repeat_target = "本章同一信息、情绪、设定提醒和人物判断尽量只打一次重击，别连着复述。";
    let mut first_hit = "第一次出现时尽量让它足够清晰、足够具体，后面就用动作和后果承接。";
    let mut later_handle = "后续若必须再提，最好带出升级、反转、误差或代价，不只原话重来。";
    let mut merge_rule = "相邻段若在做同一件事，优先删掉弱重复，保留最有效的一次表达。";
    let mut stage_line = "";
    let mut avoid_line =
        "不要前一段刚说完人物害怕、设定危险或任务困难，后一段马上换说法再提醒一遍。";

    match normalized_mode {
        Some("hook") => {
            first_hit = "钩子信息第一次出现就要够尖，别靠反复提醒硬撑抓力。";
        }
        Some("emotion") => {
            repeat_target = "情绪不要连着用近义词重复盖章，优先让余波和动作替情绪继续发声。";
        }
        Some("suspense") => {
            later_handle = "悬念再提时要带新反常或新缺口，别只是重复“事情不对劲”。";
        }
        Some("relationship") => {
            merge_rule = "关系拉扯不要连续两三轮都在说同一种疏离或暧昧，要让关系位置真的变。";
        }
        Some("payoff") => {
            first_hit = "回收点第一次兑现时就把满足感打满，别后面再靠解释重复证明它很重要。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            later_handle = "主线推进时，重复提旧问题不如让问题进入新阶段。";
        }
        Some("deepen_character") => {
            repeat_target = "人物塑形别反复旁白同一性格标签，优先换成不同场景下的新选择来证明。";
        }
        Some("escalate_conflict") => {
            later_handle = "冲突升级时要给更高代价和新碰撞，不要只反复提醒“矛盾很激烈”。";
        }
        Some("reveal_mystery") => {
            merge_rule = "谜团提示要层层推进，不重复播报同一团迷雾。";
        }
        Some("relationship_shift") => {
            later_handle = "关系变化再提时要让说话方式、站位或行动条件变化，而不是重说“他们变了”。";
        }
        Some("foreshadow_payoff") => {
            first_hit = "伏笔第一次埋下就尽量精准，后面少反复提醒存在感。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line = "发展阶段尤其容易水在重复提醒里，要尽快把同类信息压缩成一次有效命中。";
        }
        Some("climax") => {
            stage_line = "高潮阶段少复盘、少重复解释，让碰撞和后果接管篇幅。";
            avoid_line = "不要在高潮段落连续复述同一危险、同一情绪和同一动机，削弱冲击。";
        }
        Some("ending") => {
            stage_line = "结局阶段优先用结果和余波说话，不要反复回顾已经兑现的东西。";
            avoid_line = "不要在收尾用多段重复总结同一主题和同一成长，拖慢收束。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认压缩");
    let mut lines = vec![format!(
        "【章节重复压缩卡】本轮请减少同义复述与连续提醒（{}）",
        combo_text
    )];
    lines.push(format!("- 重复对象：{}", repeat_target));
    lines.push(format!("- 首次命中：{}", first_hit));
    lines.push(format!("- 后续处理：{}", later_handle));
    lines.push(format!("- 删并原则：{}", merge_rule));
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

fn build_story_viewpoint_discipline_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut camera_focus = "本章关键场景尽量贴住一个主视角，让读者跟着同一双眼睛承受信息差和压力。";
    let mut visible_boundary =
        "当前人物不知道的东西，尽量不要直接盖章给读者，先通过异常、动作和线索侧写。";
    let mut inner_access = "内心戏优先写主视角人物的当下反应，不要一句话顺手把周围所有人都看透。";
    let mut switch_rule =
        "要切视角时，尽量借章节断点、明确场景跳转或强需求切换，不在紧张现场横跳。";
    let mut stage_line = "";
    let mut avoid_line = "不要上一句还在甲的脑子里，下一句就跳进乙的内心，再下一句作者来总结真相。";

    match normalized_mode {
        Some("hook") => {
            camera_focus = "钩子段尽量贴住最先承受异常、危险或任务压力的人，让抓力更直接。";
        }
        Some("emotion") => {
            inner_access = "情绪型段落优先写体感、误读、嘴硬和停顿，不要全靠作者替人物命名情绪。";
        }
        Some("suspense") => {
            visible_boundary = "悬念型段落更要守住可见边界，不要为了省事提前透出标准答案。";
            avoid_line = "不要一边让人物发懵，一边又让旁白抢先把谜底和真意解释完。";
        }
        Some("relationship") => {
            inner_access =
                "关系戏里更适合通过对视、回避、打断和措辞变化显露双方状态，而不是双向内心旁白轮流讲解。";
        }
        Some("payoff") => {
            camera_focus = "兑现瞬间尽量贴住最能感到“终于到了”的人物，让回报更有代入感。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            camera_focus = "优先跟随最能推动主线下一步的人物视角，少切去旁支人物分散推进。";
        }
        Some("deepen_character") => {
            inner_access = "聚焦人物做选择时的偏见、软肋和自我辩解，不用全知口吻替他写人物小传。";
        }
        Some("escalate_conflict") => {
            visible_boundary = "冲突升级时更要守住局中人视角，让错误判断和迟来的发现保留张力。";
        }
        Some("reveal_mystery") => {
            switch_rule = "如需切视角揭新线索，必须让切换本身带来新证据，而不是单纯替作者补课。";
        }
        Some("relationship_shift") => {
            inner_access = "关系变化优先让读者从主视角的误判、迟疑、试探和受伤里感到变化。";
        }
        Some("foreshadow_payoff") => {
            camera_focus = "回收伏笔时尽量站在最受那条伏笔影响的人物身上，让兑现更有分量。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line = "发展阶段先把主镜头稳定住，让读者知道该跟谁看、跟谁担心。";
        }
        Some("climax") => {
            stage_line = "高潮阶段更要贴住最疼、最险、最难选的那个视角，少横跳、少俯视。";
            avoid_line = "不要在高潮现场频繁切镜头解释全局，导致碰撞被切碎、情绪被稀释。";
        }
        Some("ending") => {
            stage_line = "结局阶段的视角切换应服务收束与余味，不要为了补信息乱开上帝视角。";
            avoid_line = "不要在结尾靠作者总结式全知旁白把人物命运一次性说教完。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认视角");
    let mut lines = vec![format!(
        "【章节视角纪律卡】本轮请稳定镜头与信息边界（{}）",
        combo_text
    )];
    lines.push(format!("- 主镜头：{}", camera_focus));
    lines.push(format!("- 可见边界：{}", visible_boundary));
    lines.push(format!("- 内心准入：{}", inner_access));
    lines.push(format!("- 切换条件：{}", switch_rule));
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

fn build_story_dialogue_advancement_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut dialogue_task =
        "本章关键对白要推动局势、关系或选择，不要只是把读者已经知道的信息再说一遍。";
    let mut information_gap =
        "对白里要有信息差：有人在试探、有人在藏、有人没听懂、有人故意说半句。";
    let mut voice_split = "角色说话方式要分得开：句长、词汇、礼貌度、火气、停顿和潜台词都别一样。";
    let mut action_support =
        "对白之间穿插动作、表情、环境反应和沉默，让说出口和没说出口的东西一起工作。";
    let mut stage_line = "";
    let mut avoid_line = "不要一轮对白全是完整长句和总结句，也不要让角色轮流替作者解释世界观。";

    match normalized_mode {
        Some("hook") => {
            dialogue_task = "对白最好一开口就带压力、问题或威胁，让读者立刻感觉有事要炸。";
        }
        Some("emotion") => {
            information_gap = "情绪型对白重点不在“说清楚”，而在谁嘴硬、谁避重就轻、谁说了反话。";
            action_support = "动作陪跑优先写停顿、改口、没接住的安慰和说完后的余震。";
        }
        Some("suspense") => {
            information_gap = "悬念型对白要保留缺口：一句话只揭半层，最好带出新疑点或相互矛盾。";
        }
        Some("relationship") => {
            dialogue_task = "对白要承担站位试探、边界确认或关系升降温，别只是客观交流信息。";
            voice_split = "关系越近越敢打断、绕弯、戳痛点；关系越远越讲分寸、试探和保留。";
        }
        Some("payoff") => {
            dialogue_task =
                "兑现型对白要让人物对结果作出反应：承认、嘴硬、错愕、反咬或迟来的理解。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            dialogue_task = "对白结束后应推动行动计划、立场判断或主线下一步，而不是原地聊完。";
        }
        Some("deepen_character") => {
            voice_split = "对白重点是把人物软肋、执念、教养和惯性露出来，不是统一输出正确答案。";
        }
        Some("escalate_conflict") => {
            information_gap = "冲突型对白要让误解更深、底牌更露或退路更少，别聊完反而泄压。";
        }
        Some("reveal_mystery") => {
            dialogue_task = "对白里优先放试探、交叉验证和半真半假的线索，不要直接口述谜底。";
        }
        Some("relationship_shift") => {
            action_support = "对话结束后最好能看见站位变化、沉默拉长、目光回避或合作条件改变。";
        }
        Some("foreshadow_payoff") => {
            dialogue_task = "对白可以顺手回收旧台词、旧承诺或旧误会，让熟悉信息产生新含义。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line = "发展阶段的对白重点是尽快立清关系、任务和信息差，让后续冲突有抓手。";
        }
        Some("climax") => {
            stage_line = "高潮阶段对白要短、狠、准，优先服务摊牌、碰撞和底线暴露。";
            avoid_line = "不要在高潮对白里长篇复盘前情或讲大道理，把碰撞气口拖死。";
        }
        Some("ending") => {
            stage_line = "结局阶段对白更适合落在承认、告别、没说完的余味或代价后的新关系。";
            avoid_line = "不要在结局里靠大段解释把所有情绪说穿，留一点人味和余波。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认对白推进");
    let mut lines = vec![format!(
        "【章节对白推进卡】本轮请让关键对白真正推动故事（{}）",
        combo_text
    )];
    lines.push(format!("- 对话任务：{}", dialogue_task));
    lines.push(format!("- 信息落差：{}", information_gap));
    lines.push(format!("- 声线区分：{}", voice_split));
    lines.push(format!("- 动作陪跑：{}", action_support));
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

fn build_story_opening_hook_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut first_strike = "开篇前几段尽快给出异常、险情、冲突或打断日常的事件，不要慢热兜圈。";
    let mut trouble_seed = "第一轮动作里要埋下会继续追着人物跑的麻烦种子，而不是一次性小插曲。";
    let mut unresolved_question =
        "开场后尽快形成一个具体未决问题，让读者明确想知道下一步会发生什么。";
    let mut stage_line = "";
    let mut avoid_line = "不要用天气、环境、回忆或泛情绪独白拖长预热，却迟迟没有真正抓手。";

    match normalized_mode {
        Some("hook") => {
            first_strike = "第一击优先落在异常、险情、失衡或强制选择上，先抓住人再补信息。";
            unresolved_question = "未决问题最好带明确倒计时、后果或风险，而不是空泛地卖关子。";
        }
        Some("emotion") => {
            trouble_seed = "麻烦种子最好和关系裂缝、误伤余震或压抑失败绑定，让情绪从开头就带刺。";
            avoid_line = "不要只写情绪氛围和内心感受，却没有触发情绪的外部事件。";
        }
        Some("suspense") => {
            first_strike = "第一击优先给出异常迹象、线索反常、危险逼近或认知落差。";
            unresolved_question = "未决问题应当具体到谁在做什么、哪里不对、真相缺了哪一块。";
        }
        Some("relationship") => {
            trouble_seed = "麻烦种子最好是站位变化、信任裂缝、关系失衡或合作条件改变。";
            unresolved_question = "开头要让读者关心这段关系接下来会靠近、决裂还是暂时停摆。";
        }
        Some("payoff") => {
            first_strike = "第一击可以直接掀开旧承诺开始兑现，或让旧伏笔先产生回响和副作用。";
            trouble_seed = "兑现之后要立刻带出新的失衡、代价或连锁反应，不要只给一个爽点就停。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            first_strike = "开场动作要直接推动主线，不要热闹很多却没有实际推进。";
        }
        Some("deepen_character") => {
            trouble_seed = "麻烦种子最好能逼出人物软肋、执念或底线，而不是只补背景设定。";
        }
        Some("escalate_conflict") => {
            first_strike = "第一击最好就是一次对立碰撞、局势加压或安全区失效。";
            unresolved_question = "未决问题要落在冲突会升级到什么程度、谁先扛不住、谁会失手上。";
        }
        Some("reveal_mystery") => {
            first_strike = "开头尽快抛出异常证据、反常细节或新线索，不要把谜团完全藏在后半段。";
        }
        Some("relationship_shift") => {
            trouble_seed = "麻烦种子最好让关系一开始就处在新的拉扯位置，而不是老样子慢慢磨。";
        }
        Some("foreshadow_payoff") => {
            first_strike = "开场可以先响一下旧伏笔，让读者迅速意识到这次不是无关紧要的新事件。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line =
                "发展阶段的开篇重点是尽快把本轮主任务、变量和压力源摆上桌，别一直停在准备态。";
        }
        Some("climax") => {
            stage_line = "高潮阶段的开篇要延续既有高压，不要重新慢启动或重新铺盘子。";
            avoid_line = "不要在高潮章/卷开头突然切回长铺垫、慢解释或轻松日常，导致气压掉线。";
        }
        Some("ending") => {
            stage_line = "结局阶段的开篇优先抓回核心承诺、关键关系或最后代价，不要另起大盘。";
            avoid_line = "不要在结局阶段开头又抛全新主线，把读者注意力从收束目标上拉开。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认抓力");
    let mut lines = vec![format!(
        "【章节开篇抓力卡】开场请尽快建立抓手与牵引（{}）",
        combo_text
    )];
    lines.push(format!("- 第一击：{}", first_strike));
    lines.push(format!("- 麻烦种子：{}", trouble_seed));
    lines.push(format!("- 未决问题：{}", unresolved_question));
    lines.push(
        "- 硬指标：开篇前 20%-25% 内至少落地 1 个抓手（目标 / 异常 / 受阻 / 强制选择），且不能连续两段只做背景预热。"
            .to_string(),
    );
    lines.push(
        "- 二级硬指标：最好前 120-180 字内同时出现两类抓手（异常 / 任务 / 受阻 / 倒计时 / 强制选择 / 对立问句），并让第一轮动作立刻制造余波。"
            .to_string(),
    );
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

fn build_story_execution_checklist_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut opening = "开场 30% 内抛出目标、异常或受阻点，不平铺背景。";
    let mut pressure = "中段用动作、对话和反馈连续加压，避免解释停顿。";
    let mut pivot = "中后段安排一次改写认知或局面的关键动作。";
    let mut closing = "收尾先落结果，再留下逼出下章的余波。";

    match normalized_mode {
        Some("hook") => {
            opening = "开场尽快抛出异常、险情或未决选择，让读者立刻进入状态。";
            closing = "收尾把悬而未决的危险、选择或信息缺口钉牢，形成追读牵引。";
        }
        Some("emotion") => {
            pressure = "中段用互动、误伤、退让受阻或情绪回弹来持续加压。";
            pivot = "关键转折优先落在情绪爆裂、和解失败或认知刺痛上。";
            closing = "收尾保留情绪余震，让人物无法当场彻底消化。";
        }
        Some("suspense") => {
            opening = "开场先扔出异常线索、误判苗头或危险信号，再补背景。";
            pressure = "中段不断扩大信息差、证据变化和错误判断的代价。";
            pivot = "转折优先让线索翻面、身份异动或危险升级来改写局面。";
            closing = "收尾留下更尖锐的新疑点，而不是只把答案藏起来。";
        }
        Some("relationship") => {
            opening = "开场先把关系张力、站位差或试探动作摆上台面。";
            pressure = "中段持续通过对话、行动和站队测试来挤压关系。";
            pivot = "转折优先用关系破裂、突然靠近或立场变化来触发。";
            closing = "收尾把关系悬在未定状态，逼出下一轮互动。";
        }
        Some("payoff") => {
            opening = "开场尽快回扣前文埋设，提醒读者这轮会有兑现。";
            pressure = "中段不断把兑现条件推近，同时抬高兑现所需代价。";
            pivot = "转折优先让铺垫兑现落地，但必须伴随新后果。";
            closing = "收尾不要停在爽点，要顺手抛出兑现后的新失衡。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            opening = "开场先亮明本轮要推进的事，别让读者等太久才知道这章要干嘛。";
            pressure = "中段每次推进都要带来新结果，避免原地解释和空转。";
        }
        Some("deepen_character") => {
            pressure = "中段把压力尽量变成选择题，让人物性格在决策里显形。";
            pivot = "关键转折最好来自人物自己的选择、软肋或价值判断。";
            closing = "收尾保留人物做完选择后的余震，而不是只交代事件结束。";
        }
        Some("escalate_conflict") => {
            pressure = "中段每一轮加压都要比上一轮更狠，别重复同级冲突。";
            pivot = "转折要把冲突推向正面碰撞，而不是继续绕圈。";
            closing = "收尾把人物钉在更高代价区，确保下一轮没法轻退。";
        }
        Some("reveal_mystery") => {
            opening = "开场尽快抛出线索、异常或疑点，别先讲设定。";
            pressure = "中段通过调查、误导修正和证据变化推进认知。";
            pivot = "转折要真正修正一次认知，而不是只多说一点背景。";
        }
        Some("relationship_shift") => {
            pressure = "中段每次互动都要推动信任、亏欠、戒备或站队发生位移。";
            pivot = "转折要让关系位置真正改变，而不是嘴上吵完又回原点。";
            closing = "收尾留下新的关系姿态或未兑现承诺，逼出后续互动。";
        }
        Some("foreshadow_payoff") => {
            opening = "开场尽快把前文埋下的人、物、承诺或代价重新拉回现场。";
            pivot = "关键转折优先落实伏笔兑现，并让读者看见兑现后的连锁反应。";
            closing = "收尾保留回收后的新缺口，避免把兑现写成句号。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            opening = "发展阶段先把当前场景秩序和人物站位立稳，再推进变量入场。";
            pivot = "发展阶段至少安排一次让局面升级或关系改写的关键动作。";
            closing = "收尾先压实当前推进结果，再给后续升级留口。";
        }
        Some("climax") => {
            opening = "高潮阶段开场尽快把人物推到主碰撞现场，不再外围试探。";
            pressure = "中段持续抬高代价、时限和压迫，不能退回解释区。";
            pivot = "转折必须推动正面碰撞、关键反转或局势翻面。";
            closing = "收尾先落下当前碰撞结果，再把更大的余波推向下章。";
        }
        Some("ending") => {
            opening = "收束阶段开场尽快把待回收的承诺、关系或真相重新拉回台面。";
            pressure = "中段围绕最终代价、兑现与收束推进，不再横生新主枝线。";
            pivot = "关键转折优先完成回收并揭示最后代价，别再新开大主线。";
            closing = "收尾要完成阶段性回收，同时留下明确余味或尾问。";
        }
        _ => {}
    }

    let opening = format!(
        "{} 前 20%-25% 内至少给出目标、异常或受阻点之一。 最好前 120-180 字内就同时出现两类抓手（异常 / 任务 / 受阻 / 倒计时 / 强制选择）。",
        opening
    );
    let pressure = format!(
        "{} 中段至少完成一次“推进→受阻→决断→代价/反弹”的冲突链。",
        pressure
    );
    let pivot = format!("{} 关键动作最好伴随一条设定规则的触发、限制或反噬。", pivot);
    let closing = format!(
        "{} 最后一段必须留下新的信息缺口、危险逼近、身份位移或待做选择之一。 最后一行禁止复盘解释或抒情软收，优先落在指令、锁定、翻面信息、逼近危险或未完成选择上。",
        closing
    );

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认执行节奏");
    format!(
        "【章节执行清单】本轮优先按以下节奏执行（{}）\n- 开场：{}\n- 加压：{}\n- 转折：{}\n- 收束：{}\n",
        combo_text, opening, pressure, pivot, closing
    )
}

fn build_story_scene_anchor_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut entry_anchor = "开场3-5句内交代人在何处、正在做什么、眼前压力从哪来，让读者先站稳。";
    let mut lens_focus = "单场景优先盯住一个镜头重心（动作推进/关系碰撞/线索识别其一），别四处撒。";
    let mut info_release = "新信息优先嵌进动作、观察、对白和即时反应里，一次只释放一层。";
    let mut transition_rule =
        "切换时间、地点或行动阶段时，用简短动作或环境变化做承接，避免镜头空跳。";

    match normalized_mode {
        Some("hook") => {
            entry_anchor = "开场第一时间让异常、危险或任务阻力进入场内，别先讲完整背景。";
            lens_focus = "镜头优先跟着最能制造牵引的问题走，别被枝节说明抢掉主注意力。";
            info_release = "关键情报分两步以内放出，不一次把答案和解释全说透。";
        }
        Some("emotion") => {
            lens_focus = "镜头优先盯动作停顿、身体距离、视线变化和话没说满的地方。";
            info_release = "情绪信息优先藏在回避、试探、失控边缘和即时反应里，不整段抒情讲完。";
        }
        Some("suspense") => {
            entry_anchor = "先把异常细节、危险信号或错误判断的触发点放进场，再补必要背景。";
            lens_focus = "镜头优先盯可疑细节、认知偏差和证据变化，不被大段说明拖停。";
            info_release = "线索一次只推进半步到一步，并配一个读者可验证的细节支点。";
        }
        Some("relationship") => {
            lens_focus = "镜头优先盯站位、语气、视线和试探动作，让关系张力有身体感。";
            transition_rule = "换场要让读者明白关系位置为什么变了，而不是人物凭空突然亲疏变化。";
        }
        Some("payoff") => {
            entry_anchor = "让待兑现的人、物、承诺或麻烦尽快回到场内，别临时凭空冒出。";
            info_release = "先让兑现条件现身，再给爆发反馈与余波，不把回报写成一句结果通知。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            lens_focus = "镜头重心跟主任务走，和主推进无关的抒情或设定只保留必要量。";
        }
        Some("deepen_character") => {
            lens_focus = "镜头贴近人物决策前后的犹疑、反应和自控失效，让性格在现场显形。";
            info_release = "人物信息通过选择、动作和反应露出，不靠整段自述讲完。";
        }
        Some("escalate_conflict") => {
            transition_rule = "每次换场都要把压力抬高一级，不重复同级拉扯或相似争执。";
        }
        Some("reveal_mystery") => {
            info_release = "线索一次只推进一层，且必须挂在可见证据、异常反应或判断修正上。";
        }
        Some("relationship_shift") => {
            lens_focus = "镜头重点盯说话方式、身体距离和站队动作的变化，让关系位移可见。";
        }
        Some("foreshadow_payoff") => {
            entry_anchor = "让前文埋下的人、物、承诺或代价尽早回到场内，别临时补设定。";
            info_release = "兑现信息要让读者能认出回扣来源，再补当下反馈与新后果。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            entry_anchor = "发展阶段先把当前场景秩序和人物站位立稳，再推进变量入场。";
        }
        Some("climax") => {
            lens_focus = "高潮阶段镜头尽量贴近最核心的碰撞点，不频繁切旁枝和外围观察。";
            transition_rule = "高潮阶段减少无效横移，切换要短促直接，始终围着主碰撞服务。";
        }
        Some("ending") => {
            info_release = "收束阶段优先回收主承诺、主关系和主真相，不再新开大块信息池。";
            transition_rule = "结尾换场要服务收束或余味，别再把战线铺散到新的主空间。";
        }
        _ => {}
    }

    let combo_text =
        build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认场景调度");
    format!(
        "【章节场景调度卡】本轮优先按以下场景调度执行（{}）\n- 入场锚点：{}\n- 镜头重心：{}\n- 信息投放：{}\n- 切换规则：{}\n",
        combo_text, entry_anchor, lens_focus, info_release, transition_rule
    )
}

fn build_story_scene_density_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut scene_task =
        "本章每个重要场景都要有明确任务：推进局势、抬高压力、揭一层信息或改动关系。";
    let mut live_action =
        "关键冲突、破局和兑现尽量写出动作链和现场反馈，不要一笔带过最该看的过程。";
    let mut load_mix = "把信息、情绪和关系变化嵌进动作与对白里，减少大段静态解释。";
    let mut rhythm_breath = "短段推进、必要停顿、再继续推进，让读者有气口但不掉线。";
    let mut stage_line = "";
    let mut avoid_line = "不要连续几段都在讲、想、回忆、解释，却没有动作、反馈和局势移动。";

    match normalized_mode {
        Some("hook") => {
            scene_task = "开场场景尽量尽快入事，让第一个场景就承担抓人和立压任务。";
        }
        Some("emotion") => {
            load_mix = "情绪密度来自互动、误伤、靠近失败和余波，不是单靠大段抒情。";
            rhythm_breath = "情绪段可以稍慢，但必须有新的触发、反应或关系变化支撑。";
        }
        Some("suspense") => {
            scene_task = "悬念型场景最好每场至少多出一个新线索、新反常或新风险。";
            live_action = "危险与调查尽量现场发生，不要只在事后总结“原来很危险”。";
        }
        Some("relationship") => {
            load_mix = "关系戏也要有事件支点：试探、合作、冲突、靠近或决裂，而不是纯聊天。";
        }
        Some("payoff") => {
            live_action = "兑现型场景优先把最值钱的动作、反应和反馈写在台前，不要藏在摘要句里。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            scene_task = "场景结束后最好能看到主线确实前进了一格，而不是忙完还在原地。";
        }
        Some("deepen_character") => {
            load_mix = "人物塑形最好落在选择和反应里，不要把场景停下来专门写人物说明书。";
        }
        Some("escalate_conflict") => {
            live_action = "冲突升级优先靠更难的现场碰撞和更贵的代价，不靠口头宣布升级。";
        }
        Some("reveal_mystery") => {
            scene_task = "每个关键场景最好都让谜团多推进半步，而不是只在个别节点突然集中补答案。";
        }
        Some("relationship_shift") => {
            rhythm_breath = "关系变化要有拉扯节奏：试探、误判、碰撞、余波，不要一句话突然完成。";
        }
        Some("foreshadow_payoff") => {
            scene_task = "尽量让某个场景承担伏笔兑现或预埋，不要全章都没有回报节点。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line = "发展阶段重在把场景链铺密：每场都给一点推进，不让中段发空。";
        }
        Some("climax") => {
            stage_line = "高潮阶段要提高现场化比例，压缩解释和复盘，让动作、决断与后果顶上来。";
            avoid_line = "不要在高潮章连续堆长段回忆、讲解和心理总结，把冲击拆散。";
        }
        Some("ending") => {
            stage_line = "结局阶段的场景密度重点是回收与余波并存：既要落地，也要留一丝回味。";
            avoid_line = "不要在收尾阶段继续用很多过渡场把关键回收往后拖。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认密度");
    let mut lines = vec![format!(
        "【章节场景密度卡】本轮请提升每个场景的有效载荷与节奏（{}）",
        combo_text
    )];
    lines.push(format!("- 场景任务：{}", scene_task));
    lines.push(format!("- 现场化：{}", live_action));
    lines.push(format!("- 装载方式：{}", load_mix));
    lines.push(format!("- 节奏呼吸：{}", rhythm_breath));
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

fn build_story_repetition_risk_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut opening_risk = "不要反复用回忆、说明或同一种异常开场，容易让章节起手发闷。";
    let mut pressure_risk = "不要把受阻写成同一种争吵、误会或嘴上发狠，压力会显得空。";
    let mut pivot_risk = "不要把转折写成假反转、硬转念或只靠旁白解释。";
    let mut closing_risk = "不要每章都用同一种问句、敲门声或电话铃收尾，钩子会疲劳。";

    match normalized_mode {
        Some("hook") => {
            opening_risk = "钩子模式下不要每次都靠突发危险硬拽开场，异常类型需要变化。";
            closing_risk = "不要连续多章都用悬空危险硬切章尾，读者会识别套路。";
        }
        Some("emotion") => {
            pressure_risk = "不要反复靠争吵、沉默或内心独白制造情绪，否则张力会钝化。";
            pivot_risk = "不要把情绪转折写成突然想通，缺少事件触发会显得虚。";
        }
        Some("suspense") => {
            opening_risk = "悬念模式下不要只会丢疑点不交代有效信息，否则会像故意遮掩。";
            pivot_risk = "不要连续用“其实另有隐情”做反转，真相推进需要层次。";
            closing_risk = "不要只留空白疑问而不给新证据，悬念会变成拖延。";
        }
        Some("relationship") => {
            pressure_risk = "不要把关系推进写成重复拉扯却没有立场后果，读者会觉得没变化。";
            pivot_risk = "不要每次都靠误会触发关系变化，站队和选择也要轮换。";
        }
        Some("payoff") => {
            opening_risk = "回收模式下不要一上来就罗列旧伏笔目录，读者需要事件化兑现。";
            closing_risk = "不要每次回收完都再塞一个更大的谜团，容易冲淡回报感。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            pressure_risk = "主线推进不要只做位移和赶路，缺少阻力变化会像流水账。";
        }
        Some("deepen_character") => {
            opening_risk = "人物塑形不要总从心理描写起手，最好让性格先在动作里显形。";
            pressure_risk = "不要把成长写成同一种自责或回忆，人物弧线会发虚。";
        }
        Some("escalate_conflict") => {
            pressure_risk = "冲突升级不要一直放大音量不抬高代价，否则只是吵得更大声。";
            pivot_risk = "不要把冲突转折只写成新敌人登场，最好让旧矛盾也发生质变。";
        }
        Some("reveal_mystery") => {
            pivot_risk = "谜团揭示不要总靠旁人解释，证据和事件本身也要承担揭示功能。";
            closing_risk = "不要连续多次只留下谜面不回收谜底，读者会怀疑作者在拖。";
        }
        Some("relationship_shift") => {
            pressure_risk = "关系转折不要只换台词腔调，最好同步改变合作方式和站位。";
        }
        Some("foreshadow_payoff") => {
            closing_risk = "伏笔回收不要每次都变成新伏笔发射器，需保留真正落地的满足。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            opening_risk = "发展阶段不要长时间停在铺垫准备态，必须尽快把变量推上桌。";
            closing_risk = "发展阶段不要每章都只留一个模糊目标，任务应逐步具体化。";
        }
        Some("climax") => {
            pressure_risk = "高潮阶段不要反复假装要碰撞却不断拖开，读者会明显感到泄劲。";
            pivot_risk = "高潮阶段不要只有大声量和快节奏，没有决定性变化就不算高潮。";
        }
        Some("ending") => {
            opening_risk = "结局阶段不要又重新搭新盘子，优先收最重要的旧承诺。";
            closing_risk = "结局阶段不要为了续作感强行再开主线，否则会稀释收束力度。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认避重");
    format!(
        "【章节重复风险卡】本轮需主动规避以下高频套路（{}）\n- 开场风险：{}\n- 加压风险：{}\n- 转折风险：{}\n- 收尾风险：{}\n",
        combo_text, opening_risk, pressure_risk, pivot_risk, closing_risk
    )
}

fn build_story_acceptance_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut mission_check = "验收时先看本章是否完成了一个清晰主任务，而不是热闹但空转。";
    let mut change_check = "至少要看到局势、关系或认知有一项明确变化，不能原地踏步。";
    let mut freshness_check = "检查开场、加压、转折、收尾是否又落回同一种旧套路。";
    let mut closing_check = "章尾既要完成本章收束，也要留下合适的追读牵引或余味。";

    match normalized_mode {
        Some("hook") => {
            mission_check = "验收时重点看开场和章尾是否真正形成牵引，而不只是制造噪音。";
            closing_check = "结尾要让读者有继续读的冲动，但不能只有硬切和悬空。";
        }
        Some("emotion") => {
            change_check = "验收时要看到情绪余震和关系后果，而不是只有一段抒情。";
            freshness_check = "检查情绪推进是否又只是争吵、沉默或内心独白轮换。";
        }
        Some("suspense") => {
            change_check = "验收时至少要有一个有效线索、认知刷新或危险升级真正落地。";
            closing_check = "结尾要留下更尖锐的问题，但不能完全不给有效信息。";
        }
        Some("relationship") => {
            mission_check = "验收时看人物关系是否真的发生位移，而不是只多说了几句狠话。";
            change_check = "关系变化最好能改动人物之后的站位、合作或信任条件。";
        }
        Some("payoff") => {
            mission_check = "验收时要确认前文铺垫是否真正兑现，而不是只口头提到。";
            closing_check = "兑现之后要有后效和新失衡，不能只停在一次性爽点。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            mission_check = "验收时先看主线是否实打实前进，而不是忙了很多事却没推局势。";
        }
        Some("deepen_character") => {
            change_check = "验收时看人物是否在选择里显形，而不是只补充背景说明。";
            freshness_check = "检查人物塑形是否又回到同一种回忆、自责或旁白总结。";
        }
        Some("escalate_conflict") => {
            change_check = "验收时要能看见代价升级、对立加深或冲突进入新层级。";
            closing_check = "本轮结束后人物应被留在更难的位置，而不是轻松退回安全区。";
        }
        Some("reveal_mystery") => {
            mission_check = "验收时必须确认谜团有真实推进，而不是只多堆了一层雾。";
        }
        Some("relationship_shift") => {
            change_check = "验收时看关系是否足以改变说话方式、行动选择或站队逻辑。";
        }
        Some("foreshadow_payoff") => {
            mission_check = "验收时确认伏笔是否兑现落地，同时打开了新的后续空间。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            mission_check = "发展阶段验收重点是：有没有把局势、变量和主任务真正搭起来。";
            closing_check = "收尾应让下一轮任务更具体，而不是继续停留在准备态。";
        }
        Some("climax") => {
            change_check = "高潮阶段验收重点是：有没有形成决定性碰撞、底牌掀开或局势断裂。";
            freshness_check = "检查高潮是否只是声量更大，还是确实发生了不可逆变化。";
        }
        Some("ending") => {
            mission_check = "结局阶段验收重点是：主承诺、主悬念和关键关系线是否得到有效回收。";
            closing_check = "收尾应保留余味，但不能为了留白再次打散已经完成的收束。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认验收");
    format!(
        "【章节验收卡】成稿前请用以下标准验收本轮是否真正达标（{}）\n- 任务命中：{}\n- 变化落地：{}\n- 新鲜度：{}\n- 收束质量：{}\n",
        combo_text, mission_check, change_check, freshness_check, closing_check
    )
}

fn build_story_cliffhanger_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut unresolved_point =
        "章尾要留一个具体未决点：一个答案缺口、一个马上要做的选择，或一个刚翻面的新问题。";
    let mut next_push = "结尾最好把人物逼到下一步动作边缘，让读者自然想看下一章。";
    let mut aftertaste = "除了钩子，还要留一点情绪余味、代价回响或关系余震。";
    let mut stage_line = "";
    let mut avoid_line = "不要只靠突然打断、无信息硬切或机械性的“未完待续感”制造悬停。";

    match normalized_mode {
        Some("hook") => {
            unresolved_point =
                "未决点优先是迫近选择、倒计时危险或刚被掀开的麻烦，不要只做语气停顿。";
            next_push = "下一步逼力要明确到人物不得不马上应对，而不是以后再说。";
        }
        Some("emotion") => {
            aftertaste = "余味最好落在误伤后的沉默、靠近失败后的反弹，或关系未说破的震荡上。";
            avoid_line = "不要在情绪高点后立刻解释完、说透完，把回响全部冲掉。";
        }
        Some("suspense") => {
            unresolved_point = "未决点最好是线索翻面、认知裂缝、危险升级或答案只揭开半层。";
            aftertaste = "余味要让读者感到局势更深、更险，而不是只多了一个名词。";
        }
        Some("relationship") => {
            unresolved_point = "未决点最好和立场未定、关系悬空、合作破裂或信任临界绑定。";
            aftertaste = "余味应保留人物之间的温差、敌意、亏欠或迟到的理解。";
        }
        Some("payoff") => {
            unresolved_point = "兑现之后要留一个新失衡或新代价，说明故事没有在爽点处直接封口。";
            next_push = "下一步逼力最好来自兑现后的后效，而不是硬塞一个无关新坑。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            next_push = "结尾逼力必须能接到主线下一步，不要只留下气氛而没有行动方向。";
        }
        Some("deepen_character") => {
            aftertaste = "余味最好让读者记住人物此刻的新伤口、新认知或新自我怀疑。";
        }
        Some("escalate_conflict") => {
            unresolved_point = "未决点应落在冲突升级后的更难位置：谁先出手、谁先失控、谁先付代价。";
            next_push = "下一步逼力要让人物无法轻松退回安全区。";
        }
        Some("reveal_mystery") => {
            unresolved_point = "未决点最好是刚拿到半个答案，却暴露出更关键的缺口或反常。";
        }
        Some("relationship_shift") => {
            aftertaste = "余味要落在关系新站位上，让读者感到他们再也回不到原来的相处方式。";
        }
        Some("foreshadow_payoff") => {
            unresolved_point =
                "未决点可以是旧伏笔兑现后的新空缺，说明兑现带来了新的问题而非彻底归零。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            stage_line = "发展阶段的章尾/卷尾要把下一轮任务说得更具体，别总停在模糊愿景。";
        }
        Some("climax") => {
            stage_line = "高潮阶段的结尾要保持冲击余震与决战逼力，不要突然卸压。";
            avoid_line = "不要在高潮结尾处仓促复盘、解释一切或切回轻松缓冲，导致气势塌掉。";
        }
        Some("ending") => {
            stage_line =
                "结局阶段可以减少硬卖关子，更适合保留余波、代价、阴影或尚未完全愈合的裂口。";
            avoid_line = "不要为了续作感硬开全新主线；更适合留下收束后的余味和未尽代价。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认悬停");
    let mut lines = vec![format!(
        "【章节结尾悬停卡】收尾请留下继续阅读/推进的牵引（{}）",
        combo_text
    )];
    lines.push(format!("- 未决点：{}", unresolved_point));
    lines.push(format!("- 下一步逼力：{}", next_push));
    lines.push(format!("- 余味：{}", aftertaste));
    lines.push("- 硬指标：最后一段至少落下 2 类尾钩信号（信息缺口 / 危险逼近 / 身份位移 / 待做选择 / 事态升级），最后一句不要复盘解释。".to_string());
    if !stage_line.is_empty() {
        lines.push(format!("- 阶段提醒：{}", stage_line));
    }
    lines.push(format!("- 避免：{}", avoid_line));
    format!("{}\n", lines.join("\n"))
}

fn build_story_character_arc_card_block(
    creative_mode: &str,
    story_focus: &str,
    plot_stage: &str,
) -> String {
    let Some((normalized_mode, normalized_focus, normalized_stage)) =
        normalized_story_runtime_inputs(creative_mode, story_focus, plot_stage)
    else {
        return String::new();
    };

    let mut external_line = "本章要让人物在外在线上做出能看见后果的动作，而不是被剧情拖着走。";
    let mut internal_line = "本章要逼出一次能暴露人物软肋、执念或底线的反应。";
    let mut relationship_line = "至少让一条关系线发生可见位移，而不只是多说几句情绪台词。";
    let mut arc_landing = "章尾要留下人物状态的新落点，让后续成长有承接。";

    match normalized_mode {
        Some("hook") => {
            external_line = "人物外在线最好和迫近危险、未决选择或新任务直接绑定，让他不得不动。";
            arc_landing = "弧光落点要落在人物被推入新处境上，而不只是事件悬空。";
        }
        Some("emotion") => {
            internal_line = "内在线重点看人物如何被情绪反噬、误伤他人或压抑失败。";
            relationship_line = "关系线最好呈现安慰失败、靠近受阻或误伤后的余震。";
        }
        Some("suspense") => {
            external_line = "人物外在线尽量和追查、判断、求生或拆解异常绑定。";
            internal_line = "通过误判、恐惧和认知落差暴露人物真正的盲区与偏执。";
        }
        Some("relationship") => {
            relationship_line = "关系线必须承担主推进，最好出现站队变化、信任重排或亲疏重估。";
            arc_landing = "落点应让人物在关系位置上进入一个再也回不到原点的新阶段。";
        }
        Some("payoff") => {
            external_line = "人物外在线要和旧承诺兑现、旧目标回收或能力回报直接挂钩。";
            arc_landing = "落点要让人物因为兑现获得成长回报，或承担兑现带来的新责任。";
        }
        _ => {}
    }

    match normalized_focus {
        Some("advance_plot") => {
            external_line = "人物外在线必须和主线推进同频，行动要真的改变局势而非走流程。";
        }
        Some("deepen_character") => {
            internal_line = "内在线要让人物在选择里显形，看见他的软肋、执念和价值判断。";
            arc_landing = "落点最好形成一次人物自我认知偏移，而不只是事件结束。";
        }
        Some("escalate_conflict") => {
            internal_line = "冲突升级时要逼出人物底线，看看他在更高代价下会怎么变。";
            relationship_line = "更强冲突最好同步改写人物之间的站位与依赖结构。";
        }
        Some("reveal_mystery") => {
            external_line = "人物外在线最好围绕调查、判断和选择展开，而不是旁观真相自己掉下来。";
            internal_line = "认知刷新应反照人物偏见、恐惧或执念，而不是只补世界观信息。";
        }
        Some("relationship_shift") => {
            relationship_line =
                "关系线验收重点是：人物之后的说话方式、站位和合作条件是否真的变了。";
        }
        Some("foreshadow_payoff") => {
            arc_landing = "人物应因为伏笔兑现进入新的自我认知、责任位置或情感阶段。";
        }
        _ => {}
    }

    match normalized_stage {
        Some("development") => {
            external_line = "发展阶段先把人物眼前要争什么、躲什么、赌什么摆清楚。";
            arc_landing = "落点应把人物推入更难但更清晰的成长压力链。";
        }
        Some("climax") => {
            internal_line = "高潮阶段要逼出人物真正底线、真实选择或最不愿面对的自我。";
            relationship_line = "高潮中的关系变化最好是定向性变化，而不是小幅试探。";
        }
        Some("ending") => {
            relationship_line = "结局阶段要让关键关系线出现收束、定局或带余温的最终位移。";
            arc_landing = "落点要给人物阶段性定局、余味或代价后的新平衡。";
        }
        _ => {}
    }

    let combo_text = build_chapter_combo_text(creative_mode, story_focus, plot_stage, "默认弧光");
    format!(
        "【章节角色弧光卡】本轮至少让人物弧光出现以下推进（{}）\n- 外在线：{}\n- 内在线：{}\n- 关系线：{}\n- 落点：{}\n",
        combo_text, external_line, internal_line, relationship_line, arc_landing
    )
}

fn normalize_prompt_list(items: &[String]) -> Vec<String> {
    items
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn build_repair_target_block(targets: &[String], strengths: &[String]) -> String {
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

fn build_repair_diagnostic_block(
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

fn build_web_research_block(enabled: bool, query: Option<&str>) -> String {
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

fn build_external_assets_block(
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

fn build_quality_preference_block(quality_preset: &str, quality_notes: &str) -> String {
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

fn build_quality_generation_protocol_block() -> String {
    format!(
        "【统一协议护栏】\n- 质量块追踪标签：{}\n- 统一吸收第三版规则摘要，不在各链路重复手写散落逻辑。\n- runtime 质量块只补充规则来源，不覆盖用户模板主体与业务上下文。\n- {}\n- {}\n- 禁止输出流程化元文本、调度说明、自我评注与来源暴露。\n",
        QUALITY_RUNTIME_TRACKING_TAG, MCP_CANON_PRIORITY_RULE, MCP_SOURCE_DISCLOSURE_RULE
    )
}

fn build_quality_json_protocol_block() -> String {
    format!(
        "【统一JSON协议护栏】\n- 质量块追踪标签：{}\n- 维持纯 JSON 输出，不追加 markdown、解释说明、流程文本或来源披露。\n- {}\n- {}\n- 若证据不足，使用 null / 空数组 / 保守结论，不臆造事实。\n",
        QUALITY_RUNTIME_TRACKING_TAG, MCP_CANON_PRIORITY_RULE, MCP_SOURCE_DISCLOSURE_RULE
    )
}

fn build_quality_contract_block(params: &HashMap<String, String>) -> String {
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

fn append_prompt_block_after_tag(prompt: &str, block: &str, after_tag: &str) -> String {
    let block = block.trim();
    if block.is_empty() || prompt.contains("<quality_contract") {
        return prompt.to_string();
    }
    if let Some(index) = prompt.find(after_tag) {
        let insert_at = index + after_tag.len();
        let mut result = String::with_capacity(prompt.len() + block.len() + 2);
        result.push_str(&prompt[..insert_at]);
        result.push_str("\n\n");
        result.push_str(block);
        result.push_str(&prompt[insert_at..]);
        return result;
    }
    format!("{}\n\n{}", prompt.trim_end(), block)
}

fn inject_quality_contract(prompt: &str, params: &HashMap<String, String>) -> String {
    append_prompt_block_after_tag(
        prompt,
        params
            .get("quality_contract_block")
            .map(String::as_str)
            .unwrap_or_default(),
        "</fusion_contract>",
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

fn build_quality_profile_payload(
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

fn prompt_block_text(prompt_blocks: &Value, key: &str) -> String {
    prompt_blocks
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

pub fn chapter_template_key(outline_mode: &str, has_previous: bool) -> &'static str {
    match (outline_mode, has_previous) {
        ("one-to-many", false) => "CHAPTER_GENERATION_ONE_TO_MANY",
        ("one-to-many", true) => "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
        ("one-to-one", false) | (_, false) => "CHAPTER_GENERATION_ONE_TO_ONE",
        _ => "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
    }
}

fn build_prompt_params_with_provider_payload(
    chapter_model: &chapter::Model,
    project_model: &project::Model,
    previous_chapter_prompt_context: PreviousChapterPromptContext,
    _has_previous_chapter: bool,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
) -> HashMap<String, String> {
    let mut params = HashMap::new();
    let narrative_perspective = resolve_prompt_preference(
        overrides.narrative_perspective.as_deref(),
        project_model.narrative_perspective.as_deref(),
    );
    let creative_mode = resolve_prompt_preference(
        overrides.creative_mode.as_deref(),
        project_model.default_creative_mode.as_deref(),
    );
    let story_focus = resolve_prompt_preference(
        overrides.story_focus.as_deref(),
        project_model.default_story_focus.as_deref(),
    );
    let plot_stage = resolve_prompt_preference(
        overrides.plot_stage.as_deref(),
        project_model.default_plot_stage.as_deref(),
    );
    let story_creation_brief = resolve_prompt_preference(
        overrides.story_creation_brief.as_deref(),
        project_model.default_story_creation_brief.as_deref(),
    );
    let quality_preset = resolve_prompt_preference(
        overrides.quality_preset.as_deref(),
        project_model.default_quality_preset.as_deref(),
    );
    let quality_notes = resolve_prompt_preference(
        overrides.quality_notes.as_deref(),
        project_model.default_quality_notes.as_deref(),
    );
    let web_research_query = overrides
        .web_research_query
        .as_deref()
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .map(str::to_string);
    let story_repair_summary = overrides
        .story_repair_summary
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string();
    let story_repair_targets = normalize_prompt_list(&overrides.story_repair_targets);
    let story_preserve_strengths = normalize_prompt_list(&overrides.story_preserve_strengths);
    let mcp_references = provider_payload.mcp_references.trim().to_string();
    let quality_profile_payload =
        build_quality_profile_payload(project_model, &quality_preset, &provider_payload);
    let quality_prompt_blocks = build_novel_quality_prompt_blocks(Some(&quality_profile_payload));
    let external_assets_block = build_external_assets_block(
        &provider_payload.external_assets,
        &provider_payload.reference_assets,
        &provider_payload.mcp_references,
    );
    params.insert("project_title".to_string(), project_model.title.clone());
    params.insert(
        "genre".to_string(),
        project_model.genre.clone().unwrap_or_default(),
    );
    params.insert(
        "chapter_number".to_string(),
        chapter_model.chapter_number.to_string(),
    );
    params.insert("chapter_title".to_string(), chapter_model.title.clone());
    params.insert(
        "target_word_count".to_string(),
        target_word_count.to_string(),
    );
    params.insert(
        "narrative_perspective".to_string(),
        if narrative_perspective.is_empty() {
            "第三人称".to_string()
        } else {
            narrative_perspective
        },
    );
    params.insert(
        "chapter_outline".to_string(),
        chapter_model
            .expansion_plan
            .clone()
            .unwrap_or_else(|| "暂无大纲".to_string()),
    );
    params.insert(
        "world_time_period".to_string(),
        project_model.world_time_period.clone().unwrap_or_default(),
    );
    params.insert(
        "world_location".to_string(),
        project_model.world_location.clone().unwrap_or_default(),
    );
    params.insert(
        "world_atmosphere".to_string(),
        project_model.world_atmosphere.clone().unwrap_or_default(),
    );
    params.insert(
        "world_rules".to_string(),
        project_model.world_rules.clone().unwrap_or_default(),
    );
    params.insert("creative_mode".to_string(), creative_mode.clone());
    params.insert(
        "creative_mode_block".to_string(),
        build_creative_mode_block(&creative_mode),
    );
    params.insert("story_focus".to_string(), story_focus.clone());
    params.insert(
        "story_focus_block".to_string(),
        build_story_focus_block(&story_focus),
    );
    params.insert("plot_stage".to_string(), plot_stage.clone());
    params.insert(
        "narrative_blueprint_block".to_string(),
        build_narrative_blueprint_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_creation_brief".to_string(),
        story_creation_brief.clone(),
    );
    let web_research_block = build_web_research_block(
        overrides.web_research_enabled,
        web_research_query.as_deref(),
    );
    let story_creation_brief_block = format!(
        "{}{}",
        build_optional_instruction_block("创作总控摘要", &story_creation_brief),
        web_research_block
    );
    params.insert(
        "story_creation_brief_block".to_string(),
        story_creation_brief_block,
    );
    params.insert(
        "web_research_query".to_string(),
        web_research_query.clone().unwrap_or_default(),
    );
    params.insert("web_research_block".to_string(), web_research_block);
    params.insert("quality_preset".to_string(), quality_preset);
    params.insert("quality_notes".to_string(), quality_notes);
    let quality_preset = params.get("quality_preset").cloned().unwrap_or_default();
    let quality_notes = params.get("quality_notes").cloned().unwrap_or_default();
    params.insert(
        "quality_generation_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "generation"),
    );
    params.insert(
        "quality_analysis_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "checker"),
    );
    params.insert(
        "quality_checker_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "checker"),
    );
    params.insert(
        "quality_reviser_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "reviser"),
    );
    params.insert(
        "quality_regeneration_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "generation"),
    );
    params.insert(
        "quality_generation_protocol_block".to_string(),
        build_quality_generation_protocol_block(),
    );
    params.insert(
        "quality_json_protocol_block".to_string(),
        build_quality_json_protocol_block(),
    );
    params.insert(
        "quality_mcp_guard_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "mcp_guard"),
    );
    params.insert(
        "mcp_guard".to_string(),
        prompt_block_text(&quality_prompt_blocks, "mcp_guard"),
    );
    params.insert(
        "quality_preference_block".to_string(),
        build_quality_preference_block(&quality_preset, &quality_notes),
    );
    params.insert(
        "story_objective_card_block".to_string(),
        build_story_objective_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_result_card_block".to_string(),
        build_story_result_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_payoff_chain_card_block".to_string(),
        build_story_payoff_chain_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_rule_grounding_card_block".to_string(),
        build_story_rule_grounding_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_information_release_card_block".to_string(),
        build_story_information_release_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_emotion_landing_card_block".to_string(),
        build_story_emotion_landing_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_action_rendering_card_block".to_string(),
        build_story_action_rendering_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_summary_tone_control_card_block".to_string(),
        build_story_summary_tone_control_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_repetition_control_card_block".to_string(),
        build_story_repetition_control_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_viewpoint_discipline_card_block".to_string(),
        build_story_viewpoint_discipline_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_dialogue_advancement_card_block".to_string(),
        build_story_dialogue_advancement_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_opening_hook_card_block".to_string(),
        build_story_opening_hook_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_repair_summary".to_string(),
        story_repair_summary.clone(),
    );
    params.insert(
        "story_repair_targets".to_string(),
        story_repair_targets.join("；"),
    );
    params.insert(
        "story_preserve_strengths".to_string(),
        story_preserve_strengths.join("；"),
    );
    params.insert(
        "story_repair_target_block".to_string(),
        build_repair_target_block(&story_repair_targets, &story_preserve_strengths),
    );
    params.insert(
        "story_repair_diagnostic_block".to_string(),
        build_repair_diagnostic_block(
            &story_repair_summary,
            &story_repair_targets,
            &story_preserve_strengths,
        ),
    );
    params.insert(
        "story_execution_checklist_block".to_string(),
        build_story_execution_checklist_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_scene_anchor_card_block".to_string(),
        build_story_scene_anchor_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_scene_density_card_block".to_string(),
        build_story_scene_density_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_repetition_risk_block".to_string(),
        build_story_repetition_risk_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_acceptance_card_block".to_string(),
        build_story_acceptance_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_cliffhanger_card_block".to_string(),
        build_story_cliffhanger_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "story_character_arc_card_block".to_string(),
        build_story_character_arc_card_block(&creative_mode, &story_focus, &plot_stage),
    );
    params.insert(
        "quality_external_assets_block".to_string(),
        prompt_block_text(&quality_prompt_blocks, "external_assets"),
    );
    params.insert(
        "quality_raw_external_assets_block".to_string(),
        external_assets_block,
    );
    params.insert(
        "quality_mcp_references_block".to_string(),
        mcp_references.clone(),
    );
    params.insert(
        "quality_contract_block".to_string(),
        build_quality_contract_block(&params),
    );
    params.extend(provider_payload.into_prompt_params());
    params.insert(
        "previous_chapter_content".to_string(),
        previous_chapter_prompt_context.previous_chapter_content,
    );
    params.insert(
        "continuation_point".to_string(),
        previous_chapter_prompt_context.continuation_point,
    );
    params
}

pub fn build_prompt_with_provider_payload(
    chapter_model: &chapter::Model,
    project_model: &project::Model,
    previous_chapter_prompt_context: PreviousChapterPromptContext,
    has_previous_chapter: bool,
    target_word_count: i32,
    provider_payload: PromptContextProviderPayload,
    overrides: &ChapterGenerationPromptOverrides,
) -> Result<String, String> {
    let template_key = chapter_template_key(&project_model.outline_mode, has_previous_chapter);
    let template = PromptTemplateService::system_template_info(template_key)
        .ok_or_else(|| format!("找不到章节模板: {}", template_key))?;
    let params = build_prompt_params_with_provider_payload(
        chapter_model,
        project_model,
        previous_chapter_prompt_context,
        has_previous_chapter,
        target_word_count,
        provider_payload,
        overrides,
    );

    let rendered = PromptTemplateService::format_prompt(&template.content, &params)?;
    Ok(inject_quality_contract(&rendered, &params))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{
        build_previous_chapter_prompt_context, build_prompt_params_with_provider_payload,
        build_prompt_with_provider_payload, chapter_template_key, ChapterGenerationPromptOverrides,
    };
    use crate::models::{chapter, project};
    use crate::services::chapter_generation_prompt_context_provider_service::{
        build_placeholder_prompt_context_provider_payload, PromptContextProviderPayload,
    };

    fn build_project(outline_mode: &str) -> project::Model {
        project::Model {
            id: "project-1".to_string(),
            user_id: "user-1".to_string(),
            title: "项目标题".to_string(),
            genre: Some("奇幻".to_string()),
            description: None,
            theme: None,
            target_words: 120000,
            current_words: 0,
            status: "active".to_string(),
            wizard_status: "completed".to_string(),
            wizard_step: 0,
            outline_mode: outline_mode.to_string(),
            narrative_perspective: None,
            world_time_period: Some("近未来".to_string()),
            world_location: Some("浮空城".to_string()),
            world_atmosphere: Some("压抑".to_string()),
            world_rules: Some("魔力守恒".to_string()),
            chapter_count: Some(3),
            character_count: 0,
            default_creative_mode: None,
            default_story_focus: None,
            default_plot_stage: None,
            default_story_creation_brief: None,
            default_quality_preset: None,
            default_quality_notes: None,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    fn build_chapter(
        chapter_number: i32,
        title: &str,
        expansion_plan: Option<&str>,
        content: Option<&str>,
        summary: Option<&str>,
    ) -> chapter::Model {
        chapter::Model {
            id: format!("chapter-{chapter_number}"),
            project_id: "project-1".to_string(),
            title: title.to_string(),
            chapter_number,
            content: content.map(str::to_string),
            summary: summary.map(str::to_string),
            expansion_plan: expansion_plan.map(str::to_string),
            status: "pending".to_string(),
            word_count: 0,
            outline_id: None,
            sub_index: 0,
            created_at: Utc::now().naive_utc(),
            updated_at: None,
        }
    }

    #[test]
    fn should_select_template_keys_for_outline_mode_and_previous_chapter_state() {
        assert_eq!(
            chapter_template_key("one-to-many", false),
            "CHAPTER_GENERATION_ONE_TO_MANY"
        );
        assert_eq!(
            chapter_template_key("one-to-many", true),
            "CHAPTER_GENERATION_ONE_TO_MANY_NEXT"
        );
        assert_eq!(
            chapter_template_key("one-to-one", false),
            "CHAPTER_GENERATION_ONE_TO_ONE"
        );
        assert_eq!(
            chapter_template_key("custom-mode", true),
            "CHAPTER_GENERATION_ONE_TO_ONE_NEXT"
        );
    }

    #[test]
    fn should_inject_defaults_when_optional_prompt_fields_are_missing() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(3, "第三章", None, None, None);

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            3200,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("prompt should build");

        assert!(prompt.contains("项目标题"));
        assert!(prompt.contains("第三章"));
        assert!(prompt.contains("3200"));
        assert!(prompt.contains("第三人称"));
        assert!(prompt.contains("暂无大纲"));
    }

    #[test]
    fn should_include_previous_chapter_context_and_continuation_excerpt() {
        let project_model = build_project("one-to-many");
        let chapter_model = build_chapter(4, "第四章", Some("推进主线"), None, None);
        let previous_content = format!("{}{}", "甲".repeat(120), "乙".repeat(500));
        let previous_summary = "上一章总结";
        let previous_chapter = build_chapter(
            3,
            "第三章",
            Some("旧大纲"),
            Some(previous_content.as_str()),
            Some(previous_summary),
        );

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(Some(&previous_chapter)),
            true,
            3600,
            PromptContextProviderPayload {
                previous_chapter_summary: previous_summary.to_string(),
                ..build_placeholder_prompt_context_provider_payload()
            },
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("prompt should build");

        assert!(prompt.contains(previous_summary));
        assert!(prompt.contains(&"乙".repeat(500)));
        assert!(!prompt.contains(&"甲".repeat(120)));
    }

    #[test]
    fn should_build_prompt_with_injected_provider_payload() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(2, "第二章", Some("推进冲突"), None, None);
        let provider_payload = PromptContextProviderPayload {
            recent_chapters_context: String::new(),
            previous_chapter_summary: "上一章总结".to_string(),
            chapter_careers: "[]".to_string(),
            characters_info: "[角色甲]".to_string(),
            foreshadow_reminders: "[伏笔甲]".to_string(),
            relevant_memories: "[记忆甲]".to_string(),
            research_query: String::new(),
            research_assets: "[]".to_string(),
            external_assets: "[]".to_string(),
            reference_assets: "[]".to_string(),
            mcp_references: String::new(),
        };

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            provider_payload,
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("prompt should build");

        assert!(prompt.contains("[角色甲]"));
        assert!(prompt.contains("[伏笔甲]"));
        assert!(prompt.contains("[记忆甲]"));
    }

    #[test]
    fn should_build_prompt_params_with_defaults_and_provider_context() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(2, "第二章", None, None, None);
        let provider_payload = PromptContextProviderPayload {
            recent_chapters_context: String::new(),
            previous_chapter_summary: "上一章总结".to_string(),
            chapter_careers: "[]".to_string(),
            characters_info: "[角色甲]".to_string(),
            foreshadow_reminders: "[伏笔甲]".to_string(),
            relevant_memories: "[记忆甲]".to_string(),
            research_query: String::new(),
            research_assets: "[]".to_string(),
            external_assets: "[]".to_string(),
            reference_assets: "[]".to_string(),
            mcp_references: String::new(),
        };

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            provider_payload,
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("project_title").map(String::as_str),
            Some("项目标题")
        );
        assert_eq!(
            params.get("chapter_title").map(String::as_str),
            Some("第二章")
        );
        assert_eq!(
            params.get("target_word_count").map(String::as_str),
            Some("2800")
        );
        assert_eq!(
            params.get("narrative_perspective").map(String::as_str),
            Some("第三人称")
        );
        assert_eq!(
            params.get("chapter_outline").map(String::as_str),
            Some("暂无大纲")
        );
        assert_eq!(
            params.get("characters_info").map(String::as_str),
            Some("[角色甲]")
        );
        assert_eq!(
            params.get("previous_chapter_summary").map(String::as_str),
            Some("上一章总结")
        );
        assert_eq!(
            params.get("external_assets").map(String::as_str),
            Some("[]")
        );
    }

    #[test]
    fn should_apply_prompt_overrides_before_project_defaults() {
        let mut project_model = build_project("one-to-one");
        project_model.narrative_perspective = Some("第三人称".to_string());
        project_model.default_creative_mode = Some("balanced".to_string());
        project_model.default_story_focus = Some("advance_plot".to_string());
        project_model.default_plot_stage = Some("development".to_string());
        project_model.default_story_creation_brief = Some("项目默认总控".to_string());
        project_model.default_quality_preset = Some("balanced".to_string());
        project_model.default_quality_notes = Some("项目默认质量要求".to_string());
        let chapter_model = build_chapter(5, "第五章", Some("推进高潮"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            3200,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides {
                narrative_perspective: Some("第一人称".to_string()),
                creative_mode: Some("suspense".to_string()),
                story_focus: Some("reveal_mystery".to_string()),
                plot_stage: Some("climax".to_string()),
                story_creation_brief: Some("本章主打谜团揭晓前夜".to_string()),
                quality_preset: Some("immersive".to_string()),
                quality_notes: Some("压缩解释，强化临场感".to_string()),
                web_research_enabled: false,
                web_research_query: None,
                story_repair_summary: None,
                story_repair_targets: Vec::new(),
                story_preserve_strengths: Vec::new(),
            },
        );

        assert_eq!(
            params.get("narrative_perspective").map(String::as_str),
            Some("第一人称")
        );
        assert_eq!(
            params.get("creative_mode").map(String::as_str),
            Some("suspense")
        );
        assert_eq!(
            params.get("story_focus").map(String::as_str),
            Some("reveal_mystery")
        );
        assert_eq!(params.get("plot_stage").map(String::as_str), Some("climax"));
        assert_eq!(
            params.get("story_creation_brief").map(String::as_str),
            Some("本章主打谜团揭晓前夜")
        );
        assert_eq!(
            params.get("quality_preset").map(String::as_str),
            Some("immersive")
        );
        assert_eq!(
            params.get("quality_notes").map(String::as_str),
            Some("压缩解释，强化临场感")
        );
        assert!(params["creative_mode_block"].contains("创作模式"));
        assert!(params["creative_mode_block"].contains("悬念拉满"));
        assert!(params["story_focus_block"].contains("谜团揭示"));
        assert!(params["narrative_blueprint_block"].contains("悬念拉满 / 谜团揭示 / 高潮阶段"));
        assert!(params["narrative_blueprint_block"].contains("当前阶段要让核心矛盾正面碰撞"));
        assert!(params["story_creation_brief_block"].contains("本章主打谜团揭晓前夜"));
    }

    #[test]
    fn should_build_chapter_story_runtime_blocks_from_chinese_aliases() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(6, "第六章", Some("冲突加压"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides {
                creative_mode: Some("钩子".to_string()),
                story_focus: Some("冲突".to_string()),
                plot_stage: Some("高潮".to_string()),
                ..ChapterGenerationPromptOverrides::default()
            },
        );

        assert!(params["creative_mode_block"].contains("【创作模式】当前采用“钩子优先”"));
        assert!(params["creative_mode_block"].contains("开场尽快抛出异常、任务或危险"));
        assert!(params["story_focus_block"].contains("【结构侧重点】当前优先“冲突升级”"));
        assert!(params["story_focus_block"].contains("优先写出目标受阻、局面恶化、选择更难的过程"));
        assert!(params["narrative_blueprint_block"]
            .contains("【结构蓝图】本轮按“钩子优先 / 冲突升级 / 高潮阶段”组织章节节拍"));
        assert!(params["narrative_blueprint_block"].contains("尾段优先保留信息缺口"));
        assert!(params["narrative_blueprint_block"].contains("重点避免：不要只堆钩子和异常"));
        assert!(params["story_objective_card_block"].contains("【章节目标卡】"));
        assert!(params["story_objective_card_block"].contains("阻力要逼近正面碰撞"));
        assert!(params["story_objective_card_block"].contains("转折要接近核心碰撞点"));
        assert!(params["story_result_card_block"].contains("【章节结果卡】"));
        assert!(params["story_result_card_block"].contains("逼近或触发正面碰撞"));
        assert!(params["story_payoff_chain_card_block"].contains("【章节爽点回收卡】"));
        assert!(
            params["story_payoff_chain_card_block"].contains("高潮阶段优先回收最值钱的承诺和冲突")
        );
        assert!(params["story_rule_grounding_card_block"].contains("【章节设定落地卡】"));
        assert!(params["story_rule_grounding_card_block"]
            .contains("规则的代价、限制或反噬要把冲突抬高"));
        assert!(params["story_information_release_card_block"].contains("【章节信息投放卡】"));
        assert!(params["story_information_release_card_block"]
            .contains("不要在高潮关键碰撞前后连续长讲设定"));
        assert!(params["story_emotion_landing_card_block"].contains("【章节情绪落点卡】"));
        assert!(params["story_emotion_landing_card_block"].contains("高潮阶段情绪要跟着碰撞一起爆"));
        assert!(params["story_action_rendering_card_block"].contains("【章节动作显影卡】"));
        assert!(params["story_action_rendering_card_block"].contains("让最该爆的地方直接哑火"));
        assert!(params["story_summary_tone_control_card_block"].contains("【章节总结腔抑制卡】"));
        assert!(
            params["story_summary_tone_control_card_block"].contains("把现场冲击改写成作者感悟")
        );
        assert!(params["story_repetition_control_card_block"].contains("【章节重复压缩卡】"));
        assert!(
            params["story_repetition_control_card_block"].contains("高潮阶段少复盘、少重复解释")
        );
        assert!(params["story_viewpoint_discipline_card_block"].contains("【章节视角纪律卡】"));
        assert!(
            params["story_viewpoint_discipline_card_block"].contains("不要在高潮现场频繁切镜头")
        );
        assert!(params["story_dialogue_advancement_card_block"].contains("【章节对白推进卡】"));
        assert!(params["story_dialogue_advancement_card_block"]
            .contains("不要在高潮对白里长篇复盘前情"));
        assert!(params["story_opening_hook_card_block"].contains("【章节开篇抓力卡】"));
        assert!(params["story_opening_hook_card_block"].contains("高潮阶段的开篇要延续既有高压"));
        assert!(params["story_opening_hook_card_block"].contains("开篇前 20%-25%"));
        assert!(params["story_execution_checklist_block"].contains("【章节执行清单】"));
        assert!(params["story_execution_checklist_block"]
            .contains("高潮阶段开场尽快把人物推到主碰撞现场"));
        assert!(params["story_scene_anchor_card_block"].contains("【章节场景调度卡】"));
        assert!(
            params["story_scene_anchor_card_block"].contains("高潮阶段镜头尽量贴近最核心的碰撞点")
        );
        assert!(params["story_scene_density_card_block"].contains("【章节场景密度卡】"));
        assert!(params["story_scene_density_card_block"].contains("高潮阶段要提高现场化比例"));
        assert!(params["story_repetition_risk_block"].contains("【章节重复风险卡】"));
        assert!(params["story_repetition_risk_block"].contains("高潮阶段不要反复假装要碰撞"));
        assert!(params["story_acceptance_card_block"].contains("【章节验收卡】"));
        assert!(params["story_acceptance_card_block"].contains("高潮阶段验收重点"));
        assert!(params["story_cliffhanger_card_block"].contains("【章节结尾悬停卡】"));
        assert!(params["story_cliffhanger_card_block"].contains("高潮阶段的结尾要保持冲击余震"));
        assert!(params["story_character_arc_card_block"].contains("【章节角色弧光卡】"));
        assert!(params["story_character_arc_card_block"].contains("高潮阶段要逼出人物真正底线"));

        let contract = &params["quality_contract_block"];
        let creative_index = contract.find("【创作模式】").expect("creative block");
        let story_index = contract.find("【结构侧重点】").expect("story focus block");
        let blueprint_index = contract.find("【结构蓝图】").expect("blueprint block");
        let objective_index = contract.find("【章节目标卡】").expect("objective card");
        let result_index = contract.find("【章节结果卡】").expect("result card");
        let payoff_index = contract.find("【章节爽点回收卡】").expect("payoff card");
        let rule_index = contract
            .find("【章节设定落地卡】")
            .expect("rule grounding card");
        let information_index = contract
            .find("【章节信息投放卡】")
            .expect("information release card");
        let emotion_index = contract
            .find("【章节情绪落点卡】")
            .expect("emotion landing card");
        let action_index = contract
            .find("【章节动作显影卡】")
            .expect("action rendering card");
        let summary_tone_index = contract
            .find("【章节总结腔抑制卡】")
            .expect("summary tone control card");
        let repetition_index = contract
            .find("【章节重复压缩卡】")
            .expect("repetition control card");
        let viewpoint_index = contract
            .find("【章节视角纪律卡】")
            .expect("viewpoint discipline card");
        let dialogue_index = contract
            .find("【章节对白推进卡】")
            .expect("dialogue advancement card");
        let opening_index = contract
            .find("【章节开篇抓力卡】")
            .expect("opening hook card");
        let execution_index = contract
            .find("【章节执行清单】")
            .expect("execution checklist block");
        let scene_anchor_index = contract
            .find("【章节场景调度卡】")
            .expect("scene anchor card");
        let scene_density_index = contract
            .find("【章节场景密度卡】")
            .expect("scene density card");
        let repetition_risk_index = contract
            .find("【章节重复风险卡】")
            .expect("repetition risk block");
        let acceptance_index = contract.find("【章节验收卡】").expect("acceptance card");
        let cliffhanger_index = contract
            .find("【章节结尾悬停卡】")
            .expect("cliffhanger card");
        let character_arc_index = contract
            .find("【章节角色弧光卡】")
            .expect("character arc card");
        assert!(creative_index < story_index);
        assert!(story_index < blueprint_index);
        assert!(blueprint_index < objective_index);
        assert!(objective_index < result_index);
        assert!(result_index < payoff_index);
        assert!(payoff_index < rule_index);
        assert!(rule_index < information_index);
        assert!(information_index < emotion_index);
        assert!(emotion_index < action_index);
        assert!(action_index < summary_tone_index);
        assert!(summary_tone_index < repetition_index);
        assert!(repetition_index < viewpoint_index);
        assert!(viewpoint_index < dialogue_index);
        assert!(dialogue_index < opening_index);
        assert!(opening_index < execution_index);
        assert!(execution_index < scene_anchor_index);
        assert!(scene_anchor_index < scene_density_index);
        assert!(scene_density_index < repetition_risk_index);
        assert!(repetition_risk_index < acceptance_index);
        assert!(acceptance_index < cliffhanger_index);
        assert!(cliffhanger_index < character_arc_index);
    }

    #[test]
    fn should_fallback_to_project_prompt_defaults_when_overrides_are_missing() {
        let mut project_model = build_project("one-to-many");
        project_model.narrative_perspective = Some("全知视角".to_string());
        project_model.default_creative_mode = Some("hook".to_string());
        project_model.default_story_focus = Some("escalate_conflict".to_string());
        project_model.default_plot_stage = Some("development".to_string());
        project_model.default_story_creation_brief = Some("项目默认总控".to_string());
        project_model.default_quality_preset = Some("plot_drive".to_string());
        project_model.default_quality_notes = Some("强调推进".to_string());
        let chapter_model = build_chapter(6, "第六章", Some("冲突加压"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("narrative_perspective").map(String::as_str),
            Some("全知视角")
        );
        assert_eq!(
            params.get("creative_mode").map(String::as_str),
            Some("hook")
        );
        assert_eq!(
            params.get("story_focus").map(String::as_str),
            Some("escalate_conflict")
        );
        assert_eq!(
            params.get("plot_stage").map(String::as_str),
            Some("development")
        );
        assert_eq!(
            params.get("story_creation_brief").map(String::as_str),
            Some("项目默认总控")
        );
        assert_eq!(
            params.get("quality_preset").map(String::as_str),
            Some("plot_drive")
        );
        assert_eq!(
            params.get("quality_notes").map(String::as_str),
            Some("强调推进")
        );
        assert!(params["creative_mode_block"].contains("钩子优先"));
        assert!(params["story_focus_block"].contains("冲突升级"));
        assert!(params["narrative_blueprint_block"].contains("发展阶段"));
    }

    #[test]
    fn should_keep_repair_blocks_empty_when_repair_inputs_are_missing() {
        let project_model = build_project("one-to-many");
        let chapter_model = build_chapter(7, "第七章", Some("修复节奏"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2600,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("story_repair_summary").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_repair_targets").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_preserve_strengths").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_repair_target_block").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_repair_diagnostic_block")
                .map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn should_build_repair_blocks_from_prompt_overrides() {
        let project_model = build_project("one-to-many");
        let chapter_model = build_chapter(8, "第八章", Some("修复支线"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            3000,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides {
                narrative_perspective: None,
                creative_mode: None,
                story_focus: None,
                plot_stage: None,
                story_creation_brief: None,
                quality_preset: None,
                quality_notes: None,
                web_research_enabled: false,
                web_research_query: None,
                story_repair_summary: Some("上一章中段节奏拖慢，需要重新压缩".to_string()),
                story_repair_targets: vec!["缩短铺垫".to_string(), "提前冲突触发".to_string()],
                story_preserve_strengths: vec!["角色声音".to_string(), "悬念尾钩".to_string()],
            },
        );

        assert_eq!(
            params.get("story_repair_summary").map(String::as_str),
            Some("上一章中段节奏拖慢，需要重新压缩")
        );
        assert_eq!(
            params.get("story_repair_targets").map(String::as_str),
            Some("缩短铺垫；提前冲突触发")
        );
        assert_eq!(
            params.get("story_preserve_strengths").map(String::as_str),
            Some("角色声音；悬念尾钩")
        );
        assert!(params["story_repair_target_block"].contains("需要修复：缩短铺垫；提前冲突触发"));
        assert!(params["story_repair_target_block"].contains("必须保留：角色声音；悬念尾钩"));
        assert!(
            params["story_repair_diagnostic_block"].contains("上一章中段节奏拖慢，需要重新压缩")
        );
        assert!(
            params["story_repair_diagnostic_block"].contains("本章修复项：缩短铺垫；提前冲突触发")
        );
        assert!(params["story_repair_diagnostic_block"].contains("保留优势：角色声音；悬念尾钩"));
    }

    #[test]
    fn should_keep_web_research_block_empty_when_not_enabled() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(9, "第九章", Some("推进调查"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2600,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("web_research_query").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("web_research_block").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_creation_brief_block").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_objective_card_block").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params.get("story_result_card_block").map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_payoff_chain_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_rule_grounding_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_information_release_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_emotion_landing_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_action_rendering_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_summary_tone_control_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_repetition_control_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_viewpoint_discipline_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_dialogue_advancement_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_opening_hook_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_execution_checklist_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_scene_anchor_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_scene_density_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_repetition_risk_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_acceptance_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_cliffhanger_card_block")
                .map(String::as_str),
            Some("")
        );
        assert_eq!(
            params
                .get("story_character_arc_card_block")
                .map(String::as_str),
            Some("")
        );
    }

    #[test]
    fn should_build_web_research_block_when_enabled() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(10, "第十章", Some("收束线索"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            build_placeholder_prompt_context_provider_payload(),
            &ChapterGenerationPromptOverrides {
                narrative_perspective: None,
                creative_mode: None,
                story_focus: None,
                plot_stage: None,
                story_creation_brief: None,
                quality_preset: None,
                quality_notes: None,
                web_research_enabled: true,
                web_research_query: Some("晚清漕运与江南水路行会".to_string()),
                story_repair_summary: None,
                story_repair_targets: Vec::new(),
                story_preserve_strengths: Vec::new(),
            },
        );

        assert_eq!(
            params.get("web_research_query").map(String::as_str),
            Some("晚清漕运与江南水路行会")
        );
        assert!(params["web_research_block"].contains("已请求联网检索"));
        assert!(params["web_research_block"].contains("晚清漕运与江南水路行会"));
        assert!(params["story_creation_brief_block"].contains("晚清漕运与江南水路行会"));
    }

    #[test]
    fn should_surface_external_research_assets_from_provider_payload() {
        let project_model = build_project("one-to-one");
        let chapter_model = build_chapter(11, "第十一章", Some("追查账册"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            PromptContextProviderPayload {
                recent_chapters_context: String::new(),
                previous_chapter_summary: String::new(),
                chapter_careers: "[]".to_string(),
                characters_info: "[]".to_string(),
                foreshadow_reminders: "[]".to_string(),
                relevant_memories: "[]".to_string(),
                research_query: "晚清漕运夜航避税路线".to_string(),
                research_assets: "[]".to_string(),
                external_assets:
                    "[{\"kind\":\"web_research_query\",\"summary\":\"晚清漕运夜航避税路线\"}]"
                        .to_string(),
                reference_assets:
                    "[{\"kind\":\"web_research_query\",\"summary\":\"晚清漕运夜航避税路线\"}]"
                        .to_string(),
                mcp_references: "[]".to_string(),
            },
            &ChapterGenerationPromptOverrides::default(),
        );

        assert_eq!(
            params.get("research_query").map(String::as_str),
            Some("晚清漕运夜航避税路线")
        );
        assert!(params["quality_external_assets_block"].contains("晚清漕运夜航避税路线"));
        assert!(params["quality_generation_block"].contains("章节生成质量基线"));
        assert!(params["quality_checker_block"].contains("章节质检口径"));
        assert!(params["quality_reviser_block"].contains("章节修订口径"));
        assert!(params["quality_mcp_guard_block"].contains("summary_only=true"));
        assert!(params["reference_assets"].contains("web_research_query"));
    }

    #[test]
    fn should_build_rust_quality_runtime_contract_from_prompt_params() {
        let mut project_model = build_project("one-to-one");
        project_model.default_quality_preset = Some("plot_drive".to_string());
        project_model.default_quality_notes =
            Some("提前冲突触发；压缩解释\n- 提前冲突触发".to_string());
        let chapter_model = build_chapter(12, "第十二章", Some("夜航追账"), None, None);

        let params = build_prompt_params_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            2800,
            PromptContextProviderPayload {
                recent_chapters_context: String::new(),
                previous_chapter_summary: String::new(),
                chapter_careers: "[]".to_string(),
                characters_info: "[]".to_string(),
                foreshadow_reminders: "[]".to_string(),
                relevant_memories: "[]".to_string(),
                research_query: String::new(),
                research_assets: "[]".to_string(),
                external_assets:
                    "[{\"kind\":\"web_research_query\",\"summary\":\"漕运夜航税卡绕行线\"}]"
                        .to_string(),
                reference_assets: "[]".to_string(),
                mcp_references: "MCP 摘要能力：只作参考".to_string(),
            },
            &ChapterGenerationPromptOverrides::default(),
        );

        assert!(params["quality_generation_protocol_block"].contains("rule_v3_quality_block"));
        assert!(params["quality_json_protocol_block"].contains("统一JSON协议护栏"));
        assert!(params["quality_preference_block"].contains("强情节回报"));
        assert!(params["quality_preference_block"].contains("补充偏好："));
        assert!(params["quality_preference_block"].contains("提前冲突触发"));
        assert_eq!(
            params["quality_preference_block"]
                .matches("提前冲突触发")
                .count(),
            1
        );
        assert!(params["quality_contract_block"].contains("<quality_contract priority=\"P0\">"));
        assert!(params["quality_contract_block"].contains("章节生成质量基线"));
        assert!(params["quality_contract_block"].contains("统一协议护栏"));
        assert!(params["quality_contract_block"].contains("漕运夜航税卡绕行线"));
        assert!(params["quality_mcp_references_block"].contains("MCP 摘要能力"));
    }

    #[test]
    fn should_inject_quality_contract_into_rendered_chapter_prompt() {
        let mut project_model = build_project("one-to-one");
        project_model.default_quality_preset = Some("immersive".to_string());
        let chapter_model = build_chapter(13, "第十三章", Some("潜入税卡"), None, None);

        let prompt = build_prompt_with_provider_payload(
            &chapter_model,
            &project_model,
            build_previous_chapter_prompt_context(None),
            false,
            3000,
            PromptContextProviderPayload {
                recent_chapters_context: String::new(),
                previous_chapter_summary: String::new(),
                chapter_careers: "[]".to_string(),
                characters_info: "[]".to_string(),
                foreshadow_reminders: "[]".to_string(),
                relevant_memories: "[]".to_string(),
                research_query: String::new(),
                research_assets: "[]".to_string(),
                external_assets:
                    "[{\"kind\":\"web_research_query\",\"summary\":\"水路税卡换班规律\"}]"
                        .to_string(),
                reference_assets: "[]".to_string(),
                mcp_references: String::new(),
            },
            &ChapterGenerationPromptOverrides::default(),
        )
        .expect("prompt should build");

        let fusion_contract_index = prompt
            .find("</fusion_contract>")
            .expect("chapter template should keep fusion contract");
        let quality_contract_index = prompt
            .find("<quality_contract priority=\"P0\">")
            .expect("quality contract should be injected");

        assert!(quality_contract_index > fusion_contract_index);
        assert!(prompt.contains("章节生成质量基线"));
        assert!(prompt.contains("统一协议护栏"));
        assert!(prompt.contains("沉浸场景感"));
        assert!(prompt.contains("水路税卡换班规律"));
    }
}
