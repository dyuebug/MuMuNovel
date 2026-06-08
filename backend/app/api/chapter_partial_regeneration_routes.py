"""Partial chapter regeneration routes."""

import asyncio

from fastapi import APIRouter, Depends, HTTPException, Request
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.chapter_route_helpers import (
    load_accessible_chapter_or_404,
    require_authenticated_user_id,
)
from app.api.settings import get_user_ai_service
from app.database import get_db
from app.logger import get_logger
from app.schemas.chapter import PartialRegenerateRequest
from app.services.ai_service import AIService
from app.services.chapter_content_apply_service import apply_chapter_content_update
from app.services.chapter_generated_text_service import (
    contains_chapter_workflow_meta_text,
    sanitize_generated_narrative_text,
)
from app.services.partial_regeneration_service import prepare_partial_regeneration
from app.utils.sse_response import WizardProgressTracker, create_sse_response

router = APIRouter(prefix="/chapters", tags=["章节管理"])

logger = get_logger(__name__)

_PARTIAL_REGENERATE_PREFIXES_TO_REMOVE = [
    "重写后：",
    "重写后:",
    "改写后：",
    "改写后:",
    "以下是重写后的内容：",
    "以下是重写后的内容:",
    "重写内容：",
    "重写内容:",
]


def normalize_partial_regeneration_output(text: str) -> str:
    cleaned = (text or "").strip()
    for prefix in _PARTIAL_REGENERATE_PREFIXES_TO_REMOVE:
        if cleaned.startswith(prefix):
            cleaned = cleaned[len(prefix):].strip()
            break

    if (cleaned.startswith('"') and cleaned.endswith('"')) or (
        cleaned.startswith("'") and cleaned.endswith("'")
    ):
        cleaned = cleaned[1:-1]
    if (cleaned.startswith("「") and cleaned.endswith("」")) or (
        cleaned.startswith("『") and cleaned.endswith("』")
    ):
        cleaned = cleaned[1:-1]
    return cleaned.strip()


async def partial_regenerate_stream_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    partial_request: PartialRegenerateRequest,
    db_session,
    user_ai_service: AIService,
):
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    try:
        preparation = await prepare_partial_regeneration(
            db_session,
            chapter=chapter,
            partial_request=partial_request,
            user_id=user_id,
        )
    except HTTPException:
        raise
    except Exception as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc

    async def event_generator():
        tracker = WizardProgressTracker("Partial Rewrite")

        try:
            yield await tracker.start()
            yield await tracker.loading("Preparing rewrite context...", 0.3)
            yield await tracker.preparing("Starting generation...")

            full_content = ""
            chunk_count = 0

            yield await tracker.generating(
                current_chars=0,
                estimated_total=preparation.target_words,
            )

            async for chunk in user_ai_service.generate_text_stream(
                prompt=preparation.prompt,
                max_tokens=preparation.max_tokens,
            ):
                full_content += chunk
                chunk_count += 1

                yield await tracker.generating_chunk(chunk)

                if chunk_count % 5 == 0:
                    yield await tracker.generating(
                        current_chars=len(full_content),
                        estimated_total=preparation.target_words,
                        message=f"Generating rewrite... {len(full_content)} chars",
                    )

                await asyncio.sleep(0)

            full_content = normalize_partial_regeneration_output(full_content)
            full_content, removed_meta_lines = sanitize_generated_narrative_text(
                full_content
            )
            if removed_meta_lines > 0:
                logger.warning(
                    "Partial regeneration removed %s workflow meta lines: chapter_id=%s",
                    removed_meta_lines,
                    chapter_id,
                )
            if not full_content.strip():
                raise ValueError("Rewrite result is empty after sanitization")
            if contains_chapter_workflow_meta_text(full_content):
                raise ValueError("Rewrite result still contains workflow meta text")

            new_word_count = len(full_content)
            logger.info(
                "Partial regeneration completed: %s chars -> %s chars",
                preparation.original_word_count,
                new_word_count,
            )

            yield await tracker.complete("Rewrite complete")
            yield await tracker.result(
                {
                    "new_text": full_content,
                    "word_count": new_word_count,
                    "original_word_count": preparation.original_word_count,
                    "start_position": preparation.start_position,
                    "end_position": preparation.end_position,
                }
            )
            yield await tracker.done()
        except Exception as exc:
            logger.error("Partial regeneration failed: %s", str(exc), exc_info=True)
            yield await tracker.error(str(exc))

    return create_sse_response(event_generator())


async def apply_partial_regenerate_with_default_route_wiring(
    *,
    chapter_id: str,
    request: Request,
    apply_request: dict,
    db_session,
):
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db_session,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    new_text_raw = str(apply_request.get("new_text", "") or "")
    start_position = apply_request.get("start_position", 0)
    end_position = apply_request.get("end_position", 0)

    new_text, removed_meta_lines = sanitize_generated_narrative_text(new_text_raw)
    if removed_meta_lines > 0:
        logger.warning(
            "Partial regenerate apply removed %s workflow meta lines: chapter_id=%s",
            removed_meta_lines,
            chapter_id,
        )
    if not new_text:
        raise HTTPException(status_code=400, detail="改写内容为空")
    if contains_chapter_workflow_meta_text(new_text):
        raise HTTPException(status_code=400, detail="改写内容仍包含工作流提示文本")

    content_length = len(chapter.content or "")
    if start_position < 0 or end_position > content_length or start_position >= end_position:
        raise HTTPException(status_code=400, detail="改写位置非法")

    new_content = (
        (chapter.content or "")[:start_position]
        + new_text
        + (chapter.content or "")[end_position:]
    )
    apply_result = await apply_chapter_content_update(
        db_session,
        chapter=chapter,
        content=new_content,
    )

    logger.info(
        "Partial regenerate applied: chapter_id=%s, %s -> %s",
        chapter_id,
        apply_result.old_word_count,
        apply_result.new_word_count,
    )

    return {
        "success": True,
        "chapter_id": chapter_id,
        "word_count": apply_result.new_word_count,
        "old_word_count": apply_result.old_word_count,
        "message": "局部改写已应用",
    }


@router.post("/{chapter_id}/partial-regenerate-stream", summary="局部重写章节片段")
async def partial_regenerate_stream(
    chapter_id: str,
    request: Request,
    partial_request: PartialRegenerateRequest,
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    """对章节选中片段进行局部重写并返回 SSE 流。"""
    return await partial_regenerate_stream_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        partial_request=partial_request,
        db_session=db,
        user_ai_service=user_ai_service,
    )


@router.post("/{chapter_id}/apply-partial-regenerate", summary="应用局部改写")
async def apply_partial_regenerate(
    chapter_id: str,
    request: Request,
    apply_request: dict,
    db: AsyncSession = Depends(get_db),
):
    """将局部重写结果写回到章节内容。"""
    return await apply_partial_regenerate_with_default_route_wiring(
        chapter_id=chapter_id,
        request=request,
        apply_request=apply_request,
        db_session=db,
    )
