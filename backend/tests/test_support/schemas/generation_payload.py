from __future__ import annotations

from datetime import datetime
from typing import Any, Dict, Literal, Mapping, Optional

from pydantic import BaseModel, Field

from tests.test_support.schemas.quality import (
    StoryQualityMetricsPayload,
    build_quality_gate_decision,
    build_story_repair_guidance,
    normalize_story_quality_metrics_payload,
)


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


class ChapterGenerationQualityHistoryPayload(BaseModel):
    log_type: Literal["chapter_generation_quality_v1"] = "chapter_generation_quality_v1"
    preview: str
    quality_metrics: StoryQualityMetricsPayload = Field(default_factory=StoryQualityMetricsPayload)
    generated_at: str
    content_applied: bool
    attempt_state: str
    story_runtime_snapshot: Optional[Dict[str, Any]] = None
    story_runtime_contract: Optional[Dict[str, Any]] = None


class ChapterGenerationStreamResultPayload(BaseModel):
    word_count: int
    analysis_task_id: Optional[str] = None
    quality_metrics: Optional[StoryQualityMetricsPayload] = None
    quality_gate_action: Optional[str] = None
    quality_gate_message: Optional[str] = None
    content_applied: bool
    chapter_status: str
    saved_word_count: int
    hard_gate_blocked: bool = False
    story_runtime_contract: Optional[Dict[str, Any]] = None
    candidate_draft: Optional[Dict[str, Any]] = None


class ChapterRegenerationStreamResultPayload(BaseModel):
    task_id: str
    word_count: int
    version_number: int
    auto_applied: bool
    diff_stats: Dict[str, Any] = Field(default_factory=dict)
    story_runtime_contract: Optional[Dict[str, Any]] = None


def build_chapter_generation_quality_history_payload(
    content: str,
    metrics: Optional[Dict[str, Any]],
    *,
    content_applied: bool = True,
    attempt_state: Optional[str] = None,
    story_runtime_contract: Optional[Dict[str, Any]] = None,
) -> ChapterGenerationQualityHistoryPayload:
    normalized_metrics = attach_story_runtime_contract(metrics, story_runtime_contract)
    if normalized_metrics and not isinstance(normalized_metrics.get("repair_guidance"), dict):
        normalized_metrics["repair_guidance"] = build_story_repair_guidance(normalized_metrics, scope="chapter")
    if normalized_metrics and not isinstance(normalized_metrics.get("quality_gate"), dict):
        normalized_metrics["quality_gate"] = build_quality_gate_decision(normalized_metrics, scope="chapter")

    quality_metrics_payload = normalize_story_quality_metrics_payload(normalized_metrics) or StoryQualityMetricsPayload()
    resolved_attempt_state = str(attempt_state or ("applied" if content_applied else "candidate")).strip() or (
        "applied" if content_applied else "candidate"
    )
    runtime_contract_payload = quality_metrics_payload.story_runtime_contract
    runtime_snapshot = (
        quality_metrics_payload.quality_runtime_context.model_dump(exclude_none=True)
        if quality_metrics_payload.quality_runtime_context is not None
        else None
    )
    if not isinstance(runtime_snapshot, dict) or not runtime_snapshot:
        runtime_snapshot = extract_story_runtime_snapshot_from_contract(runtime_contract_payload)

    return ChapterGenerationQualityHistoryPayload(
        preview=content[:500] if len(content) > 500 else content,
        quality_metrics=quality_metrics_payload,
        generated_at=datetime.now().isoformat(),
        content_applied=bool(content_applied),
        attempt_state=resolved_attempt_state,
        story_runtime_snapshot=runtime_snapshot if isinstance(runtime_snapshot, dict) and runtime_snapshot else None,
        story_runtime_contract=(
            runtime_contract_payload if isinstance(runtime_contract_payload, dict) and runtime_contract_payload else None
        ),
    )


def build_chapter_generation_stream_result_payload(
    *,
    word_count: int,
    analysis_task_id: Optional[str],
    quality_metrics: Optional[Dict[str, Any]],
    quality_gate_action: Optional[str],
    quality_gate_message: Optional[str],
    content_applied: bool,
    chapter_status: str,
    saved_word_count: int,
    hard_gate_blocked: bool,
    story_runtime_contract: Optional[Dict[str, Any]] = None,
    candidate_draft: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    normalized_contract = attach_story_runtime_result_payload({}, story_runtime_contract).get("story_runtime_contract")
    payload = ChapterGenerationStreamResultPayload(
        word_count=word_count,
        analysis_task_id=analysis_task_id,
        quality_metrics=normalize_story_quality_metrics_payload(quality_metrics) if isinstance(quality_metrics, dict) else None,
        quality_gate_action=quality_gate_action,
        quality_gate_message=quality_gate_message,
        content_applied=content_applied,
        chapter_status=chapter_status,
        saved_word_count=saved_word_count,
        hard_gate_blocked=hard_gate_blocked,
        story_runtime_contract=normalized_contract if isinstance(normalized_contract, dict) else None,
        candidate_draft=dict(candidate_draft) if isinstance(candidate_draft, dict) and candidate_draft else None,
    )
    return payload.model_dump(exclude_none=True)


def build_chapter_regeneration_stream_result_payload(
    *,
    task_id: str,
    word_count: int,
    version_number: int,
    auto_applied: bool,
    diff_stats: Optional[Dict[str, Any]],
    story_runtime_contract: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    normalized_contract = attach_story_runtime_result_payload({}, story_runtime_contract).get("story_runtime_contract")
    payload = ChapterRegenerationStreamResultPayload(
        task_id=task_id,
        word_count=word_count,
        version_number=version_number,
        auto_applied=auto_applied,
        diff_stats=diff_stats if isinstance(diff_stats, dict) else {},
        story_runtime_contract=normalized_contract if isinstance(normalized_contract, dict) else None,
    )
    return payload.model_dump(exclude_none=True)


