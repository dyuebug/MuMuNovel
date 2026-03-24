"""记忆重排策略 - 为生成场景提供稳定的混合排序能力。"""

from __future__ import annotations

import json
from typing import Any, Dict, Iterable, List, Optional, Sequence


DEFAULT_MEMORY_TYPE_WEIGHTS: Dict[str, float] = {
    "character_event": 1.0,
    "plot_point": 0.96,
    "hook": 0.92,
    "foreshadow": 0.9,
    "chapter_summary": 0.86,
    "dialogue": 0.76,
    "world_detail": 0.72,
    "scene": 0.68,
}


def _safe_float(value: Any, default: float = 0.0) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def _safe_int(value: Any, default: int = 0) -> int:
    try:
        return int(value)
    except (TypeError, ValueError):
        return default


def _clamp(value: float, minimum: float = 0.0, maximum: float = 1.0) -> float:
    return max(minimum, min(value, maximum))


def _normalize_names(values: Optional[Iterable[str]]) -> set[str]:
    if not values:
        return set()

    normalized: set[str] = set()
    for value in values:
        text = str(value or "").strip().lower()
        if text:
            normalized.add(text)
    return normalized


def _parse_related_names(metadata: Dict[str, Any]) -> set[str]:
    raw = metadata.get("related_characters")
    if raw is None:
        return set()

    if isinstance(raw, str):
        text = raw.strip()
        if not text:
            return set()
        if text.startswith("["):
            try:
                raw = json.loads(text)
            except json.JSONDecodeError:
                raw = [item.strip() for item in text.split(",") if item.strip()]
        else:
            raw = [text]

    if isinstance(raw, (list, tuple, set)):
        return _normalize_names(raw)

    return _normalize_names([raw])


def memory_matches_related_name(memory: Dict[str, Any], related_names: Optional[Iterable[str]]) -> bool:
    """判断记忆是否命中关注角色，用于生成上下文重排。"""
    normalized_names = _normalize_names(related_names)
    if not normalized_names:
        return False

    metadata = memory.get("metadata") or {}
    candidate_text = " ".join(
        str(part or "")
        for part in [
            memory.get("content", ""),
            metadata.get("title", ""),
            metadata.get("tags", ""),
        ]
    ).lower()

    related = _parse_related_names(metadata)
    if related.intersection(normalized_names):
        return True

    return any(name in candidate_text for name in normalized_names)


def rank_memories_for_generation(
    memories: Sequence[Dict[str, Any]],
    current_chapter: int,
    preferred_types: Optional[Sequence[str]] = None,
    related_names: Optional[Sequence[str]] = None,
    limit: Optional[int] = None,
) -> List[Dict[str, Any]]:
    """基于相似度、重要性、时序、类型和角色命中对记忆进行混合重排。"""
    preferred_type_set = {str(item).strip() for item in (preferred_types or []) if str(item).strip()}
    normalized_related_names = _normalize_names(related_names)

    ranked_items: List[tuple[float, float, float, int, int, Dict[str, Any]]] = []
    for index, memory in enumerate(memories or []):
        metadata = memory.get("metadata") or {}
        similarity = _clamp(_safe_float(memory.get("similarity"), 0.0))
        importance = _clamp(
            _safe_float(
                metadata.get("importance", metadata.get("importance_score", 0.0)),
                0.0,
            )
        )
        chapter_number = _safe_int(metadata.get("chapter_number", metadata.get("story_timeline")), 0)
        distance = max(current_chapter - chapter_number, 0) if chapter_number > 0 else max(current_chapter, 1)
        recency = 1.0 / (1.0 + (distance / 4.0))
        memory_type = str(metadata.get("memory_type") or metadata.get("type") or "").strip()
        type_weight = DEFAULT_MEMORY_TYPE_WEIGHTS.get(memory_type, 0.5)
        preferred_bonus = 0.12 if preferred_type_set and memory_type in preferred_type_set else 0.0
        character_bonus = (
            0.15 if normalized_related_names and memory_matches_related_name(memory, normalized_related_names) else 0.0
        )
        foreshadow_bonus = 0.1 if _safe_int(metadata.get("is_foreshadow"), 0) == 1 else 0.0

        score = (
            similarity * 0.48 +
            importance * 0.2 +
            recency * 0.14 +
            type_weight * 0.12 +
            preferred_bonus +
            character_bonus +
            foreshadow_bonus
        )

        enriched_memory = dict(memory)
        enriched_memory["ranking_score"] = round(score, 4)
        ranked_items.append((score, similarity, importance, chapter_number, -index, enriched_memory))

    ranked_items.sort(reverse=True)

    deduped: List[Dict[str, Any]] = []
    seen_keys: set[str] = set()
    for _, _, _, chapter_number, _, memory in ranked_items:
        metadata = memory.get("metadata") or {}
        unique_key = str(memory.get("id") or "").strip()
        if not unique_key:
            unique_key = "|".join(
                [
                    str(metadata.get("title") or "").strip(),
                    str(metadata.get("memory_type") or metadata.get("type") or "").strip(),
                    str(chapter_number),
                    str(memory.get("content") or "").strip()[:120],
                ]
            )
        if unique_key in seen_keys:
            continue
        seen_keys.add(unique_key)
        deduped.append(memory)
        if limit and len(deduped) >= limit:
            break

    return deduped
