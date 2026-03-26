from __future__ import annotations

from datetime import datetime
from typing import Any, Dict, Literal, Optional

from pydantic import BaseModel, Field

from app.services.story_quality_feedback_service import (
    build_quality_gate_decision,
    build_story_repair_guidance,
)
from app.services.story_runtime_serialization_service import (
    attach_story_runtime_contract,
    attach_story_runtime_result_payload,
    extract_story_runtime_snapshot_from_contract,
)


class ChapterGenerationQualityHistoryPayload(BaseModel):
    log_type: Literal["chapter_generation_quality_v1"] = "chapter_generation_quality_v1"
    preview: str
    quality_metrics: Dict[str, Any] = Field(default_factory=dict)
    generated_at: str
    content_applied: bool
    attempt_state: str
    story_runtime_snapshot: Optional[Dict[str, Any]] = None
    story_runtime_contract: Optional[Dict[str, Any]] = None


class ChapterGenerationStreamResultPayload(BaseModel):
    word_count: int
    analysis_task_id: Optional[str] = None
    quality_metrics: Optional[Dict[str, Any]] = None
    quality_gate_action: Optional[str] = None
    quality_gate_message: Optional[str] = None
    content_applied: bool
    chapter_status: str
    saved_word_count: int
    hard_gate_blocked: bool = False
    story_runtime_contract: Optional[Dict[str, Any]] = None


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

    resolved_attempt_state = str(attempt_state or ("applied" if content_applied else "candidate")).strip() or (
        "applied" if content_applied else "candidate"
    )
    runtime_contract_payload = normalized_metrics.get("story_runtime_contract")
    runtime_snapshot = normalized_metrics.get("quality_runtime_context")
    if not isinstance(runtime_snapshot, dict) or not runtime_snapshot:
        runtime_snapshot = extract_story_runtime_snapshot_from_contract(runtime_contract_payload)

    return ChapterGenerationQualityHistoryPayload(
        preview=content[:500] if len(content) > 500 else content,
        quality_metrics=normalized_metrics,
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
) -> Dict[str, Any]:
    normalized_contract = attach_story_runtime_result_payload({}, story_runtime_contract).get("story_runtime_contract")
    payload = ChapterGenerationStreamResultPayload(
        word_count=word_count,
        analysis_task_id=analysis_task_id,
        quality_metrics=quality_metrics if isinstance(quality_metrics, dict) else None,
        quality_gate_action=quality_gate_action,
        quality_gate_message=quality_gate_message,
        content_applied=content_applied,
        chapter_status=chapter_status,
        saved_word_count=saved_word_count,
        hard_gate_blocked=hard_gate_blocked,
        story_runtime_contract=normalized_contract if isinstance(normalized_contract, dict) else None,
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
