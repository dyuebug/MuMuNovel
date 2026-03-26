from __future__ import annotations

from typing import Any, Dict, Mapping, Optional


def _normalize_json_value(value: Any) -> Any:
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, Mapping):
        return {str(key): _normalize_json_value(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [_normalize_json_value(item) for item in value]
    return str(value)


def extract_story_runtime_snapshot_from_contract(
    story_runtime_contract: Optional[Mapping[str, Any]],
) -> Optional[Dict[str, Any]]:
    if not isinstance(story_runtime_contract, Mapping):
        return None

    guidance = story_runtime_contract.get("guidance")
    blueprint = story_runtime_contract.get("blueprint")
    if not isinstance(guidance, Mapping) and not isinstance(blueprint, Mapping):
        return None

    snapshot: Dict[str, Any] = {}
    if isinstance(guidance, Mapping):
        for field_name in (
            "creative_mode",
            "story_focus",
            "plot_stage",
            "story_creation_brief",
            "quality_preset",
            "quality_notes",
        ):
            value = guidance.get(field_name)
            if value is not None:
                snapshot[field_name] = value

    if isinstance(blueprint, Mapping):
        snapshot.update(
            {
                "story_long_term_goal": blueprint.get("long_term_goal") or "",
                "chapter_count": blueprint.get("chapter_count"),
                "current_chapter_number": blueprint.get("current_chapter_number"),
                "target_word_count": blueprint.get("target_word_count"),
                "character_focus": list(blueprint.get("character_focus_names") or []),
                "foreshadow_payoff_plan": list(blueprint.get("foreshadow_payoff_plan") or []),
                "character_state_ledger": list(blueprint.get("character_state_ledger") or []),
                "relationship_state_ledger": list(blueprint.get("relationship_state_ledger") or []),
                "foreshadow_state_ledger": list(blueprint.get("foreshadow_state_ledger") or []),
                "organization_state_ledger": list(blueprint.get("organization_state_ledger") or []),
                "career_state_ledger": list(blueprint.get("career_state_ledger") or []),
            }
        )

    normalized_snapshot = _normalize_json_value(snapshot)
    return normalized_snapshot if isinstance(normalized_snapshot, dict) and normalized_snapshot else None


def attach_story_runtime_contract(
    metrics: Optional[Mapping[str, Any]],
    story_runtime_contract: Optional[Mapping[str, Any]],
) -> Dict[str, Any]:
    normalized_metrics = dict(metrics or {}) if isinstance(metrics, Mapping) else {}
    normalized_contract = (
        _normalize_json_value(story_runtime_contract)
        if isinstance(story_runtime_contract, Mapping)
        else None
    )
    if not isinstance(normalized_contract, dict) or not normalized_contract:
        return normalized_metrics

    normalized_metrics["story_runtime_contract"] = normalized_contract
    existing_runtime_context = normalized_metrics.get("quality_runtime_context")
    if not isinstance(existing_runtime_context, Mapping) or not existing_runtime_context:
        runtime_snapshot = extract_story_runtime_snapshot_from_contract(normalized_contract)
        if runtime_snapshot:
            normalized_metrics["quality_runtime_context"] = runtime_snapshot
    return normalized_metrics


def attach_story_runtime_result_payload(
    payload: Optional[Mapping[str, Any]],
    story_runtime_contract: Optional[Mapping[str, Any]],
) -> Dict[str, Any]:
    normalized_payload = dict(payload or {}) if isinstance(payload, Mapping) else {}
    normalized_contract = (
        _normalize_json_value(story_runtime_contract)
        if isinstance(story_runtime_contract, Mapping)
        else None
    )
    if isinstance(normalized_contract, dict) and normalized_contract:
        normalized_payload["story_runtime_contract"] = normalized_contract
    return normalized_payload
