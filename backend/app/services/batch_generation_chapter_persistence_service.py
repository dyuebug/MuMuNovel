"""??????????? helper?"""
from __future__ import annotations

from datetime import datetime
from typing import Any, Dict, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.chapter import Chapter
from app.models.chapter_draft_attempt import ChapterDraftAttempt
from app.models.generation_history import GenerationHistory
from app.models.project import Project
from app.schemas.generation_payload import build_chapter_generation_quality_history_payload
from app.services.foreshadow_service import foreshadow_service


logger = get_logger(__name__)


def _normalize_json_payload(value: Any) -> Any:
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, dict):
        return {str(key): _normalize_json_payload(item) for key, item in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [_normalize_json_payload(item) for item in value]
    if hasattr(value, "model_dump"):
        return _normalize_json_payload(value.model_dump())
    if hasattr(value, "dict"):
        return _normalize_json_payload(value.dict())
    return str(value)


def build_batch_chapter_draft_attempt(
    *,
    project_id: str,
    chapter_id: Optional[str],
    batch_task_id: Optional[str] = None,
    source: str,
    attempt_state: str,
    quality_gate_action: Optional[str],
    quality_gate_decision: Optional[str],
    full_content: str,
    summary_preview: Optional[str] = None,
    quality_metrics: Optional[Dict[str, Any]] = None,
    repair_payload: Optional[Dict[str, Any]] = None,
) -> ChapterDraftAttempt:
    normalized_content = str(full_content or "")
    normalized_summary = str(summary_preview or "").strip()
    if not normalized_summary and normalized_content:
        normalized_summary = normalized_content[:220]

    normalized_repair_payload: Optional[Dict[str, Any]]
    if isinstance(repair_payload, dict):
        normalized_repair_payload = dict(repair_payload)
    else:
        normalized_repair_payload = {}
    if normalized_content:
        normalized_repair_payload.setdefault("candidate_full_content", normalized_content)
        normalized_repair_payload["content_complete"] = True

    return ChapterDraftAttempt(
        project_id=project_id,
        chapter_id=chapter_id,
        batch_task_id=batch_task_id,
        source=source,
        attempt_state=str(attempt_state or "candidate"),
        quality_gate_action=quality_gate_action,
        quality_gate_decision=quality_gate_decision,
        word_count=len(normalized_content),
        summary_preview=normalized_summary[:500] or None,
        content_preview=normalized_content[:4000] or None,
        quality_metrics=_normalize_json_payload(quality_metrics) if isinstance(quality_metrics, dict) else None,
        repair_payload=_normalize_json_payload(normalized_repair_payload) if normalized_repair_payload else None,
    )


def _build_generation_history_payload(
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


async def apply_generated_batch_chapter_candidate(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
    project: Project,
    write_lock,
    full_content: str,
    word_count: int,
    quality_metrics: Optional[Dict[str, Any]] = None,
    story_runtime_contract: Optional[Dict[str, Any]] = None,
) -> None:
    async with write_lock:
        old_word_count = chapter.word_count or 0
        chapter.content = full_content
        chapter.word_count = word_count
        chapter.status = "completed"
        project.current_words = (project.current_words or 0) - old_word_count + word_count

        history = GenerationHistory(
            project_id=chapter.project_id,
            chapter_id=chapter.id,
            prompt=f"????: ?{chapter.chapter_number}??{chapter.title}?",
            generated_content=_build_generation_history_payload(
                full_content,
                quality_metrics if isinstance(quality_metrics, dict) else None,
                story_runtime_contract=story_runtime_contract,
            ),
            model="default",
        )
        db_session.add(history)

        await db_session.commit()
        await db_session.refresh(chapter)

    logger.info(f"???????: ?{chapter.chapter_number}??? {word_count} ?")

    try:
        async with write_lock:
            plant_result = await foreshadow_service.auto_plant_pending_foreshadows(
                db=db_session,
                project_id=chapter.project_id,
                chapter_id=chapter.id,
                chapter_number=chapter.chapter_number,
                chapter_content=full_content,
            )
        if plant_result.get('planted_count', 0) > 0:
            logger.info(
                f"???? - ????????? {plant_result['planted_count']} ?"
            )
    except Exception as plant_error:
        logger.warning(f"???? - ??????????: {str(plant_error)}")
