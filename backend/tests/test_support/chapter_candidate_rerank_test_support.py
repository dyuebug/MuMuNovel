"""Test-only chapter candidate rerank support migrated out of app/services."""
from __future__ import annotations

from typing import Any, Dict, Mapping, Optional, Sequence


QUALITY_GATE_DECISION_PRIORITY = {
    "allow_save": 3,
    "auto_repair": 2,
    "manual_review": 1,
}

STRUCTURAL_REPAIR_FOCUS_AREAS = frozenset({"conflict", "rule_grounding", "payoff"})
CONTENT_SENSITIVE_REPAIR_FOCUS_AREAS = frozenset({
    "conflict",
    "rule_grounding",
    "payoff",
    "cliffhanger",
    "dialogue",
    "outline",
    "opening",
})
TARGETED_FINAL_REPAIR_FOCUS_AREAS = frozenset({"cliffhanger", "dialogue", "outline", "rule_grounding", "conflict", "opening"})

STRUCTURAL_REPAIR_LABEL_HINTS: Dict[str, tuple[str, ...]] = {
    "conflict": ("conflict", "冲突", "对抗", "受阻", "阻力", "升级", "张力"),
    "rule_grounding": ("rule", "ground", "规则", "设定", "限制", "约束", "机制", "法则"),
    "payoff": ("payoff", "兑现", "回收", "伏笔", "承诺", "反馈", "闭环"),
}

CONTINUITY_REPAIR_LABEL_HINTS: tuple[str, ...] = (
    "continuity", "连续性", "接力", "账本", "ledger", "handoff", "character_continuity",
    "relationship_continuity", "organization_continuity", "career_continuity", "foreshadow_continuity",
)



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


def _infer_focus_area_from_text(value: Any) -> str:
    normalized = _safe_text(value).lower()
    if not normalized:
        return ""
    for focus_area, hints in STRUCTURAL_REPAIR_LABEL_HINTS.items():
        if any(hint in normalized for hint in hints):
            return focus_area
    return ""


def _is_continuity_focus_area(value: Any) -> bool:
    normalized = _safe_text(value).lower()
    if not normalized:
        return False
    return any(hint in normalized for hint in CONTINUITY_REPAIR_LABEL_HINTS)


def _resolve_runtime_items(runtime_context: Mapping[str, Any], *keys: str, limit: int = 4) -> list[str]:
    for key in keys:
        items = _normalize_items(runtime_context.get(key), limit=limit)
        if items:
            return items
    return []


def _compact_anchor_text(value: str, *, max_chars: int = 90) -> str:
    normalized = " ".join(str(value or "").replace("\r", "").split())
    if len(normalized) <= max_chars:
        return normalized
    return f"{normalized[:max_chars].rstrip()}…"


def _extract_edge_anchors(current_content: Any) -> tuple[str, str]:
    raw_text = str(current_content or "").replace("\r", "").strip()
    if not raw_text:
        return "", ""

    paragraphs = [segment.strip() for segment in raw_text.split("\n") if segment.strip()]
    if paragraphs:
        opening_source = paragraphs[0]
        closing_source = paragraphs[-1]
    else:
        opening_source = raw_text
        closing_source = raw_text

    if len(paragraphs) <= 1:
        opening_source = raw_text[:140]
        closing_source = raw_text[-140:]

    opening_anchor = _compact_anchor_text(opening_source)
    closing_anchor = _compact_anchor_text(closing_source)
    if closing_anchor == opening_anchor and len(raw_text) > 160:
        closing_anchor = _compact_anchor_text(raw_text[-140:])
    return opening_anchor, closing_anchor


def _extract_failed_metric_labels_and_focus_areas(
    quality_gate_plan: Optional[Mapping[str, Any]],
) -> tuple[list[str], list[str]]:
    if not isinstance(quality_gate_plan, Mapping):
        return [], []

    quality_gate = quality_gate_plan.get("quality_gate")
    active_payload = quality_gate_plan.get("active_story_repair_payload")
    labels: list[str] = []
    focus_areas: list[str] = []

    def add_focus_area(raw_value: Any) -> None:
        normalized = _safe_text(raw_value).lower()
        if not normalized:
            return
        if normalized not in STRUCTURAL_REPAIR_FOCUS_AREAS:
            normalized = _infer_focus_area_from_text(normalized) or normalized
        if normalized and normalized not in focus_areas:
            focus_areas.append(normalized)

    if isinstance(quality_gate, Mapping):
        for item in quality_gate.get("failed_metrics") or []:
            if not isinstance(item, Mapping):
                continue
            label = _safe_text(item.get("label"))
            if label and label not in labels:
                labels.append(label)
            add_focus_area(item.get("focus_area") or item.get("key") or label)
            if len(labels) >= 4 and len(focus_areas) >= 3:
                break

    if isinstance(active_payload, Mapping):
        for item in _normalize_items(active_payload.get("focus_areas"), limit=4):
            add_focus_area(item)

    return labels[:4], focus_areas[:4]


def _build_continuity_repair_lines(
    quality_gate_plan: Optional[Mapping[str, Any]],
    runtime_context: Mapping[str, Any],
    *,
    hard_mode: bool,
) -> list[str]:
    if not isinstance(quality_gate_plan, Mapping):
        return []

    quality_gate = quality_gate_plan.get("quality_gate") if isinstance(quality_gate_plan.get("quality_gate"), Mapping) else {}
    active_payload = quality_gate_plan.get("active_story_repair_payload") if isinstance(quality_gate_plan.get("active_story_repair_payload"), Mapping) else {}

    has_continuity_pressure = False
    for item in quality_gate.get("failed_metrics") or []:
        if isinstance(item, Mapping) and any(
            _is_continuity_focus_area(item.get(key))
            for key in ("focus_area", "key", "label", "repair_target")
        ):
            has_continuity_pressure = True
            break

    if not has_continuity_pressure:
        has_continuity_pressure = any(
            _is_continuity_focus_area(item)
            for item in (
                list(_normalize_items(active_payload.get("focus_areas"), limit=4))
                + list(_normalize_items(active_payload.get("repair_targets"), limit=4))
                + [_safe_text(active_payload.get("summary"))]
            )
        )

    if not has_continuity_pressure:
        return []

    lines: list[str] = []
    if hard_mode:
        lines.append("- 中文连续性硬约束：至少显式接住 1-2 项跨章账本，把它写成动作、站位变化、资源调度、关系反馈或组织指令，不能只在旁白里顺手提名字。")
    else:
        lines.append("- 中文连续性要求：优先把跨章账本改写成现场动作、关系反馈或组织变化，避免只做摘要式提及。")

    repair_targets = _normalize_items(active_payload.get("repair_targets"), limit=3)
    if repair_targets:
        lines.append(f"- 优先补齐这些连续性接力点：{' / '.join(repair_targets)}。")

    character_states = _resolve_runtime_items(runtime_context, "character_state_ledger", "story_character_state_ledger", limit=2)
    relationship_states = _resolve_runtime_items(runtime_context, "relationship_state_ledger", "story_relationship_state_ledger", limit=2)
    organization_states = _resolve_runtime_items(runtime_context, "organization_state_ledger", "story_organization_state_ledger", limit=2)
    continuity_items = character_states[:1] + relationship_states[:1] + organization_states[:1]
    if continuity_items:
        lines.append(f"- 本轮至少落地其中一项连续性账本：{' / '.join(continuity_items)}。")

    return lines


def _build_structural_repair_lines(
    structural_focus_areas: Sequence[str],
    runtime_context: Mapping[str, Any],
    *,
    hard_mode: bool,
) -> list[str]:
    normalized_focus_areas = [
        focus_area
        for focus_area in structural_focus_areas
        if focus_area in STRUCTURAL_REPAIR_FOCUS_AREAS
    ]
    if not normalized_focus_areas:
        return []

    lines: list[str] = [
        "- Hard checklist: objective -> resistance -> forced choice -> consequence -> payoff/hook.",
    ]
    if hard_mode:
        lines.extend(
            [
                "- 中文硬约束：正文必须写出“目标/受阻 → 被迫投择 → 代价/后果 → 阶段性兑现或悬念钩子”，缺一项就替换弱场景，不要新增支线。",
                "- 不要输出标题、提纲、括号说明或清单，只输出章节正文。",
            ]
        )
    else:
        lines.append("- 中文结构要求：优先把剧情压成“目标/受阻 → 投择 → 代价 → 兑现/钩子”的连续链条。")

    if "conflict" in normalized_focus_areas:
        lines.append("- 必须至少出现 1 次明确受阻与升级：角色推进时被拦住、被迫换招，并立刻付出代价或承担后果。")
    if "rule_grounding" in normalized_focus_areas:
        lines.append("- 必须至少出现 1 次规则/限制改变行动结果：把世界设定、组织规约或资源约束直接写成阻碍、风险或代价。")
    if "payoff" in normalized_focus_areas:
        lines.append("- 必须至少出现 1 次兑现/回收：让前文伏笔、承诺或阶段目标在本章落地产生具体结果，而不是口头提醒。")

    lines.append("- 在正文里真实落地“受阻、只能、代价、后果、限制、兑现”等结果导向语义，但不要生硬堆词。")

    character_focus = _resolve_runtime_items(runtime_context, "character_focus", "story_character_focus", limit=3)
    if character_focus:
        lines.append(f"- 关键动作优先落在这些角色身上：{' / '.join(character_focus)}。")

    character_states = _resolve_runtime_items(runtime_context, "character_state_ledger", "story_character_state_ledger", limit=2)
    if character_states and any(item in normalized_focus_areas for item in ("conflict", "payoff")):
        lines.append(f"- 角色状态要转成动作与结果：{' / '.join(character_states)}。")

    organization_states = _resolve_runtime_items(runtime_context, "organization_state_ledger", "story_organization_state_ledger", limit=2)
    if organization_states and "rule_grounding" in normalized_focus_areas:
        lines.append(f"- 组织/规则压力要直接入戏：{' / '.join(organization_states)}。")

    foreshadow_payoff_plan = _resolve_runtime_items(runtime_context, "foreshadow_payoff_plan", "story_foreshadow_payoff_plan", limit=2)
    if foreshadow_payoff_plan and "payoff" in normalized_focus_areas:
        lines.append(f"- 优先兑现或推进这些伏笔/承诺：{' / '.join(foreshadow_payoff_plan)}。")

    return lines


def _build_quality_gate_focus_repair_lines(
    failed_focus_areas: Sequence[str],
    *,
    hard_mode: bool,
) -> list[str]:
    normalized_focus_areas = {str(focus_area or "").strip().lower() for focus_area in failed_focus_areas if str(focus_area or "").strip()}
    if not normalized_focus_areas:
        return []

    lines: list[str] = []
    if "outline" in normalized_focus_areas:
        lines.append(
            "- Outline repair: stay on the promised outline rail and explicitly land the chapter's required beats in-scene—opening anomaly, verification move, major obstruction or ally reaction, cost spike, and a tail unresolved point—even if each beat needs to be compressed to one dense paragraph."
        )
        if hard_mode:
            lines.append(
                "- Outline hard rule: before the final paragraph, cover every mandatory outline beat in compressed form; do not invent a substitute branch that consumes space while dropping the promised outcome."
            )
    if "opening" in normalized_focus_areas:
        lines.append(
            "- Opening repair: within the first 120-180 Chinese chars, surface at least two on-page hooks from abnormal signal, concrete task, immediate obstruction, countdown pressure, forced choice, or stance collision; do not open with pure background recap, weather, or mood only."
        )
        if hard_mode:
            lines.append(
                "- Opening hard rule: the first two paragraphs cannot both stay in explanation or atmosphere; at least one paragraph must contain an interruptive event, warning, order, failed attempt, or public anomaly signal."
            )
    if "rule_grounding" in normalized_focus_areas:
        lines.append(
            "- Rule-grounding repair: convert at least one active rule, countdown, platform mechanism, or organization constraint into an on-page obstacle, cost, or action result; do not leave it as explanation only."
        )
    if "cliffhanger" in normalized_focus_areas:
        lines.append(
            "- Cliffhanger repair: the final paragraph must end on a fresh imbalance, pending choice, approaching danger, identity shift, or new access signal, and the last line cannot soften or explain it away."
        )
        if hard_mode:
            lines.append(
                "- Cliffhanger hard rule: the final line must land on a concrete signal, order, timer, reveal, lock, or threat; do not use the last line to summarize feelings, explain lessons, or soften the hook."
            )
    if "dialogue" in normalized_focus_areas:
        lines.append(
            "- Dialogue repair: keep at least one back-and-forth exchange with stance collision or subtext pressure, and make each spoken line either probe, conceal, counter, threaten, or force a decision."
        )
        if hard_mode:
            lines.append(
                "- Dialogue hard rule: do not use dialogue to repeat background information; pair key lines with interruption, gesture, hesitation, or immediate fallout."
            )
    return lines


def _extract_runtime_context(
    quality_metrics: Optional[Mapping[str, Any]],
) -> Dict[str, Any]:
    if not isinstance(quality_metrics, Mapping):
        return {}
    runtime_context = quality_metrics.get("quality_runtime_context")
    return dict(runtime_context) if isinstance(runtime_context, Mapping) else {}


def _extract_candidate_selection(
    quality_metrics: Optional[Mapping[str, Any]],
) -> Dict[str, Any]:
    if not isinstance(quality_metrics, Mapping):
        return {}
    candidate_selection = quality_metrics.get("candidate_selection")
    return dict(candidate_selection) if isinstance(candidate_selection, Mapping) else {}


def _resolve_target_word_bounds(target_word_count: int) -> tuple[int, int]:
    safe_target_word_count = max(200, int(target_word_count or 0))
    target_lower_bound = max(
        200,
        min(safe_target_word_count - 120, int(safe_target_word_count * 0.9)),
    )
    target_upper_bound = max(
        target_lower_bound + 80,
        min(safe_target_word_count + 150, int(safe_target_word_count * 1.15)),
    )
    return target_lower_bound, target_upper_bound


def _resolve_severe_word_budget_pressure(
    *,
    word_count: int,
    target_word_count: int,
) -> tuple[bool, str]:
    normalized_target_word_count = max(int(target_word_count or 0), 0)
    normalized_word_count = max(int(word_count or 0), 0)
    if normalized_target_word_count <= 0 or normalized_word_count <= 0:
        return False, ""

    target_lower_bound, target_upper_bound = _resolve_target_word_bounds(normalized_target_word_count)
    severe_upper_bound = max(target_upper_bound + 120, int(target_upper_bound * 1.1))
    severe_lower_bound = max(200, min(target_lower_bound - 120, int(target_lower_bound * 0.9)))
    severe_word_budget_pressure = (
        normalized_word_count > severe_upper_bound
        or (0 < normalized_word_count < severe_lower_bound)
    )
    if not severe_word_budget_pressure:
        return False, ""

    reason = (
        "Word count deviates too far from the target window "
        f"(current {normalized_word_count}, target {normalized_target_word_count}, "
        f"ideal range {target_lower_bound}-{target_upper_bound})."
    )
    return True, reason


def normalize_candidate_quality_gate(
    quality_gate: Optional[Mapping[str, Any]],
    *,
    word_count: int,
    target_word_count: int,
) -> Dict[str, Any]:
    normalized_quality_gate = dict(quality_gate or {})
    decision = _safe_text(normalized_quality_gate.get("decision") or "allow_save") or "allow_save"
    severe_word_budget_pressure, severe_word_budget_reason = _resolve_severe_word_budget_pressure(
        word_count=word_count,
        target_word_count=target_word_count,
    )
    if severe_word_budget_pressure and decision == "allow_save":
        normalized_quality_gate["decision"] = "auto_repair"
        normalized_quality_gate["status"] = "repairable"
        normalized_quality_gate["label"] = _safe_text(normalized_quality_gate.get("label")) or "Needs repair"
        normalized_quality_gate["reason"] = (
            _safe_text(normalized_quality_gate.get("reason")) or severe_word_budget_reason
        )
        normalized_quality_gate["summary"] = (
            _safe_text(normalized_quality_gate.get("summary"))
            or "The draft still needs a targeted revision before it should be saved."
        )
        normalized_quality_gate["allow_save"] = False
        normalized_quality_gate["can_auto_repair"] = True
        normalized_quality_gate["requires_manual_review"] = False
    return normalized_quality_gate


def normalize_candidate_quality_gate_plan(
    quality_gate_plan: Optional[Mapping[str, Any]],
    *,
    word_count: int,
    target_word_count: int,
    quality_metrics: Optional[Mapping[str, Any]] = None,
) -> Dict[str, Any]:
    normalized_quality_gate_plan = dict(quality_gate_plan or {})
    raw_quality_gate = normalized_quality_gate_plan.get("quality_gate")
    if not isinstance(raw_quality_gate, Mapping) and isinstance(quality_metrics, Mapping):
        metrics_quality_gate = quality_metrics.get("quality_gate")
        if isinstance(metrics_quality_gate, Mapping):
            raw_quality_gate = metrics_quality_gate

    normalized_quality_gate = normalize_candidate_quality_gate(
        raw_quality_gate,
        word_count=word_count,
        target_word_count=target_word_count,
    )
    if normalized_quality_gate:
        normalized_quality_gate_plan["quality_gate"] = normalized_quality_gate
    return normalized_quality_gate_plan


def resolve_word_budget_repair_char_limit(
    target_word_count: int,
    *,
    relax_content_budget: bool = False,
) -> int:
    safe_target_word_count = max(200, int(target_word_count or 0))
    _, target_upper_bound = _resolve_target_word_bounds(safe_target_word_count)
    if relax_content_budget:
        buffer_chars = max(40, min(120, int(safe_target_word_count * 0.06)))
    else:
        buffer_chars = max(24, min(48, int(safe_target_word_count * 0.03)))
    return target_upper_bound + buffer_chars


def resolve_word_budget_repair_max_tokens(
    target_word_count: int,
    *,
    current_word_count: int = 0,
    relax_content_budget: bool = False,
) -> int:
    safe_target_word_count = max(200, int(target_word_count or 0))
    target_lower_bound, target_upper_bound = _resolve_target_word_bounds(safe_target_word_count)
    normalized_current_word_count = max(int(current_word_count or 0), 0)

    if normalized_current_word_count > target_upper_bound:
        calculated_max_tokens = int(target_upper_bound * (0.48 if relax_content_budget else 0.45))
    elif 0 < normalized_current_word_count < target_lower_bound:
        calculated_max_tokens = int(target_upper_bound * 0.60)
    else:
        calculated_max_tokens = int(safe_target_word_count * 0.52)

    return max(520, min(calculated_max_tokens, 7200))


def should_relax_word_budget_repair_limits(
    quality_gate_plan: Optional[Mapping[str, Any]],
) -> bool:
    _, failed_focus_areas = _extract_failed_metric_labels_and_focus_areas(quality_gate_plan)
    return any(
        str(focus_area or "").strip().lower() in CONTENT_SENSITIVE_REPAIR_FOCUS_AREAS
        for focus_area in failed_focus_areas
    )


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
    candidate_selection = _extract_candidate_selection(quality_metrics)
    failed_metric_labels, failed_focus_areas = _extract_failed_metric_labels_and_focus_areas(quality_gate_plan)
    structural_focus_areas = [
        focus_area
        for focus_area in failed_focus_areas
        if focus_area in STRUCTURAL_REPAIR_FOCUS_AREAS
    ]

    lines = [
        f"Alternative candidate strategy #{attempt_index}",
        "- Recast the same chapter intent with a visibly different scene progression, not just local word swaps.",
        f"- Keep the same target outcome for this {source} draft while varying scene sequencing and emphasis.",
    ]
    if failed_metric_labels:
        lines.append(f"- Counter the weak metrics through scene design: {' / '.join(failed_metric_labels[:3])}")
    lines.extend(_build_focus_strategy_lines(runtime_context))
    lines.extend(
        _build_quality_gate_focus_repair_lines(
            failed_focus_areas,
            hard_mode=False,
        )
    )
    lines.extend(
        _build_structural_repair_lines(
            structural_focus_areas,
            runtime_context,
            hard_mode=False,
        )
    )
    if {"conflict", "rule_grounding"}.issubset(set(structural_focus_areas)):
        lines.append(
            "- Joint pressure repair: make the visible blocker come from an active rule, platform check, organization restriction, countdown, or resource constraint, so each push forward triggers immediate resistance and cost on-page."
        )
    lines.extend(
        _build_continuity_repair_lines(
            quality_gate_plan,
            runtime_context,
            hard_mode=False,
        )
    )

    current_word_count = int(candidate_selection.get("word_count") or 0)
    target_word_count = int(
        candidate_selection.get("target_word_count")
        or runtime_context.get("target_word_count")
        or 0
    )
    if target_word_count > 0:
        target_lower_bound, target_upper_bound = _resolve_target_word_bounds(target_word_count)
        if current_word_count > target_upper_bound:
            lines.append(
                f"- The previous draft ran long at about {current_word_count} chars; rewrite to stay within "
                f"{target_lower_bound}-{target_upper_bound} Chinese characters."
            )
            lines.append(
                "- Compress by merging repeated beats, removing recap/exposition, and ending immediately once the hook lands."
            )
        elif 0 < current_word_count < target_lower_bound:
            lines.append(
                f"- The previous draft landed short at about {current_word_count} chars; expand to roughly "
                f"{target_lower_bound}-{target_upper_bound} Chinese characters through concrete action and consequence."
            )
    return "\n".join(lines)


def resolve_candidate_retry_temperature(
    base_temperature: float,
    *,
    quality_metrics: Optional[Mapping[str, Any]] = None,
    quality_gate_plan: Optional[Mapping[str, Any]] = None,
    attempt_index: int,
) -> float:
    runtime_context = _extract_runtime_context(quality_metrics)
    candidate_selection = _extract_candidate_selection(quality_metrics)
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

    current_word_count = int(candidate_selection.get("word_count") or 0)
    target_word_count = int(
        candidate_selection.get("target_word_count")
        or runtime_context.get("target_word_count")
        or 0
    )
    if target_word_count > 0:
        target_lower_bound, target_upper_bound = _resolve_target_word_bounds(target_word_count)
        if current_word_count > target_upper_bound:
            temperature -= 0.12
        elif 0 < current_word_count < target_lower_bound:
            temperature += 0.02

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
    generation_path: Optional[str] = None,
    attempt_kind: Optional[str] = None,
    rerank_used: Optional[bool] = None,
    word_budget_repair_used: Optional[bool] = None,
    winner_candidate_index: Optional[int] = None,
    repair_seed_candidate_index: Optional[int] = None,
    repair_seed_generation_path: Optional[str] = None,
    repair_seed_attempt_kind: Optional[str] = None,
) -> Dict[str, Any]:
    metrics = dict(quality_metrics or {})
    existing_selection_metadata = (
        dict(metrics.get("candidate_selection") or {})
        if isinstance(metrics.get("candidate_selection"), Mapping)
        else {}
    )
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
    target_lower_bound, target_upper_bound = _resolve_target_word_bounds(normalized_target_word_count)
    out_of_window_chars = 0
    if normalized_word_count > target_upper_bound:
        out_of_window_chars = normalized_word_count - target_upper_bound
    elif 0 < normalized_word_count < target_lower_bound:
        out_of_window_chars = target_lower_bound - normalized_word_count
    out_of_window_penalty = round((out_of_window_chars / normalized_target_word_count) * 24.0, 2)

    decision_priority = QUALITY_GATE_DECISION_PRIORITY.get(decision, 0)
    decision_bonus = {
        "allow_save": 18.0,
        "auto_repair": 4.0,
        "manual_review": -18.0,
    }.get(decision, 0.0)

    selection_score = round(
        overall_score
        + decision_bonus
        + word_count_fit_score * 0.12
        + max(pacing_score - 7.0, 0.0) * 1.5
        - continuity_warning_count * 4.0
        - out_of_window_penalty,
        2,
    )

    selection_metadata = {
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
        "out_of_window_penalty": out_of_window_penalty,
        "continuity_warning_count": continuity_warning_count,
    }
    if isinstance(generation_path, str) and generation_path.strip():
        selection_metadata["generation_path"] = generation_path.strip()
    if isinstance(attempt_kind, str) and attempt_kind.strip():
        selection_metadata["attempt_kind"] = attempt_kind.strip()
    if rerank_used is not None:
        selection_metadata["rerank_used"] = bool(rerank_used)
    if word_budget_repair_used is not None:
        selection_metadata["word_budget_repair_used"] = bool(word_budget_repair_used)
    if winner_candidate_index is not None:
        selection_metadata["winner_candidate_index"] = max(int(winner_candidate_index or 1), 1)

    resolved_repair_seed_candidate_index = repair_seed_candidate_index
    if resolved_repair_seed_candidate_index is None:
        existing_repair_seed_candidate_index = existing_selection_metadata.get("repair_seed_candidate_index")
        if existing_repair_seed_candidate_index is not None:
            try:
                resolved_repair_seed_candidate_index = max(int(existing_repair_seed_candidate_index or 1), 1)
            except (TypeError, ValueError):
                resolved_repair_seed_candidate_index = None
    resolved_repair_seed_generation_path = repair_seed_generation_path
    if resolved_repair_seed_generation_path is None:
        existing_repair_seed_generation_path = existing_selection_metadata.get("repair_seed_generation_path")
        if isinstance(existing_repair_seed_generation_path, str) and existing_repair_seed_generation_path.strip():
            resolved_repair_seed_generation_path = existing_repair_seed_generation_path.strip()
    resolved_repair_seed_attempt_kind = repair_seed_attempt_kind
    if resolved_repair_seed_attempt_kind is None:
        existing_repair_seed_attempt_kind = existing_selection_metadata.get("repair_seed_attempt_kind")
        if isinstance(existing_repair_seed_attempt_kind, str) and existing_repair_seed_attempt_kind.strip():
            resolved_repair_seed_attempt_kind = existing_repair_seed_attempt_kind.strip()

    if resolved_repair_seed_candidate_index is not None:
        selection_metadata["repair_seed_candidate_index"] = max(int(resolved_repair_seed_candidate_index or 1), 1)
    if isinstance(resolved_repair_seed_generation_path, str) and resolved_repair_seed_generation_path.strip():
        selection_metadata["repair_seed_generation_path"] = resolved_repair_seed_generation_path.strip()
    if isinstance(resolved_repair_seed_attempt_kind, str) and resolved_repair_seed_attempt_kind.strip():
        selection_metadata["repair_seed_attempt_kind"] = resolved_repair_seed_attempt_kind.strip()
    return selection_metadata

def attach_candidate_selection_metadata(
    quality_metrics: Optional[Mapping[str, Any]],
    *,
    selection_metadata: Mapping[str, Any],
) -> Dict[str, Any]:
    metrics = dict(quality_metrics or {})
    metrics["candidate_selection"] = dict(selection_metadata or {})
    return metrics


def build_candidate_pool_summary(
    candidates: Sequence[Mapping[str, Any]],
    *,
    winner_candidate_index: Optional[int] = None,
    repair_seed_candidate_index: Optional[int] = None,
) -> list[Dict[str, Any]]:
    summary: list[Dict[str, Any]] = []
    resolved_winner_candidate_index = max(int(winner_candidate_index or 0), 0)
    resolved_repair_seed_candidate_index = max(int(repair_seed_candidate_index or 0), 0)

    for candidate in candidates:
        if not isinstance(candidate, Mapping):
            continue

        candidate_quality_metrics = (
            candidate.get("quality_metrics") if isinstance(candidate.get("quality_metrics"), Mapping) else {}
        )
        candidate_selection = (
            candidate_quality_metrics.get("candidate_selection")
            if isinstance(candidate_quality_metrics.get("candidate_selection"), Mapping)
            else {}
        )
        candidate_quality_gate = (
            candidate_quality_metrics.get("quality_gate")
            if isinstance(candidate_quality_metrics.get("quality_gate"), Mapping)
            else {}
        )
        failed_metrics_raw = candidate_quality_gate.get("failed_metrics") or []
        failed_metrics: list[str] = []
        for item in failed_metrics_raw:
            if isinstance(item, Mapping):
                label = _safe_text(item.get("label") or item.get("key"))
                if label:
                    failed_metrics.append(label)
            else:
                label = _safe_text(item)
                if label:
                    failed_metrics.append(label)

        candidate_index = max(int(candidate.get("candidate_index") or 0), 0)
        summary.append(
            {
                "candidate_index": candidate_index,
                "generation_path": _safe_text(
                    candidate_selection.get("generation_path") or candidate.get("generation_path")
                ),
                "attempt_kind": _safe_text(
                    candidate_selection.get("attempt_kind") or candidate.get("attempt_kind")
                ),
                "quality_gate_decision": _safe_text(
                    candidate_selection.get("quality_gate_decision") or candidate_quality_gate.get("decision")
                ),
                "quality_gate_status": _safe_text(
                    candidate_selection.get("quality_gate_status") or candidate_quality_gate.get("status")
                ),
                "word_count": max(
                    int(candidate_selection.get("word_count") or candidate.get("word_count") or 0),
                    0,
                ),
                "target_word_count": max(int(candidate_selection.get("target_word_count") or 0), 0),
                "overall_score": round(
                    _safe_float(candidate_selection.get("overall_score") or candidate.get("overall_score") or 0.0),
                    1,
                ),
                "selection_score": round(
                    _safe_float(candidate_selection.get("selection_score") or candidate.get("selection_score") or 0.0),
                    2,
                ),
                "repair_seed_candidate_index": max(
                    int(candidate_selection.get("repair_seed_candidate_index") or 0),
                    0,
                ),
                "repair_seed_generation_path": _safe_text(candidate_selection.get("repair_seed_generation_path")),
                "repair_seed_attempt_kind": _safe_text(candidate_selection.get("repair_seed_attempt_kind")),
                "failed_metrics": failed_metrics,
                "is_winner": candidate_index == resolved_winner_candidate_index,
                "is_repair_seed": candidate_index == resolved_repair_seed_candidate_index,
            }
        )

    return sorted(summary, key=lambda item: int(item.get("candidate_index") or 0))


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


def is_candidate_word_count_in_target_window(candidate: Optional[Mapping[str, Any]]) -> bool:
    if not isinstance(candidate, Mapping):
        return False
    target_word_count = int(candidate.get("target_word_count") or 0)
    current_word_count = int(candidate.get("word_count") or 0)
    if target_word_count <= 0 or current_word_count <= 0:
        return False
    target_lower_bound, target_upper_bound = _resolve_target_word_bounds(target_word_count)
    return target_lower_bound <= current_word_count <= target_upper_bound



def _candidate_has_explicit_quality_repair_pressure(candidate: Optional[Mapping[str, Any]]) -> bool:
    if not isinstance(candidate, Mapping):
        return False

    quality_gate_plan = candidate.get("quality_gate_plan")
    if not isinstance(quality_gate_plan, Mapping):
        return False

    quality_gate = quality_gate_plan.get("quality_gate")
    if isinstance(quality_gate, Mapping):
        failed_metrics = quality_gate.get("failed_metrics") or []
        for item in failed_metrics:
            if not isinstance(item, Mapping):
                continue
            if (
                _safe_text(item.get("label"))
                or _safe_text(item.get("key"))
                or _safe_text(item.get("focus_area"))
            ):
                return True

    active_payload = quality_gate_plan.get("active_story_repair_payload")
    if isinstance(active_payload, Mapping):
        if _safe_text(active_payload.get("summary")):
            return True
        if _normalize_items(active_payload.get("repair_targets"), limit=1):
            return True
        if _normalize_items(active_payload.get("focus_areas"), limit=1):
            return True

    return False



def should_apply_word_budget_repair(candidate: Optional[Mapping[str, Any]]) -> bool:
    if not isinstance(candidate, Mapping):
        return False
    target_word_count = int(candidate.get("target_word_count") or 0)
    current_word_count = int(candidate.get("word_count") or 0)
    if target_word_count <= 0 or current_word_count <= 0:
        return False
    target_lower_bound, target_upper_bound = _resolve_target_word_bounds(target_word_count)
    severe_upper_bound = max(target_upper_bound + 120, int(target_upper_bound * 1.1))
    severe_lower_bound = max(200, min(target_lower_bound - 120, int(target_lower_bound * 0.9)))
    return current_word_count > severe_upper_bound or (0 < current_word_count < severe_lower_bound)



def should_prefer_word_budget_repair_candidate(
    selected_candidate: Optional[Mapping[str, Any]],
    repair_candidate: Optional[Mapping[str, Any]],
) -> bool:
    if not isinstance(repair_candidate, Mapping):
        return False
    if not isinstance(selected_candidate, Mapping):
        return True

    selected_target_word_count = int(
        selected_candidate.get("target_word_count")
        or repair_candidate.get("target_word_count")
        or 0
    )
    if selected_target_word_count <= 0:
        return False

    selected_word_count = int(selected_candidate.get("word_count") or 0)
    repair_word_count = int(repair_candidate.get("word_count") or 0)
    selected_word_delta = abs(selected_word_count - selected_target_word_count)
    repair_word_delta = abs(repair_word_count - selected_target_word_count)
    if repair_word_delta >= selected_word_delta:
        return False

    selected_priority = QUALITY_GATE_DECISION_PRIORITY.get(
        _safe_text(selected_candidate.get("quality_gate_decision")),
        0,
    )
    repair_priority = QUALITY_GATE_DECISION_PRIORITY.get(
        _safe_text(repair_candidate.get("quality_gate_decision")),
        0,
    )

    selected_in_window = is_candidate_word_count_in_target_window(selected_candidate)
    repair_in_window = is_candidate_word_count_in_target_window(repair_candidate)
    target_lower_bound, target_upper_bound = _resolve_target_word_bounds(selected_target_word_count)
    severe_upper_bound = max(target_upper_bound + 120, int(target_upper_bound * 1.1))
    repair_soft_upper_bound = target_upper_bound + max(80, int(selected_target_word_count * 0.06))

    selected_overall_score = _safe_float(selected_candidate.get("overall_score"))
    repair_overall_score = _safe_float(repair_candidate.get("overall_score"))
    quality_drop = max(selected_overall_score - repair_overall_score, 0.0)
    selected_failed_metric_count = _extract_failed_metric_count(selected_candidate)
    repair_failed_metric_count = _extract_failed_metric_count(repair_candidate)
    delta_improvement = selected_word_delta - repair_word_delta
    substantial_improvement = delta_improvement >= max(120, int(selected_target_word_count * 0.10))
    decisive_improvement = delta_improvement >= max(240, int(selected_target_word_count * 0.20))
    selected_severely_over_budget = selected_word_count > severe_upper_bound
    repair_near_target_ceiling = 0 < repair_word_count <= repair_soft_upper_bound

    if repair_in_window and not selected_in_window:
        return quality_drop <= 8.0
    if selected_severely_over_budget and repair_near_target_ceiling and substantial_improvement:
        if repair_failed_metric_count <= selected_failed_metric_count:
            return quality_drop <= 14.0
        return quality_drop <= 8.0
    if repair_priority < selected_priority:
        return False
    if repair_priority > selected_priority:
        return quality_drop <= 8.0
    if should_apply_word_budget_repair(selected_candidate) and substantial_improvement:
        return quality_drop <= 6.0
    if decisive_improvement:
        return quality_drop <= 3.5
    return False


def should_keep_word_budget_repair_candidate(
    selected_candidate: Optional[Mapping[str, Any]],
    repair_candidate: Optional[Mapping[str, Any]],
) -> bool:
    if not isinstance(repair_candidate, Mapping):
        return False
    if not isinstance(selected_candidate, Mapping):
        return True
    if should_prefer_word_budget_repair_candidate(selected_candidate, repair_candidate):
        return True

    target_word_count = int(
        selected_candidate.get("target_word_count")
        or repair_candidate.get("target_word_count")
        or 0
    )
    if target_word_count <= 0:
        return True

    selected_word_count = int(selected_candidate.get("word_count") or 0)
    repair_word_count = int(repair_candidate.get("word_count") or 0)
    target_lower_bound, target_upper_bound = _resolve_target_word_bounds(target_word_count)
    repair_hard_lower_bound = max(
        max(200, target_lower_bound - max(60, int(target_word_count * 0.05))),
        int(max(min(selected_word_count, target_upper_bound), target_lower_bound) * 0.72),
    )
    if 0 < repair_word_count < repair_hard_lower_bound:
        return False

    selected_failed_metric_count = _extract_failed_metric_count(selected_candidate)
    repair_failed_metric_count = _extract_failed_metric_count(repair_candidate)
    selected_overall_score = _safe_float(selected_candidate.get("overall_score"))
    repair_overall_score = _safe_float(repair_candidate.get("overall_score"))

    if repair_failed_metric_count > selected_failed_metric_count + 1 and repair_overall_score + 10.0 < selected_overall_score:
        return False
    if selected_failed_metric_count <= 1 and repair_failed_metric_count >= selected_failed_metric_count + 2 and repair_overall_score + 10.0 < selected_overall_score:
        return False
    if repair_overall_score + 24.0 < selected_overall_score:
        return False
    return True



def _extract_quality_gate_payload(candidate: Optional[Mapping[str, Any]]) -> Dict[str, Any]:
    if not isinstance(candidate, Mapping):
        return {}
    quality_gate_plan = candidate.get("quality_gate_plan")
    if isinstance(quality_gate_plan, Mapping):
        quality_gate = quality_gate_plan.get("quality_gate")
        if isinstance(quality_gate, Mapping):
            return dict(quality_gate)
    quality_metrics = candidate.get("quality_metrics")
    if isinstance(quality_metrics, Mapping):
        quality_gate = quality_metrics.get("quality_gate")
        if isinstance(quality_gate, Mapping):
            return dict(quality_gate)
    return {}


def _extract_failed_focus_areas_from_candidate(candidate: Optional[Mapping[str, Any]]) -> list[str]:
    quality_gate = _extract_quality_gate_payload(candidate)
    focus_areas: list[str] = []
    for item in quality_gate.get("failed_metrics") or []:
        if not isinstance(item, Mapping):
            continue
        normalized = _safe_text(item.get("focus_area") or item.get("key") or item.get("label")).lower()
        if normalized and normalized not in STRUCTURAL_REPAIR_FOCUS_AREAS:
            normalized = _infer_focus_area_from_text(normalized) or normalized
        if normalized and normalized not in focus_areas:
            focus_areas.append(normalized)
    return focus_areas


def _extract_failed_metric_count(candidate: Optional[Mapping[str, Any]]) -> int:
    quality_gate = _extract_quality_gate_payload(candidate)
    failed_metrics = quality_gate.get("failed_metrics") or []
    return sum(1 for item in failed_metrics if isinstance(item, Mapping))


def resolve_targeted_final_repair_char_limit(target_word_count: int) -> int:
    safe_target_word_count = max(200, int(target_word_count or 0))
    _, target_upper_bound = _resolve_target_word_bounds(safe_target_word_count)
    return target_upper_bound + max(80, min(140, int(safe_target_word_count * 0.07)))


def resolve_targeted_final_repair_max_tokens(
    target_word_count: int,
    *,
    current_word_count: int = 0,
) -> int:
    safe_target_word_count = max(200, int(target_word_count or 0))
    _, target_upper_bound = _resolve_target_word_bounds(safe_target_word_count)
    normalized_current_word_count = max(int(current_word_count or 0), 0)
    base_limit = max(target_upper_bound, normalized_current_word_count)
    calculated_max_tokens = int(base_limit * 0.50)
    return max(520, min(calculated_max_tokens, 6400))


def _is_word_budget_repair_candidate(candidate: Optional[Mapping[str, Any]]) -> bool:
    if not isinstance(candidate, Mapping):
        return False
    attempt_kind = _safe_text(candidate.get("attempt_kind"))
    generation_path = _safe_text(candidate.get("generation_path"))
    return attempt_kind == "word_budget_repair" or generation_path == "word_budget_repair"



def _is_rule_grounding_only_final_polish_candidate(
    candidate: Optional[Mapping[str, Any]],
    *,
    overall_score: float,
    failed_focus_areas: Sequence[str],
    failed_metric_count: int,
    relaxed: bool = False,
) -> bool:
    if set(failed_focus_areas) != {"rule_grounding"}:
        return False
    if failed_metric_count != 1:
        return False

    score_floor = 90.0
    if relaxed:
        score_floor = 86.0 if _is_word_budget_repair_candidate(candidate) else 88.0
    return overall_score >= score_floor


def _is_cliffhanger_only_final_polish_candidate(
    candidate: Optional[Mapping[str, Any]],
    *,
    overall_score: float,
    failed_focus_areas: Sequence[str],
    failed_metric_count: int,
    relaxed: bool = False,
) -> bool:
    if set(failed_focus_areas) != {"cliffhanger"}:
        return False
    if failed_metric_count != 1:
        return False

    score_floor = 89.0
    if relaxed:
        score_floor = 85.0 if _is_word_budget_repair_candidate(candidate) else 87.0
    return overall_score >= score_floor


def _is_rule_grounding_cliffhanger_final_polish_candidate(
    candidate: Optional[Mapping[str, Any]],
    *,
    overall_score: float,
    failed_focus_areas: Sequence[str],
    failed_metric_count: int,
    relaxed: bool = False,
) -> bool:
    if set(failed_focus_areas) != {"rule_grounding", "cliffhanger"}:
        return False
    if failed_metric_count != 2:
        return False

    score_floor = 91.0
    if relaxed:
        score_floor = 87.0 if _is_word_budget_repair_candidate(candidate) else 89.0
    return overall_score >= score_floor


def _is_opening_rule_grounding_final_polish_candidate(
    candidate: Optional[Mapping[str, Any]],
    *,
    overall_score: float,
    failed_focus_areas: Sequence[str],
    failed_metric_count: int,
    relaxed: bool = False,
) -> bool:
    if set(failed_focus_areas) != {"opening", "rule_grounding"}:
        return False
    if failed_metric_count != 2:
        return False

    score_floor = 90.0
    if relaxed:
        score_floor = 86.0 if _is_word_budget_repair_candidate(candidate) else 88.0
    return overall_score >= score_floor


def _is_opening_rule_grounding_cliffhanger_final_polish_candidate(
    candidate: Optional[Mapping[str, Any]],
    *,
    overall_score: float,
    failed_focus_areas: Sequence[str],
    failed_metric_count: int,
    relaxed: bool = False,
) -> bool:
    if set(failed_focus_areas) != {"opening", "rule_grounding", "cliffhanger"}:
        return False
    if failed_metric_count != 3:
        return False

    score_floor = 90.0
    if relaxed:
        score_floor = 86.0 if _is_word_budget_repair_candidate(candidate) else 88.0
    return overall_score >= score_floor


def _is_dialogue_cliffhanger_final_polish_candidate(
    candidate: Optional[Mapping[str, Any]],
    *,
    overall_score: float,
    failed_focus_areas: Sequence[str],
    failed_metric_count: int,
    relaxed: bool = False,
) -> bool:
    if set(failed_focus_areas) != {"dialogue", "cliffhanger"}:
        return False
    if failed_metric_count != 2:
        return False

    score_floor = 89.0
    if relaxed:
        score_floor = 85.0 if _is_word_budget_repair_candidate(candidate) else 87.0
    return overall_score >= score_floor


def _is_opening_conflict_cliffhanger_final_polish_candidate(
    candidate: Optional[Mapping[str, Any]],
    *,
    overall_score: float,
    failed_focus_areas: Sequence[str],
    failed_metric_count: int,
    relaxed: bool = False,
) -> bool:
    if set(failed_focus_areas) != {"opening", "conflict", "cliffhanger"}:
        return False
    if failed_metric_count != 3:
        return False

    score_floor = 90.0
    if relaxed:
        score_floor = 86.0 if _is_word_budget_repair_candidate(candidate) else 88.0
    return overall_score >= score_floor



def _can_seed_targeted_final_repair_candidate(
    candidate: Optional[Mapping[str, Any]],
    *,
    relaxed: bool = False,
) -> bool:
    if not isinstance(candidate, Mapping):
        return False

    quality_gate = _extract_quality_gate_payload(candidate)
    if _safe_text(quality_gate.get("decision") or candidate.get("quality_gate_decision")) != "manual_review":
        return False

    target_word_count = int(candidate.get("target_word_count") or 0)
    current_word_count = int(candidate.get("word_count") or 0)
    if target_word_count <= 0 or current_word_count <= 0:
        return False

    target_lower_bound, target_upper_bound = _resolve_target_word_bounds(target_word_count)
    polish_upper_bound = target_upper_bound + max(80, min(140, int(target_word_count * 0.07)))
    relaxed_upper_bound = polish_upper_bound + max(80, min(200, int(target_word_count * 0.12)))
    allowed_upper_bound = relaxed_upper_bound if relaxed else polish_upper_bound
    if current_word_count < target_lower_bound or current_word_count > allowed_upper_bound:
        return False

    continuity_warning_count = int(quality_gate.get("continuity_warning_count") or 0)
    if continuity_warning_count > 1:
        return False

    overall_score = _safe_float(candidate.get("overall_score") or quality_gate.get("overall_score"))
    overall_score_floor = 84.0
    if relaxed:
        overall_score_floor = 76.0 if _is_word_budget_repair_candidate(candidate) else 80.0
    if overall_score < overall_score_floor:
        return False

    failed_focus_areas = _extract_failed_focus_areas_from_candidate(candidate)
    if not failed_focus_areas or not set(failed_focus_areas).issubset(TARGETED_FINAL_REPAIR_FOCUS_AREAS):
        return False

    failed_metric_count = _extract_failed_metric_count(candidate)
    if "cliffhanger" not in failed_focus_areas:
        return (
            _is_rule_grounding_only_final_polish_candidate(
                candidate,
                overall_score=overall_score,
                failed_focus_areas=failed_focus_areas,
                failed_metric_count=failed_metric_count,
                relaxed=relaxed,
            )
            or _is_opening_rule_grounding_final_polish_candidate(
                candidate,
                overall_score=overall_score,
                failed_focus_areas=failed_focus_areas,
                failed_metric_count=failed_metric_count,
                relaxed=relaxed,
            )
        )

    max_failed_metric_count = 4 if relaxed and _is_word_budget_repair_candidate(candidate) else 3
    return 1 <= failed_metric_count <= max_failed_metric_count



def should_apply_targeted_final_repair(candidate: Optional[Mapping[str, Any]]) -> bool:
    return _can_seed_targeted_final_repair_candidate(candidate, relaxed=False)



def _can_fallback_seed_targeted_final_repair_candidate(
    candidate: Optional[Mapping[str, Any]],
) -> bool:
    if not isinstance(candidate, Mapping):
        return False
    if _is_word_budget_repair_candidate(candidate):
        return False

    quality_gate = _extract_quality_gate_payload(candidate)
    if _safe_text(quality_gate.get("decision") or candidate.get("quality_gate_decision")) != "manual_review":
        return False

    target_word_count = int(candidate.get("target_word_count") or 0)
    current_word_count = int(candidate.get("word_count") or 0)
    if target_word_count <= 0 or current_word_count <= 0:
        return False

    failed_focus_areas = _extract_failed_focus_areas_from_candidate(candidate)
    failed_metric_count = _extract_failed_metric_count(candidate)
    overall_score = _safe_float(candidate.get("overall_score") or quality_gate.get("overall_score"))
    continuity_warning_count = int(quality_gate.get("continuity_warning_count") or 0)

    if continuity_warning_count > 1:
        return False
    if not _is_cliffhanger_only_final_polish_candidate(
        candidate,
        overall_score=overall_score,
        failed_focus_areas=failed_focus_areas,
        failed_metric_count=failed_metric_count,
        relaxed=False,
    ):
        return False

    _, target_upper_bound = _resolve_target_word_bounds(target_word_count)
    polish_upper_bound = target_upper_bound + max(80, min(140, int(target_word_count * 0.07)))
    fallback_upper_bound = target_upper_bound + max(600, int(target_word_count * 0.50))
    return polish_upper_bound < current_word_count <= fallback_upper_bound



def should_apply_followup_targeted_final_repair(candidate: Optional[Mapping[str, Any]]) -> bool:
    if not isinstance(candidate, Mapping):
        return False
    if _safe_text(candidate.get("attempt_kind")) != "targeted_quality_repair":
        return False

    quality_gate = _extract_quality_gate_payload(candidate)
    overall_score = _safe_float(candidate.get("overall_score") or quality_gate.get("overall_score"))
    failed_focus_areas = _extract_failed_focus_areas_from_candidate(candidate)
    failed_metric_count = _extract_failed_metric_count(candidate)
    return _can_seed_targeted_final_repair_candidate(candidate, relaxed=False) and (
        _is_rule_grounding_only_final_polish_candidate(
            candidate,
            overall_score=overall_score,
            failed_focus_areas=failed_focus_areas,
            failed_metric_count=failed_metric_count,
            relaxed=False,
        )
        or _is_opening_rule_grounding_final_polish_candidate(
            candidate,
            overall_score=overall_score,
            failed_focus_areas=failed_focus_areas,
            failed_metric_count=failed_metric_count,
            relaxed=False,
        )
        or _is_opening_rule_grounding_cliffhanger_final_polish_candidate(
            candidate,
            overall_score=overall_score,
            failed_focus_areas=failed_focus_areas,
            failed_metric_count=failed_metric_count,
            relaxed=False,
        )
        or _is_dialogue_cliffhanger_final_polish_candidate(
            candidate,
            overall_score=overall_score,
            failed_focus_areas=failed_focus_areas,
            failed_metric_count=failed_metric_count,
            relaxed=False,
        )
        or _is_cliffhanger_only_final_polish_candidate(
            candidate,
            overall_score=overall_score,
            failed_focus_areas=failed_focus_areas,
            failed_metric_count=failed_metric_count,
            relaxed=False,
        )
        or _is_rule_grounding_cliffhanger_final_polish_candidate(
            candidate,
            overall_score=overall_score,
            failed_focus_areas=failed_focus_areas,
            failed_metric_count=failed_metric_count,
            relaxed=False,
        )
        or _is_opening_conflict_cliffhanger_final_polish_candidate(
            candidate,
            overall_score=overall_score,
            failed_focus_areas=failed_focus_areas,
            failed_metric_count=failed_metric_count,
            relaxed=False,
        )
    )


def build_targeted_final_repair_suffix(
    *,
    quality_metrics: Optional[Mapping[str, Any]] = None,
    quality_gate_plan: Optional[Mapping[str, Any]] = None,
    target_word_count: int,
    attempt_index: int,
    source: str,
) -> str:
    if target_word_count <= 0:
        return ""

    candidate_selection = _extract_candidate_selection(quality_metrics)
    failed_metric_labels, failed_focus_areas = _extract_failed_metric_labels_and_focus_areas(quality_gate_plan)
    normalized_focus_areas = {
        focus_area
        for focus_area in failed_focus_areas
        if focus_area in TARGETED_FINAL_REPAIR_FOCUS_AREAS
    }
    if not normalized_focus_areas:
        return ""

    current_word_count = int(candidate_selection.get("word_count") or 0)
    target_lower_bound, target_upper_bound = _resolve_target_word_bounds(target_word_count)
    polish_upper_bound = target_upper_bound + max(80, min(140, int(target_word_count * 0.07)))
    lines = [
        f"Targeted quality repair pass #{attempt_index}",
        f"- Rewrite the same {source} draft into {target_lower_bound}-{polish_upper_bound} Chinese characters; stay close to the current length while fixing only the weak quality gaps.",
        "- Preserve the same scene order, revealed facts, character decisions, and already-landed rule payoffs.",
        "- Do not reopen the whole chapter; keep the opening and middle beats stable, and spend most revisions on the final 2-4 paragraphs plus any weak dialogue exchange.",
        "- Replace soft summary, afterthought explanation, and reflective cooldown lines with concrete signal, interruption, pressure, or decision fallout.",
    ]
    if current_word_count > 0:
        lines.append(
            f"- The current draft is about {current_word_count} chars; keep the revision tight and avoid regrowing the chapter."
        )
    if failed_metric_labels:
        lines.append(f"- Repair these weak metrics without changing the chapter mission: {' / '.join(failed_metric_labels[:3])}")
    if "opening" in normalized_focus_areas:
        lines.append(
            "- Opening repair focus: the first 120-180 chars must present a live anomaly, concrete objective, obstruction, warning, or forced choice on-page instead of easing in with recap."
        )
        lines.append(
            "- Opening hard rule: the first two paragraphs cannot both stay in setup or explanation; at least one must contain a visible interruption, failed attempt, order, signal, or public abnormality."
        )
    if "outline" in normalized_focus_areas:
        lines.append(
            "- Outline preservation rule: keep every already-landed required beat on-page; tighten wording instead of deleting the promised chapter turn."
        )
    if "dialogue" in normalized_focus_areas:
        lines.append(
            "- Dialogue repair focus: keep at least one two-sided exchange with stance collision, interruption, or subtext pressure; cut monologue exposition first."
        )
    if "conflict" in normalized_focus_areas:
        lines.append(
            "- Conflict repair focus: preserve a visible blocker, counter-move, or leverage swing on-page so the protagonist pays a concrete price before the closing turn lands."
        )
    if "rule_grounding" in normalized_focus_areas:
        lines.append(
            "- Rule-grounding repair focus: keep at least one active rule, platform check, timer, or organization constraint on-page, and make it change the immediate action result instead of staying in explanation."
        )
        if normalized_focus_areas == {"rule_grounding"}:
            lines.append(
                "- Rule-grounding hard rule: show a named rule, quota, seal, timer, platform limit, or organization protocol being checked, triggered, enforced, or exploited on-page, and let it redirect the protagonist's very next move."
            )
            lines.append(
                "- Rule-grounding hard rule: do not add lore explanation; convert at least one explanation beat into an action-result pair where the rule changes cost, access, timing, or authority inside the scene."
            )
    if "cliffhanger" in normalized_focus_areas:
        lines.append(
            "- Cliffhanger repair focus: the final paragraph must escalate into a concrete reveal, order, timer, threat, identity shift, or pending forced choice."
        )
        lines.append(
            "- Cliffhanger hard rule: the last line cannot resolve tension, summarize emotion, or explain the meaning; it must leave a sharper unstable state than the line before it."
        )
        lines.append(
            "- Cliffhanger closing runway: use the final 3-5 lines to move from visible cost or turn, into a narrowing threat signal, and end on one concrete unresolved next beat."
        )
        if normalized_focus_areas == {"cliffhanger"}:
            lines.append(
                "- Cliffhanger escalation rule: in the final 2-3 lines, introduce one newly surfaced command, reveal, countdown, arrival, detection, or irreversible signal that was not fully explicit in the prior closing beat."
            )
            lines.append(
                "- Cliffhanger framing rule: stop on the external trigger itself (spoken order, ringing device, opened door, detected identity, countdown change, or incoming threat), not on reflection about that trigger."
            )
            lines.append(
                "- Cliffhanger conversion rule: the penultimate paragraph must already cause a visible irreversible change, exposure, cutoff, order, or timer shift, and the final 1-2 lines must add one fresh external trigger rather than merely rephrasing the same danger."
            )
    if {"rule_grounding", "cliffhanger"}.issubset(normalized_focus_areas):
        lines.append(
            "- Joint repair focus: let one active rule, countdown, platform check, or organization constraint directly trigger the closing turn, so the final hook comes from the rule firing instead of a separate explanation beat."
        )
        lines.append(
            "- Joint closing hard rule: the penultimate beat must show the rule consequence landing on-page, and the final line must leave the protagonist facing one concrete blocked, risky, or time-bound next move."
        )
    if {"opening", "rule_grounding"}.issubset(normalized_focus_areas):
        lines.append(
            "- Joint repair focus: make the opening anomaly, warning, task, or interruption immediately expose one active rule, platform check, timer, or organization constraint, so the chapter starts with grounded pressure instead of abstract setup."
        )
        lines.append(
            "- Joint opening hard rule: within the first two paragraphs, show both the hook and the rule consequence on-page; the rule must alter access, timing, authority, or safety before the scene settles."
        )
    if {"opening", "rule_grounding", "cliffhanger"}.issubset(normalized_focus_areas):
        lines.append(
            "- Joint repair focus: make the opening anomaly or urgent demand immediately expose the active rule, timer, or authority constraint, then let that exact grounded pressure detonate the closing spike instead of drifting into separate setup and ending beats."
        )
        lines.append(
            "- Joint triad hard rule: the first two paragraphs must hook with a live anomaly or forced task, the middle must show the rule causing an immediate cost, blockage, or leverage shift, and the final line must stop on the unresolved consequence implied by that same rule, timer, or authority limit."
        )
    if {"dialogue", "cliffhanger"}.issubset(normalized_focus_areas):
        lines.append(
            "- Joint repair focus: make one two-sided exchange, interruption, or spoken threat directly create the closing spike, so the hook lands through dialogue pressure instead of narration summary."
        )
        lines.append(
            "- Joint dialogue-cliffhanger hard rule: in the last 2-4 paragraphs, at least one line of dialogue must change leverage, reveal a hidden stance, or force a next move, and the final line must stop on the unstable result of that exchange."
        )
    if {"opening", "conflict", "cliffhanger"}.issubset(normalized_focus_areas):
        lines.append(
            "- Three-beat repair focus: rebuild the chapter as opening anomaly or task pressure -> visible blocker or counter-move -> unresolved closing spike, with each beat staying concrete on-page."
        )
        lines.append(
            "- Three-beat hard rule: the opening hook must cause the conflict lane, and the conflict lane must create the final unresolved hook; do not let any of the three beats drift into standalone explanation."
        )
    return "\n".join(lines)


def resolve_targeted_final_repair_temperature(
    base_temperature: float,
    *,
    quality_gate_plan: Optional[Mapping[str, Any]] = None,
) -> float:
    _, failed_focus_areas = _extract_failed_metric_labels_and_focus_areas(quality_gate_plan)
    normalized_focus_areas = {focus_area for focus_area in failed_focus_areas if focus_area in TARGETED_FINAL_REPAIR_FOCUS_AREAS}

    temperature = min(_safe_float(base_temperature) or 0.8, 0.62)
    if "dialogue" in normalized_focus_areas:
        temperature += 0.02
    if "cliffhanger" in normalized_focus_areas:
        temperature += 0.01
    return round(max(0.5, min(temperature, 0.65)), 2)


def should_prefer_targeted_final_repair_candidate(
    selected_candidate: Optional[Mapping[str, Any]],
    repair_candidate: Optional[Mapping[str, Any]],
) -> bool:
    if not isinstance(repair_candidate, Mapping):
        return False
    if not isinstance(selected_candidate, Mapping):
        return True

    selected_priority = QUALITY_GATE_DECISION_PRIORITY.get(
        _safe_text(selected_candidate.get("quality_gate_decision")),
        0,
    )
    repair_priority = QUALITY_GATE_DECISION_PRIORITY.get(
        _safe_text(repair_candidate.get("quality_gate_decision")),
        0,
    )
    selected_target_word_count = int(
        selected_candidate.get("target_word_count")
        or repair_candidate.get("target_word_count")
        or 0
    )
    selected_word_count = int(selected_candidate.get("word_count") or 0)
    repair_word_count = int(repair_candidate.get("word_count") or 0)
    selected_overall_score = _safe_float(selected_candidate.get("overall_score"))
    repair_overall_score = _safe_float(repair_candidate.get("overall_score"))
    quality_drop = max(selected_overall_score - repair_overall_score, 0.0)

    selected_failed_metric_count = _extract_failed_metric_count(selected_candidate)
    repair_failed_metric_count = _extract_failed_metric_count(repair_candidate)
    selected_in_window = is_candidate_word_count_in_target_window(selected_candidate)
    repair_in_window = is_candidate_word_count_in_target_window(repair_candidate)

    selected_word_delta = abs(selected_word_count - selected_target_word_count)
    repair_word_delta = abs(repair_word_count - selected_target_word_count)
    delta_improvement = selected_word_delta - repair_word_delta
    substantial_improvement = (
        selected_target_word_count > 0
        and delta_improvement >= max(120, int(selected_target_word_count * 0.10))
    )
    target_upper_bound = 0
    if selected_target_word_count > 0:
        _, target_upper_bound = _resolve_target_word_bounds(selected_target_word_count)
    severe_upper_bound = max(target_upper_bound + 120, int(target_upper_bound * 1.1)) if target_upper_bound > 0 else 0
    repair_soft_upper_bound = (
        target_upper_bound + max(100, int(selected_target_word_count * 0.08))
        if target_upper_bound > 0
        else 0
    )
    selected_severely_over_budget = severe_upper_bound > 0 and selected_word_count > severe_upper_bound
    repair_near_target_ceiling = repair_soft_upper_bound > 0 and 0 < repair_word_count <= repair_soft_upper_bound

    if repair_priority > selected_priority:
        return quality_drop <= 6.0
    if repair_in_window and not selected_in_window:
        return quality_drop <= 6.0
    if selected_severely_over_budget and repair_near_target_ceiling and substantial_improvement:
        return quality_drop <= 6.0
    if repair_priority < selected_priority:
        return False
    if repair_failed_metric_count < selected_failed_metric_count:
        return quality_drop <= 4.5

    selected_focus_areas = set(_extract_failed_focus_areas_from_candidate(selected_candidate))
    repair_focus_areas = set(_extract_failed_focus_areas_from_candidate(repair_candidate))
    same_focus_profile = bool(repair_focus_areas) and selected_focus_areas == repair_focus_areas
    selected_near_target_ceiling = repair_soft_upper_bound > 0 and 0 < selected_word_count <= repair_soft_upper_bound
    if (
        repair_failed_metric_count == selected_failed_metric_count
        and selected_near_target_ceiling
        and repair_near_target_ceiling
        and same_focus_profile
        and "cliffhanger" in repair_focus_areas
        and repair_overall_score >= selected_overall_score + 1.5
        and repair_word_delta <= selected_word_delta + 40
    ):
        return True
    return False


def should_adopt_targeted_final_repair_candidate(
    seed_candidate: Optional[Mapping[str, Any]],
    repair_candidate: Optional[Mapping[str, Any]],
) -> bool:
    if not isinstance(repair_candidate, Mapping):
        return False
    if not isinstance(seed_candidate, Mapping):
        return True

    seed_failed_metric_count = _extract_failed_metric_count(seed_candidate)
    repair_failed_metric_count = _extract_failed_metric_count(repair_candidate)
    if repair_failed_metric_count > seed_failed_metric_count:
        return False

    seed_overall_score = _safe_float(seed_candidate.get("overall_score"))
    repair_overall_score = _safe_float(repair_candidate.get("overall_score"))
    seed_target_word_count = int(
        seed_candidate.get("target_word_count")
        or repair_candidate.get("target_word_count")
        or 0
    )
    seed_word_count = int(seed_candidate.get("word_count") or 0)
    repair_word_count = int(repair_candidate.get("word_count") or 0)
    seed_word_delta = abs(seed_word_count - seed_target_word_count)
    repair_word_delta = abs(repair_word_count - seed_target_word_count)
    seed_in_window = is_candidate_word_count_in_target_window(seed_candidate)
    repair_in_window = is_candidate_word_count_in_target_window(repair_candidate)

    if seed_in_window and not repair_in_window and repair_failed_metric_count >= seed_failed_metric_count:
        return False
    if repair_failed_metric_count == seed_failed_metric_count:
        if repair_word_delta > seed_word_delta:
            return False
        if repair_overall_score + 0.3 < seed_overall_score:
            return False
    elif repair_overall_score + 4.0 < seed_overall_score and repair_word_delta >= seed_word_delta:
        return False
    return True


def should_keep_targeted_final_repair_candidate(
    seed_candidate: Optional[Mapping[str, Any]],
    repair_candidate: Optional[Mapping[str, Any]],
) -> bool:
    if not isinstance(repair_candidate, Mapping):
        return False
    if not isinstance(seed_candidate, Mapping):
        return True

    target_word_count = int(
        seed_candidate.get("target_word_count")
        or repair_candidate.get("target_word_count")
        or 0
    )
    if target_word_count <= 0:
        return True

    seed_word_count = int(seed_candidate.get("word_count") or 0)
    repair_word_count = int(repair_candidate.get("word_count") or 0)
    target_lower_bound, target_upper_bound = _resolve_target_word_bounds(target_word_count)
    repair_hard_lower_bound = max(
        max(200, target_lower_bound - max(60, int(target_word_count * 0.05))),
        int(max(min(seed_word_count, target_upper_bound), target_lower_bound) * 0.72),
    )
    if 0 < repair_word_count < repair_hard_lower_bound:
        return False

    seed_failed_metric_count = _extract_failed_metric_count(seed_candidate)
    repair_failed_metric_count = _extract_failed_metric_count(repair_candidate)
    if repair_failed_metric_count > seed_failed_metric_count + 1:
        return False

    seed_overall_score = _safe_float(seed_candidate.get("overall_score"))
    repair_overall_score = _safe_float(repair_candidate.get("overall_score"))
    if repair_overall_score + 10.0 < seed_overall_score and repair_failed_metric_count >= seed_failed_metric_count:
        return False
    return True



def _targeted_final_repair_seed_sort_key(candidate: Mapping[str, Any]) -> tuple[int, int, int, int, float]:
    target_word_count = int(candidate.get("target_word_count") or 0)
    current_word_count = int(candidate.get("word_count") or 0)
    word_delta = abs(current_word_count - target_word_count)
    return (
        1 if is_candidate_word_count_in_target_window(candidate) else 0,
        1 if _is_word_budget_repair_candidate(candidate) else 0,
        -word_delta,
        -_extract_failed_metric_count(candidate),
        _safe_float(candidate.get("overall_score")),
    )



def select_targeted_final_repair_seed_candidate(
    selected_candidate: Optional[Mapping[str, Any]],
    candidates: Sequence[Mapping[str, Any]],
) -> Optional[Mapping[str, Any]]:
    eligible_candidates: list[Mapping[str, Any]] = []

    if _can_seed_targeted_final_repair_candidate(selected_candidate, relaxed=False):
        eligible_candidates.append(selected_candidate)
    elif _can_seed_targeted_final_repair_candidate(selected_candidate, relaxed=True):
        eligible_candidates.append(selected_candidate)

    for candidate in candidates:
        if not isinstance(candidate, Mapping):
            continue
        candidate_index = int(candidate.get("candidate_index") or 0)
        selected_candidate_index = int(selected_candidate.get("candidate_index") or 0) if isinstance(selected_candidate, Mapping) else 0
        if selected_candidate_index > 0 and candidate_index == selected_candidate_index:
            continue
        if _can_seed_targeted_final_repair_candidate(candidate, relaxed=True):
            eligible_candidates.append(candidate)

    if eligible_candidates:
        return max(eligible_candidates, key=_targeted_final_repair_seed_sort_key)

    if _can_fallback_seed_targeted_final_repair_candidate(selected_candidate):
        return selected_candidate

    return None


def build_word_budget_repair_suffix(
    *,
    quality_metrics: Optional[Mapping[str, Any]] = None,
    quality_gate_plan: Optional[Mapping[str, Any]] = None,
    current_content: Any = None,
    target_word_count: int,
    attempt_index: int,
    source: str,
) -> str:
    if target_word_count <= 0:
        return ""

    runtime_context = _extract_runtime_context(quality_metrics)
    candidate_selection = _extract_candidate_selection(quality_metrics)
    failed_metric_labels, failed_focus_areas = _extract_failed_metric_labels_and_focus_areas(quality_gate_plan)
    structural_focus_areas = [
        focus_area
        for focus_area in failed_focus_areas
        if focus_area in STRUCTURAL_REPAIR_FOCUS_AREAS
    ]

    current_word_count = int(candidate_selection.get("word_count") or 0)
    target_lower_bound, target_upper_bound = _resolve_target_word_bounds(target_word_count)
    opening_anchor, closing_anchor = _extract_edge_anchors(current_content)
    lines = [
        f"Word-budget repair pass #{attempt_index}",
        f"- Rewrite the same {source} draft from scratch into {target_lower_bound}-{target_upper_bound} Chinese characters; do not exceed {target_upper_bound}.",
        "- Preserve the same POV, continuity, established facts, and chapter mission.",
        "- Protect the first-paragraph incident hook and the final unresolved hook; cut the middle before weakening either edge.",
        "- Keep only the beats that directly advance conflict, rule payoff, outline progression, and the closing hook.",
        "- Remove recap, repeated explanation, and side detours; convert exposition into action, dialogue, and consequence.",
        "- Compress from the middle first: cut transition padding, duplicate reactions, and explanatory recap before trimming the verification beat, outline turn, or final hook.",
        "- Preserve the last-two-paragraph skeleton: the penultimate paragraph must land the chapter turn or cost, and the final paragraph must deliver the unresolved hook instead of a soft wrap-up.",
        "- Treat the closing 3-5 lines as protected runway: first land the irreversible turn or cost, then narrow into one concrete unanswered pressure, and stop on that pressure.",
        "- If dialogue is present, keep at least one two-sided pressure exchange that changes leverage; cut monologue exposition first.",
        "- End immediately once the hook or payoff lands; do not append cooldown paragraphs.",
        "- Hard constraint: output continuous in-scene chapter prose only; no title, outline bullets, bracket notes, or meta commentary.",
    ]
    if opening_anchor:
        lines.append(f"- Preserve this opening anchor beat in equivalent dramatic form: {opening_anchor}")
    if closing_anchor:
        lines.append(f"- Preserve this closing hook beat in equivalent dramatic form: {closing_anchor}")
    if current_word_count > target_upper_bound:
        lines.append(
            f"- The previous draft ran to about {current_word_count} chars; compress structure aggressively and merge overlapping beats."
        )
    elif 0 < current_word_count < target_lower_bound:
        lines.append(
            f"- The previous draft landed short at about {current_word_count} chars; expand with concrete action, consequence, and one stronger closing turn while staying inside the target range."
        )
    if failed_metric_labels:
        lines.append(f"- Repair the weak metrics while compressing: {' / '.join(failed_metric_labels[:3])}")
    if {"rule_grounding", "cliffhanger"}.issubset(set(structural_focus_areas).union(set(failed_focus_areas))):
        lines.append(
            "- Joint compression rule: keep the rule trigger and the closing hook in the same causal chain; cut setup or commentary before cutting either the on-page rule consequence or the final unresolved pressure."
        )
    if {"opening", "rule_grounding"}.issubset(set(structural_focus_areas).union(set(failed_focus_areas))):
        lines.append(
            "- Joint compression rule: preserve one sharp opening anomaly or demand and the immediate rule consequence it reveals; cut backstory or atmosphere before cutting either the hook or the grounded constraint."
        )
    if {"opening", "rule_grounding", "cliffhanger"}.issubset(set(structural_focus_areas).union(set(failed_focus_areas))):
        lines.append(
            "- Joint compression rule: preserve the causal chain of opening anomaly or task -> grounded rule consequence -> unresolved closing hook; cut recap, atmosphere, and side explanation before cutting any link in that chain."
        )
    if {"dialogue", "cliffhanger"}.issubset(set(structural_focus_areas).union(set(failed_focus_areas))):
        lines.append(
            "- Joint compression rule: preserve one decisive back-and-forth exchange and the unresolved hook it creates; cut explanation around the dialogue before cutting the leverage shift or closing spike itself."
        )
    if set(failed_focus_areas) == {"cliffhanger"}:
        lines.append(
            "- Cliffhanger compression rule: preserve one visible cost or turn in the penultimate paragraph and one newly introduced unresolved trigger in the final 1-2 lines; cut reflection and recap before cutting either beat."
        )
        lines.append(
            "- Cliffhanger novelty rule: after compression, the final hook must add one fresh external trigger, order, reveal, timer change, arrival, or access cutoff; do not spend the last 1-2 lines paraphrasing an already-known danger."
        )
    if {"opening", "conflict", "cliffhanger"}.issubset(set(structural_focus_areas).union(set(failed_focus_areas))):
        lines.append(
            "- Three-beat compression rule: preserve one sharp opening anomaly or task, one visible blocker escalation in the middle, and one unresolved closing spike; merge or delete every beat that does not serve this chain."
        )
    lines.extend(
        _build_quality_gate_focus_repair_lines(
            failed_focus_areas,
            hard_mode=True,
        )
    )
    lines.extend(
        _build_structural_repair_lines(
            structural_focus_areas,
            runtime_context,
            hard_mode=True,
        )
    )
    lines.extend(
        _build_continuity_repair_lines(
            quality_gate_plan,
            runtime_context,
            hard_mode=True,
        )
    )
    lines.extend(_build_focus_strategy_lines(runtime_context))
    return "\n".join(lines)


def resolve_word_budget_repair_temperature(
    base_temperature: float,
    *,
    quality_metrics: Optional[Mapping[str, Any]] = None,
) -> float:
    runtime_context = _extract_runtime_context(quality_metrics)
    quality_preset = _safe_text(runtime_context.get("quality_preset"))
    creative_mode = _safe_text(runtime_context.get("creative_mode"))

    temperature = min(_safe_float(base_temperature) or 0.8, 0.62)
    if quality_preset == "plot_drive":
        temperature -= 0.06
    elif quality_preset == "clean_prose":
        temperature -= 0.08

    if creative_mode in {"hook", "suspense", "payoff"}:
        temperature -= 0.04

    return round(max(0.42, min(temperature, 0.62)), 2)



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

    has_quality_repair_pressure = _candidate_has_explicit_quality_repair_pressure(candidate)
    decision = _safe_text(candidate.get("quality_gate_decision"))
    if decision == "auto_repair":
        return has_quality_repair_pressure

    target_word_count = int(candidate.get("target_word_count") or 0)
    current_word_count = int(candidate.get("word_count") or 0)
    if target_word_count > 0:
        target_lower_bound, target_upper_bound = _resolve_target_word_bounds(target_word_count)
        if current_word_count > target_upper_bound or (0 < current_word_count < target_lower_bound):
            return has_quality_repair_pressure

    return False
