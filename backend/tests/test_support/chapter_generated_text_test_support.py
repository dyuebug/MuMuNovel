from __future__ import annotations

import re
from typing import List


_CHAPTER_WORKFLOW_META_PATTERNS = (
    r"^\s*(?:步骤|step)\s*\d+\b",
    r"^\s*执行\s*\d+(?:\.\d+)*\b",
    r"调用\s*agent",
    r"(?:流程|步骤)\s*(?:说明|日志|总结|复盘|评审)",
    r"(?:方案对比|方案评审|复盘结论|执行计划)",
    r"^\s*(?:作为|身为)\s*(?:ai|助手|模型)[：:?,，]",
)
_CHAPTER_META_PREFIXES = {
    "以下是章节正文：",
    "以下是正文：",
    "章节正文：",
    "正文：",
}

_LIGHT_TEMPLATE_SENTENCE_LEADS = (
    "下一秒",
    "那一瞬",
)

_LIGHT_TEMPLATE_SIMILE_PATTERN = re.compile(
    r"像(?P<body>[^，。！？；\n]{1,18})一样"
)


def is_likely_chapter_meta_line(line: str) -> bool:
    stripped = (line or "").strip()
    if not stripped:
        return False
    if stripped.startswith("```"):
        return True
    if stripped in _CHAPTER_META_PREFIXES:
        return True
    return any(
        re.search(pattern, stripped, flags=re.IGNORECASE)
        for pattern in _CHAPTER_WORKFLOW_META_PATTERNS
    )


def contains_chapter_workflow_meta_text(text: str) -> bool:
    if not text:
        return False
    return any(is_likely_chapter_meta_line(line) for line in text.splitlines())


def lightly_polish_template_phrases(text: str) -> str:
    cleaned = text

    sentence_boundary_pattern = r"""(^|[。！？!?；;\n])([?"'\u201c\u201d\u2018\u2019(]*)"""
    sentence_lead_suffix_pattern = r"""(?:[，、,]\s*)?"""
    leading_punctuation_pattern = re.compile(
        r"""(^|[。！？!?；;\n])([?"'\u201c\u201d\u2018\u2019(]*)[，、,]\s*""",
        flags=re.MULTILINE,
    )

    for sentence_lead in _LIGHT_TEMPLATE_SENTENCE_LEADS:
        pattern = re.compile(
            sentence_boundary_pattern
            + re.escape(sentence_lead)
            + sentence_lead_suffix_pattern,
            flags=re.MULTILINE,
        )
        seen_count = 0

        def _replace_sentence_lead(match: re.Match[str]) -> str:
            nonlocal seen_count
            seen_count += 1
            if seen_count <= 1:
                return match.group(0)
            return f"{match.group(1)}{match.group(2)}"

        cleaned = pattern.sub(_replace_sentence_lead, cleaned)

    simile_seen_count = 0

    def _replace_simile(match: re.Match[str]) -> str:
        nonlocal simile_seen_count
        simile_seen_count += 1
        if simile_seen_count <= 2:
            return match.group(0)

        body = (match.group("body") or "").strip()
        if not body:
            return match.group(0)
        return f"像{body}那样"

    cleaned = _LIGHT_TEMPLATE_SIMILE_PATTERN.sub(_replace_simile, cleaned)
    cleaned = re.sub("像是有什么", "像有", cleaned)
    cleaned = re.sub("像有什么", "像有", cleaned)
    cleaned = leading_punctuation_pattern.sub(
        lambda match: f"{match.group(1)}{match.group(2)}",
        cleaned,
    )
    return cleaned


def trim_text_to_sentence_boundary(
    text: str,
    *,
    hard_limit: int,
    lookback_chars: int = 220,
) -> str:
    normalized_text = str(text or "")
    if hard_limit <= 0 or len(normalized_text) <= hard_limit:
        return normalized_text.strip()

    search_start = max(0, hard_limit - max(int(lookback_chars or 0), 80))
    best_boundary_index = -1
    for boundary_char in ("。", "！", "？", "!", "?", "；", ";", "\n"):
        boundary_index = normalized_text.rfind(
            boundary_char,
            search_start,
            hard_limit + 1,
        )
        if boundary_index > best_boundary_index:
            best_boundary_index = boundary_index

    if best_boundary_index >= search_start:
        return normalized_text[: best_boundary_index + 1].strip()

    trimmed_text = normalized_text[:hard_limit].rstrip("，,、 ")
    if trimmed_text and trimmed_text[-1] not in {"。", "！", "？", "!", "?", "；", ";"}:
        trimmed_text += "。"
    return trimmed_text.strip()


def sanitize_generated_narrative_text(text: str) -> tuple[str, int]:
    original = (text or "").replace("\r\n", "\n").strip()
    if not original:
        return "", 0

    removed_line_count = 0
    kept_lines: List[str] = []

    for raw_line in original.split("\n"):
        stripped = raw_line.strip()
        if not stripped:
            kept_lines.append("")
            continue

        if is_likely_chapter_meta_line(stripped):
            removed_line_count += 1
            continue

        kept_lines.append(raw_line)

    cleaned = re.sub(r"\n{3,}", "\n\n", "\n".join(kept_lines)).strip()
    cleaned = lightly_polish_template_phrases(cleaned)
    cleaned = re.sub(r"\n{3,}", "\n\n", cleaned).strip()
    return cleaned, removed_line_count
