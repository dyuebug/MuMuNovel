"""小说质量规则底座。"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Optional, Tuple

QUALITY_PROFILE_VERSION = "novel_quality_profile_v1_20260307"
QUALITY_BASELINE_ID = "fanqie_serial_baseline_v1"
DEFAULT_STYLE_PROFILE = "default"
DEFAULT_GENRE_PROFILE = "generic"
QUALITY_BLOCK_ORDER: Tuple[str, ...] = (
    "generation",
    "checker",
    "reviser",
    "mcp_guard",
    "external_assets",
)
QUALITY_BLOCK_TITLES = {
    "generation": "章节生成质量基线",
    "checker": "章节质检口径",
    "reviser": "章节修订口径",
    "mcp_guard": "MCP与外部参考护栏",
    "external_assets": "外部资产摘要",
}
MAX_EXTERNAL_ASSET_COUNT = 6
MAX_EXTERNAL_ASSET_SUMMARY_LENGTH = 240
MAX_EXTERNAL_ASSET_TITLE_LENGTH = 60
MAX_EXTERNAL_ASSET_SOURCE_LENGTH = 120
MAX_EXTERNAL_ASSET_USAGE_HINT_LENGTH = 80
EXTERNAL_ASSET_SUMMARY_ONLY_NOTICE = "只接受摘要，不接受 raw_content、全文、网页原文或大段摘录进入默认规则块。"
EXTERNAL_ASSET_IGNORE_REASON_NO_SUMMARY = "缺少摘要，默认规则块不接收原文直入"
EXTERNAL_ASSET_IGNORE_REASON_RAW_ONLY = "仅提供原始内容，未提供摘要，已按 summary-only 策略忽略"
EXTERNAL_ASSET_IGNORE_REASON_LIMIT = "超过摘要资产数量上限，已忽略"
EXTERNAL_ASSET_IGNORE_REASON_DUPLICATE = "重复摘要资产已折叠"


@dataclass(frozen=True)
class QualityDimension:
    key: str
    label: str
    generation_goal: str
    checker_focus: str
    reviser_focus: str


@dataclass(frozen=True)
class QualityRelaxationRule:
    key: str
    label: str
    triggers: Tuple[str, ...]
    generation_relaxations: Tuple[str, ...]
    checker_adjustments: Tuple[str, ...]
    reviser_adjustments: Tuple[str, ...]


QUALITY_DIMENSIONS: Tuple[QualityDimension, ...] = (
    QualityDimension(
        key="conflict_chain",
        label="冲突链",
        generation_goal="单章至少让“目标→阻力→选择→即时后果”可见，避免只有概述没有代价。",
        checker_focus="重点检查冲突是否真的逼出选择，以及选择之后是否带来损失、新麻烦或关系变化。",
        reviser_focus="优先补齐选择与代价链，避免把关键桥段改成空泛总结。",
    ),
    QualityDimension(
        key="rule_grounding",
        label="规则落地",
        generation_goal="世界规则、行业规则或力量规则要落到角色行动与后果，不只停留在设定说明。",
        checker_focus="检查设定术语是否只讲不演，是否缺少触发条件、限制、反噬或现实代价。",
        reviser_focus="把规则说明压回场景，让角色通过动作、反馈和后果把规则演出来。",
    ),
    QualityDimension(
        key="outline_alignment",
        label="大纲对齐",
        generation_goal="本章必须覆盖当前大纲锚点，但允许换顺序、换切口，不机械逐条照抄。",
        checker_focus="检查正文是否跑题、漏掉主锚点，或把关键事件写成无效铺垫。",
        reviser_focus="优先修复漏写的大纲锚点与剧情承接断层，保持主线稳定。",
    ),
    QualityDimension(
        key="dialogue_naturalness",
        label="对白自然度",
        generation_goal="对白像真人交流，要有停顿、反问、改口、潜台词和角色声线差异。",
        checker_focus="检查角色是否同口吻讲道理，或对白过长、过整齐、过说明书化。",
        reviser_focus="优先压短生硬对白，补动作、语气和信息落差，保留角色各自的声音。",
    ),
    QualityDimension(
        key="opening_hook",
        label="开场钩子",
        generation_goal="开场尽快给异常、任务压力、关系摩擦或信息缺口，让读者知道“这一章为什么要看”。",
        checker_focus="检查前段是否只是背景铺陈，缺少当前进行中的麻烦、风险或悬念。",
        reviser_focus="优先把静态介绍改成正在发生的动作、压力或冲突切入。",
    ),
    QualityDimension(
        key="payoff_chain",
        label="小爽点链条",
        generation_goal="尽量形成“铺垫→爆发→反馈”的小满足，不要求每章都打脸，但要给追更回报。",
        checker_focus="检查是否只铺不收、只喊结果不写反馈，或爽点与人物选择脱节。",
        reviser_focus="优先补反馈与余波，让关键得失落到人物与场面。",
    ),
    QualityDimension(
        key="cliffhanger",
        label="章尾牵引",
        generation_goal="章尾优先停在信息缺口、危险临门、身份反转或选择未决，不要用总结腔收束。",
        checker_focus="检查结尾是否提前把情绪说尽、把问题讲完，导致追更牵引不足。",
        reviser_focus="优先把总结句改成动作、对话或未落地的后果，让章尾留白但不空洞。",
    ),
)


DEFAULT_TOMATO_BASELINE_RULES: Tuple[str, ...] = (
    "默认采用番茄连载基线：开场尽快入事，中段持续推进，末尾保留自然未完感。",
    "正文优先写正在发生的动作、人物反应和局面变化，再补必要解释。",
    "关键桥段尽量落成“动作→反馈→余波/代价”，避免大段概述替代现场。",
    "单章允许只有一个主冲突，但必须让角色做出选择，并看到即时后果。",
    "设定术语、行业术语和力量规则出现时，要在三句内补一句读者能听懂的人话解释。",
    "对白要分角色声线，不把所有人写成同一张嘴，也不把情绪一步写到终点。",
    "禁止流程化元文本、模型自述、总结腔、预告腔和模板化口号。",
)


GENRE_RELAXATION_RULES: Tuple[QualityRelaxationRule, ...] = (
    QualityRelaxationRule(
        key="romance_slice_of_life",
        label="情感/生活流松绑",
        triggers=(
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
        ),
        generation_relaxations=(
            "允许用关系压力、生活摩擦、尴尬局面或情绪错位替代高烈度外部危机。",
            "开场钩子可以是秘密、误会、反常态度或当场难堪，不强求立刻上大事件。",
            "小爽点可以是关系推进、认知翻转、情绪回弹或立场改变，不只看打脸与胜负。",
        ),
        checker_adjustments=(
            "不要仅因缺少战斗或灾难就判定节奏弱，重点看关系是否推进、情绪是否有层次。",
            "对白自然度和潜台词权重上调，但仍要避免角色轮流端着讲道理。",
        ),
        reviser_adjustments=(
            "修订时优先保留细腻情绪与日常颗粒感，不强塞额外大危机。",
            "若章尾较柔和，至少补一个未说破的关系张力或下一步选择。",
        ),
    ),
    QualityRelaxationRule(
        key="suspense_mystery",
        label="悬疑/惊悚松绑",
        triggers=("悬疑", "推理", "惊悚", "刑侦", "无限流", "规则怪谈", "恐怖", "志怪"),
        generation_relaxations=(
            "允许暂时不解释真相，但必须持续提供线索、反常细节或压力升级。",
            "章尾可以优先强化信息缺口与危险临门，阶段性小爽点不是硬性要求。",
        ),
        checker_adjustments=(
            "不要因为故意留白就判逻辑断裂，先看线索是否公平、悬念是否持续有效。",
            "对设定说明的要求可略放宽，但必须能从后果反推到规则存在。",
        ),
        reviser_adjustments=(
            "修订时优先补足线索可读性与压力升级，不提前泄底。",
            "若信息压得过深，补一处可验证细节而不是整段解释。",
        ),
    ),
    QualityRelaxationRule(
        key="xianxia_fantasy",
        label="玄幻/仙侠松绑",
        triggers=("玄幻", "仙侠", "修真", "修仙", "奇幻", "魔法", "西幻"),
        generation_relaxations=(
            "允许术法、境界、门派或种族术语密度略高，但必须贴着动作反馈出现。",
            "小爽点可以是悟道、破局、压制、收获资源或境界突破，不局限于正面打脸。",
        ),
        checker_adjustments=(
            "不要把所有术语都当成阅读障碍，先看是否给出场景内的人话解释与代价。",
            "重点检查规则边界、资源代价与强度升级是否自洽。",
        ),
        reviser_adjustments=(
            "修订时优先补规则触发条件和代价，不削弱题材风味。",
            "如说明过密，用角色误解、追问或身体反馈压缩解释。",
        ),
    ),
    QualityRelaxationRule(
        key="science_fiction_tech",
        label="科幻/技术流松绑",
        triggers=("科幻", "赛博", "机甲", "硬科幻", "软科幻", "技术流"),
        generation_relaxations=(
            "允许出现机制推演、任务流程或设备细节，但每段都要挂在行动结果上。",
            "开场钩子可以是异常数据、系统故障、任务时限或技术失控。",
        ),
        checker_adjustments=(
            "不要单凭术语略多就判AI味重，重点看术语是否推动决策与后果。",
            "对白可保留少量专业表达，但不能整段写成会议纪要。",
        ),
        reviser_adjustments=(
            "修订时优先把讲义感压缩到动作反馈里，保留必要的专业可信度。",
            "若信息量过载，先删重复解释，再补一个可感知的现实后果。",
        ),
    ),
    QualityRelaxationRule(
        key="history_power",
        label="历史/权谋松绑",
        triggers=("历史", "架空历史", "古代", "古言", "权谋", "朝堂", "宫斗", "官场", "战争"),
        generation_relaxations=(
            "允许开场先落礼制、身份压力、局势变化或筹码差，而不是立刻高噪声冲突。",
            "小爽点可以是试探得手、地位变化、筹码反转或一句话压人，不强求动作爆点。",
        ),
        checker_adjustments=(
            "重点检查动机、身份秩序和信息差，不因表达克制就误判节奏不足。",
            "对白可更克制含蓄，但必须听得出身份层级和潜台词。",
        ),
        reviser_adjustments=(
            "修订时优先稳住礼制语境与权力关系，不硬塞现代口语式高爆冲突。",
            "若章尾较稳，补一个未明说的筹码变化或风险外溢。",
        ),
    ),
)


STYLE_RELAXATION_RULES: Tuple[QualityRelaxationRule, ...] = (
    QualityRelaxationRule(
        key="low_ai_life",
        label="低AI生活化松绑",
        triggers=("low_ai_life", "低ai生活化", "生活化", "口语", "日常感"),
        generation_relaxations=(
            "允许开场更贴近日常现场，只要前段能看见眼前麻烦、情绪摩擦或局面变化。",
            "对白允许打断、改口、留白和少量口语毛边，不要求句句工整。",
            "章尾可以更柔和，但至少留下情绪余震、关系余波或下一步动作牵引。",
        ),
        checker_adjustments=(
            "不要把口语毛边误判成低质文风，重点看声线区分和信息效率。",
            "爽点权重略降，日常真实感与情绪层次权重上调。",
        ),
        reviser_adjustments=(
            "修订时优先保留生活噪声、动作细节和人物嘴感，不把文本修成说明文。",
            "若对白过顺，补停顿、接话失败或动作遮挡，而不是加鸡汤解释。",
        ),
    ),
    QualityRelaxationRule(
        key="low_ai_serial",
        label="低AI连载感松绑",
        triggers=("low_ai_serial", "低ai连载感", "连载感", "追更", "番茄"),
        generation_relaxations=(
            "允许句子更短、更口语、更带现场感，不以书面工整度换取连载节奏。",
            "配角只要做出会改变局面的主动选择，即可视为有效推进，不强求每个人都长篇输出。",
        ),
        checker_adjustments=(
            "不要因为语言偏直接、颗粒偏粗就误判为文风差，先看追更牵引是否成立。",
            "开场钩子、冲突链和章尾牵引的权重上调，但仍需允许局部呼吸段。",
        ),
        reviser_adjustments=(
            "修订时优先保住现场感、推进力和角色情绪反差，不把连载文修成端正但没劲的稿子。",
            "如章尾过满，宁可留动作停顿，也不要补总结句。",
        ),
    ),
    QualityRelaxationRule(
        key="urban_finance",
        label="都市金融松绑",
        triggers=("urban_finance", "都市金融", "金融", "商战"),
        generation_relaxations=(
            "允许出现专业术语，但必须同步写出利益得失、信息差或筹码变化。",
        ),
        checker_adjustments=(
            "重点检查术语是否真正推动博弈，不因题材专业性本身判定晦涩。",
        ),
        reviser_adjustments=(
            "修订时优先把抽象术语落回利益链和人物选择。",
        ),
    ),
    QualityRelaxationRule(
        key="tech_xianxia",
        label="技术流修仙松绑",
        triggers=("tech_xianxia", "技术流修仙", "技术流", "修仙"),
        generation_relaxations=(
            "允许规则推演略长，但必须持续附着在试错、消耗或破局动作上。",
        ),
        checker_adjustments=(
            "重点看推演是否能反推出行动方案，不把所有推导都当作赘述。",
        ),
        reviser_adjustments=(
            "修订时优先删重复推导，保留真正改变局面的关键步骤。",
        ),
    ),
    QualityRelaxationRule(
        key="light_humor",
        label="轻松幽默松绑",
        triggers=("light_humor", "轻松幽默", "幽默", "搞笑"),
        generation_relaxations=(
            "允许多一点插科打诨，但每轮玩笑都应推动关系、冲突或信息揭示。",
        ),
        checker_adjustments=(
            "不要因角色互怼就误判不严肃，重点看笑点是否服务剧情而非打断剧情。",
        ),
        reviser_adjustments=(
            "修订时保留人物互怼节奏，优先删除与局面无关的重复包袱。",
        ),
    ),
    QualityRelaxationRule(
        key="era_plain",
        label="朴实年代松绑",
        triggers=("era_plain", "朴实年代", "年代风", "年代文"),
        generation_relaxations=(
            "允许表达更克制、事件更生活化，但要保证人物压力和现实阻力具体可见。",
        ),
        checker_adjustments=(
            "不要用高爆点模板要求年代文，重点看生活细节、人情压力和选择后果。",
        ),
        reviser_adjustments=(
            "修订时优先稳住朴素语气和时代质感，避免强塞网络热梗与悬浮金句。",
        ),
    ),
)


CHECKER_ALLOWED_CATEGORIES: Tuple[str, ...] = (
    "设定冲突",
    "逻辑连贯",
    "角色失真",
    "文风表达",
    "对话质量",
    "结尾处理",
    "术语可读性",
    "开场抓力",
    "爽点链条",
    "章尾牵引",
)

CHECKER_SEVERITY_ORDER: Tuple[str, ...] = ("critical", "major", "minor")
CHECKER_ASSESSMENT_SCALE: Tuple[str, ...] = ("优秀", "良好", "一般", "较差", "存在严重问题")
CHECKER_REVIEW_ORDER: Tuple[str, ...] = (
    "先查设定冲突、逻辑断裂和角色失真，再看文风表达与对白自然度。",
    "有明确证据再判错；证据不足时保守处理，不杜撰问题。",
    "优先标记会直接破坏阅读连续性的关键问题，避免用大量轻微问题掩盖真正主伤口。",
)
CHECKER_SEVERITY_RULES: Tuple[str, ...] = (
    "critical：会直接破坏设定自洽、剧情因果、角色核心行为边界或正文可读性的问题。",
    "major：明显削弱追更体验、节奏、情绪层次或信息传达，但不至于读不下去的问题。",
    "minor：局部表达、生硬句、轻微重复或可优化但不影响主链理解的问题。",
)

REVISER_CORE_RULES: Tuple[str, ...] = (
    "先修 critical，再修最影响阅读流的 major；minor 只在不破坏节奏时顺手处理。",
    "最小改动优先：能改一句不改一段，能改一段不重写整章主线。",
    "保持原人称、角色关系、剧情方向和题材声线，不为修文新造重大剧情。",
    "若问题证据不足或缺少上游信息，明确标为 unresolved，不强行改写。",
    "修订结果必须仍是可直接阅读的小说正文或可执行建议，不能变成流程说明。",
)

MCP_GUARD_RULES: Tuple[str, ...] = (
    "外部资料只能作为参考，不得覆盖项目既有设定、本章大纲和角色边界。",
    "先抽取摘要，再注入 prompt；禁止把网页原文、长段摘录或大块资料直接塞进规则块。",
    "引用外部知识时，优先保留与当前剧情最相关的事实、意象或行业细节，避免整页搬运。",
    "若外部资料与项目内设定冲突，一律以内生设定为准，并把外部信息降级为可选灵感。",
    "禁止照抄资料原句，必须转成服务剧情的简短摘要或执行提醒。",
)

EXTERNAL_ASSET_RULES: Tuple[str, ...] = (
    "外部资产默认只接收 summary/摘要，不接收 raw_content、全文、网页正文或长篇摘录。",
    "单条摘要应控制在短摘要范围内，只保留当前任务直接需要的事实、风味或禁忌提醒。",
    "没有摘要的资料不进入默认规则块；若仅提供原文，视为未提供合规资产。",
    "同类资料优先去重合并，最多保留有限条目，避免资料噪音反客为主。",
)


def _normalize_text(value: Optional[str]) -> str:
    return (value or "").strip().lower()


def detect_style_profile(
    style_name: Optional[str],
    style_preset_id: Optional[str],
    style_content: Optional[str] = None,
) -> str:
    """识别写作风格画像。"""
    preset = _normalize_text(style_preset_id)
    name = _normalize_text(style_name)
    content = _normalize_text(style_content)
    merged = " ".join(part for part in (preset, name, content) if part)

    for rule in STYLE_RELAXATION_RULES:
        if preset and preset == rule.key:
            return rule.key
        if any(trigger in merged for trigger in rule.triggers):
            return rule.key
    return DEFAULT_STYLE_PROFILE


def detect_genre_profiles(genre: Optional[str]) -> Tuple[str, ...]:
    """识别题材画像，可命中多个标签。"""
    normalized = _normalize_text(genre)
    if not normalized:
        return (DEFAULT_GENRE_PROFILE,)

    matched = []
    for rule in GENRE_RELAXATION_RULES:
        if any(trigger in normalized for trigger in rule.triggers):
            matched.append(rule.key)

    if not matched:
        return (DEFAULT_GENRE_PROFILE,)

    seen = set()
    ordered = []
    for key in matched:
        if key in seen:
            continue
        seen.add(key)
        ordered.append(key)
    return tuple(ordered)


def get_style_relaxation(style_profile: str) -> Optional[QualityRelaxationRule]:
    """按风格画像返回松绑规则。"""
    normalized = _normalize_text(style_profile)
    for rule in STYLE_RELAXATION_RULES:
        if rule.key == normalized:
            return rule
    return None


def get_genre_relaxations(genre_profiles: Tuple[str, ...]) -> Tuple[QualityRelaxationRule, ...]:
    """按题材画像返回松绑规则。"""
    profile_keys = {_normalize_text(item) for item in genre_profiles}
    if not profile_keys:
        return ()

    matched = []
    for rule in GENRE_RELAXATION_RULES:
        if rule.key in profile_keys:
            matched.append(rule)
    return tuple(matched)


def get_active_relaxations(
    genre: Optional[str],
    style_name: Optional[str],
    style_preset_id: Optional[str],
    style_content: Optional[str] = None,
) -> Tuple[str, Tuple[QualityRelaxationRule, ...], Optional[QualityRelaxationRule]]:
    """统一解析当前激活的题材/风格松绑规则。"""
    style_profile = detect_style_profile(
        style_name=style_name,
        style_preset_id=style_preset_id,
        style_content=style_content,
    )
    genre_profiles = detect_genre_profiles(genre)
    return style_profile, get_genre_relaxations(genre_profiles), get_style_relaxation(style_profile)
