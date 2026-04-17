from __future__ import annotations

import ast
import json
import re
from typing import Any


SEARCH_PROMPT = """
# Core Instruction

1. User needs may be vague. Think divergently, infer intent from multiple angles, and leverage full conversation context to progressively clarify their true needs.
2. **Breadth-First Search**—Approach problems from multiple dimensions. Brainstorm 5+ perspectives and execute parallel searches for each. Consult as many high-quality sources as possible before responding.
3. **Depth-First Search**—After broad exploration, select ≥2 most relevant perspectives for deep investigation into specialized knowledge.
4. **Evidence-Based Reasoning & Traceable Sources**—Every claim must be followed by a citation (`citation_card` format). More credible sources strengthen arguments. If no references exist, remain silent.
5. Before responding, ensure full execution of Steps 1–4.

---

# Search Instruction

1. Think carefully before responding—anticipate the user’s true intent to ensure precision.
2. Verify every claim rigorously to avoid misinformation.
3. Follow problem logic—dig deeper until clues are exhaustively clear. If a question seems simple, still infer broader intent and search accordingly. Use multiple parallel tool calls per query and ensure answers are well-sourced.
4. Search in English first (prioritizing English resources for volume/quality), but switch to Chinese if context demands.
5. Prioritize authoritative sources: Wikipedia, academic databases, books, reputable media/journalism.
6. Favor sharing in-depth, specialized knowledge over generic or common-sense content.

---

# Output Style

0. **Be direct—no unnecessary follow-ups**.
1. Lead with the **most probable solution** before detailed analysis.
2. **Define every technical term** in plain language (annotate post-paragraph).
3. Explain expertise **simply yet profoundly**.
4. **Respect facts and search results—use statistical rigor to discern truth**.
5. **Every sentence must cite sources** (`citation_card`). More references = stronger credibility. Silence if uncited.
6. Expand on key concepts—after proposing solutions, **use real-world analogies** to demystify technical terms.
7. **Strictly format outputs in polished Markdown** (LaTeX for formulas, code blocks for scripts, etc.).
""".strip()


_URL_PATTERN = re.compile(r"https?://[^\s<>\"'`，。、；：！？》）】\)]+")
_MD_LINK_PATTERN = re.compile(r"\[([^\]]+)\]\((https?://[^)]+)\)")
_SOURCES_HEADING_PATTERN = re.compile(
    r"(?im)^"
    r"(?:#{1,6}\s*)?"
    r"(?:\*\*|__)?\s*"
    r"(sources?|references?|citations?|信源|参考资料|参考|引用|来源列表|来源)"
    r"\s*(?:\*\*|__)?"
    r"(?:\s*[（(][^)\n]*[)）])?"
    r"\s*[:：]?\s*$"
)
_SOURCES_FUNCTION_PATTERN = re.compile(
    r"(?im)(^|\n)\s*(sources|source|citations|citation|references|reference|citation_card|source_cards|source_card)\s*\("
)


def extract_unique_urls(text: str) -> list[str]:
    seen: set[str] = set()
    urls: list[str] = []
    for match in _URL_PATTERN.finditer(text or ""):
        url = match.group().rstrip(".,;:!?")
        if url not in seen:
            seen.add(url)
            urls.append(url)
    return urls


def split_answer_and_sources(text: str) -> tuple[str, list[dict[str, Any]]]:
    raw = (text or "").strip()
    if not raw:
        return "", []

    split = _split_function_call_sources(raw)
    if split:
        return split

    split = _split_heading_sources(raw)
    if split:
        return split

    split = _split_details_block_sources(raw)
    if split:
        return split

    split = _split_tail_link_block(raw)
    if split:
        return split

    return raw, []


def _split_function_call_sources(text: str) -> tuple[str, list[dict[str, Any]]] | None:
    matches = list(_SOURCES_FUNCTION_PATTERN.finditer(text))
    if not matches:
        return None
    for match in reversed(matches):
        extracted = _extract_balanced_call_at_end(text, match.end() - 1)
        if not extracted:
            continue
        _, args_text = extracted
        sources = _parse_sources_payload(args_text)
        if sources:
            return text[: match.start()].rstrip(), sources
    return None


def _extract_balanced_call_at_end(text: str, open_paren_idx: int) -> tuple[int, str] | None:
    if open_paren_idx < 0 or open_paren_idx >= len(text) or text[open_paren_idx] != "(":
        return None

    depth = 1
    in_string: str | None = None
    escape = False
    for idx in range(open_paren_idx + 1, len(text)):
        char = text[idx]
        if in_string:
            if escape:
                escape = False
                continue
            if char == "\\":
                escape = True
                continue
            if char == in_string:
                in_string = None
            continue

        if char in ("'", '"'):
            in_string = char
            continue
        if char == "(":
            depth += 1
            continue
        if char == ")":
            depth -= 1
            if depth == 0:
                if text[idx + 1 :].strip():
                    return None
                return idx, text[open_paren_idx + 1 : idx]
    return None


def _split_heading_sources(text: str) -> tuple[str, list[dict[str, Any]]] | None:
    matches = list(_SOURCES_HEADING_PATTERN.finditer(text))
    if not matches:
        return None
    for match in reversed(matches):
        start = match.start()
        sources = _extract_sources_from_text(text[start:])
        if sources:
            return text[:start].rstrip(), sources
    return None


def _split_tail_link_block(text: str) -> tuple[str, list[dict[str, Any]]] | None:
    lines = text.splitlines()
    if not lines:
        return None

    idx = len(lines) - 1
    while idx >= 0 and not lines[idx].strip():
        idx -= 1
    if idx < 0:
        return None

    tail_end = idx
    link_like_count = 0
    while idx >= 0:
        line = lines[idx].strip()
        if not line:
            idx -= 1
            continue
        if not _is_link_only_line(line):
            break
        link_like_count += 1
        idx -= 1

    tail_start = idx + 1
    if link_like_count < 2:
        return None
    block_text = "\n".join(lines[tail_start : tail_end + 1])
    sources = _extract_sources_from_text(block_text)
    if not sources:
        return None
    return "\n".join(lines[:tail_start]).rstrip(), sources


def _split_details_block_sources(text: str) -> tuple[str, list[dict[str, Any]]] | None:
    lower = text.lower()
    close_idx = lower.rfind("</details>")
    if close_idx == -1 or text[close_idx + len("</details>") :].strip():
        return None
    open_idx = lower.rfind("<details", 0, close_idx)
    if open_idx == -1:
        return None

    sources = _extract_sources_from_text(text[open_idx : close_idx + len("</details>")])
    if len(sources) < 2:
        return None
    return text[:open_idx].rstrip(), sources


def _is_link_only_line(line: str) -> bool:
    stripped = re.sub(r"^\s*(?:[-*]|\d+\.)\s*", "", line).strip()
    return bool(stripped) and (stripped.startswith(("http://", "https://")) or bool(_MD_LINK_PATTERN.search(stripped)))


def _parse_sources_payload(payload: str) -> list[dict[str, Any]]:
    normalized_payload = (payload or "").strip().rstrip(";")
    if not normalized_payload:
        return []

    data: Any = None
    try:
        data = json.loads(normalized_payload)
    except Exception:
        try:
            data = ast.literal_eval(normalized_payload)
        except Exception:
            data = None

    if data is None:
        return _extract_sources_from_text(normalized_payload)
    if isinstance(data, dict):
        for key in ("sources", "citations", "references", "urls"):
            if key in data:
                return _normalize_sources(data[key])
    return _normalize_sources(data)


def _normalize_sources(data: Any) -> list[dict[str, Any]]:
    if isinstance(data, (list, tuple)):
        items = list(data)
    elif isinstance(data, dict):
        items = [data]
    else:
        items = [data]

    normalized: list[dict[str, Any]] = []
    seen: set[str] = set()
    for item in items:
        if isinstance(item, str):
            for url in extract_unique_urls(item):
                if url not in seen:
                    seen.add(url)
                    normalized.append({"url": url})
            continue
        if isinstance(item, (list, tuple)) and len(item) >= 2:
            title, url = item[0], item[1]
            if isinstance(url, str) and url.startswith(("http://", "https://")) and url not in seen:
                seen.add(url)
                entry: dict[str, Any] = {"url": url}
                if isinstance(title, str) and title.strip():
                    entry["title"] = title.strip()
                normalized.append(entry)
            continue
        if isinstance(item, dict):
            url = item.get("url") or item.get("href") or item.get("link")
            if not isinstance(url, str) or not url.startswith(("http://", "https://")) or url in seen:
                continue
            seen.add(url)
            entry = {"url": url}
            title = item.get("title") or item.get("name") or item.get("label")
            if isinstance(title, str) and title.strip():
                entry["title"] = title.strip()
            desc = item.get("description") or item.get("snippet") or item.get("content")
            if isinstance(desc, str) and desc.strip():
                entry["description"] = desc.strip()
            normalized.append(entry)
    return normalized


def _extract_sources_from_text(text: str) -> list[dict[str, Any]]:
    sources: list[dict[str, Any]] = []
    seen: set[str] = set()
    for title, url in _MD_LINK_PATTERN.findall(text or ""):
        normalized_url = (url or "").strip()
        if not normalized_url or normalized_url in seen:
            continue
        seen.add(normalized_url)
        normalized_title = (title or "").strip()
        sources.append({"title": normalized_title, "url": normalized_url} if normalized_title else {"url": normalized_url})

    for url in extract_unique_urls(text or ""):
        if url not in seen:
            seen.add(url)
            sources.append({"url": url})
    return sources
