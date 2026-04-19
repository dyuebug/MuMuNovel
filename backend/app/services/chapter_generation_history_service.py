from __future__ import annotations

import json
import re
from datetime import datetime
from typing import Any, Dict, List, Optional, Tuple

from fastapi import HTTPException
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.models.chapter_draft_attempt import ChapterDraftAttempt
from app.models.generation_history import GenerationHistory
from app.schemas.generation_payload import build_chapter_generation_quality_history_payload
from app.services.story_quality_feedback_service import build_story_continuity_preflight


def parse_reviser_result_from_history(generated_content: Optional[str]) -> Optional[dict[str, Any]]:
    if not generated_content:
        return None
    try:
        payload = json.loads(generated_content)
        if not isinstance(payload, dict):
            return None
        if payload.get("log_type") != "chapter_text_reviser_v1":
            return None
        reviser_result = payload.get("reviser_result")
        if isinstance(reviser_result, dict):
            return reviser_result
    except Exception:
        return None
    return None




def require_candidate_draft_full_content(draft_attempt: ChapterDraftAttempt) -> str:
    candidate_content_raw, has_full_content = _extract_candidate_draft_full_content(draft_attempt)
    if not has_full_content or not candidate_content_raw.strip():
        raise HTTPException(status_code=409, detail="当前候选草稿缺少可回放的完整内容")
    return candidate_content_raw

def is_reviser_draft_stale(
    chapter_updated_at: Optional[datetime],
    draft_created_at: Optional[datetime],
) -> bool:
    if not chapter_updated_at or not draft_created_at:
        return False
    return chapter_updated_at > draft_created_at


def build_auto_revision_draft_payload(
    *,
    reviser_result: dict[str, Any],
    history_id: Optional[str],
    created_at: Optional[datetime],
    chapter_updated_at: Optional[datetime],
    include_full_text: bool = False,
) -> dict[str, Any]:
    revised_text = str(reviser_result.get("revised_text") or "")
    revised_text_preview = str(reviser_result.get("revised_text_preview") or "").strip()
    if not revised_text_preview and revised_text:
        revised_text_preview = revised_text[:500]

    critical_count = int(reviser_result.get("critical_count") or 0)
    major_count = int(reviser_result.get("major_count") or 0)
    priority_issue_count = int(
        reviser_result.get("priority_issue_count") or (critical_count + major_count)
    )
    applied_issue_count = int(
        reviser_result.get("applied_issue_count")
        or reviser_result.get("applied_critical_count")
        or 0
    )

    payload: dict[str, Any] = {
        "history_id": history_id,
        "critical_count": critical_count,
        "major_count": major_count,
        "priority_issue_count": priority_issue_count,
        "applied_critical_count": int(
            reviser_result.get("applied_critical_count") or applied_issue_count
        ),
        "applied_issue_count": applied_issue_count,
        "change_summary": reviser_result.get("change_summary"),
        "revised_word_count": reviser_result.get("revised_word_count", len(revised_text)),
        "unresolved_issues": reviser_result.get("unresolved_issues", []),
        "revised_text_preview": revised_text_preview,
        "has_full_text": bool(revised_text),
        "is_stale": is_reviser_draft_stale(chapter_updated_at, created_at),
        "created_at": created_at.isoformat() if created_at else None,
    }
    if include_full_text:
        payload["revised_text"] = revised_text
    return payload


def build_reviser_apply_history_payload(
    *,
    source_history_id: str,
    source_created_at: Optional[datetime],
    critical_count: int,
    major_count: int,
    priority_issue_count: int,
    applied_critical_count: int,
    applied_issue_count: int,
    old_word_count: int,
    new_word_count: int,
    stale_applied: bool,
    allow_stale: bool,
) -> str:
    payload = {
        "log_type": "chapter_text_reviser_apply_v1",
        "source_history_id": source_history_id,
        "source_created_at": source_created_at.isoformat() if source_created_at else None,
        "critical_count": critical_count,
        "major_count": major_count,
        "priority_issue_count": priority_issue_count,
        "applied_critical_count": applied_critical_count,
        "applied_issue_count": applied_issue_count,
        "old_word_count": old_word_count,
        "new_word_count": new_word_count,
        "stale_applied": stale_applied,
        "allow_stale": allow_stale,
        "applied_at": datetime.now().isoformat(),
    }
    return json.dumps(payload, ensure_ascii=False)


def build_generation_history_payload(
    content: str,
    metrics: Optional[Dict[str, Any]],
    *,
    content_applied: bool = True,
    attempt_state: Optional[str] = None,
    story_runtime_contract: Optional[Dict[str, Any]] = None,
) -> str:
    payload = build_chapter_generation_quality_history_payload(
        content,
        metrics,
        content_applied=content_applied,
        attempt_state=attempt_state,
        story_runtime_contract=story_runtime_contract,
    )
    return payload.model_dump_json(exclude_none=True)


def parse_checker_result_from_history(generated_content: Optional[str]) -> Optional[dict[str, Any]]:
    if not generated_content:
        return None
    try:
        payload = json.loads(generated_content)
        if not isinstance(payload, dict):
            return None
        if payload.get("log_type") != "chapter_text_checker_v1":
            return None
        checker_result = payload.get("checker_result")
        if isinstance(checker_result, dict):
            return checker_result
    except Exception:
        return None
    return None


async def load_latest_reviser_history(
    db: AsyncSession,
    chapter_id: str,
    history_id: Optional[str] = None,
    scan_limit: int = 60,
) -> Optional[tuple[GenerationHistory, dict[str, Any]]]:
    if history_id:
        result = await db.execute(
            select(GenerationHistory).where(
                GenerationHistory.id == history_id,
                GenerationHistory.chapter_id == chapter_id,
            )
        )
        history = result.scalar_one_or_none()
        if not history:
            return None
        parsed = parse_reviser_result_from_history(history.generated_content)
        if not parsed:
            return None
        return history, parsed

    result = await db.execute(
        select(GenerationHistory)
        .where(GenerationHistory.chapter_id == chapter_id)
        .order_by(GenerationHistory.created_at.desc())
        .limit(scan_limit)
    )
    histories = result.scalars().all()
    for history in histories:
        parsed = parse_reviser_result_from_history(history.generated_content)
        if parsed:
            return history, parsed
    return None


def _normalize_candidate_draft_items(values: Any, *, limit: int = 4) -> List[str]:
    if values is None:
        return []
    if isinstance(values, str):
        raw_items = [values]
    elif isinstance(values, (list, tuple, set)):
        raw_items = list(values)
    else:
        raw_items = [values]

    items: List[str] = []
    seen: set[str] = set()
    for value in raw_items:
        text = str(value or "").strip()
        if not text or text in seen:
            continue
        seen.add(text)
        items.append(text)
        if len(items) >= limit:
            break
    return items


def _collect_candidate_runtime_items(runtime_context: Dict[str, Any], keys: Tuple[str, ...], *, limit: int = 6) -> List[str]:
    items: List[str] = []
    seen: set[str] = set()
    for key in keys:
        for item in _normalize_candidate_draft_items(runtime_context.get(key), limit=limit):
            if item in seen:
                continue
            seen.add(item)
            items.append(item)
            if len(items) >= limit:
                return items
    return items


def _normalize_candidate_quality_match_text(value: Any) -> str:
    return re.sub(r"\s+", "", str(value or "")).lower()


_CANDIDATE_QUALITY_STOPWORDS = {
    "the", "and", "for", "with", "from", "that", "this", "into", "onto", "over", "under",
    "still", "now", "then", "than", "are", "was", "were", "is", "be", "been", "being",
    "have", "has", "had", "who", "whom", "whose", "will", "would", "shall", "should",
    "can", "could", "may", "might", "must", "just", "only", "very", "more", "most",
    "less", "least", "not", "out", "off", "our", "your", "their", "his", "her", "its",
}

_CANDIDATE_QUALITY_TRANSLATION = str.maketrans({
    "\u3010": " ",
    "\u3011": " ",
    "[": " ",
    "]": " ",
    "\uff08": " ",
    "\uff09": " ",
    "(": " ",
    ")": " ",
    "<": " ",
    ">": " ",
    "\u300a": " ",
    "\u300b": " ",
    '"': " ",
    "'": " ",
    "`": " ",
    ":": " ",
    "\uff1a": " ",
    "\uff0c": " ",
    ",": " ",
    "\u3002": " ",
    ";": " ",
    "\uff1b": " ",
    "!": " ",
    "\uff01": " ",
    "?": " ",
    "\uff1f": " ",
})


def _append_candidate_quality_token(tokens: List[str], seen: set[str], token: str, *, max_tokens: int) -> None:
    normalized = token.strip().lower()
    if len(normalized) < 2 or normalized in seen or normalized in _CANDIDATE_QUALITY_STOPWORDS or len(tokens) >= max_tokens:
        return
    seen.add(normalized)
    tokens.append(normalized)



def _tokenize_candidate_quality_text(value: Any, *, max_tokens: int = 24) -> List[str]:
    cleaned = str(value or "").translate(_CANDIDATE_QUALITY_TRANSLATION)
    tokens: List[str] = []
    seen: set[str] = set()
    for raw_token in re.findall(r"[A-Za-z0-9_\-]{2,}|[\u4e00-\u9fff]{2,12}", cleaned):
        _append_candidate_quality_token(tokens, seen, raw_token, max_tokens=max_tokens)
        if re.fullmatch(r"[\u4e00-\u9fff]{3,12}", raw_token):
            for window_size in (2, 3, 4):
                if len(raw_token) < window_size:
                    continue
                for start in range(0, len(raw_token) - window_size + 1):
                    _append_candidate_quality_token(
                        tokens,
                        seen,
                        raw_token[start:start + window_size],
                        max_tokens=max_tokens,
                    )
                    if len(tokens) >= max_tokens:
                        break
                if len(tokens) >= max_tokens:
                    break
        if len(tokens) >= max_tokens:
            break
    return tokens




def _extract_candidate_quality_anchor_candidates(item: Any) -> List[str]:
    text = str(item or "").strip()
    if not text:
        return []

    parts = [part.strip() for part in re.split(r"[:：]", text, maxsplit=1) if part.strip()]
    head = parts[0] if parts else text
    tail = parts[1] if len(parts) > 1 else ""
    segments = [
        segment.strip()
        for segment in re.split(r"[、,\/|&＆和与+·•]+", head)
        if segment.strip()
    ]
    if tail:
        tail_clauses = [
            clause.strip()
            for clause in re.split(r"[，,。；;]", tail)
            if clause.strip()
        ]
        segments.extend(tail_clauses[:2])
    if not segments:
        segments = [text]

    anchors: List[str] = []
    seen: set[str] = set()
    for segment in segments[:4]:
        for token in _tokenize_candidate_quality_text(segment, max_tokens=10):
            if token in seen:
                continue
            seen.add(token)
            anchors.append(token)
            if len(anchors) >= 6:
                return anchors
    if anchors:
        return anchors

    fallback = _normalize_candidate_quality_match_text(head)
    return [fallback] if len(fallback) >= 2 else []



def _extract_candidate_quality_item_head(item: Any) -> str:
    text = str(item or "").strip()
    if not text:
        return ""
    parts = [part.strip() for part in re.split(r"[:：]", text, maxsplit=1) if part.strip()]
    return parts[0] if parts else text



def _candidate_quality_items_overlap(left: str, right: str) -> bool:
    left_text = str(left or "").strip()
    right_text = str(right or "").strip()
    if not left_text or not right_text:
        return False
    if left_text == right_text:
        return True

    left_head = _normalize_candidate_quality_match_text(_extract_candidate_quality_item_head(left_text))
    right_head = _normalize_candidate_quality_match_text(_extract_candidate_quality_item_head(right_text))
    if left_head and right_head and (left_head in right_head or right_head in left_head):
        return True

    left_tokens = set(_extract_candidate_quality_anchor_candidates(left_text)[:4])
    right_tokens = set(_extract_candidate_quality_anchor_candidates(right_text)[:4])
    if len(left_tokens) >= 2 and len(right_tokens) >= 2:
        return len(left_tokens & right_tokens) >= min(len(left_tokens), len(right_tokens))
    return False



def _merge_candidate_quality_items(values: Any, *, limit: int = 4) -> List[str]:
    items = _normalize_candidate_draft_items(values, limit=max(limit * 4, 12))
    merged: List[str] = []
    for item in items:
        overlap_index = next(
            (index for index, existing in enumerate(merged) if _candidate_quality_items_overlap(existing, item)),
            None,
        )
        if overlap_index is None:
            merged.append(item)
            continue
        if len(item) > len(merged[overlap_index]):
            merged[overlap_index] = item
    return merged[:limit]


def _split_candidate_quality_content_segments(content: str, *, limit: int = 24) -> List[str]:
    segments: List[str] = []
    seen: set[str] = set()
    paragraphs = [part.strip() for part in re.split(r"(?:\r?\n)+", str(content or "")) if part.strip()]
    for paragraph in paragraphs or [str(content or "").strip()]:
        sentence_parts = [
            part.strip()
            for part in re.split(r"(?<=[\u3002\uff01\uff1f!?\uff1b;])", paragraph)
            if part.strip()
        ]
        if not sentence_parts:
            sentence_parts = [paragraph]
        for index, part in enumerate(sentence_parts):
            candidates = [part]
            if index + 1 < len(sentence_parts):
                candidates.append(f"{part} {sentence_parts[index + 1]}")
            for candidate in candidates:
                compact = re.sub(r"\s+", " ", candidate).strip()
                if not compact or compact in seen:
                    continue
                seen.add(compact)
                segments.append(compact)
                if len(segments) >= limit:
                    return segments
    return segments[:limit]



def _truncate_candidate_quality_snippet(snippet: str, *, focus: Optional[str] = None, max_chars: int = 96) -> str:
    compact = re.sub(r"\s+", " ", str(snippet or "")).strip()
    if len(compact) <= max_chars:
        return compact
    if focus:
        focus_index = compact.lower().find(str(focus).lower())
        if focus_index >= 0:
            start = max(0, focus_index - 24)
            end = min(len(compact), focus_index + len(str(focus)) + 36)
            prefix = "\u2026" if start > 0 else ""
            suffix = "\u2026" if end < len(compact) else ""
            return prefix + compact[start:end] + suffix
    return compact[: max_chars - 1] + "\u2026"




def _build_candidate_quality_segment_contexts(content_segments: List[str]) -> List[Dict[str, Any]]:
    contexts: List[Dict[str, Any]] = []
    for segment in content_segments:
        normalized_segment = _normalize_candidate_quality_match_text(segment)
        if not normalized_segment:
            continue
        contexts.append(
            {
                "raw": segment,
                "normalized": normalized_segment,
                "tokens": set(_tokenize_candidate_quality_text(segment, max_tokens=18)),
            }
        )
    return contexts



def _evaluate_candidate_quality_item_match(
    *,
    item: str,
    anchors: List[str],
    normalized_content: str,
    content_tokens: set[str],
    content_segment_contexts: List[Dict[str, Any]],
) -> Dict[str, Any]:
    required_match_count = 2 if len(anchors) >= 4 else 1
    if len(anchors) >= 2 and any(sep in item for sep in ('/', '&', '\u4e0e', '\u548c')):
        required_match_count = max(required_match_count, 2)
    exact_hits = [anchor for anchor in anchors if len(anchor) >= 2 and anchor in normalized_content]
    token_hits = [anchor for anchor in anchors if anchor in content_tokens]

    best_segment = ""
    best_focus = None
    best_anchors: List[str] = []
    best_score = -1.0
    for segment_context in content_segment_contexts:
        normalized_segment = str(segment_context.get("normalized") or "")
        if not normalized_segment:
            continue
        segment_exact_hits = [anchor for anchor in anchors if len(anchor) >= 2 and anchor in normalized_segment]
        segment_tokens = segment_context.get("tokens")
        if not isinstance(segment_tokens, set):
            segment_tokens = set()
        segment_token_hits = [anchor for anchor in anchors if anchor in segment_tokens]
        score = float(len(segment_exact_hits) * 4 + len(segment_token_hits))
        if score <= 0:
            continue
        if score > best_score:
            best_score = score
            best_segment = str(segment_context.get("raw") or "")
            best_anchors = segment_exact_hits or segment_token_hits
            best_focus = best_anchors[0] if best_anchors else None

    matched = len(set(exact_hits)) >= required_match_count
    if not matched:
        semantic_threshold = max(required_match_count, 2 if len(anchors) >= 2 else 1)
        matched = len(set(token_hits)) >= semantic_threshold

    return {
        "matched": matched,
        "matched_anchors": list(dict.fromkeys(exact_hits or token_hits))[:3],
        "snippet": _truncate_candidate_quality_snippet(best_segment, focus=best_focus) if matched and best_segment else None,
    }


def _split_candidate_quality_item_matches(
    content: str,
    items: List[str],
    *,
    limit: int = 3,
) -> Tuple[List[str], List[str], List[Dict[str, Any]]]:
    normalized_content = _normalize_candidate_quality_match_text(content)
    if not normalized_content:
        return [], [], []

    content_tokens = set(_tokenize_candidate_quality_text(content, max_tokens=48))
    content_segments = _split_candidate_quality_content_segments(content)
    content_segment_contexts = _build_candidate_quality_segment_contexts(content_segments)
    matched_items: List[str] = []
    missing_items: List[str] = []
    matched_evidence: List[Dict[str, Any]] = []
    for item in items:
        anchors = _extract_candidate_quality_anchor_candidates(item)
        match_result = _evaluate_candidate_quality_item_match(
            item=item,
            anchors=anchors,
            normalized_content=normalized_content,
            content_tokens=content_tokens,
            content_segment_contexts=content_segment_contexts,
        )
        if match_result["matched"]:
            matched_items.append(item)
            if match_result.get("snippet"):
                matched_evidence.append(
                    {
                        "item": item,
                        "snippet": match_result["snippet"],
                        "matched_anchors": match_result.get("matched_anchors") or [],
                    }
                )
        else:
            missing_items.append(item)
        if len(matched_items) >= limit and len(missing_items) >= limit:
            break
    return matched_items[:limit], missing_items[:limit], matched_evidence[:limit]



def _build_candidate_draft_quality_highlights(
    *,
    content: str,
    quality_metrics: Dict[str, Any],
) -> Dict[str, Any]:
    normalized_content = str(content or "").strip()
    if not normalized_content or not isinstance(quality_metrics, dict):
        return {}

    runtime_context = (
        dict(quality_metrics.get("quality_runtime_context"))
        if isinstance(quality_metrics.get("quality_runtime_context"), dict)
        else {}
    )
    continuity_preflight = (
        dict(quality_metrics.get("continuity_preflight"))
        if isinstance(quality_metrics.get("continuity_preflight"), dict)
        else {}
    )
    if not continuity_preflight and runtime_context:
        continuity_preflight = build_story_continuity_preflight(normalized_content, runtime_context)

    continuity_items = _collect_candidate_runtime_items(
        runtime_context,
        (
            "character_state_ledger",
            "relationship_state_ledger",
            "foreshadow_state_ledger",
            "organization_state_ledger",
            "career_state_ledger",
        ),
        limit=6,
    )
    continuity_matched, continuity_missing, continuity_evidence = _split_candidate_quality_item_matches(normalized_content, continuity_items)
    warning_items = _normalize_candidate_draft_items(
        [warning.get("item") for warning in (continuity_preflight.get("warnings") or []) if isinstance(warning, dict)],
        limit=3,
    )
    if warning_items:
        continuity_missing = _merge_candidate_quality_items(warning_items + continuity_missing, limit=3)
    continuity_summary = str(continuity_preflight.get("summary") or "").strip()
    continuity_status = str(continuity_preflight.get("status") or "").strip().lower()
    if not continuity_status:
        continuity_status = "warning" if continuity_missing else ("ok" if continuity_matched else "unknown")
    if not continuity_summary:
        if continuity_missing:
            continuity_summary = f"\u5019\u9009\u7a3f\u4ecd\u6709 {len(continuity_missing)} \u9879\u8fde\u7eed\u6027\u63a5\u529b\u5f85\u8865\u9f50\u3002"
        elif continuity_matched:
            continuity_summary = "\u5019\u9009\u7a3f\u5df2\u7ecf\u63a5\u4f4f\u5f53\u524d\u8fde\u7eed\u6027\u8d26\u672c\u4e2d\u7684\u5173\u952e\u9879\u3002"

    foreshadow_payoff_delay = (
        dict(quality_metrics.get("foreshadow_payoff_delay"))
        if isinstance(quality_metrics.get("foreshadow_payoff_delay"), dict)
        else {}
    )
    foreshadow_items = _collect_candidate_runtime_items(
        runtime_context,
        ("foreshadow_payoff_plan", "foreshadow_state_ledger"),
        limit=6,
    )
    foreshadow_matched, foreshadow_missing, foreshadow_evidence = _split_candidate_quality_item_matches(normalized_content, foreshadow_items)
    foreshadow_summary = str(foreshadow_payoff_delay.get("summary") or "").strip()
    foreshadow_status = str(foreshadow_payoff_delay.get("status") or "").strip().lower()
    if not foreshadow_status:
        foreshadow_status = "warning" if foreshadow_missing else ("stable" if foreshadow_matched else "unknown")
    if not foreshadow_summary:
        if foreshadow_missing:
            foreshadow_summary = f"\u5019\u9009\u7a3f\u4ecd\u6709 {len(foreshadow_missing)} \u9879\u4f0f\u7b14/\u627f\u8bfa\u5f85\u5151\u73b0\u3002"
        elif foreshadow_matched:
            foreshadow_summary = "\u5019\u9009\u7a3f\u5df2\u7ecf\u8986\u76d6\u5f53\u524d\u4f18\u5148\u5151\u73b0\u7684\u4f0f\u7b14\u9879\u3002"

    highlights: Dict[str, Any] = {}
    if continuity_summary or continuity_matched or continuity_missing:
        highlights["continuity"] = {
            "status": continuity_status or "unknown",
            "summary": continuity_summary or None,
            "matched_items": continuity_matched,
            "missing_items": continuity_missing,
            "repair_targets": _normalize_candidate_draft_items(continuity_preflight.get("repair_targets"), limit=3),
            "matched_evidence": continuity_evidence,
        }
    if foreshadow_summary or foreshadow_matched or foreshadow_missing:
        highlights["foreshadow"] = {
            "status": foreshadow_status or "unknown",
            "summary": foreshadow_summary or None,
            "matched_items": foreshadow_matched,
            "missing_items": foreshadow_missing,
            "repair_targets": _normalize_candidate_draft_items(foreshadow_payoff_delay.get("repair_targets"), limit=3),
            "matched_evidence": foreshadow_evidence,
        }
    return highlights


def _extract_candidate_draft_full_content(draft_attempt: ChapterDraftAttempt) -> Tuple[str, bool]:
    repair_payload = draft_attempt.repair_payload if isinstance(draft_attempt.repair_payload, dict) else {}
    full_content = str(repair_payload.get("candidate_full_content") or "").strip()
    if full_content:
        return full_content, True

    preview_content = str(draft_attempt.content_preview or "").strip()
    if not preview_content:
        return "", False

    if bool(repair_payload.get("content_complete")):
        return preview_content, True

    word_count = int(draft_attempt.word_count or 0)
    if word_count > 0 and len(preview_content) == word_count:
        return preview_content, True

    return "", False


def _build_candidate_draft_apply_risk(
    *,
    quality_gate: Dict[str, Any],
    quality_highlights: Dict[str, Any],
    quality_gate_action: Optional[str],
    quality_gate_decision: Optional[str],
) -> Optional[Dict[str, Any]]:
    items: List[str] = []

    continuity = quality_highlights.get("continuity") if isinstance(quality_highlights.get("continuity"), dict) else {}
    continuity_missing = _normalize_candidate_draft_items(continuity.get("missing_items"), limit=3)
    if continuity_missing:
        items.append(f"连续性待补齐：{'；'.join(continuity_missing)}")

    foreshadow = quality_highlights.get("foreshadow") if isinstance(quality_highlights.get("foreshadow"), dict) else {}
    foreshadow_missing = _normalize_candidate_draft_items(foreshadow.get("missing_items"), limit=3)
    if foreshadow_missing:
        items.append(f"伏笔/回收待补齐：{'；'.join(foreshadow_missing)}")

    failed_metric_labels = _normalize_candidate_draft_items(
        [item.get("label") for item in (quality_gate.get("failed_metrics") or []) if isinstance(item, dict)],
        limit=3,
    )
    if failed_metric_labels:
        items.append(f"质量门禁关注项：{'；'.join(failed_metric_labels)}")

    quality_gate_status = str(quality_gate.get("status") or "").strip().lower()
    normalized_action = str(quality_gate_action or "").strip().lower()
    normalized_decision = str(quality_gate_decision or quality_gate.get("decision") or "").strip().lower()
    if not items and (
        quality_gate_status in {"warning", "blocked"}
        or normalized_action in {"manual_review", "auto_repair"}
        or normalized_decision in {"manual_review", "auto_repair"}
    ):
        items.append("当前候选稿仍建议先做一致性复核，再决定是否直接恢复。")

    if not items:
        return None

    return {
        "status": "warning",
        "summary": "恢复前请先确认这些一致性 / 质量风险是否可接受。",
        "items": items[:4],
    }


def _build_candidate_draft_payload(
    *,
    draft_attempt: ChapterDraftAttempt,
    chapter_updated_at: Optional[datetime],
    include_full_text: bool = False,
) -> Dict[str, Any]:
    quality_metrics = dict(draft_attempt.quality_metrics or {}) if isinstance(draft_attempt.quality_metrics, dict) else {}
    repair_payload = dict(draft_attempt.repair_payload or {}) if isinstance(draft_attempt.repair_payload, dict) else {}
    quality_gate = quality_metrics.get("quality_gate") if isinstance(quality_metrics.get("quality_gate"), dict) else {}
    repair_guidance = quality_metrics.get("repair_guidance") if isinstance(quality_metrics.get("repair_guidance"), dict) else {}
    selection_metadata = quality_metrics.get("candidate_selection") if isinstance(quality_metrics.get("candidate_selection"), dict) else None
    full_content, has_full_content = _extract_candidate_draft_full_content(draft_attempt)

    preview_text = str(draft_attempt.content_preview or draft_attempt.summary_preview or "").strip()
    if not preview_text and full_content:
        preview_text = full_content[:500]

    failed_metrics: List[Dict[str, Any]] = []
    for item in quality_gate.get("failed_metrics") or []:
        if not isinstance(item, dict):
            continue
        failed_metrics.append(
            {
                "key": str(item.get("key") or "").strip(),
                "label": str(item.get("label") or item.get("key") or "").strip(),
                "value": float(item.get("value") or 0.0),
                "threshold": float(item.get("threshold") or 0.0),
                "gap": float(item.get("gap") or 0.0),
                "focus_area": str(item.get("focus_area") or "").strip() or None,
                "repair_target": str(item.get("repair_target") or "").strip() or None,
            }
        )

    highlight_content = full_content if has_full_content else ""
    quality_highlights = _build_candidate_draft_quality_highlights(
        content=highlight_content,
        quality_metrics=quality_metrics,
    )
    apply_risk = _build_candidate_draft_apply_risk(
        quality_gate=quality_gate,
        quality_highlights=quality_highlights,
        quality_gate_action=draft_attempt.quality_gate_action,
        quality_gate_decision=draft_attempt.quality_gate_decision,
    )

    payload: Dict[str, Any] = {
        "attempt_id": draft_attempt.id,
        "source": str(draft_attempt.source or "").strip(),
        "attempt_state": str(draft_attempt.attempt_state or "").strip(),
        "quality_gate_action": draft_attempt.quality_gate_action,
        "quality_gate_decision": draft_attempt.quality_gate_decision,
        "word_count": int(draft_attempt.word_count or len(full_content)),
        "summary_preview": str(draft_attempt.summary_preview or "").strip(),
        "content_preview": preview_text,
        "has_full_content": has_full_content,
        "content_complete": has_full_content,
        "can_apply": has_full_content,
        "is_stale": is_reviser_draft_stale(chapter_updated_at, draft_attempt.created_at),
        "created_at": draft_attempt.created_at.isoformat() if draft_attempt.created_at else None,
        "repair_summary": str(
            repair_payload.get("summary")
            or repair_guidance.get("summary")
            or ""
        ).strip() or None,
        "repair_targets": _normalize_candidate_draft_items(
            repair_payload.get("repair_targets") or repair_guidance.get("repair_targets")
        ),
        "preserve_strengths": _normalize_candidate_draft_items(
            repair_payload.get("preserve_strengths") or repair_guidance.get("preserve_strengths")
        ),
        "focus_areas": _normalize_candidate_draft_items(
            repair_guidance.get("focus_areas") or quality_gate.get("focus_areas")
        ),
        "failed_metrics": failed_metrics,
        "candidate_selection": dict(selection_metadata) if isinstance(selection_metadata, dict) else None,
        "quality_highlights": quality_highlights or None,
        "apply_risk": apply_risk,
    }
    if include_full_text and has_full_content:
        payload["content"] = full_content
    return payload

async def _load_latest_candidate_draft_attempt(
    db: AsyncSession,
    chapter_id: str,
    attempt_id: Optional[str] = None,
) -> Optional[ChapterDraftAttempt]:
    query = select(ChapterDraftAttempt).where(ChapterDraftAttempt.chapter_id == chapter_id)
    if attempt_id:
        query = query.where(ChapterDraftAttempt.id == attempt_id)
    else:
        query = query.order_by(ChapterDraftAttempt.created_at.desc()).limit(1)
    result = await db.execute(query)
    return result.scalar_one_or_none()

