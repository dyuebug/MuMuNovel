"""章节局部重写相关 API。"""

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
from app.services.chapter_generated_text_service import (
    contains_chapter_workflow_meta_text,
    sanitize_generated_narrative_text,
)
from app.services.chapter_content_apply_service import apply_chapter_content_update
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


def _normalize_partial_regeneration_output(text: str) -> str:
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


@router.post("/{chapter_id}/partial-regenerate-stream", summary="流式局部重写选中内容")
async def partial_regenerate_stream(
    chapter_id: str,
    request: Request,
    partial_request: PartialRegenerateRequest,
    db: AsyncSession = Depends(get_db),
    user_ai_service: AIService = Depends(get_user_ai_service),
):
    """对章节中选中的部分内容进行流式重写。"""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    try:
        preparation = await prepare_partial_regeneration(
            db,
            chapter=chapter,
            partial_request=partial_request,
            user_id=user_id,
        )
    except HTTPException:
        raise
    except Exception as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc

    async def event_generator():
        tracker = WizardProgressTracker("局部重写")

        try:
            yield await tracker.start()
            yield await tracker.loading("准备重写上下文...", 0.3)
            yield await tracker.preparing("开始生成...")

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
                        message=f"正在重写中... 已生成 {len(full_content)} 字",
                    )

                await asyncio.sleep(0)

            full_content = _normalize_partial_regeneration_output(full_content)
            full_content, removed_meta_lines = sanitize_generated_narrative_text(full_content)
            if removed_meta_lines > 0:
                logger.warning(
                    "⚠️ 局部重写检测到流程化元文本，已清理 %s 行: chapter_id=%s",
                    removed_meta_lines,
                    chapter_id,
                )
            if not full_content.strip():
                raise ValueError("重写结果为空或仅包含流程化元文本，请重试")
            if contains_chapter_workflow_meta_text(full_content):
                raise ValueError("重写结果包含流程化元文本，请重试")

            new_word_count = len(full_content)
            logger.info(
                "✅ 局部重写完成: 原文%s字 -> 新文%s字",
                preparation.original_word_count,
                new_word_count,
            )

            yield await tracker.complete("重写完成！")
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
            logger.error(f"❌ 局部重写失败: {str(exc)}", exc_info=True)
            yield await tracker.error(str(exc))

    return create_sse_response(event_generator())

@router.post("/{chapter_id}/apply-partial-regenerate", summary="应用局部重写结果")
async def apply_partial_regenerate(
    chapter_id: str,
    request: Request,
    apply_request: dict,
    db: AsyncSession = Depends(get_db),
):
    """将局部重写结果写回章节内容。"""
    user_id = require_authenticated_user_id(request)
    chapter = await load_accessible_chapter_or_404(
        db=db,
        chapter_id=chapter_id,
        user_id=user_id,
    )

    new_text_raw = str(apply_request.get("new_text", "") or "")
    start_position = apply_request.get("start_position", 0)
    end_position = apply_request.get("end_position", 0)

    new_text, removed_meta_lines = sanitize_generated_narrative_text(new_text_raw)
    if removed_meta_lines > 0:
        logger.warning(
            "局部重写应用前检测到 %s 行流程化元文本: chapter_id=%s",
            removed_meta_lines,
            chapter_id,
        )
    if not new_text:
        raise HTTPException(status_code=400, detail="重写结果为空")
    if contains_chapter_workflow_meta_text(new_text):
        raise HTTPException(status_code=400, detail="重写结果包含流程化元文本")

    content_length = len(chapter.content or "")
    if start_position < 0 or end_position > content_length or start_position >= end_position:
        raise HTTPException(status_code=400, detail="选区范围无效")

    new_content = (chapter.content or "")[:start_position] + new_text + (chapter.content or "")[end_position:]
    apply_result = await apply_chapter_content_update(
        db,
        chapter=chapter,
        content=new_content,
    )

    logger.info(
        "已应用局部重写: chapter_id=%s, %s字 -> %s字",
        chapter_id,
        apply_result.old_word_count,
        apply_result.new_word_count,
    )

    return {
        "success": True,
        "chapter_id": chapter_id,
        "word_count": apply_result.new_word_count,
        "old_word_count": apply_result.old_word_count,
        "message": "局部重写结果已应用",
    }
