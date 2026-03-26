from __future__ import annotations

from typing import Any, Dict, Mapping, Optional, Sequence


QUALITY_GATE_DECISION_PRIORITY = {
    "allow_save": 3,
    "auto_repair": 2,
    "manual_review": 1,
}


def _safe_float(value: Any) -> float:
    try:
        if value is None:
            return 0.0
        return float(value)
    except (TypeError, ValueError):
        return 0.0


def _safe_text(value: Any) -> str:
    return str(value or "").strip()


def _normalize_items(values: Any, *, limit: int = 4) -> list[str]:
    if values is None:
        return []
    if isinstance(values, str):
        raw_items = [values]
    elif isinstance(values, Sequence) and not isinstance(values, (str, bytes, bytearray)):
        raw_items = list(values)
    else:
        raw_items = [values]

    items: list[str] = []
    seen: set[str] = set()
    for value in raw_items:
        text = _safe_text(value)
        if not text or text in seen:
            continue
        seen.add(text)
        items.append(text)
        if len(items) >= limit:
            break
    return items


def _extract_runtime_context(
    quality_metrics: Optional[Mapping[str, Any]],
) -> Dict[str, Any]:
    if not isinstance(quality_metrics, Mapping):
        return {}
    runtime_context = quality_metrics.get("quality_runtime_context")
    return dict(runtime_context) if isinstance(runtime_context, Mapping) else {}


def _build_focus_strategy_lines(runtime_context: Mapping[str, Any]) -> list[str]:
    story_focus = _safe_text(runtime_context.get("story_focus"))
    creative_mode = _safe_text(runtime_context.get("creative_mode"))
    quality_preset = _safe_text(runtime_context.get("quality_preset"))

    lines: list[str] = []
    if story_focus == "advance_plot":
        lines.append("- Rebuild the scene around visible objectives, resistance, and a changed situation.")
    elif story_focus == "deepen_character":
        lines.append("- Surface the protagonist's tradeoff through decisive action and emotional aftershock.")
    elif story_focus == "escalate_conflict":
        lines.append("- Make the opposition push back harder and force a more costly next move.")
    elif story_focus == "reveal_mystery":
        lines.append("- Introduce one concrete clue while preserving a sharper unanswered question.")
    elif story_focus == "relationship_shift":
        lines.append("- Let dialogue and power balance visibly shift the relationship by scene end.")
    elif story_focus == "foreshadow_payoff":
        lines.append("- Cash out at least one prior setup with a visible consequence on the page.")

    if creative_mode in {"hook", "suspense"}:
        lines.append("- Finish on a tighter question, approaching risk, or decision under pressure.")
    elif creative_mode == "emotion":
        lines.append("- Strengthen nonverbal reactions, hesitation, and emotional recoil instead of explanation.")
    elif creative_mode == "relationship":
        lines.append("- Increase push-pull dialogue and protect each character's distinct voice.")
    elif creative_mode == "payoff":
        lines.append("- Emphasize setup -> action -> feedback so the scene lands with payoff, not summary.")

    if quality_preset == "plot_drive":
        lines.append("- Prefer sharper action-counteraction beats over extra exposition.")
    elif quality_preset == "immersive":
        lines.append("- Add sensory anchors at the decisive beats without stalling the scene.")
    elif quality_preset == "emotion_drama":
        lines.append("- Sharpen subtext, contradiction, and relational tension in dialogue.")
    elif quality_preset == "clean_prose":
        lines.append("- Cut repeated explanation and keep sentence rhythm cleaner and tighter.")

    return lines


def build_candidate_retry_prompt_suffix(
    quality_gate_plan: Optional[Mapping[str, Any]],
    *,
    attempt_index: int,
) -> str:
    if not isinstance(quality_gate_plan, Mapping):
        return ""

    quality_gate = quality_gate_plan.get("quality_gate")
    active_payload = quality_gate_plan.get("active_story_repair_payload")
    payload = active_payload if isinstance(active_payload, Mapping) else {}

    summary = _safe_text(payload.get("summary") or quality_gate_plan.get("message"))
    repair_targets = _normalize_items(payload.get("repair_targets"), limit=3)
    preserve_strengths = _normalize_items(payload.get("preserve_strengths"), limit=2)
    failed_metric_labels = [
        _safe_text(item.get("label"))
        for item in (quality_gate.get("failed_metrics") or [])
        if isinstance(quality_gate, Mapping) and isinstance(item, Mapping) and _safe_text(item.get("label"))
    ][:3] if isinstance(quality_gate, Mapping) else []
    recommended_action = (
        _safe_text(quality_gate.get("recommended_action_label") or quality_gate.get("recommended_action"))
        if isinstance(quality_gate, Mapping)
        else ""
    )

    lines = [
        f"Revision attempt #{attempt_index}",
        "- Keep the narrative voice, continuity, and established facts intact.",
        "- Repair the weak spots identified by the quality gate before finalizing.",
    ]
    if summary:
        lines.append(f"- Focus summary: {summary}")
    if failed_metric_labels:
        lines.append(f"- Failed metrics: {' / '.join(failed_metric_labels)}")
    if repair_targets:
        lines.append(f"- Repair targets: {' / '.join(repair_targets)}")
    if preserve_strengths:
        lines.append(f"- Preserve strengths: {' / '.join(preserve_strengths)}")
    if recommended_action:
        lines.append(f"- Recommended action: {recommended_action}")
    return "\n".join(lines)


def build_candidate_retry_strategy_suffix(
    quality_gate_plan: Optional[Mapping[str, Any]],
    *,
    quality_metrics: Optional[Mapping[str, Any]] = None,
    attempt_index: int,
    source: str,
) -> str:
    runtime_context = _extract_runtime_context(quality_metrics)
    quality_gate = quality_gate_plan.get("quality_gate") if isinstance(quality_gate_plan, Mapping) else None
    failed_metric_labels = [
        _safe_text(item.get("label"))
        for item in (quality_gate.get("failed_metrics") or [])
        if isinstance(quality_gate, Mapping) and isinstance(item, Mapping) and _safe_text(item.get("label"))
    ][:3] if isinstance(quality_gate, Mapping) else []

    lines = [
        f"Alternative candidate strategy #{attempt_index}",
        "- Recast the same chapter intent with a visibly different scene progression, not just local word swaps.",
        f"- Keep the same target outcome for this {source} draft while varying scene sequencing and emphasis.",
    ]
    if failed_metric_labels:
        lines.append(f"- Counter the weak metrics through scene design: {' / '.join(failed_metric_labels)}")
    lines.extend(_build_focus_strategy_lines(runtime_context))
    return "\n".join(lines)


def resolve_candidate_retry_temperature(
    base_temperature: float,
    *,
    quality_metrics: Optional[Mapping[str, Any]] = None,
    quality_gate_plan: Optional[Mapping[str, Any]] = None,
    attempt_index: int,
) -> float:
    runtime_context = _extract_runtime_context(quality_metrics)
    quality_preset = _safe_text(runtime_context.get("quality_preset"))
    creative_mode = _safe_text(runtime_context.get("creative_mode"))
    quality_gate = quality_gate_plan.get("quality_gate") if isinstance(quality_gate_plan, Mapping) else None
    decision = _safe_text(quality_gate.get("decision")) if isinstance(quality_gate, Mapping) else ""

    temperature = _safe_float(base_temperature) or 0.8
    if quality_preset == "clean_prose":
        temperature -= 0.08
    elif quality_preset in {"immersive", "emotion_drama"}:
        temperature += 0.05
    elif quality_preset == "plot_drive":
        temperature += 0.02

    if creative_mode in {"hook", "suspense", "relationship", "emotion"}:
        temperature += 0.04
    elif creative_mode == "payoff":
        temperature += 0.02

    if decision == "manual_review":
        temperature += 0.03
    elif decision == "allow_save":
        temperature -= 0.02

    temperature -= max(attempt_index - 2, 0) * 0.05
    return round(max(0.45, min(temperature, 1.05)), 2)


def build_candidate_selection_metadata(
    quality_metrics: Optional[Mapping[str, Any]],
    *,
    word_count: int,
    target_word_count: int,
    candidate_index: int,
    candidate_count: int,
    source: str,
    quality_gate_plan: Optional[Mapping[str, Any]] = None,
) -> Dict[str, Any]:
    metrics = dict(quality_metrics or {})
    quality_gate = None
    if isinstance(quality_gate_plan, Mapping):
        candidate_quality_gate = quality_gate_plan.get("quality_gate")
        if isinstance(candidate_quality_gate, Mapping):
            quality_gate = dict(candidate_quality_gate)
    if quality_gate is None and isinstance(metrics.get("quality_gate"), Mapping):
        quality_gate = dict(metrics.get("quality_gate") or {})
    quality_gate = quality_gate or {}

    decision = _safe_text(quality_gate.get("decision") or "allow_save") or "allow_save"
    status = _safe_text(quality_gate.get("status") or "pass") or "pass"
    overall_score = _safe_float(metrics.get("overall_score"))
    pacing_score = _safe_float(metrics.get("pacing_score"))
    continuity_preflight = metrics.get("continuity_preflight") if isinstance(metrics.get("continuity_preflight"), Mapping) else {}
    continuity_warning_count = int(continuity_preflight.get("warning_count") or 0) if isinstance(continuity_preflight, Mapping) else 0

    normalized_target_word_count = max(int(target_word_count or 0), 1)
    normalized_word_count = max(int(word_count or 0), 0)
    word_count_delta = abs(normalized_word_count - normalized_target_word_count)
    word_count_fit_ratio = max(0.0, 1.0 - word_count_delta / normalized_target_word_count)
    word_count_fit_score = round(word_count_fit_ratio * 100.0, 1)

    decision_priority = QUALITY_GATE_DECISION_PRIORITY.get(decision, 0)
    decision_bonus = {
        "allow_save": 18.0,
        "auto_repair": 4.0,
        "manual_review": -18.0,
    }.get(decision, 0.0)

    selection_score = round(
        overall_score
        + decision_bonus
        + word_count_fit_score * 0.08
        + max(pacing_score - 7.0, 0.0) * 1.5
        - continuity_warning_count * 2.0,
        2,
    )

    return {
        "candidate_index": candidate_index,
        "candidate_count": candidate_count,
        "source": source,
        "selection_score": selection_score,
        "overall_score": round(overall_score, 1),
        "quality_gate_decision": decision,
        "quality_gate_status": status,
        "quality_gate_priority": decision_priority,
        "word_count": normalized_word_count,
        "target_word_count": normalized_target_word_count,
        "word_count_fit_score": word_count_fit_score,
        "word_count_delta": word_count_delta,
        "continuity_warning_count": continuity_warning_count,
    }


def attach_candidate_selection_metadata(
    quality_metrics: Optional[Mapping[str, Any]],
    *,
    selection_metadata: Mapping[str, Any],
) -> Dict[str, Any]:
    metrics = dict(quality_metrics or {})
    metrics["candidate_selection"] = dict(selection_metadata or {})
    return metrics


def select_best_generation_candidate(candidates: Sequence[Mapping[str, Any]]) -> Optional[Dict[str, Any]]:
    normalized_candidates = [dict(candidate) for candidate in candidates if isinstance(candidate, Mapping)]
    if not normalized_candidates:
        return None

    ranked_candidates = sorted(
        normalized_candidates,
        key=lambda candidate: (
            int(candidate.get("quality_gate_priority") or 0),
            float(candidate.get("selection_score") or 0.0),
            float(candidate.get("overall_score") or 0.0),
            float(candidate.get("word_count_fit_score") or 0.0),
            -int(candidate.get("candidate_index") or 0),
        ),
        reverse=True,
    )
    winner = dict(ranked_candidates[0])
    winner["rerank_pool_size"] = len(normalized_candidates)
    return winner


def should_generate_additional_candidate(
    candidate: Optional[Mapping[str, Any]],
    *,
    produced_candidates: int,
    max_candidates: int,
) -> bool:
    if produced_candidates >= max(int(max_candidates or 0), 1):
        return False
    if not isinstance(candidate, Mapping):
        return False
    decision = _safe_text(candidate.get("quality_gate_decision"))
    return decision != "allow_save"
