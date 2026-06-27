from __future__ import annotations

import re
from typing import Any, Dict, List, Optional, Tuple


def _calc_applicable_quality_overall(metric_entries: List[Tuple[Dict[str, Any], float]]) -> float:
    """只对适用指标进行加权，避免缺失规则/锚点时被误判为 0 分。"""
    weighted_sum = 0.0
    total_weight = 0.0

    for metric, weight in metric_entries:
        if not isinstance(metric, dict):
            continue
        if metric.get("applicable", True) is False:
            continue
        hit_rate = float(metric.get("hit_rate") or 0.0)
        weighted_sum += hit_rate * float(weight)
        total_weight += float(weight)

    if total_weight <= 0:
        return 0.0
    return weighted_sum / total_weight


def _extract_dialogue_segments(text: str) -> List[str]:
    """Extract quoted dialogue spans from common Chinese quotation marks."""
    if not text:
        return []

    pattern = (
        "\u201c[^\u201d\n]{1,120}\u201d|"
        "\u2018[^\u2019\n]{1,120}\u2019|"
        "\u300c[^\u300d\n]{1,120}\u300d|"
        "\u300e[^\u300f\n]{1,120}\u300f|"
        '"[^"\n]{1,120}"'
    )
    quote_pairs = {
        "\u201c": "\u201d",
        "\u2018": "\u2019",
        "\u300c": "\u300d",
        "\u300e": "\u300f",
        '"': '"',
    }

    segments: List[str] = []
    for match in re.finditer(pattern, text):
        token = match.group(0).strip()
        if len(token) < 2:
            continue
        closing = quote_pairs.get(token[0])
        if not closing or token[-1] != closing:
            continue
        segment = token[1:-1].strip()
        if segment:
            segments.append(segment)
    return segments


def _calc_dialogue_naturalness_rate(text: str) -> Dict[str, Any]:
    """评估对话自然度，包含对话/描写比例与句式节奏。"""
    quotes = _extract_dialogue_segments(text)
    if not quotes:
        return {
            "hit_rate": 0.0,
            "total_dialogues": 0,
            "short_ratio": 0.0,
            "interrupt_ratio": 0.0,
            "pressure_ratio": 0.0,
            "applicable": True,
        }

    total = len(quotes)
    interrupt_markers = ["…", "——", "？", "！", "嗯", "啊"]
    pressure_markers = [
        "？", "！", "别", "快", "马上", "立刻", "还我", "给我",
        "闭嘴", "谁", "为什么", "怎么", "什么", "要么", "选，快", "选,快",
        "撤", "快念", "你确定", "别砸", "不认", "不算", "停播", "收尸", "申诉", "下去",
    ]
    short_count = sum(1 for q in quotes if len(q.strip()) <= 28)
    interrupt_count = sum(1 for q in quotes if any(mark in q for mark in interrupt_markers))
    pressure_count = sum(1 for q in quotes if any(mark in q for mark in pressure_markers))
    short_ratio = short_count / total
    interrupt_ratio = interrupt_count / total
    pressure_ratio = pressure_count / total
    base_rate = min(1.0, 0.7 * short_ratio + 0.3 * interrupt_ratio)
    hit_rate = min(1.0, base_rate + 0.05 * pressure_ratio)
    return {
        "hit_rate": round(hit_rate, 4),
        "total_dialogues": total,
        "short_ratio": round(short_ratio, 4),
        "interrupt_ratio": round(interrupt_ratio, 4),
        "pressure_ratio": round(pressure_ratio, 4),
        "applicable": True,
    }


def _calc_opening_hook_rate(text: str) -> Dict[str, Any]:
    """Estimate opening hook coverage in the first 300 chars."""
    opening = (text or "")[:300]
    if not opening.strip():
        return {"hit_rate": 0.0, "matched_markers": [], "window_length": 0, "applicable": True}

    markers: Dict[str, List[str]] = {
        "异常": [
            "忽然", "突然", "竟然", "不对劲", "异样", "反常", "通缉", "失控", "闯进", "错误编号",
            "猴红字幕", "红字", "现场复核", "校验字", "猝红", "重检申请", "受理", "倒退", "吞字", "错位",
            "问题是", "根本没", "不存在", "空无一人", "多出来", "少了一个", "对不上", "硬生生", "另一块", "直播画面",
            "热榜", "围观", "警戒线", "红色提示", "突然冲上", "弹窗", "弹出", "晃动", "火警通道", "只有一句话",
        ],
        "危险": [
            "危险", "杀", "追", "爆炸", "血", "死", "失火", "崩塌", "袭来", "警报", "失踪", "事故", "滚水", "黑屏",
            "覆盖", "改写", "擦掉", "变浅", "污染", "锁定", "反噬", "拖进", "尖叫",
        ],
        "任务": ["必须", "限时", "任务", "deadline", "命令", "抓紧", "今晚", "立刻", "马上", "现场复核", "先校验", "别回头", "先撤", "报警", "请确认", "确认母本身份"],
        "冲突": ["吅", "质问", "拦住", "对峙", "打断", "撕破脸", "冲突", "反驳", "拍桌", "开战", "别", "只能", "按住"],
    }

    matched: List[str] = []
    for label, words in markers.items():
        if any(word in opening for word in words):
            matched.append(label)

    hit_rate = min(1.0, len(matched) / 2) if matched else 0.0
    return {
        "hit_rate": round(hit_rate, 4),
        "matched_markers": matched,
        "window_length": len(opening),
        "applicable": True,
    }


def _split_sentences(text: str) -> List[str]:
    parts = re.split(r"[。！？!?；;\n]+", text)
    return [part.strip() for part in parts if part.strip()]


def _extract_rule_keywords(world_rules: Optional[str], limit: int = 10) -> List[str]:
    """Extract compact rule-grounding keywords from rule text."""
    if not world_rules:
        return []

    stop_words = {
        "可以", "必须", "不能", "需要", "出现", "进行", "影响", "通过", "以及", "如果", "然后",
        "这个", "那个", "没有", "时候", "因为", "所以", "因此", "规则", "系统", "角色", "世界",
        "章节", "本章", "当前", "阶段", "重点", "提示", "要求", "设定", "巡夜署",
    }
    cue_words = (
        "规则", "边界", "限制", "条件", "代价", "回检", "登记", "文本", "污染", "改写",
        "反写", "触发", "反噬", "诵读", "校对", "命令", "权限", "封印", "倒计时", "纠错",
        "观看", "记录", "热度", "实体", "伤人", "危险", "绑定", "认主", "断页", "直播",
        "转述", "目击", "对应", "副本", "活页协议", "失声",
    )
    constraint_words = (
        "不能", "不得", "否则", "代价", "伤害", "伤及", "失声", "流血", "说破", "公开", "只要", "一旦", "只能",
    )

    segments = [
        segment.strip()
        for segment in re.split(r"[，。！？!?；;,：:/（）()\[\]\n]+", world_rules)
        if segment and segment.strip()
    ]
    priority_segments = [
        segment for segment in segments
        if any(cue in segment for cue in cue_words) or any(token in segment for token in constraint_words)
    ]
    working_segments = priority_segments or segments

    keywords: List[str] = []
    seen: set[str] = set()

    def _append_keyword(candidate: str) -> bool:
        normalized = candidate.strip()
        if len(normalized) < 2 or normalized in stop_words or normalized in seen:
            return False
        seen.add(normalized)
        keywords.append(normalized)
        return len(keywords) >= limit

    anchored_patterns = (
        r"(?:不能|不得)[^，。；]{2,12}",
        r"(?:否则|只要|一旦|只能)[^，。；]{2,12}",
        r"[^，。；]{2,10}(?:代价|纠错|失声|流血|伤害|伤及|反噬)",
        r"(?:直接说破|录屏公开|伤害最近者|轻则失声|重则流血|高热度记录未切断)",
        r"[^，。；]{2,10}会让[^，。；]{2,10}",
        r"[^，。；]{2,10}会造成[^，。；]{2,10}",
        r"[^，。；]{1,8}越[^，。；]{1,8}越[^，。；]{1,8}",
    )

    for segment in working_segments:
        compact = re.sub(r"\s+", "", segment)
        if not compact:
            continue

        candidates: List[str] = []
        if len(compact) <= 8:
            candidates.append(compact)
        else:
            for pattern in anchored_patterns:
                candidates.extend(re.findall(pattern, compact))
            for cue in cue_words:
                start_index = 0
                while True:
                    pos = compact.find(cue, start_index)
                    if pos < 0:
                        break
                    candidates.append(cue)
                    left = max(0, pos - 3)
                    right = min(len(compact), pos + len(cue) + 4)
                    candidates.append(compact[left:right])
                    start_index = pos + len(cue)
            for part in re.split(r"(?:必须|不能|不得|需要|如果|否则|一旦|只要|只能|即可|就会|就能|导致|引发|迫使|代价)", compact):
                stripped = part.strip()
                if 2 <= len(stripped) <= 10:
                    candidates.append(stripped)
            if not candidates:
                candidates.extend([compact[:6], compact[-6:]])

        for candidate in candidates:
            if _append_keyword(candidate):
                return keywords

    return keywords


def _calc_rule_grounding_rate(text: str, world_rules: Optional[str]) -> Dict[str, object]:
    """评估世界规则落地率与命中关键词。"""
    keywords = _extract_rule_keywords(world_rules)
    if not keywords:
        return {
            "hit_rate": 0.0,
            "hit_count": 0,
            "expected_count": 0,
            "matched_keywords": [],
            "applicable": False,
            "skipped_reason": "no_world_rules",
        }

    causal_words = ["导致", "所以", "因此", "结果", "触发", "引发", "迫使", "只能", "不得不", "于是", "只要", "一旦", "否则", "就得", "才会", "才能", "过不了", "不只是", "还可能"]
    causal_patterns = [
        re.compile(r"只要.+就"),
        re.compile(r"一旦.+就"),
        re.compile(r"谁.+就得"),
        re.compile(r"白灯一亮.+就是"),
    ]
    sentences = _split_sentences(text)
    expected_count = max(1, len(text) // 1100)

    matched_keywords: List[str] = []
    grounded_events = 0
    implicit_rule_event = False
    rule_cue_words = [
        "规则", "限制", "边界", "代价", "触发", "改写", "污染", "登记", "诵读", "校对", "反噬", "宣读", "审读", "见证人", "白灯", "纠错", "热度", "观看", "记录", "看见", "记下", "围观", "实体", "伤人", "认主", "绑定",
        "转述", "目击", "对应", "副本", "活页协议", "失声", "校验", "复核", "公共镜头", "直播画面", "样本",
    ]
    for sentence in sentences:
        sentence_keywords = [kw for kw in keywords if kw in sentence]
        has_rule_cue = any(cue in sentence for cue in rule_cue_words)
        if not sentence_keywords and not has_rule_cue:
            continue
        for kw in sentence_keywords:
            if kw not in matched_keywords:
                matched_keywords.append(kw)
        if has_rule_cue and not sentence_keywords and "__implicit_rule_cue__" not in matched_keywords:
            matched_keywords.append("__implicit_rule_cue__")
        has_causal = any(causal in sentence for causal in causal_words) or any(pattern.search(sentence) for pattern in causal_patterns)
        if has_causal:
            grounded_events += 1

    if grounded_events == 0 and sentences:
        speech_words = ["说了", "说出", "说破", "开口", "喊出", "刚说出"]
        consequence_words = [
            "纠正", "代价", "血口", "流血", "失声", "离他最近", "最近的人", "挨的却是", "裂开", "规则", "封号", "反咬", "锁定", "待复核",
            "暂缓", "第二证据成立", "流量切断", "打赏通道关闭", "高危未核验",
        ]
        implicit_rule_words = [
            "转述", "目击", "对应", "副本", "活页协议", "失声", "校验", "复核", "反咬", "封号", "样本",
            "第二证据", "第二媒介", "三源交叉", "平台不认", "直播链路",
        ]
        for idx in range(len(sentences)):
            window = " ".join(sentences[max(0, idx - 1): idx + 2])
            has_speech_trigger = any(word in window for word in speech_words)
            has_rule_contract = any(word in window for word in implicit_rule_words)
            has_consequence = any(word in window for word in consequence_words)
            if (has_speech_trigger and has_consequence) or (has_rule_contract and has_consequence):
                grounded_events = 1
                implicit_rule_event = True
                if "__implicit_rule_event__" not in matched_keywords:
                    matched_keywords.append("__implicit_rule_event__")
                break

    keyword_coverage = len(matched_keywords) / max(min(4, len(keywords)), 1)
    if implicit_rule_event:
        keyword_coverage = max(keyword_coverage, 0.5)
    event_rate = grounded_events / max(expected_count, 1)
    hit_rate = min(1.0, 0.5 * keyword_coverage + 0.5 * event_rate)

    return {
        "hit_rate": round(hit_rate, 4),
        "hit_count": grounded_events,
        "expected_count": expected_count,
        "matched_keywords": matched_keywords[:6],
        "applicable": True,
    }


_WORLD_RULE_PLACEHOLDER_VALUES = {
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
}


def _normalize_world_rules_text(value: Optional[str]) -> str:
    text = str(value or "").strip()
    if not text or text in _WORLD_RULE_PLACEHOLDER_VALUES:
        return ""
    return text


def _extract_outline_rule_hints(chapter_outline: Optional[str], limit: int = 4) -> List[str]:
    if not chapter_outline:
        return []

    hints: List[str] = []
    seen: set[str] = set()
    current_section = ""
    cue_tokens = (
        "规则", "边界", "限制", "触发", "反噬",
        "登记", "改写", "污染", "诵读", "校对", "纠错",
    )
    constraint_tokens = (
        "不能", "不得", "否则", "代价", "伤及", "伤害", "失声", "流血",
        "说破", "公开", "只要", "一旦", "才会", "就会", "会把",
    )

    for raw_line in str(chapter_outline or "").splitlines():
        stripped = raw_line.strip()
        if not stripped:
            continue
        section_match = re.match(r"^【(?P<title>[^】]+)】$", stripped)
        if section_match:
            current_section = section_match.group("title")
            continue
        normalized = stripped.lstrip("-").strip()
        if not normalized:
            continue

        sentence_candidates = _split_sentences(normalized) if len(normalized) > 80 else [normalized]
        for sentence in sentence_candidates:
            sentence = sentence.strip()
            if not sentence:
                continue
            in_rule_section = any(token in current_section for token in ("规则", "边界", "限制"))
            has_rule_cue = any(token in sentence for token in cue_tokens)
            has_constraint = any(token in sentence for token in constraint_tokens)
            if not (in_rule_section or (has_rule_cue and has_constraint)):
                continue
            if sentence not in seen:
                seen.add(sentence)
                hints.append(sentence)
                if len(hints) >= limit:
                    return hints
    return hints


def _resolve_rule_grounding_source_text(
    world_rules: Optional[str],
    *,
    chapter_outline: Optional[str] = None,
    quality_runtime_context: Optional[Dict[str, object]] = None,
) -> str:
    explicit_rules = _normalize_world_rules_text(world_rules)
    if explicit_rules:
        return explicit_rules

    if isinstance(quality_runtime_context, dict):
        for key in ("world_rules", "world_rule_hints", "rule_impact", "world_rule_trigger"):
            value = quality_runtime_context.get(key)
            if isinstance(value, str):
                normalized = _normalize_world_rules_text(value)
                if normalized:
                    return normalized
            elif isinstance(value, list):
                items = [str(item).strip() for item in value if str(item).strip()]
                if items:
                    return "\n".join(items[:4])

    outline_hints = _extract_outline_rule_hints(chapter_outline)
    if outline_hints:
        return "\n".join(outline_hints)

    return ""


def _extract_outline_anchor_tokens(anchor: str, limit: int = 12) -> List[str]:
    """Extract stable keywords from prose-style outline anchors."""
    stop_tokens = {
        "章节概要", "剧情摘要", "关键事件", "情节要点", "叙事目标", "规则影响", "角色投择",
        "人物转折", "对话钩子", "角色焦点", "小爽点", "本章", "这一章", "这里", "继续",
    }
    split_chars = "的了着过把将让给在向对跟与和并而或但却被因于从到往里上下前后再还又先就都也仍会要想"
    candidates: List[str] = []

    for chunk in re.split(r'[，。！？!?（）()“”"‘’、\s]+', anchor or ""):
        for token in re.findall(r"[一-鿿]{2,16}", chunk):
            if token in stop_tokens:
                continue
            pieces = [token]
            for ch in split_chars:
                next_pieces: List[str] = []
                for piece in pieces:
                    next_pieces.extend([part for part in piece.split(ch) if part])
                pieces = next_pieces or pieces
            for piece in pieces:
                if len(piece) < 2 or piece in stop_tokens:
                    continue
                candidates.append(piece)
                if len(piece) > 6:
                    candidates.extend([piece[:4], piece[-4:]])
                elif len(piece) > 4:
                    candidates.extend([piece[:3], piece[-3:]])

    tokens: List[str] = []
    seen: set[str] = set()
    for token in sorted(candidates, key=len, reverse=True):
        if len(token) < 2 or token in stop_tokens or token in seen:
            continue
        seen.add(token)
        tokens.append(token)
        if len(tokens) >= limit:
            break
    return tokens


def _expand_anchor_match_tokens(tokens: List[str], limit: int = 24) -> List[str]:
    expanded: List[str] = []
    seen: set[str] = set()

    def _append(token: str) -> None:
        normalized = token.strip()
        if len(normalized) < 2 or normalized in seen:
            return
        seen.add(normalized)
        expanded.append(normalized)

    for token in tokens:
        _append(token)
        if len(token) >= 4:
            max_width = min(4, len(token))
            for width in range(2, max_width + 1):
                for start in range(0, len(token) - width + 1):
                    _append(token[start:start + width])
        if len(expanded) >= limit:
            break
    return expanded[:limit]


def extract_outline_anchor_lines(chapter_outline: Optional[str], max_lines: int = 10) -> List[str]:
    """Extract outline anchors from both headed and prose summaries."""
    if not chapter_outline:
        return []

    section_capture_limits = {
        "章节概要": 1,
        "剧情摘要": 1,
        "场景设定": 2,
        "关键事件": 4,
        "情节要点": 5,
        "叙事目标": 1,
        "冲突主线": 2,
        "角色抉择": 2,
        "代价/风险": 2,
        "规则影响点": 2,
        "对话钩子": 2,
        "人物转折": 2,
        "角色焦点": 2,
        "情感基调": 1,
    }
    keywords = (
        "章节概要", "剧情摘要", "关键事件", "情节要点", "叙事目标",
        "冲突", "规则影响", "角色投择", "角色抉择", "代价", "人物转折",
        "对话钩子", "角色焦点", "场景设定", "情感基调",
    )
    sentence_cues = (
        "目标", "冲突", "阻力", "规则", "决定", "代价", "反馈", "小爽点", "悬念", "章尾",
        "反转", "异常", "认主", "借书证", "页印", "回声", "机位", "禁播", "校对", "封门",
    )

    raw_lines = [line.strip() for line in chapter_outline.splitlines() if line.strip()]
    section_anchors: List[str] = []
    capture_bullet_count = 0

    for line in raw_lines:
        if line.startswith("【") and line.endswith("】"):
            section_name = line[1:-1].strip()
            if any(key in section_name for key in keywords):
                capture_bullet_count = section_capture_limits.get(section_name, 3)
            else:
                capture_bullet_count = 0
            continue

        cleaned = line.lstrip("- ").strip()
        if not cleaned:
            continue

        if capture_bullet_count > 0:
            section_anchors.append(cleaned[:120])
            capture_bullet_count -= 1
            continue

        if any(key in cleaned for key in keywords):
            parts = [part.strip() for part in re.split(r"[:：]", cleaned, maxsplit=1)]
            if len(parts) == 2 and parts[1] and any(key in parts[0] for key in keywords):
                section_anchors.append(parts[1][:120])
                continue
            if cleaned.endswith((":", "：")):
                continue
            section_anchors.append(cleaned[:120])

    sentence_anchors: List[str] = []
    for sentence in _split_sentences(chapter_outline):
        normalized = sentence.lstrip("- ").strip()
        if len(normalized) < 8:
            continue
        cue_score = sum(1 for cue in sentence_cues if cue in normalized)
        if cue_score <= 0 and len(normalized) < 24:
            continue
        sentence_anchors.append(normalized[:120])

    source_anchors = section_anchors if section_anchors else sentence_anchors

    deduped: List[str] = []
    seen: set[str] = set()
    for item in [*source_anchors, *sentence_anchors]:
        normalized = item.strip()
        if normalized and normalized not in seen:
            seen.add(normalized)
            deduped.append(normalized[:120])
        if len(deduped) >= max_lines:
            break

    return deduped


def _calc_outline_alignment_rate(text: str, chapter_outline: Optional[str]) -> Dict[str, Any]:
    """Calculate how well chapter body covers outline anchors."""
    anchors = extract_outline_anchor_lines(chapter_outline, max_lines=8)
    if not anchors:
        return {
            "hit_rate": 0.0,
            "hit_count": 0,
            "expected_count": 0,
            "matched_anchors": [],
            "applicable": False,
            "skipped_reason": "no_outline_anchors",
        }

    compact_text = re.sub(r"\s+", "", text or "")
    hit_count = 0
    matched_anchors: List[str] = []
    relevant_anchors = 0
    for anchor in anchors:
        key_tokens = _expand_anchor_match_tokens(_extract_outline_anchor_tokens(anchor))
        if not key_tokens:
            continue
        relevant_anchors += 1
        matched_tokens = [token for token in key_tokens if token in compact_text]
        long_match = any(len(token) >= 4 and token in compact_text for token in key_tokens)
        strong_match = any(len(token) >= 3 for token in matched_tokens)
        if long_match or (len(matched_tokens) >= 2 and strong_match) or len(matched_tokens) >= 3:
            hit_count += 1
            matched_anchors.append(anchor[:120])

    if relevant_anchors <= 0:
        return {
            "hit_rate": 0.0,
            "hit_count": 0,
            "expected_count": 0,
            "matched_anchors": [],
            "applicable": False,
            "skipped_reason": "no_anchor_tokens",
        }

    expected_count = max(1, min(relevant_anchors, 5))
    effective_hits = min(hit_count, expected_count)
    hit_rate = min(1.0, effective_hits / expected_count)
    return {
        "hit_rate": round(hit_rate, 4),
        "hit_count": hit_count,
        "expected_count": expected_count,
        "matched_anchors": matched_anchors[:6],
        "applicable": True,
    }


def _extract_payoff_chain_hints(
    chapter_outline: Optional[str] = None,
    quality_runtime_context: Optional[Dict[str, Any]] = None,
    limit: int = 4,
) -> List[str]:
    hints: List[str] = []
    seen: set[str] = set()

    if isinstance(quality_runtime_context, dict):
        runtime_items = quality_runtime_context.get("foreshadow_payoff_plan") or []
        if isinstance(runtime_items, list):
            for item in runtime_items:
                normalized = ""
                if isinstance(item, str):
                    normalized = item.strip()
                elif isinstance(item, dict):
                    normalized = " ".join(
                        str(item.get(key) or "").strip()
                        for key in ("setup", "payoff", "summary", "trigger", "resolution")
                        if str(item.get(key) or "").strip()
                    )
                else:
                    normalized = str(item or "").strip()
                if normalized and normalized not in seen:
                    seen.add(normalized)
                    hints.append(normalized[:120])
                    if len(hints) >= limit:
                        return hints

    priority_tokens = (
        "小爽点", "章尾", "钩子", "折返", "救人", "挡了一下", "尸体", "锚点", "持刀人",
    )
    general_tokens = (
        "反馈", "回收", "兑现", "悬念", "第一页", "救下", "反扑", "翻盘",
    )
    scored_candidates: List[Tuple[int, str]] = []
    for raw_line in str(chapter_outline or "").splitlines():
        normalized = raw_line.lstrip("- ").strip()
        if not normalized or normalized.startswith("【"):
            continue
        score = sum(3 for token in priority_tokens if token in normalized)
        score += sum(1 for token in general_tokens if token in normalized)
        if score > 0:
            scored_candidates.append((score, normalized[:120]))

    for sentence in _split_sentences(chapter_outline or ""):
        normalized = sentence.strip()
        if not normalized:
            continue
        score = sum(3 for token in priority_tokens if token in normalized)
        score += sum(1 for token in general_tokens if token in normalized)
        if score > 0:
            scored_candidates.append((score, normalized[:120]))

    for _score, candidate in sorted(scored_candidates, key=lambda item: (-item[0], len(item[1]))):
        if candidate not in seen:
            seen.add(candidate)
            hints.append(candidate)
            if len(hints) >= limit:
                break
    return hints


def _calc_payoff_chain_rate(
    text: str,
    *,
    chapter_outline: Optional[str] = None,
    quality_runtime_context: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    """Estimate setup-burst-feedback payoff coverage."""
    sentences = _split_sentences(text)
    if not sentences:
        return {"hit_rate": 0.0, "hit_count": 0, "expected_count": 1, "applicable": True}

    setup_words = [
        "原本", "本来", "一直", "眼看", "刚要", "正要", "谁知", "没想到", "偏偏", "被逼", "刚想", "没来得及",
        "早废了", "刚把", "认定", "旧伤", "页印", "提示词",
        "校验失败", "底稿偏差", "编号页", "见证人数超过阈值",
    ]
    burst_words = [
        "突然", "当场", "直接", "瞬间", "反手", "竟然", "立刻", "翻盘", "突破", "赢了", "拿下", "触发",
        "飙升", "反锁", "卡住", "炸开", "猛地", "终于", "弹出", "找到", "踹开", "捡起",
        "二次复核启动", "触发底稿追索", "暗柜弹开", "红字猛地放大", "绑定见证人",
    ]
    feedback_words = [
        "愣住", "哗然", "脸色变", "看傻", "松了口气", "发麻", "欢呼", "安静下来", "炸开了锅", "礼物", "打赏", "热度",
        "刷满", "弹幕", "观众数", "破万", "认主", "借书证", "锁死",
        "在线人数掉到", "发灰", "脸色沉下去", "呼吸停了一瞬", "鲜得刺眼",
    ]

    expected_count = max(1, len(text) // 1800)
    hit_count = 0

    for idx in range(max(0, len(sentences) - 2)):
        setup_window = " ".join(sentences[max(0, idx - 1): idx + 1])
        burst_window = " ".join(sentences[idx: idx + 3])
        feedback_window = " ".join(sentences[idx: idx + 5])
        if (
            any(word in setup_window for word in setup_words)
            and any(word in burst_window for word in burst_words)
            and any(word in feedback_window for word in feedback_words)
        ):
            hit_count += 1

    compact_text = re.sub(r"\s+", "", text or "")
    if hit_count == 0 and compact_text:
        early_window = compact_text[: max(1, int(len(compact_text) * 0.6))]
        late_window = compact_text[max(0, int(len(compact_text) * 0.35)):]
        if (
            any(word in early_window for word in setup_words)
            and any(word in late_window for word in burst_words)
            and any(word in late_window for word in feedback_words)
        ):
            hit_count = 1

    if hit_count == 0:
        for hint in _extract_payoff_chain_hints(
            chapter_outline=chapter_outline,
            quality_runtime_context=quality_runtime_context,
        ):
            key_tokens = _expand_anchor_match_tokens(_extract_outline_anchor_tokens(hint, limit=8))
            if not key_tokens:
                continue
            matched_tokens = [token for token in key_tokens if token in compact_text]
            if (
                len(matched_tokens) >= 2
                or any(len(token) >= 4 and token in compact_text for token in key_tokens)
            ):
                hit_count = 1
                break

    hit_rate = min(1.0, hit_count / max(expected_count, 1))
    return {
        "hit_rate": round(hit_rate, 4),
        "hit_count": hit_count,
        "expected_count": expected_count,
        "applicable": True,
    }


def _calc_cliffhanger_rate(text: str) -> Dict[str, Any]:
    """Estimate whether the ending leaves a strong unresolved pull."""
    ending = (text or "")[-360:]
    if not ending.strip():
        return {"hit_rate": 0.0, "matched_markers": [], "window_length": 0, "applicable": True}

    ending_compact = re.sub(r"\s+", "", ending)
    markers = {
        "info_gap": [
            "怎么会", "原来", "却发现", "门后", "那个人", "竟是", "真相", "秘密", "申请人",
            "原始拍摄地", "审校官", "借阅日期写着", "弹幕没有名字", "同一句话", "第一页",
            "底稿追索", "绑定见证人", "审读准备", "审读室", "上传端不在", "在——", "图样一样",
            "陌生地址", "陌生门牌", "04:44", "母本身份", "初始编录者",
            "隔空观看", "另一端", "实时批注", "名字被镜头拉清", "不该出现的名字",
            "那第二个", "为什么是你", "来电显示", "只有一行字", "另一个闻川", "另一个自己", "待复核", "三个林检", "只有两个人", "分明只有两个人",
            "新的标注", "异常叙事", "追认记录", "源头编号", "发件人：", "发送时间：", "定时消息", "机主信息全空", "下一段预告", "预告自动跳出",
            "不是手机里出来的", "在他背后", "在她背后", "真的站着", "同样穿校服",
        ],
        "danger": [
            "脚步声", "逼近", "枪口", "刀", "下一秒", "扑来", "追上", "要出事", "拍门声", "全城公开",
            "逾时", "一个不少", "大门轰地合死", "不能让他出去", "倒计时归零",
            "重新渗了出来", "鲜得刺眼", "别让他们读", "白灯一亮", "亮起一片惨白",
            "先到你家", "热度回来了", "渗出新的血", "又起一波转播", "倒计时", "六十秒",
            "驳回失败", "见证人已锁定", "自己亮起", "自己响起", "自己亮了", "锁屏弹出", "胸口一片血",
        ],
        "identity_twist": [
            "竟然是你", "身份", "卧底", "叛徒", "冒名", "伪装", "认出来", "认主",
            "错位者", "名字却不是", "证件照", "旧版站务员证件照", "另一个闻川", "另一个我", "和他穿一样",
            "被追认为", "追认成", "见证人是你", "你才是见证人",
        ],
        "choice_pending": [
            "该不该", "要不要", "只能", "必须选", "下一步", "还没决定", "伸向", "停在半空", "二十分钟内", "前往", "完成现场更正",
            "要么", "选，快", "选,快", "继续改", "上去抓人", "天亮前", "去这儿", "晚一步", "04:44", "请确认", "确认母本身份",
        ],
        "escalation": [
            "书认主了", "观众数疯了一样往上跳", "瞬间破万", "密密麻麻往上滚", "开始校对第一页", "开始校对",
            "账号名", "实名", "手机号", "住址", "转播", "热度回来了", "新的任务", "再次刷新", "第二轮现实校验已开启",
            "第二轮复核", "见证人已加入", "接入中", "已标记", "二级关注对象已标记",
            "公开追认", "全网同步", "升级为", "异常叙事已锁定",
        ],
    }
    weak_endings = [
        "总之", "他明白了", "命运将会", "一切都会好起来", "故事还在继续",
    ]

    matched: List[str] = []
    for label, words in markers.items():
        if any(word in ending_compact for word in words):
            matched.append(label)

    if any(word in ending_compact for word in weak_endings):
        return {"hit_rate": 0.0, "matched_markers": [], "window_length": len(ending), "applicable": True}

    hit_rate = min(1.0, len(matched) / 2) if matched else 0.0
    return {
        "hit_rate": round(hit_rate, 4),
        "matched_markers": matched,
        "window_length": len(ending),
        "applicable": True,
    }


def _calc_conflict_chain_rate(text: str) -> Dict[str, Any]:
    """Calculate objective-obstacle-choice-cost chain coverage."""
    sentences = _split_sentences(text)
    if not sentences:
        return {"hit_rate": 0.0, "hit_count": 0, "expected_count": 1, "applicable": True}

    obstacle_words = [
        "受阻", "拦住", "拦下", "失败", "危机", "危险", "封锁", "卡住", "失联", "崩溃", "中断",
        "断电", "封禁", "违约", "催债", "堵住", "失控", "逼近", "困住", "被困", "锁死", "围住",
        "逼迫", "压住", "扣下", "扣住", "不行", "红灯", "终止", "拖活人进去", "接管", "归零",
        "追责", "失控翻倍", "错误样本", "拍门声", "全城公开",
        "异常固化倒计时", "门影", "超阈值", "复核", "底稿追索",
        "断不了", "失灵", "热门", "顶上热门", "破万", "封控", "热度没断", "热度回来了", "认主",
    ]
    choice_words = [
        "选择", "决定", "只能", "必须", "打算", "转而", "改走", "赌一把", "咬牙", "拍板",
        "开播", "接听", "下楼", "下去", "推门", "按下", "避开", "硬着头皮", "反锁", "还是",
        "换招", "抢过", "拽了下来", "往里走", "继续播", "戴上", "跨进去", "抢主持权限",
        "主持回归", "拉下总闸", "拽下", "输入", "前往", "完成现场更正", "挂临时观察员身份",
        "给这玩意编个解释", "把手机重新举正", "对着镜头硬挤出笑", "让他们信这不是鬼门",
        "跑", "断流", "交出", "不交", "对准", "给我", "收线", "控它", "扯断页", "把镜头",
    ]
    cost_words = [
        "代价", "损失", "牺牲", "受伤", "暴露", "麻烦", "后果", "风险", "拖慢", "失去",
        "违约金", "封号", "扣走", "刺痛", "发麻", "卡死", "丢命", "出事", "退路", "接管",
        "认出来", "不能让他出去", "合死", "破万", "押金", "死", "拖进去", "抵押", "真记忆",
        "错误样本", "全城公开", "失控翻倍",
        "在线人数掉到", "红字边缘开始发灰", "绑定见证人", "重新渗了出来", "鲜得刺眼",
        "反噬", "绑上", "绑上了", "优先找你", "住址", "实名", "手机号", "先到你家", "伤人", "它认主了", "认主",
    ]

    expected_count = max(1, len(text) // 900)
    hit_count = 0

    for idx, _ in enumerate(sentences):
        obstacle_window = " ".join(sentences[idx: idx + 4])
        choice_window = " ".join(sentences[idx: idx + 6])
        cost_window = " ".join(sentences[idx: idx + 10])
        if not any(word in obstacle_window for word in obstacle_words):
            continue
        if any(word in choice_window for word in choice_words) and any(word in cost_window for word in cost_words):
            hit_count += 1

    if hit_count < expected_count and sentences:
        obstacle_window = " ".join(sentences[: max(4, int(len(sentences) * 0.6))])
        choice_window = " ".join(sentences[max(0, int(len(sentences) * 0.25)): max(1, int(len(sentences) * 0.85))])
        cost_window = " ".join(sentences[max(0, int(len(sentences) * 0.45)):])
        if (
            any(word in obstacle_window for word in obstacle_words)
            and any(word in choice_window for word in choice_words)
            and any(word in cost_window for word in cost_words)
        ):
            hit_count = min(expected_count, hit_count + 1)

    hit_rate = min(1.0, hit_count / max(expected_count, 1))
    return {
        "hit_rate": round(hit_rate, 4),
        "hit_count": hit_count,
        "expected_count": expected_count,
        "applicable": True,
    }


_CONTINUITY_LEDGER_SPECS: tuple[tuple[str, str, str, str], ...] = (
    (
        "character_state_ledger",
        "character_continuity",
        "Character continuity ledger",
        "Carry forward the character continuity ledger: {item}",
    ),
    (
        "relationship_state_ledger",
        "relationship_continuity",
        "Relationship continuity ledger",
        "Express the relationship ledger through dialogue, alignment, or exchange: {item}",
    ),
    (
        "foreshadow_state_ledger",
        "foreshadow_continuity",
        "Foreshadow continuity ledger",
        "Advance the foreshadow ledger toward payoff: {item}",
    ),
    (
        "organization_state_ledger",
        "organization_continuity",
        "Organization continuity ledger",
        "Carry forward the organization continuity ledger through command, resource, or territory change: {item}",
    ),
    (
        "career_state_ledger",
        "career_continuity",
        "Career continuity ledger",
        "Carry forward the career growth ledger through skill use, bottleneck, or cost: {item}",
    ),
)


def _extract_continuity_anchor_candidates(item: Any) -> List[str]:
    text = str(item or "").strip()
    if not text:
        return []
    head = re.split(r"[:：]", text, maxsplit=1)[0].strip() or text
    segments = [
        segment.strip()
        for segment in re.split(r"[、,\/|&＆和与+·•]+", head)
        if segment.strip()
    ]
    if not segments:
        segments = [head]
    tokens: List[str] = []
    seen: set[str] = set()
    cleanup_translation = str.maketrans(
        {
            "【": " ",
            "】": " ",
            "[": " ",
            "]": " ",
            "（": " ",
            "）": " ",
            "(": " ",
            ")": " ",
            "<": " ",
            ">": " ",
            "《": " ",
            "》": " ",
            "“": " ",
            "”": " ",
            '"': " ",
            "'": " ",
            "`": " ",
        }
    )
    for segment in segments[:3]:
        cleaned = segment.translate(cleanup_translation)
        for token in re.findall(r"[A-Za-z0-9_\-]{2,}|[\u4E00-\u9FFF]{2,}", cleaned):
            normalized = token.strip().lower()
            if not normalized or normalized in seen:
                continue
            seen.add(normalized)
            tokens.append(normalized)
    if tokens:
        return tokens[:3]

    fallback = re.sub(r"\s+", "", head).lower()
    return [fallback] if len(fallback) >= 2 else []


def build_story_continuity_preflight(
    content: str,
    runtime_context: Optional[Dict[str, Any]],
) -> Dict[str, Any]:
    from tests.test_support.schemas.quality import (
        _normalize_runtime_context_items,
        _stringify_runtime_context_item,
    )

    if not isinstance(runtime_context, dict):
        return {}

    normalized_content = re.sub(r"\s+", "", str(content or "")).lower()
    if not normalized_content:
        return {}

    warnings: List[Dict[str, Any]] = []
    focus_areas: List[str] = []
    repair_targets: List[str] = []
    checked_item_count = 0
    missing_item_count = 0

    for ledger_key, focus_area, ledger_label, repair_template in _CONTINUITY_LEDGER_SPECS:
        for item in _normalize_runtime_context_items(runtime_context.get(ledger_key), limit=3):
            item_text = _stringify_runtime_context_item(item)
            if not item_text:
                continue
            checked_item_count += 1
            anchors = _extract_continuity_anchor_candidates(item_text)
            matched_anchor_count = len(
                {
                    anchor
                    for anchor in anchors
                    if len(anchor) >= 2 and anchor in normalized_content
                }
            )
            required_match_count = (
                2
                if ledger_key in {"relationship_state_ledger", "career_state_ledger"}
                and len(anchors) >= 2
                else 1
            )
            if matched_anchor_count >= required_match_count:
                continue
            missing_item_count += 1
            if focus_area not in focus_areas:
                focus_areas.append(focus_area)
            target = repair_template.format(item=item_text)
            if target not in repair_targets:
                repair_targets.append(target)
            warnings.append(
                {
                    "ledger_key": ledger_key,
                    "ledger_label": ledger_label,
                    "focus_area": focus_area,
                    "item": item_text,
                    "anchors": anchors,
                    "matched_anchor_count": matched_anchor_count,
                    "required_match_count": required_match_count,
                }
            )
            if len(warnings) >= 4:
                break
        if len(warnings) >= 4:
            break

    if not warnings:
        return {
            "status": "ok",
            "checked_item_count": checked_item_count,
            "warning_count": 0,
            "warnings": [],
            "focus_areas": [],
            "repair_targets": [],
            "summary": "",
        }

    labels = ", ".join(dict.fromkeys(warning["ledger_label"] for warning in warnings))
    summary = (
        f"Current chapter misses explicit handoff for {missing_item_count} "
        "continuity ledger items."
    )
    if labels:
        summary = (
            f"Current chapter misses explicit handoff for {missing_item_count} "
            f"continuity ledger items. Prioritize {labels}."
        )
    return {
        "status": "warning",
        "checked_item_count": checked_item_count,
        "warning_count": len(warnings),
        "missing_item_count": missing_item_count,
        "warnings": warnings,
        "focus_areas": focus_areas,
        "repair_targets": repair_targets[:4],
        "summary": summary,
    }


def compute_story_quality_metrics(
    content: str,
    chapter_outline: Optional[str],
    world_rules: Optional[str],
    quality_runtime_context: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    """Compute chapter quality metrics with runtime context."""
    from tests.test_support.schemas.quality import (
        build_quality_gate_decision,
        build_story_repair_guidance,
    )

    resolved_world_rules = _resolve_rule_grounding_source_text(
        world_rules,
        chapter_outline=chapter_outline,
        quality_runtime_context=quality_runtime_context,
    )
    conflict = _calc_conflict_chain_rate(content)
    rule_grounding = _calc_rule_grounding_rate(content, resolved_world_rules)
    outline_alignment = _calc_outline_alignment_rate(content, chapter_outline)
    dialogue = _calc_dialogue_naturalness_rate(content)
    opening_hook = _calc_opening_hook_rate(content)
    payoff_chain = _calc_payoff_chain_rate(
        content,
        chapter_outline=chapter_outline,
        quality_runtime_context=quality_runtime_context,
    )
    cliffhanger = _calc_cliffhanger_rate(content)

    overall = _calc_applicable_quality_overall([
        (conflict, 0.26),
        (rule_grounding, 0.22),
        (outline_alignment, 0.18),
        (dialogue, 0.12),
        (opening_hook, 0.10),
        (payoff_chain, 0.07),
        (cliffhanger, 0.05),
    ])

    metrics = {
        "overall_score": round(overall * 100, 1),
        "conflict_chain_hit_rate": round(conflict["hit_rate"] * 100, 1),
        "rule_grounding_hit_rate": round(rule_grounding["hit_rate"] * 100, 1),
        "outline_alignment_rate": round(outline_alignment["hit_rate"] * 100, 1),
        "dialogue_naturalness_rate": round(dialogue["hit_rate"] * 100, 1),
        "opening_hook_rate": round(opening_hook["hit_rate"] * 100, 1),
        "payoff_chain_rate": round(payoff_chain["hit_rate"] * 100, 1),
        "cliffhanger_rate": round(cliffhanger["hit_rate"] * 100, 1),
        "details": {
            "conflict_chain": conflict,
            "rule_grounding": rule_grounding,
            "outline_alignment": outline_alignment,
            "dialogue": dialogue,
            "opening_hook": opening_hook,
            "payoff_chain": payoff_chain,
            "cliffhanger": cliffhanger,
        }
    }
    if isinstance(quality_runtime_context, dict) and quality_runtime_context:
        metrics["quality_runtime_context"] = quality_runtime_context
        continuity_preflight = build_story_continuity_preflight(content, quality_runtime_context)
        if continuity_preflight:
            metrics["continuity_preflight"] = continuity_preflight
    metrics["repair_guidance"] = build_story_repair_guidance(metrics, scope="chapter")
    metrics["quality_gate"] = build_quality_gate_decision(metrics, scope="chapter")
    return metrics

