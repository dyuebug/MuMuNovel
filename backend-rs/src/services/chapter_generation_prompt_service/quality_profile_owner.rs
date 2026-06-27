use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

use crate::services::chapter_generation_prompt_service::QUALITY_RUNTIME_TRACKING_TAG;

const QUALITY_PROFILE_VERSION: &str = "novel_quality_profile_v1_20260322";
const QUALITY_BASELINE_ID: &str = "fanqie_serial_baseline_v2";
const DEFAULT_STYLE_PROFILE: &str = "default";
const DEFAULT_GENRE_PROFILE: &str = "generic";
const MAX_EXTERNAL_ASSET_COUNT: usize = 6;
const MAX_EXTERNAL_ASSET_SUMMARY_LENGTH: usize = 240;
const MAX_EXTERNAL_ASSET_TITLE_LENGTH: usize = 60;
const MAX_EXTERNAL_ASSET_SOURCE_LENGTH: usize = 120;
const MAX_EXTERNAL_ASSET_USAGE_HINT_LENGTH: usize = 80;
const EXTERNAL_ASSET_SUMMARY_ONLY_NOTICE: &str =
    "只接受摘要，不接受 raw_content、全文、网页原文或大段摘录进入默认规则块。";
const EXTERNAL_ASSET_IGNORE_REASON_NO_SUMMARY: &str = "缺少摘要，默认规则块不接收原文直入";
const EXTERNAL_ASSET_IGNORE_REASON_RAW_ONLY: &str =
    "仅提供原始内容，未提供摘要，已按 summary-only 策略忽略";
const EXTERNAL_ASSET_IGNORE_REASON_LIMIT: &str = "超过摘要资产数量上限，已忽略";
const EXTERNAL_ASSET_IGNORE_REASON_DUPLICATE: &str = "重复摘要资产已折叠";

const QUALITY_BLOCK_ORDER: [&str; 5] = [
    "generation",
    "checker",
    "reviser",
    "mcp_guard",
    "external_assets",
];

const QUALITY_BLOCK_TITLES: [(&str, &str); 5] = [
    ("generation", "章节生成质量基线"),
    ("checker", "章节质检口径"),
    ("reviser", "章节修订口径"),
    ("mcp_guard", "MCP与外部参考护栏"),
    ("external_assets", "外部资产摘要"),
];

const QUALITY_FOCUS_LABELS: [(&str, &str); 8] = [
    ("opening", "开篇抓力"),
    ("conflict", "冲突升级"),
    ("outline", "大纲推进"),
    ("pacing", "节奏控制"),
    ("payoff", "兑现回收"),
    ("cliffhanger", "章尾牵引"),
    ("dialogue", "对白质感"),
    ("rule_grounding", "设定落地"),
];

const QUALITY_PROFILE_STYLE_LABELS: [(&str, &str); 6] = [
    ("low_ai_serial", "低 AI 连载感"),
    ("low_ai_life", "低 AI 生活化"),
    ("urban_finance", "都市金融"),
    ("tech_xianxia", "科技仙侠"),
    ("light_humor", "轻喜节奏"),
    ("era_plain", "年代朴素风"),
];

const QUALITY_PROFILE_GENRE_LABELS: [(&str, &str); 5] = [
    ("romance_slice_of_life", "言情 / 生活流"),
    ("suspense_mystery", "悬疑 / 谜案"),
    ("xianxia_fantasy", "仙侠 / 奇幻"),
    ("science_fiction_tech", "科幻 / 科技流"),
    ("history_power", "历史 / 权谋"),
];

const QUALITY_PROFILE_PRESET_LABELS: [(&str, &str); 4] = [
    ("plot_drive", "剧情推进"),
    ("immersive", "沉浸细节"),
    ("emotion_drama", "情绪戏剧"),
    ("clean_prose", "干净文风"),
];

const GENRE_PROFILE_TRIGGERS: [(&str, &[&str]); 5] = [
    (
        "romance_slice_of_life",
        &[
            "言情",
            "恋爱",
            "婚恋",
            "青春",
            "校园",
            "日常",
            "生活流",
            "治愈",
            "家庭",
            "现实",
            "现代情感",
            "都市情感",
            "职场言情",
            "年代",
        ],
    ),
    (
        "suspense_mystery",
        &[
            "悬疑",
            "推理",
            "惊悚",
            "刑侦",
            "无限流",
            "规则怪谈",
            "恐怖",
            "志怪",
        ],
    ),
    (
        "xianxia_fantasy",
        &["玄幻", "仙侠", "修真", "修仙", "奇幻", "魔法", "西幻"],
    ),
    (
        "science_fiction_tech",
        &["科幻", "赛博", "机甲", "硬科幻", "软科幻", "技术流"],
    ),
    (
        "history_power",
        &[
            "历史",
            "架空历史",
            "古代",
            "古言",
            "权谋",
            "朝堂",
            "宫斗",
            "官场",
            "战争",
        ],
    ),
];

const STYLE_PROFILE_TRIGGERS: [(&str, &[&str]); 6] = [
    (
        "low_ai_life",
        &["low_ai_life", "低ai生活化", "生活化", "口语", "日常感"],
    ),
    (
        "low_ai_serial",
        &["low_ai_serial", "低ai连载感", "连载感", "追更", "番茄"],
    ),
    (
        "urban_finance",
        &["urban_finance", "都市金融", "金融", "商战"],
    ),
    (
        "tech_xianxia",
        &["tech_xianxia", "技术流修仙", "技术流", "修仙"],
    ),
    ("light_humor", &["light_humor", "轻松幽默", "幽默", "搞笑"]),
    ("era_plain", &["era_plain", "朴实年代", "年代风", "年代文"]),
];

const DEFAULT_TOMATO_BASELINE_RULES: [&str; 6] = [
    "默认采用番茄连载基线：开场尽快入事，中段持续推进，末尾保留自然未完感。",
    "正文优先写正在发生的动作、人物反应和局面变化，再补必要解释。",
    "关键桥段尽量落成“动作→反馈→余波/代价”，避免大段概述替代现场。",
    "单章允许只有一个主冲突，但必须让角色做出选择，并看到即时后果。",
    "叙事视角默认贴近当前主镜头，除特殊设计外不无故切入多人内心或替角色下全知判断。",
    "禁止流程化元文本、模型自述、总结腔、预告腔和模板化口号。",
];

const CHECKER_REVIEW_ORDER: [&str; 3] = [
    "先查设定冲突、逻辑断裂和角色失真，再看文风表达与对白自然度。",
    "有明确证据再判错；证据不足时保守处理，不杜撰问题。",
    "优先标记会直接破坏阅读连续性的关键问题，避免用大量轻微问题掩盖真正主伤口。",
];

const CHECKER_SEVERITY_RULES: [&str; 3] = [
    "critical：会直接破坏设定自洽、剧情因果、角色核心行为边界或正文可读性的问题。",
    "major：明显削弱追更体验、节奏、情绪层次或信息传达，但不至于读不下去的问题。",
    "minor：局部表达、生硬句、轻微重复或可优化但不影响主链理解的问题。",
];

const CHECKER_ALLOWED_CATEGORIES: [&str; 8] = [
    "设定冲突",
    "逻辑连贯",
    "角色失真",
    "文风表达",
    "对话质量",
    "结尾处理",
    "术语可读性",
    "视角纪律",
];

const CHECKER_ASSESSMENT_SCALE: [&str; 5] = ["优秀", "良好", "一般", "较差", "存在严重问题"];
const CHECKER_SEVERITY_ORDER: [&str; 3] = ["critical", "major", "minor"];

const REVISER_CORE_RULES: [&str; 5] = [
    "先修 critical，再修最影响阅读流的 major；minor 只在不破坏节奏时顺手处理。",
    "最小改动优先：能改一句不改一段，能改一段不重写整章主线。",
    "保持原人称、角色关系、剧情方向和题材声线，不为修文新造重大剧情。",
    "若问题证据不足或缺少上游信息，明确标为 unresolved，不强行改写。",
    "修订结果必须仍是可直接阅读的小说正文或可执行建议，不能变成流程说明。",
];

const MCP_GUARD_RULES: [&str; 5] = [
    "外部资料只能作为参考，不得覆盖项目既有设定、本章大纲和角色边界。",
    "先抽取摘要，再注入 prompt；禁止把网页原文、长段摘录或大块资料直接塞进规则块。",
    "引用外部知识时，优先保留与当前剧情最相关的事实、意象或行业细节，避免整页搬运。",
    "若外部资料与项目内设定冲突，一律以内生设定为准，并把外部信息降级为可选灵感。",
    "禁止照抄资料原句，必须转成服务剧情的简短摘要或执行提醒。",
];

const EXTERNAL_ASSET_RULES: [&str; 4] = [
    "外部资产默认只接收 summary/摘要，不接收 raw_content、全文、网页正文或长篇摘录。",
    "单条摘要应控制在短摘要范围内，只保留当前任务直接需要的事实、风味或禁忌提醒。",
    "没有摘要的资料不进入默认规则块；若仅提供原文，视为未提供合规资产。",
    "同类资料优先去重合并，最多保留有限条目，避免资料噪音反客为主。",
];

pub(crate) fn build_quality_profile_owner_contract() -> Value {
    json!({
        "owner": "chapter_generation_prompt_service::quality_profile_owner",
        "scope": "shared_generation_prompt_quality_profile_owner",
        "python_source_map": [],
        "rust_owner_map": [
            "backend-rs/src/services/chapter_generation_prompt_service/quality_profile_owner.rs",
            "backend-rs/src/services/chapter_generation_prompt_service.rs",
            "backend-rs/src/api/health.rs"
        ],
        "behavior_contract": {
            "entrypoints": [
                "build_novel_quality_prompt_blocks",
                "resolve_quality_weight_profile",
                "resolve_adaptive_quality_gate_profile",
                "resolve_metric_threshold_adjustments"
            ],
            "quality_block_order": QUALITY_BLOCK_ORDER,
            "default_style_profile": DEFAULT_STYLE_PROFILE,
            "default_genre_profile": DEFAULT_GENRE_PROFILE,
            "external_asset_policy": [
                "summary_only_assets",
                "duplicate_assets_collapsed",
                "max_external_asset_count = 6"
            ],
            "runtime_tracking_tag": QUALITY_RUNTIME_TRACKING_TAG
        },
        "active_consumers": [
            "chapter_generation_prompt_service",
            "chapter_single_generation_prepare_service",
            "chapter_single_generation_runtime_restore_workflow_service",
            "chapter_single_generation_stream_workflow_service",
            "chapter_batch_generation_runtime_state_service",
            "chapter_regeneration_prepare_service",
            "chapter_single_generation_active_gateway_smoke_service",
            "chapter_batch_generation_active_gateway_smoke_service"
        ],
        "validation_boundary": [
            "cargo test chapter_generation_prompt_service",
            "cargo test api::health",
            "cargo check --manifest-path backend-rs/Cargo.toml"
        ],
        "rollback_boundary": {
            "source_map_policy": "production_python_quality_profile_source_map_deleted_after_rust_owner_validation",
            "compatibility_note": "quality profile blocks, weight policy, gate profile, external asset summary rules, quality-focus protected block resolution, shared runtime prompt helper normalization, and profile derivation stay stable for single, batch, and regeneration prompt consumers; historical Python behavior lives only under backend/tests/test_support/story_prompt_block_test_support.py"
        }
    })
}

#[derive(Clone, Copy)]
struct QualityDimension {
    key: &'static str,
    label: &'static str,
    generation_goal: &'static str,
    checker_focus: &'static str,
    reviser_focus: &'static str,
}

const QUALITY_DIMENSIONS: [QualityDimension; 8] = [
    QualityDimension {
        key: "conflict_chain",
        label: "冲突链",
        generation_goal: "单章至少让“目标→阻力→选择→即时后果”可见，避免只有概述没有代价。",
        checker_focus: "重点检查冲突是否真的逼出选择，以及选择之后是否带来损失、新麻烦或关系变化。",
        reviser_focus: "优先补齐选择与代价链，避免把关键桥段改成空泛总结。",
    },
    QualityDimension {
        key: "rule_grounding",
        label: "规则落地",
        generation_goal: "世界规则、行业规则或力量规则要落到角色行动与后果，不只停留在设定说明。",
        checker_focus: "检查设定术语是否只讲不演，是否缺少触发条件、限制、反噬或现实代价。",
        reviser_focus: "把规则说明压回场景，让角色通过动作、反馈和后果把规则演出来。",
    },
    QualityDimension {
        key: "outline_alignment",
        label: "大纲对齐",
        generation_goal: "本章必须覆盖当前大纲锚点，但允许换顺序、换切口，不机械逐条照抄。",
        checker_focus: "检查正文是否跑题、漏掉主锚点，或把关键事件写成无效铺垫。",
        reviser_focus: "优先修复漏写的大纲锚点与剧情承接断层，保持主线稳定。",
    },
    QualityDimension {
        key: "viewpoint_discipline",
        label: "视角纪律",
        generation_goal: "叙事镜头默认贴近当前主视角，除明确设计外不无故切入多人内心。",
        checker_focus: "检查是否出现视角漂移、替角色解释真实想法，或作者俯视式替人物总结命运。",
        reviser_focus: "优先删改无依据的内心切换与全知判断，保持人称、镜头重心和情绪来源稳定。",
    },
    QualityDimension {
        key: "dialogue_naturalness",
        label: "对白自然度",
        generation_goal: "对白像真人交流，要有停顿、反问、改口、潜台词和角色声线差异。",
        checker_focus: "检查角色是否同口吻讲道理，或对白过长、过整齐、过说明书化。",
        reviser_focus: "优先压短生硬对白，补动作、语气和信息落差，保留角色各自的声音。",
    },
    QualityDimension {
        key: "opening_hook",
        label: "开场钩子",
        generation_goal:
            "开场尽快给异常、任务压力、关系摩擦或信息缺口，让读者知道“这一章为什么要看”。",
        checker_focus: "检查前段是否只是背景铺陈，缺少当前进行中的麻烦、风险或悬念。",
        reviser_focus: "优先把静态介绍改成正在发生的动作、压力或冲突切入。",
    },
    QualityDimension {
        key: "payoff_chain",
        label: "小爽点链条",
        generation_goal: "尽量形成“铺垫→爆发→反馈”的小满足，不要求每章都打脸，但要给追更回报。",
        checker_focus: "检查是否只铺不收、只喊结果不写反馈，或爽点与人物选择脱节。",
        reviser_focus: "优先补反馈与余波，让关键得失落到人物与场面。",
    },
    QualityDimension {
        key: "cliffhanger",
        label: "章尾牵引",
        generation_goal: "章尾优先停在信息缺口、危险临门、身份反转或选择未决，不要用总结腔收束。",
        checker_focus: "检查结尾是否提前把情绪说尽、把问题讲完，导致追更牵引不足。",
        reviser_focus: "优先把总结句改成动作、对话或未落地的后果，让章尾留白但不空洞。",
    },
];

#[derive(Clone, Copy)]
struct QualityRelaxationSnapshot {
    scope: &'static str,
    key: &'static str,
    label: &'static str,
    generation_relaxations: &'static [&'static str],
    checker_adjustments: &'static [&'static str],
    reviser_adjustments: &'static [&'static str],
}

const ROMANCE_GENERATION_RELAXATIONS: [&str; 3] = [
    "允许用关系压力、生活摩擦、尴尬局面或情绪错位替代高烈度外部危机。",
    "开场钩子可以是秘密、误会、反常态度或当场难堪，不强求立刻上大事件。",
    "小爽点可以是关系推进、认知翻转、情绪回弹或立场改变，不只看打脸与胜负。",
];
const ROMANCE_CHECKER_ADJUSTMENTS: [&str; 2] = [
    "不要仅因缺少战斗或灾难就判定节奏弱，重点看关系是否推进、情绪是否有层次。",
    "对白自然度和潜台词权重上调，但仍要避免角色轮流端着讲道理。",
];
const ROMANCE_REVISER_ADJUSTMENTS: [&str; 2] = [
    "修订时优先保留细腻情绪与日常颗粒感，不强塞额外大危机。",
    "若章尾较柔和，至少补一个未说破的关系张力或下一步选择。",
];

const SUSPENSE_GENERATION_RELAXATIONS: [&str; 2] = [
    "允许暂时不解释真相，但必须持续提供线索、反常细节或压力升级。",
    "章尾可以优先强化信息缺口与危险临门，阶段性小爽点不是硬性要求。",
];
const SUSPENSE_CHECKER_ADJUSTMENTS: [&str; 2] = [
    "不要因为故意留白就判逻辑断裂，先看线索是否公平、悬念是否持续有效。",
    "对设定说明的要求可略放宽，但必须能从后果反推到规则存在。",
];
const SUSPENSE_REVISER_ADJUSTMENTS: [&str; 2] = [
    "修订时优先补足线索可读性与压力升级，不提前泄底。",
    "若信息压得过深，补一处可验证细节而不是整段解释。",
];

const XIANXIA_GENERATION_RELAXATIONS: [&str; 2] = [
    "允许术法、境界、门派或种族术语密度略高，但必须贴着动作反馈出现。",
    "小爽点可以是悟道、破局、压制、收获资源或境界突破，不局限于正面打脸。",
];
const XIANXIA_CHECKER_ADJUSTMENTS: [&str; 2] = [
    "不要把所有术语都当成阅读障碍，先看是否给出场景内的人话解释与代价。",
    "重点检查规则边界、资源代价与强度升级是否自洽。",
];
const XIANXIA_REVISER_ADJUSTMENTS: [&str; 2] = [
    "修订时优先补规则触发条件和代价，不削弱题材风味。",
    "如说明过密，用角色误解、追问或身体反馈压缩解释。",
];

const TECH_GENERATION_RELAXATIONS: [&str; 2] = [
    "允许出现机制推演、任务流程或设备细节，但每段都要挂在行动结果上。",
    "开场钩子可以是异常数据、系统故障、任务时限或技术失控。",
];
const TECH_CHECKER_ADJUSTMENTS: [&str; 2] = [
    "不要单凭术语略多就判AI味重，重点看术语是否推动决策与后果。",
    "对白可保留少量专业表达，但不能整段写成会议纪要。",
];
const TECH_REVISER_ADJUSTMENTS: [&str; 2] = [
    "修订时优先把讲义感压缩到动作反馈里，保留必要的专业可信度。",
    "若信息量过载，先删重复解释，再补一个可感知的现实后果。",
];

const HISTORY_GENERATION_RELAXATIONS: [&str; 2] = [
    "允许开场先落礼制、身份压力、局势变化或筹码差，而不是立刻高噪声冲突。",
    "小爽点可以是试探得手、地位变化、筹码反转或一句话压人，不强求动作爆点。",
];
const HISTORY_CHECKER_ADJUSTMENTS: [&str; 2] = [
    "重点检查动机、身份秩序和信息差，不因表达克制就误判节奏不足。",
    "对白可更克制含蓄，但必须听得出身份层级和潜台词。",
];
const HISTORY_REVISER_ADJUSTMENTS: [&str; 2] = [
    "修订时优先稳住礼制语境与权力关系，不硬塞现代口语式高爆冲突。",
    "若章尾较稳，补一个未明说的筹码变化或风险外溢。",
];

const LOW_AI_LIFE_GENERATION_RELAXATIONS: [&str; 3] = [
    "允许开场更贴近日常现场，只要前段能看见眼前麻烦、情绪摩擦或局面变化。",
    "对白允许打断、改口、留白和少量口语毛边，不要求句句工整。",
    "章尾可以更柔和，但至少留下情绪余震、关系余波或下一步动作牵引。",
];
const LOW_AI_LIFE_CHECKER_ADJUSTMENTS: [&str; 2] = [
    "不要把口语毛边误判成低质文风，重点看声线区分和信息效率。",
    "爽点权重略降，日常真实感与情绪层次权重上调。",
];
const LOW_AI_LIFE_REVISER_ADJUSTMENTS: [&str; 2] = [
    "修订时优先保留生活噪声、动作细节和人物嘴感，不把文本修成说明文。",
    "若对白过顺，补停顿、接话失败或动作遮挡，而不是加鸡汤解释。",
];

const LOW_AI_SERIAL_GENERATION_RELAXATIONS: [&str; 2] = [
    "允许句子更短、更口语、更带现场感，不以书面工整度换取连载节奏。",
    "配角只要做出会改变局面的主动选择，即可视为有效推进，不强求每个人都长篇输出。",
];
const LOW_AI_SERIAL_CHECKER_ADJUSTMENTS: [&str; 2] = [
    "不要因为语言偏直接、颗粒偏粗就误判为文风差，先看追更牵引是否成立。",
    "开场钩子、冲突链和章尾牵引的权重上调，但仍需允许局部呼吸段。",
];
const LOW_AI_SERIAL_REVISER_ADJUSTMENTS: [&str; 2] = [
    "修订时优先保住现场感、推进力和角色情绪反差，不把连载文修成端正但没劲的稿子。",
    "如章尾过满，宁可留动作停顿，也不要补总结句。",
];

const URBAN_FINANCE_RELAXATIONS: [&str; 1] =
    ["允许出现专业术语，但必须同步写出利益得失、信息差或筹码变化。"];
const URBAN_FINANCE_CHECKER: [&str; 1] =
    ["重点检查术语是否真正推动博弈，不因题材专业性本身判定晦涩。"];
const URBAN_FINANCE_REVISER: [&str; 1] = ["修订时优先把抽象术语落回利益链和人物选择。"];
const TECH_XIANXIA_RELAXATIONS: [&str; 1] =
    ["允许规则推演略长，但必须持续附着在试错、消耗或破局动作上。"];
const TECH_XIANXIA_CHECKER: [&str; 1] =
    ["重点看推演是否能反推出行动方案，不把所有推导都当作赘述。"];
const TECH_XIANXIA_REVISER: [&str; 1] = ["修订时优先删重复推导，保留真正改变局面的关键步骤。"];
const LIGHT_HUMOR_RELAXATIONS: [&str; 1] =
    ["允许多一点插科打诨，但每轮玩笑都应推动关系、冲突或信息揭示。"];
const LIGHT_HUMOR_CHECKER: [&str; 1] =
    ["不要因角色互怼就误判不严肃，重点看笑点是否服务剧情而非打断剧情。"];
const LIGHT_HUMOR_REVISER: [&str; 1] = ["修订时保留人物互怼节奏，优先删除与局面无关的重复包袱。"];
const ERA_PLAIN_RELAXATIONS: [&str; 1] =
    ["允许表达更克制、事件更生活化，但要保证人物压力和现实阻力具体可见。"];
const ERA_PLAIN_CHECKER: [&str; 1] =
    ["不要用高爆点模板要求年代文，重点看生活细节、人情压力和选择后果。"];
const ERA_PLAIN_REVISER: [&str; 1] =
    ["修订时优先稳住朴素语气和时代质感，避免强塞网络热梗与悬浮金句。"];

#[derive(Clone)]
struct ExternalAssetSummary {
    title: String,
    source: String,
    summary: String,
    usage_hint: String,
    asset_type: String,
}

impl ExternalAssetSummary {
    fn to_line(&self) -> String {
        let mut parts = vec![self.title.clone()];
        if !self.asset_type.is_empty() {
            parts.push(format!("类型：{}", self.asset_type));
        }
        if !self.source.is_empty() {
            parts.push(format!("来源：{}", self.source));
        }
        if !self.usage_hint.is_empty() {
            parts.push(format!("使用提醒：{}", self.usage_hint));
        }
        parts.push(format!("摘要：{}", self.summary));
        parts.join("；")
    }

    fn to_value(&self) -> Value {
        json!({
            "title": self.title,
            "source": self.source,
            "summary": self.summary,
            "usage_hint": self.usage_hint,
            "asset_type": self.asset_type,
            "summary_only": true,
        })
    }
}

#[derive(Clone)]
struct IgnoredExternalAsset {
    title: String,
    reason: &'static str,
}

impl IgnoredExternalAsset {
    fn to_value(&self) -> Value {
        json!({
            "title": self.title,
            "reason": self.reason,
        })
    }
}

pub(crate) fn build_novel_quality_profile(payload: Option<&Map<String, Value>>) -> Value {
    let genre = map_text(payload, "genre");
    let style_name = map_text(payload, "style_name");
    let style_preset_id = map_text(payload, "style_preset_id");
    let style_content = map_text(payload, "style_content");
    let style_profile = detect_style_profile(&style_name, &style_preset_id, &style_content);
    let genre_profiles = detect_genre_profiles(&genre);
    let active_relaxations = build_relaxation_snapshots(&genre_profiles, &style_profile);
    let raw_assets = payload
        .and_then(|item| item.get("external_assets"))
        .or_else(|| payload.and_then(|item| item.get("reference_assets")));
    let (external_assets, ignored_assets) = sanitize_external_assets(raw_assets);

    let generation = build_profile_block(
        "generation",
        lookup_label(&QUALITY_BLOCK_TITLES, "generation"),
        build_generation_lines(&active_relaxations, &external_assets),
    );
    let checker = build_profile_block(
        "checker",
        lookup_label(&QUALITY_BLOCK_TITLES, "checker"),
        build_checker_lines(&active_relaxations),
    );
    let reviser = build_profile_block(
        "reviser",
        lookup_label(&QUALITY_BLOCK_TITLES, "reviser"),
        build_reviser_lines(&active_relaxations),
    );
    let mcp_guard = build_profile_block(
        "mcp_guard",
        lookup_label(&QUALITY_BLOCK_TITLES, "mcp_guard"),
        build_mcp_guard_lines(&external_assets, &ignored_assets),
    );
    let external_assets_block = build_profile_block(
        "external_assets",
        lookup_label(&QUALITY_BLOCK_TITLES, "external_assets"),
        build_external_asset_lines(&external_assets, &ignored_assets),
    );

    let block_items = [
        ("generation", generation),
        ("checker", checker),
        ("reviser", reviser),
        ("mcp_guard", mcp_guard),
        ("external_assets", external_assets_block),
    ];
    let blocks = QUALITY_BLOCK_ORDER
        .iter()
        .filter_map(|key| {
            block_items
                .iter()
                .find(|(item_key, _)| item_key == key)
                .map(|(_, value)| ((*key).to_string(), value.clone()))
        })
        .collect::<Map<String, Value>>();
    let prompt_blocks = blocks
        .iter()
        .filter_map(|(key, block)| block.get("text").cloned().map(|text| (key.clone(), text)))
        .collect::<Map<String, Value>>();

    json!({
        "version": QUALITY_PROFILE_VERSION,
        "baseline_id": QUALITY_BASELINE_ID,
        "genre_profiles": genre_profiles,
        "style_profile": style_profile,
        "quality_dimensions": QUALITY_DIMENSIONS.iter().map(|item| item.key).collect::<Vec<_>>(),
        "active_relaxations": active_relaxations.iter().map(relaxation_to_value).collect::<Vec<_>>(),
        "external_assets": external_assets.iter().map(ExternalAssetSummary::to_value).collect::<Vec<_>>(),
        "ignored_external_assets": ignored_assets.iter().map(IgnoredExternalAsset::to_value).collect::<Vec<_>>(),
        "generation": blocks.get("generation").cloned().unwrap_or_else(|| json!({})),
        "checker": blocks.get("checker").cloned().unwrap_or_else(|| json!({})),
        "reviser": blocks.get("reviser").cloned().unwrap_or_else(|| json!({})),
        "mcp_guard": blocks.get("mcp_guard").cloned().unwrap_or_else(|| json!({})),
        "external_assets_block": blocks.get("external_assets").cloned().unwrap_or_else(|| json!({})),
        "blocks": blocks,
        "policy": build_profile_policy(),
        "prompt_blocks": prompt_blocks,
    })
}

pub(crate) fn build_novel_quality_prompt_blocks(payload: Option<&Map<String, Value>>) -> Value {
    build_novel_quality_profile(payload)
        .get("prompt_blocks")
        .cloned()
        .unwrap_or_else(|| json!({}))
}

pub(crate) fn resolve_runtime_quality_profile(
    runtime_context: Option<&Map<String, Value>>,
) -> Value {
    let genre = map_text(runtime_context, "genre");
    let style_name = map_text(runtime_context, "style_name");
    let style_preset_id = map_text(runtime_context, "style_preset_id");
    let mut style_profile =
        normalize_profile_token(runtime_context.and_then(|ctx| ctx.get("style_profile")));
    if style_profile.is_empty() {
        style_profile = detect_style_profile(&style_name, &style_preset_id, "");
    }

    let mut genre_profiles = normalize_profile_token_sequence(
        runtime_context.and_then(|ctx| ctx.get("genre_profiles")),
        4,
    );
    if genre_profiles.is_empty() || genre_profiles == ["default".to_string()] {
        genre_profiles = detect_genre_profiles(&genre);
    }

    json!({
        "genre": genre,
        "genre_profiles": genre_profiles,
        "style_name": style_name,
        "style_preset_id": style_preset_id,
        "style_profile": style_profile,
        "quality_preset": normalize_profile_token(runtime_context.and_then(|ctx| ctx.get("quality_preset"))),
    })
}

pub(crate) fn resolve_quality_weight_profile(
    runtime_context: Option<&Map<String, Value>>,
    resolved_stage: Option<&str>,
) -> Value {
    let mut weights = default_focus_weights();
    match resolved_stage.unwrap_or_default() {
        "opening" => {
            apply_focus_weight(&mut weights, "opening", 1.15);
            apply_focus_weight(&mut weights, "conflict", 1.08);
            apply_focus_weight(&mut weights, "outline", 1.05);
        }
        "ending" => {
            apply_focus_weight(&mut weights, "payoff", 1.18);
            apply_focus_weight(&mut weights, "cliffhanger", 1.10);
            apply_focus_weight(&mut weights, "outline", 1.08);
            apply_focus_weight(&mut weights, "conflict", 1.05);
        }
        _ => {
            apply_focus_weight(&mut weights, "conflict", 1.12);
            apply_focus_weight(&mut weights, "pacing", 1.08);
            apply_focus_weight(&mut weights, "payoff", 1.05);
        }
    }

    let profile = resolve_runtime_quality_profile(runtime_context);
    let style_profile = profile_text(&profile, "style_profile");
    let genre_profiles = profile
        .get("genre_profiles")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let quality_preset = profile_text(&profile, "quality_preset");

    apply_style_weights(&mut weights, &style_profile);
    apply_genre_weights(&mut weights, &genre_profiles);
    apply_preset_weights(&mut weights, &quality_preset);

    let mut ranked_weights = weights.iter().collect::<Vec<_>>();
    ranked_weights
        .sort_by(|left, right| right.1.total_cmp(left.1).then_with(|| left.0.cmp(right.0)));
    let emphasized_focuses = ranked_weights
        .iter()
        .filter(|(_, weight)| **weight >= 1.08)
        .take(3)
        .map(|(focus, _)| (*focus).to_string())
        .collect::<Vec<_>>();
    let focus_labels = emphasized_focuses
        .iter()
        .map(|focus| lookup_label(&QUALITY_FOCUS_LABELS, focus))
        .collect::<Vec<_>>();
    let summary = build_profile_summary(
        &genre_profiles,
        &style_profile,
        &quality_preset,
        &focus_labels,
    );
    let weights_json = weights
        .into_iter()
        .map(|(key, value)| (key.to_string(), json!(value)))
        .collect::<Map<String, Value>>();

    json!({
        "weights": weights_json,
        "focus_areas": emphasized_focuses,
        "focus_labels": focus_labels,
        "summary": summary,
        "genre_profiles": genre_profiles,
        "style_profile": style_profile,
        "quality_preset": quality_preset,
    })
}

pub(crate) fn resolve_adaptive_quality_gate_profile(
    runtime_context: Option<&Map<String, Value>>,
    resolved_stage: Option<&str>,
) -> Value {
    let runtime_profile = resolve_runtime_quality_profile(runtime_context);
    let weight_profile = resolve_quality_weight_profile(runtime_context, resolved_stage);
    json!({
        "resolved_stage": resolved_stage.unwrap_or_default(),
        "quality_preset": runtime_profile.get("quality_preset").cloned().unwrap_or(Value::String(String::new())),
        "style_profile": runtime_profile.get("style_profile").cloned().unwrap_or(Value::String(String::new())),
        "genre_profiles": runtime_profile.get("genre_profiles").cloned().unwrap_or_else(|| json!([])),
        "focus_areas": weight_profile.get("focus_areas").cloned().unwrap_or_else(|| json!([])),
        "weight_profile": weight_profile,
    })
}

pub(crate) fn resolve_metric_threshold_adjustments(
    runtime_context: Option<&Map<String, Value>>,
    resolved_stage: Option<&str>,
) -> HashMap<&'static str, f64> {
    let profile = resolve_runtime_quality_profile(runtime_context);
    let weight_profile = resolve_quality_weight_profile(runtime_context, resolved_stage);
    let quality_preset = profile_text(&profile, "quality_preset");
    let style_profile = profile_text(&profile, "style_profile");
    let genre_profiles = profile
        .get("genre_profiles")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let focus_areas = weight_profile
        .get("focus_areas")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let creative_mode = map_text(runtime_context, "creative_mode");
    let story_focus = map_text(runtime_context, "story_focus");
    let mut adjustments = HashMap::new();

    match resolved_stage.unwrap_or_default() {
        "opening" => {
            add_adjustment(&mut adjustments, "opening_hook_rate", 6.0);
            add_adjustment(&mut adjustments, "outline_alignment_rate", 3.0);
            add_adjustment(&mut adjustments, "payoff_chain_rate", -4.0);
            add_adjustment(&mut adjustments, "cliffhanger_rate", 1.0);
        }
        "development" => {
            add_adjustment(&mut adjustments, "conflict_chain_hit_rate", 4.0);
            add_adjustment(&mut adjustments, "dialogue_naturalness_rate", 1.0);
            add_adjustment(&mut adjustments, "opening_hook_rate", -2.0);
            add_adjustment(&mut adjustments, "pacing_score", 0.4);
        }
        "ending" => {
            add_adjustment(&mut adjustments, "payoff_chain_rate", 6.0);
            add_adjustment(&mut adjustments, "cliffhanger_rate", 4.0);
            add_adjustment(&mut adjustments, "conflict_chain_hit_rate", 2.0);
            add_adjustment(&mut adjustments, "opening_hook_rate", -4.0);
            add_adjustment(&mut adjustments, "outline_alignment_rate", 1.0);
            add_adjustment(&mut adjustments, "pacing_score", 0.4);
        }
        _ => {}
    }

    apply_profile_threshold_adjustments(
        &mut adjustments,
        &quality_preset,
        &style_profile,
        &genre_profiles,
    );
    apply_intent_threshold_adjustments(&mut adjustments, &creative_mode, &story_focus);
    for focus_area in focus_areas {
        match focus_area.as_str() {
            "opening" => add_adjustment(&mut adjustments, "opening_hook_rate", 1.0),
            "conflict" => add_adjustment(&mut adjustments, "conflict_chain_hit_rate", 1.0),
            "outline" => add_adjustment(&mut adjustments, "outline_alignment_rate", 1.0),
            "dialogue" => add_adjustment(&mut adjustments, "dialogue_naturalness_rate", 1.0),
            "payoff" => add_adjustment(&mut adjustments, "payoff_chain_rate", 1.0),
            "cliffhanger" => add_adjustment(&mut adjustments, "cliffhanger_rate", 1.0),
            "rule_grounding" => add_adjustment(&mut adjustments, "rule_grounding_hit_rate", 1.0),
            "pacing" => add_adjustment(&mut adjustments, "pacing_score", 0.2),
            _ => {}
        }
    }

    adjustments
}

fn default_focus_weights() -> HashMap<&'static str, f64> {
    [
        ("opening", 1.0),
        ("conflict", 1.0),
        ("outline", 1.0),
        ("pacing", 1.0),
        ("payoff", 1.0),
        ("cliffhanger", 1.0),
        ("dialogue", 1.0),
        ("rule_grounding", 1.0),
    ]
    .into_iter()
    .collect()
}

fn normalize_profile_token(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_lowercase()
}

fn normalize_profile_token_sequence(value: Option<&Value>, limit: usize) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    let values = match value {
        Value::Array(items) => items.iter().collect::<Vec<_>>(),
        Value::Null => Vec::new(),
        _ => vec![value],
    };
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for item in values {
        let token = normalize_profile_token(Some(item));
        if token.is_empty() || !seen.insert(token.clone()) {
            continue;
        }
        normalized.push(token);
        if normalized.len() >= limit {
            break;
        }
    }
    normalized
}

fn detect_style_profile(style_name: &str, style_preset_id: &str, style_content: &str) -> String {
    let preset = style_preset_id.trim().to_lowercase();
    let merged = [preset.as_str(), style_name, style_content]
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    for (key, triggers) in STYLE_PROFILE_TRIGGERS {
        if !preset.is_empty() && preset == key {
            return key.to_string();
        }
        if triggers.iter().any(|trigger| merged.contains(trigger)) {
            return key.to_string();
        }
    }
    DEFAULT_STYLE_PROFILE.to_string()
}

fn detect_genre_profiles(genre: &str) -> Vec<String> {
    let normalized = genre.trim().to_lowercase();
    if normalized.is_empty() {
        return vec![DEFAULT_GENRE_PROFILE.to_string()];
    }
    let mut matched = Vec::new();
    let mut seen = HashSet::new();
    for (key, triggers) in GENRE_PROFILE_TRIGGERS {
        if triggers.iter().any(|trigger| normalized.contains(trigger)) && seen.insert(key) {
            matched.push(key.to_string());
        }
    }
    if matched.is_empty() {
        vec![DEFAULT_GENRE_PROFILE.to_string()]
    } else {
        matched
    }
}

fn map_text(runtime_context: Option<&Map<String, Value>>, key: &str) -> String {
    runtime_context
        .and_then(|ctx| ctx.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

fn profile_text(profile: &Value, key: &str) -> String {
    profile
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn apply_focus_weight(
    weights: &mut HashMap<&'static str, f64>,
    focus_area: &'static str,
    multiplier: f64,
) {
    let Some(weight) = weights.get_mut(focus_area) else {
        return;
    };
    *weight = ((*weight * multiplier).clamp(0.78, 1.45) * 10_000.0).round() / 10_000.0;
}

fn apply_style_weights(weights: &mut HashMap<&'static str, f64>, style_profile: &str) {
    match style_profile {
        "low_ai_serial" => {
            apply_focus_weight(weights, "conflict", 1.10);
            apply_focus_weight(weights, "payoff", 1.08);
            apply_focus_weight(weights, "cliffhanger", 1.12);
        }
        "low_ai_life" => {
            apply_focus_weight(weights, "dialogue", 1.14);
            apply_focus_weight(weights, "payoff", 1.08);
            apply_focus_weight(weights, "cliffhanger", 0.94);
            apply_focus_weight(weights, "outline", 1.04);
        }
        "urban_finance" => {
            apply_focus_weight(weights, "rule_grounding", 1.10);
            apply_focus_weight(weights, "dialogue", 1.08);
            apply_focus_weight(weights, "conflict", 1.06);
        }
        "tech_xianxia" => {
            apply_focus_weight(weights, "rule_grounding", 1.14);
            apply_focus_weight(weights, "payoff", 1.08);
            apply_focus_weight(weights, "outline", 1.06);
        }
        "light_humor" => {
            apply_focus_weight(weights, "dialogue", 1.12);
            apply_focus_weight(weights, "cliffhanger", 0.95);
            apply_focus_weight(weights, "payoff", 1.05);
        }
        "era_plain" => {
            apply_focus_weight(weights, "rule_grounding", 1.08);
            apply_focus_weight(weights, "outline", 1.05);
            apply_focus_weight(weights, "dialogue", 1.05);
        }
        _ => {}
    }
}

fn apply_genre_weights(weights: &mut HashMap<&'static str, f64>, genre_profiles: &[String]) {
    if genre_profiles
        .iter()
        .any(|item| item == "romance_slice_of_life")
    {
        apply_focus_weight(weights, "dialogue", 1.10);
        apply_focus_weight(weights, "payoff", 1.08);
        apply_focus_weight(weights, "cliffhanger", 0.95);
    }
    if genre_profiles.iter().any(|item| item == "suspense_mystery") {
        apply_focus_weight(weights, "conflict", 1.08);
        apply_focus_weight(weights, "cliffhanger", 1.12);
        apply_focus_weight(weights, "outline", 1.06);
    }
    if genre_profiles.iter().any(|item| item == "xianxia_fantasy") {
        apply_focus_weight(weights, "rule_grounding", 1.12);
        apply_focus_weight(weights, "payoff", 1.08);
        apply_focus_weight(weights, "outline", 1.06);
    }
    if genre_profiles
        .iter()
        .any(|item| item == "science_fiction_tech")
    {
        apply_focus_weight(weights, "rule_grounding", 1.12);
        apply_focus_weight(weights, "outline", 1.08);
        apply_focus_weight(weights, "opening", 1.04);
    }
    if genre_profiles.iter().any(|item| item == "history_power") {
        apply_focus_weight(weights, "outline", 1.10);
        apply_focus_weight(weights, "rule_grounding", 1.08);
        apply_focus_weight(weights, "conflict", 1.06);
    }
}

fn apply_preset_weights(weights: &mut HashMap<&'static str, f64>, quality_preset: &str) {
    match quality_preset {
        "immersive" => {
            apply_focus_weight(weights, "dialogue", 1.10);
            apply_focus_weight(weights, "rule_grounding", 1.08);
            apply_focus_weight(weights, "pacing", 1.05);
        }
        "plot_drive" => {
            apply_focus_weight(weights, "conflict", 1.12);
            apply_focus_weight(weights, "cliffhanger", 1.08);
            apply_focus_weight(weights, "outline", 1.06);
        }
        "emotion_drama" => {
            apply_focus_weight(weights, "dialogue", 1.12);
            apply_focus_weight(weights, "payoff", 1.10);
            apply_focus_weight(weights, "conflict", 1.04);
        }
        "clean_prose" => {
            apply_focus_weight(weights, "pacing", 1.08);
            apply_focus_weight(weights, "dialogue", 1.06);
            apply_focus_weight(weights, "rule_grounding", 1.04);
        }
        _ => {}
    }
}

fn apply_profile_threshold_adjustments(
    adjustments: &mut HashMap<&'static str, f64>,
    quality_preset: &str,
    style_profile: &str,
    genre_profiles: &HashSet<String>,
) {
    match quality_preset {
        "emotion_drama" => {
            add_adjustment(adjustments, "dialogue_naturalness_rate", 2.0);
            add_adjustment(adjustments, "payoff_chain_rate", 2.0);
            add_adjustment(adjustments, "conflict_chain_hit_rate", 1.0);
        }
        "clean_prose" => {
            add_adjustment(adjustments, "pacing_score", 0.5);
            add_adjustment(adjustments, "dialogue_naturalness_rate", 1.0);
            add_adjustment(adjustments, "rule_grounding_hit_rate", 0.5);
        }
        "plot_drive" => {
            add_adjustment(adjustments, "conflict_chain_hit_rate", 2.0);
            add_adjustment(adjustments, "cliffhanger_rate", 1.0);
            add_adjustment(adjustments, "outline_alignment_rate", 1.0);
        }
        "immersive" => {
            add_adjustment(adjustments, "dialogue_naturalness_rate", 1.0);
            add_adjustment(adjustments, "rule_grounding_hit_rate", 1.0);
            add_adjustment(adjustments, "pacing_score", 0.2);
        }
        _ => {}
    }

    match style_profile {
        "urban_finance" => {
            add_adjustment(adjustments, "rule_grounding_hit_rate", 3.0);
            add_adjustment(adjustments, "outline_alignment_rate", 1.0);
            add_adjustment(adjustments, "dialogue_naturalness_rate", 1.0);
        }
        "tech_xianxia" => {
            add_adjustment(adjustments, "rule_grounding_hit_rate", 4.0);
            add_adjustment(adjustments, "payoff_chain_rate", 1.0);
            add_adjustment(adjustments, "outline_alignment_rate", 1.0);
        }
        "low_ai_life" => {
            add_adjustment(adjustments, "dialogue_naturalness_rate", 2.0);
            add_adjustment(adjustments, "payoff_chain_rate", 1.0);
            add_adjustment(adjustments, "cliffhanger_rate", -1.0);
        }
        "low_ai_serial" => {
            add_adjustment(adjustments, "conflict_chain_hit_rate", 1.0);
            add_adjustment(adjustments, "payoff_chain_rate", 1.0);
            add_adjustment(adjustments, "cliffhanger_rate", 1.0);
        }
        _ => {}
    }

    if genre_profiles.contains("romance_slice_of_life") {
        add_adjustment(adjustments, "dialogue_naturalness_rate", 2.0);
        add_adjustment(adjustments, "payoff_chain_rate", 1.0);
        add_adjustment(adjustments, "cliffhanger_rate", -1.0);
    }
    if genre_profiles.contains("suspense_mystery") {
        add_adjustment(adjustments, "conflict_chain_hit_rate", 1.0);
        add_adjustment(adjustments, "cliffhanger_rate", 2.0);
        add_adjustment(adjustments, "outline_alignment_rate", 1.0);
    }
    if genre_profiles.contains("xianxia_fantasy") {
        add_adjustment(adjustments, "rule_grounding_hit_rate", 2.0);
        add_adjustment(adjustments, "payoff_chain_rate", 1.0);
    }
    if genre_profiles.contains("science_fiction_tech") {
        add_adjustment(adjustments, "rule_grounding_hit_rate", 2.0);
        add_adjustment(adjustments, "outline_alignment_rate", 1.0);
    }
    if genre_profiles.contains("history_power") {
        add_adjustment(adjustments, "rule_grounding_hit_rate", 2.0);
        add_adjustment(adjustments, "outline_alignment_rate", 1.0);
        add_adjustment(adjustments, "conflict_chain_hit_rate", 1.0);
    }
}

fn apply_intent_threshold_adjustments(
    adjustments: &mut HashMap<&'static str, f64>,
    creative_mode: &str,
    story_focus: &str,
) {
    match creative_mode {
        "hook" | "suspense" => {
            add_adjustment(adjustments, "opening_hook_rate", 1.0);
            add_adjustment(adjustments, "cliffhanger_rate", 1.0);
        }
        "emotion" => {
            add_adjustment(adjustments, "dialogue_naturalness_rate", 1.0);
            add_adjustment(adjustments, "payoff_chain_rate", 1.0);
        }
        "relationship" => {
            add_adjustment(adjustments, "dialogue_naturalness_rate", 2.0);
            add_adjustment(adjustments, "conflict_chain_hit_rate", 1.0);
        }
        "payoff" => add_adjustment(adjustments, "payoff_chain_rate", 2.0),
        _ => {}
    }

    match story_focus {
        "advance_plot" => {
            add_adjustment(adjustments, "conflict_chain_hit_rate", 1.0);
            add_adjustment(adjustments, "outline_alignment_rate", 1.0);
        }
        "deepen_character" => {
            add_adjustment(adjustments, "dialogue_naturalness_rate", 1.0);
            add_adjustment(adjustments, "payoff_chain_rate", 1.0);
        }
        "escalate_conflict" => {
            add_adjustment(adjustments, "conflict_chain_hit_rate", 2.0);
            add_adjustment(adjustments, "cliffhanger_rate", 1.0);
        }
        "reveal_mystery" => {
            add_adjustment(adjustments, "outline_alignment_rate", 1.0);
            add_adjustment(adjustments, "cliffhanger_rate", 1.0);
        }
        "relationship_shift" => {
            add_adjustment(adjustments, "dialogue_naturalness_rate", 2.0);
            add_adjustment(adjustments, "conflict_chain_hit_rate", 1.0);
        }
        "foreshadow_payoff" => add_adjustment(adjustments, "payoff_chain_rate", 2.0),
        _ => {}
    }
}

fn add_adjustment(adjustments: &mut HashMap<&'static str, f64>, key: &'static str, delta: f64) {
    *adjustments.entry(key).or_insert(0.0) += delta;
}

fn lookup_label(labels: &[(&str, &str)], key: &str) -> String {
    labels
        .iter()
        .find_map(|(item_key, label)| (*item_key == key).then_some(*label))
        .unwrap_or(key)
        .to_string()
}

fn build_profile_summary(
    genre_profiles: &[String],
    style_profile: &str,
    quality_preset: &str,
    focus_labels: &[String],
) -> String {
    let mut profile_parts = Vec::new();
    let visible_genres = genre_profiles
        .iter()
        .filter(|item| !item.is_empty() && item.as_str() != DEFAULT_GENRE_PROFILE)
        .take(2)
        .map(|item| lookup_label(&QUALITY_PROFILE_GENRE_LABELS, item))
        .collect::<Vec<_>>();
    if !visible_genres.is_empty() {
        profile_parts.push(format!("题材：{}", visible_genres.join(" / ")));
    }
    if !style_profile.is_empty() && style_profile != DEFAULT_STYLE_PROFILE {
        profile_parts.push(format!(
            "风格：{}",
            lookup_label(&QUALITY_PROFILE_STYLE_LABELS, style_profile)
        ));
    }
    if !quality_preset.is_empty() && quality_preset != "balanced" {
        profile_parts.push(format!(
            "预设：{}",
            lookup_label(&QUALITY_PROFILE_PRESET_LABELS, quality_preset)
        ));
    }

    if !focus_labels.is_empty() {
        let focus_text = focus_labels.join(" / ");
        if profile_parts.is_empty() {
            format!("当前画像更看重 {focus_text}。")
        } else {
            format!("{}，当前更看重 {focus_text}。", profile_parts.join(" / "))
        }
    } else {
        profile_parts.join(" / ")
    }
}

fn build_relaxation_snapshots(
    genre_profiles: &[String],
    style_profile: &str,
) -> Vec<QualityRelaxationSnapshot> {
    let mut snapshots = Vec::new();
    let genre_profile_set = genre_profiles
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    for key in [
        "romance_slice_of_life",
        "suspense_mystery",
        "xianxia_fantasy",
        "science_fiction_tech",
        "history_power",
    ] {
        if genre_profile_set.contains(key) {
            if let Some(snapshot) = relaxation_snapshot("genre", key) {
                snapshots.push(snapshot);
            }
        }
    }
    if let Some(snapshot) = relaxation_snapshot("style", style_profile) {
        snapshots.push(snapshot);
    }
    snapshots
}

fn relaxation_snapshot(scope: &'static str, key: &str) -> Option<QualityRelaxationSnapshot> {
    let (key, label, generation_relaxations, checker_adjustments, reviser_adjustments) = match key {
        "romance_slice_of_life" => (
            "romance_slice_of_life",
            "情感/生活流松绑",
            ROMANCE_GENERATION_RELAXATIONS.as_slice(),
            ROMANCE_CHECKER_ADJUSTMENTS.as_slice(),
            ROMANCE_REVISER_ADJUSTMENTS.as_slice(),
        ),
        "suspense_mystery" => (
            "suspense_mystery",
            "悬疑/惊悚松绑",
            SUSPENSE_GENERATION_RELAXATIONS.as_slice(),
            SUSPENSE_CHECKER_ADJUSTMENTS.as_slice(),
            SUSPENSE_REVISER_ADJUSTMENTS.as_slice(),
        ),
        "xianxia_fantasy" => (
            "xianxia_fantasy",
            "玄幻/仙侠松绑",
            XIANXIA_GENERATION_RELAXATIONS.as_slice(),
            XIANXIA_CHECKER_ADJUSTMENTS.as_slice(),
            XIANXIA_REVISER_ADJUSTMENTS.as_slice(),
        ),
        "science_fiction_tech" => (
            "science_fiction_tech",
            "科幻/技术流松绑",
            TECH_GENERATION_RELAXATIONS.as_slice(),
            TECH_CHECKER_ADJUSTMENTS.as_slice(),
            TECH_REVISER_ADJUSTMENTS.as_slice(),
        ),
        "history_power" => (
            "history_power",
            "历史/权谋松绑",
            HISTORY_GENERATION_RELAXATIONS.as_slice(),
            HISTORY_CHECKER_ADJUSTMENTS.as_slice(),
            HISTORY_REVISER_ADJUSTMENTS.as_slice(),
        ),
        "low_ai_life" => (
            "low_ai_life",
            "低AI生活化松绑",
            LOW_AI_LIFE_GENERATION_RELAXATIONS.as_slice(),
            LOW_AI_LIFE_CHECKER_ADJUSTMENTS.as_slice(),
            LOW_AI_LIFE_REVISER_ADJUSTMENTS.as_slice(),
        ),
        "low_ai_serial" => (
            "low_ai_serial",
            "低AI连载感松绑",
            LOW_AI_SERIAL_GENERATION_RELAXATIONS.as_slice(),
            LOW_AI_SERIAL_CHECKER_ADJUSTMENTS.as_slice(),
            LOW_AI_SERIAL_REVISER_ADJUSTMENTS.as_slice(),
        ),
        "urban_finance" => (
            "urban_finance",
            "都市金融松绑",
            URBAN_FINANCE_RELAXATIONS.as_slice(),
            URBAN_FINANCE_CHECKER.as_slice(),
            URBAN_FINANCE_REVISER.as_slice(),
        ),
        "tech_xianxia" => (
            "tech_xianxia",
            "技术流修仙松绑",
            TECH_XIANXIA_RELAXATIONS.as_slice(),
            TECH_XIANXIA_CHECKER.as_slice(),
            TECH_XIANXIA_REVISER.as_slice(),
        ),
        "light_humor" => (
            "light_humor",
            "轻松幽默松绑",
            LIGHT_HUMOR_RELAXATIONS.as_slice(),
            LIGHT_HUMOR_CHECKER.as_slice(),
            LIGHT_HUMOR_REVISER.as_slice(),
        ),
        "era_plain" => (
            "era_plain",
            "朴实年代松绑",
            ERA_PLAIN_RELAXATIONS.as_slice(),
            ERA_PLAIN_CHECKER.as_slice(),
            ERA_PLAIN_REVISER.as_slice(),
        ),
        _ => return None,
    };

    Some(QualityRelaxationSnapshot {
        scope,
        key,
        label,
        generation_relaxations,
        checker_adjustments,
        reviser_adjustments,
    })
}

fn relaxation_to_value(snapshot: &QualityRelaxationSnapshot) -> Value {
    json!({
        "scope": snapshot.scope,
        "key": snapshot.key,
        "label": snapshot.label,
        "generation_relaxations": snapshot.generation_relaxations,
        "checker_adjustments": snapshot.checker_adjustments,
        "reviser_adjustments": snapshot.reviser_adjustments,
    })
}

fn sanitize_external_assets(
    raw_assets: Option<&Value>,
) -> (Vec<ExternalAssetSummary>, Vec<IgnoredExternalAsset>) {
    let asset_values = normalize_asset_values(raw_assets);
    let mut accepted = Vec::new();
    let mut ignored = Vec::new();
    let mut accepted_count = 0_usize;

    for (index, asset) in asset_values.iter().enumerate() {
        let title = clip_text(
            first_text(asset, &["title", "name", "label"]),
            MAX_EXTERNAL_ASSET_TITLE_LENGTH,
        )
        .unwrap_or_else(|| format!("外部资产{}", index + 1));
        let summary = clip_text(
            first_text(
                asset,
                &[
                    "summary",
                    "content_summary",
                    "excerpt_summary",
                    "abstract",
                    "note_summary",
                ],
            ),
            MAX_EXTERNAL_ASSET_SUMMARY_LENGTH,
        )
        .unwrap_or_default();
        let usage_hint = clip_text(
            first_text(asset, &["usage_hint", "focus", "reason", "hint", "usage"]),
            MAX_EXTERNAL_ASSET_USAGE_HINT_LENGTH,
        )
        .unwrap_or_default();
        let source = clip_text(
            first_text(asset, &["source", "url", "reference", "origin"]),
            MAX_EXTERNAL_ASSET_SOURCE_LENGTH,
        )
        .unwrap_or_default();
        let asset_type = clip_text(first_text(asset, &["asset_type", "type", "category"]), 40)
            .unwrap_or_default();
        let raw_content = value_text(extract_first(
            asset,
            &["raw_content", "content", "text", "body", "excerpt"],
        ));

        if summary.is_empty() {
            ignored.push(IgnoredExternalAsset {
                title,
                reason: if raw_content.is_empty() {
                    EXTERNAL_ASSET_IGNORE_REASON_NO_SUMMARY
                } else {
                    EXTERNAL_ASSET_IGNORE_REASON_RAW_ONLY
                },
            });
            continue;
        }
        if accepted_count >= MAX_EXTERNAL_ASSET_COUNT {
            ignored.push(IgnoredExternalAsset {
                title,
                reason: EXTERNAL_ASSET_IGNORE_REASON_LIMIT,
            });
            continue;
        }
        accepted.push(ExternalAssetSummary {
            title,
            source,
            summary,
            usage_hint,
            asset_type,
        });
        accepted_count += 1;
    }

    let mut deduped = Vec::new();
    let mut seen = HashSet::new();
    for asset in accepted {
        let signature = format!("{}\n{}", asset.title, asset.summary);
        if !seen.insert(signature) {
            ignored.push(IgnoredExternalAsset {
                title: asset.title,
                reason: EXTERNAL_ASSET_IGNORE_REASON_DUPLICATE,
            });
            continue;
        }
        deduped.push(asset);
    }

    (deduped, ignored)
}

fn normalize_asset_values(raw_assets: Option<&Value>) -> Vec<Value> {
    let Some(raw_assets) = raw_assets else {
        return Vec::new();
    };
    match raw_assets {
        Value::Array(items) => items.clone(),
        Value::Object(_) => vec![raw_assets.clone()],
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() || trimmed == "[]" {
                Vec::new()
            } else if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                normalize_asset_values(Some(&parsed))
            } else {
                vec![json!({ "raw_content": trimmed })]
            }
        }
        Value::Null => Vec::new(),
        _ => vec![raw_assets.clone()],
    }
}

fn extract_first<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let object = value.as_object()?;
    keys.iter().find_map(|key| object.get(*key))
}

fn first_text(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| {
            value
                .as_object()
                .and_then(|object| object.get(*key))
                .and_then(|item| {
                    let text = value_text(Some(item));
                    (!text.is_empty()).then_some(text)
                })
        })
        .next()
}

fn value_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn clip_text(value: Option<String>, limit: usize) -> Option<String> {
    let compact = value?
        .replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if compact.is_empty() {
        None
    } else {
        Some(compact.chars().take(limit).collect())
    }
}

fn build_profile_block(key: &str, title: String, lines: Vec<String>) -> Value {
    let cleaned = unique_non_empty(lines);
    let mut rendered_lines = vec![format!("【{title}】")];
    rendered_lines.extend(cleaned.iter().map(|line| format!("- {line}")));
    json!({
        "key": key,
        "title": title,
        "lines": cleaned,
        "text": rendered_lines.join("\n"),
    })
}

fn unique_non_empty(lines: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut cleaned = Vec::new();
    for line in lines {
        let text = line.trim().to_string();
        if text.is_empty() || !seen.insert(text.clone()) {
            continue;
        }
        cleaned.push(text);
    }
    cleaned
}

fn build_generation_lines(
    active_relaxations: &[QualityRelaxationSnapshot],
    external_assets: &[ExternalAssetSummary],
) -> Vec<String> {
    let mut lines = vec![format!(
        "质量画像版本：{QUALITY_PROFILE_VERSION}；默认基线：{QUALITY_BASELINE_ID}。"
    )];
    lines.extend(
        DEFAULT_TOMATO_BASELINE_RULES
            .iter()
            .map(ToString::to_string),
    );
    lines.push("统一命中目标：".to_string());
    lines.extend(
        QUALITY_DIMENSIONS
            .iter()
            .map(|dimension| format!("[{}] {}", dimension.label, dimension.generation_goal)),
    );
    lines.push("当前松绑策略：".to_string());
    if active_relaxations.is_empty() {
        lines.push("未命中特殊题材/风格松绑，使用默认番茄基线。".to_string());
    } else {
        for relaxation in active_relaxations {
            lines.push(format!(
                "[{}:{}] {}",
                relaxation.scope,
                relaxation.label,
                relaxation.generation_relaxations.join("；")
            ));
        }
    }
    if external_assets.is_empty() {
        lines.push("当前无外部摘要资产，按项目内设定与章节上下文执行。".to_string());
    } else {
        lines.push("当前可用外部摘要资产：".to_string());
        lines.extend(external_assets.iter().map(ExternalAssetSummary::to_line));
    }
    lines
}

fn build_checker_lines(active_relaxations: &[QualityRelaxationSnapshot]) -> Vec<String> {
    let mut lines = vec!["质检只做证据驱动判断，不杜撰问题，不输出流程化元文本。".to_string()];
    lines.extend(CHECKER_REVIEW_ORDER.iter().map(ToString::to_string));
    lines.push("严重度定义：".to_string());
    lines.extend(CHECKER_SEVERITY_RULES.iter().map(ToString::to_string));
    lines.push(format!(
        "允许分类：{}。",
        CHECKER_ALLOWED_CATEGORIES.join("、")
    ));
    lines.push(format!(
        "总评枚举：{}。",
        CHECKER_ASSESSMENT_SCALE.join("、")
    ));
    lines.push("统一检查维度：".to_string());
    lines.extend(
        QUALITY_DIMENSIONS
            .iter()
            .map(|dimension| format!("[{}] {}", dimension.label, dimension.checker_focus)),
    );
    lines.push("当前松绑口径：".to_string());
    if active_relaxations.is_empty() {
        lines.push("未命中特殊松绑规则，按默认连载质检口径执行。".to_string());
    } else {
        for relaxation in active_relaxations {
            lines.push(format!(
                "[{}:{}] {}",
                relaxation.scope,
                relaxation.label,
                relaxation.checker_adjustments.join("；")
            ));
        }
    }
    lines
}

fn build_reviser_lines(active_relaxations: &[QualityRelaxationSnapshot]) -> Vec<String> {
    let mut lines =
        vec!["修订输出必须仍是可直接阅读的小说正文或可执行建议，不得夹带说明腔。".to_string()];
    lines.extend(REVISER_CORE_RULES.iter().map(ToString::to_string));
    lines.push(format!(
        "严重度处理顺序：{}。",
        CHECKER_SEVERITY_ORDER.join(" > ")
    ));
    lines.push("统一修补重点：".to_string());
    lines.extend(
        QUALITY_DIMENSIONS
            .iter()
            .map(|dimension| format!("[{}] {}", dimension.label, dimension.reviser_focus)),
    );
    lines.push("当前松绑口径：".to_string());
    if active_relaxations.is_empty() {
        lines.push("未命中特殊松绑规则，按默认最小改动修订策略执行。".to_string());
    } else {
        for relaxation in active_relaxations {
            lines.push(format!(
                "[{}:{}] {}",
                relaxation.scope,
                relaxation.label,
                relaxation.reviser_adjustments.join("；")
            ));
        }
    }
    lines
}

fn build_mcp_guard_lines(
    external_assets: &[ExternalAssetSummary],
    ignored_assets: &[IgnoredExternalAsset],
) -> Vec<String> {
    let mut lines = MCP_GUARD_RULES
        .iter()
        .chain(EXTERNAL_ASSET_RULES.iter())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    lines.push(format!(
        "当前接入结果：accepted={}，ignored={}，summary_only=true。",
        external_assets.len(),
        ignored_assets.len()
    ));
    if !external_assets.is_empty() {
        lines.push("已接入的外部摘要资产：".to_string());
        lines.extend(external_assets.iter().map(ExternalAssetSummary::to_line));
    }
    if !ignored_assets.is_empty() {
        lines.push("已忽略的外部资产：".to_string());
        lines.extend(
            ignored_assets
                .iter()
                .map(|item| format!("{}：{}", item.title, item.reason)),
        );
    }
    lines
}

fn build_external_asset_lines(
    external_assets: &[ExternalAssetSummary],
    ignored_assets: &[IgnoredExternalAsset],
) -> Vec<String> {
    if external_assets.is_empty() {
        let mut lines = vec![
            "未提供合规的外部摘要资产。".to_string(),
            EXTERNAL_ASSET_SUMMARY_ONLY_NOTICE.to_string(),
        ];
        lines.extend(
            ignored_assets
                .iter()
                .map(|item| format!("{}：{}", item.title, item.reason)),
        );
        return lines;
    }

    let mut lines = vec![format!(
        "共接入 {} 条摘要资产；所有资产均按 summary-only 策略注入。",
        external_assets.len()
    )];
    lines.extend(external_assets.iter().map(ExternalAssetSummary::to_line));
    if !ignored_assets.is_empty() {
        lines.push("其余资产已忽略：".to_string());
        lines.extend(
            ignored_assets
                .iter()
                .map(|item| format!("{}：{}", item.title, item.reason)),
        );
    }
    lines
}

fn build_profile_policy() -> Value {
    json!({
        "quality_profile_version": QUALITY_PROFILE_VERSION,
        "baseline_id": QUALITY_BASELINE_ID,
        "block_order": QUALITY_BLOCK_ORDER,
        "block_titles": QUALITY_BLOCK_TITLES
            .into_iter()
            .map(|(key, value)| (key.to_string(), json!(value)))
            .collect::<Map<String, Value>>(),
        "checker_allowed_categories": CHECKER_ALLOWED_CATEGORIES,
        "checker_severity_order": CHECKER_SEVERITY_ORDER,
        "checker_assessment_scale": CHECKER_ASSESSMENT_SCALE,
        "external_assets": {
            "summary_only": true,
            "summary_only_notice": EXTERNAL_ASSET_SUMMARY_ONLY_NOTICE,
            "max_count": MAX_EXTERNAL_ASSET_COUNT,
            "max_summary_length": MAX_EXTERNAL_ASSET_SUMMARY_LENGTH,
            "max_title_length": MAX_EXTERNAL_ASSET_TITLE_LENGTH,
            "max_source_length": MAX_EXTERNAL_ASSET_SOURCE_LENGTH,
            "max_usage_hint_length": MAX_EXTERNAL_ASSET_USAGE_HINT_LENGTH,
            "ignore_reasons": {
                "no_summary": EXTERNAL_ASSET_IGNORE_REASON_NO_SUMMARY,
                "raw_only": EXTERNAL_ASSET_IGNORE_REASON_RAW_ONLY,
                "limit": EXTERNAL_ASSET_IGNORE_REASON_LIMIT,
                "duplicate": EXTERNAL_ASSET_IGNORE_REASON_DUPLICATE,
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_novel_quality_profile, build_novel_quality_prompt_blocks,
        resolve_adaptive_quality_gate_profile, resolve_metric_threshold_adjustments,
        resolve_quality_weight_profile, resolve_runtime_quality_profile,
    };

    #[test]
    fn should_resolve_runtime_profile_from_genre_and_style_inputs() {
        let context = json!({
            "genre": "科幻 技术流",
            "style_name": "技术流修仙",
            "quality_preset": "plot_drive"
        });
        let profile = resolve_runtime_quality_profile(context.as_object());

        assert_eq!(profile["style_profile"], "tech_xianxia");
        assert_eq!(profile["genre_profiles"], json!(["science_fiction_tech"]));
        assert_eq!(profile["quality_preset"], "plot_drive");
    }

    #[test]
    fn should_resolve_weight_profile_with_labels_and_summary() {
        let context = json!({
            "genre_profiles": ["xianxia_fantasy", "science_fiction_tech"],
            "style_profile": "tech_xianxia",
            "quality_preset": "immersive"
        });
        let profile = resolve_quality_weight_profile(context.as_object(), Some("ending"));

        assert!(profile["focus_areas"]
            .as_array()
            .expect("focus areas")
            .iter()
            .any(|item| item == "rule_grounding"));
        assert!(profile["summary"]
            .as_str()
            .expect("summary")
            .contains("科技仙侠"));
    }

    #[test]
    fn should_resolve_adaptive_profile_and_threshold_adjustments() {
        let context = json!({
            "genre_profiles": ["romance_slice_of_life"],
            "style_profile": "low_ai_life",
            "quality_preset": "emotion_drama",
            "creative_mode": "relationship",
            "story_focus": "relationship_shift"
        });
        let adaptive =
            resolve_adaptive_quality_gate_profile(context.as_object(), Some("development"));
        let adjustments =
            resolve_metric_threshold_adjustments(context.as_object(), Some("development"));

        assert_eq!(adaptive["resolved_stage"], "development");
        assert_eq!(adaptive["style_profile"], "low_ai_life");
        assert!(adaptive["weight_profile"]["focus_labels"]
            .as_array()
            .expect("focus labels")
            .iter()
            .any(|item| item == "对白质感"));
        assert_eq!(
            adjustments
                .get("dialogue_naturalness_rate")
                .copied()
                .unwrap_or_default(),
            12.0
        );
        assert_eq!(
            adjustments
                .get("conflict_chain_hit_rate")
                .copied()
                .unwrap_or_default(),
            8.0
        );
    }

    #[test]
    fn should_build_quality_profile_prompt_blocks_with_sanitized_assets() {
        let context = json!({
            "genre": "言情 生活流",
            "style_name": "低AI生活化",
            "external_assets": [
                {
                    "title": "江南夜航资料",
                    "source": "web",
                    "summary": "晚清漕运夜航避税路线",
                    "usage_hint": "只取水路质感",
                    "asset_type": "research"
                },
                {
                    "title": "江南夜航资料",
                    "summary": "晚清漕运夜航避税路线"
                },
                {
                    "title": "原文资料",
                    "raw_content": "一整页原文"
                }
            ]
        });
        let profile = build_novel_quality_profile(context.as_object());
        let prompt_blocks = build_novel_quality_prompt_blocks(context.as_object());

        assert_eq!(profile["style_profile"], "low_ai_life");
        assert_eq!(profile["genre_profiles"], json!(["romance_slice_of_life"]));
        assert_eq!(
            profile["external_assets"].as_array().expect("assets").len(),
            1
        );
        assert_eq!(
            profile["ignored_external_assets"]
                .as_array()
                .expect("ignored")
                .len(),
            2
        );
        assert!(prompt_blocks["generation"]
            .as_str()
            .expect("generation block")
            .contains("质量画像版本"));
        assert!(prompt_blocks["external_assets"]
            .as_str()
            .expect("external block")
            .contains("晚清漕运夜航避税路线"));
        assert!(prompt_blocks["mcp_guard"]
            .as_str()
            .expect("mcp guard")
            .contains("summary_only=true"));
    }

    #[test]
    fn should_parse_json_string_external_assets_for_prompt_blocks() {
        let context = json!({
            "external_assets": "[{\"title\":\"税卡\",\"summary\":\"夜航税卡协商\",\"usage_hint\":\"只取制度压力\"}]"
        });
        let prompt_blocks = build_novel_quality_prompt_blocks(context.as_object());

        assert!(prompt_blocks["external_assets"]
            .as_str()
            .expect("external block")
            .contains("夜航税卡协商"));
        assert!(prompt_blocks["external_assets"]
            .as_str()
            .expect("external block")
            .contains("summary-only"));
    }
}
