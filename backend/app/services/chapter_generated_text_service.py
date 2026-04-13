from __future__ import annotations

import re
from typing import List


_CHAPTER_WORKFLOW_META_PATTERNS = (
    r"^\s*(?:\u6b65\u9aa4|step)\s*\d+\b",
    r"^\s*\u6267\u884c\s*\d+(?:\.\d+)*\b",
    r"\u8c03\u7528\s*agent",
    r"(?:\u6d41\u7a0b|\u6b65\u9aa4)\s*(?:\u8bf4\u660e|\u65e5\u5fd7|\u603b\u7ed3|\u590d\u76d8|\u8bc4\u5ba1)",
    r"(?:\u65b9\u6848\u5bf9\u6bd4|\u65b9\u6848\u8bc4\u5ba1|\u590d\u76d8\u7ed3\u8bba|\u6267\u884c\u8ba1\u5212)",
    r"^\s*(?:\u4f5c\u4e3a|\u8eab\u4e3a)\s*(?:ai|\u52a9\u624b|\u6a21\u578b)[\uff1a:?,]",
)
_CHAPTER_META_PREFIXES = {
    "\u4ee5\u4e0b\u662f\u7ae0\u8282\u6b63\u6587\uff1a",
    "\u4ee5\u4e0b\u662f\u6b63\u6587\uff1a",
    "\u7ae0\u8282\u6b63\u6587\uff1a",
    "\u6b63\u6587\uff1a",
}

_LIGHT_TEMPLATE_SENTENCE_LEADS = (
    "\u4e0b\u4e00\u79d2",
    "\u90a3\u4e00\u77ac",
)

_LIGHT_TEMPLATE_SIMILE_PATTERN = re.compile(
    r"\u50cf(?P<body>[^\uff0c\u3002\uff01\uff1f\uff1b\n]{1,18})\u4e00\u6837"
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
    """\u8f7b\u5ea6\u6253\u78e8\u9ad8\u9891\u6a21\u677f\u53e5\u5f0f\uff0c\u51cf\u5c11\u660e\u663e AI \u8154\u8c03\u3002"""
    cleaned = text

    sentence_boundary_pattern = r"""(^|[\u3002\uff01\uff1f!?\uff1b;\n])([?"'????(]*)"""
    sentence_lead_suffix_pattern = r"""(?:[\uff0c\u3001,]\s*)?"""
    leading_punctuation_pattern = re.compile(
        r"""(^|[\u3002\uff01\uff1f!?\uff1b;\n])([?"'????(]*)[\uff0c\u3001,]\s*""",
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
        return f"\u50cf{body}\u90a3\u6837"

    cleaned = _LIGHT_TEMPLATE_SIMILE_PATTERN.sub(_replace_simile, cleaned)
    cleaned = re.sub("\u50cf\u662f\u6709\u4ec0\u4e48", "\u50cf\u6709", cleaned)
    cleaned = re.sub("\u50cf\u6709\u4ec0\u4e48", "\u50cf\u6709", cleaned)
    cleaned = leading_punctuation_pattern.sub(
        lambda match: f"{match.group(1)}{match.group(2)}",
        cleaned,
    )
    return cleaned


def trim_text_to_sentence_boundary(text: str, *, hard_limit: int, lookback_chars: int = 220) -> str:
    normalized_text = str(text or "")
    if hard_limit <= 0 or len(normalized_text) <= hard_limit:
        return normalized_text.strip()

    search_start = max(0, hard_limit - max(int(lookback_chars or 0), 80))
    best_boundary_index = -1
    for boundary_char in ("\u3002", "\uff01", "\uff1f", "!", "?", "\uff1b", ";", "\n"):
        boundary_index = normalized_text.rfind(boundary_char, search_start, hard_limit + 1)
        if boundary_index > best_boundary_index:
            best_boundary_index = boundary_index

    if best_boundary_index >= search_start:
        return normalized_text[: best_boundary_index + 1].strip()

    trimmed_text = normalized_text[:hard_limit].rstrip("\uff0c,\u3001 ")
    if trimmed_text and trimmed_text[-1] not in {"\u3002", "\uff01", "\uff1f", "!", "?", "\uff1b", ";"}:
        trimmed_text += "\u3002"
    return trimmed_text.strip()


def sanitize_generated_narrative_text(text: str) -> tuple[str, int]:
    """\u6e05\u7406\u6a21\u578b\u5076\u53d1\u8f93\u51fa\u7684\u6d41\u7a0b\u5316\u5143\u6587\u672c\uff0c\u907f\u514d\u6c61\u67d3\u7ae0\u8282\u6b63\u6587\u3002"""
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
