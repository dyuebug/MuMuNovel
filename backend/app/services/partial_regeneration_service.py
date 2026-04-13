from __future__ import annotations

from dataclasses import dataclass
from typing import Optional

from fastapi import HTTPException
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.chapter import Chapter
from app.models.project_default_style import ProjectDefaultStyle
from app.models.writing_style import WritingStyle
from app.schemas.chapter import PartialRegenerateRequest
from app.services.prompt_service import PromptService
from app.services.writing_style_sync_service import sync_low_ai_presets

logger = get_logger(__name__)


@dataclass(frozen=True)
class PartialRegenerationPreparation:
    start_position: int
    end_position: int
    original_text: str
    original_word_count: int
    style_id: Optional[int]
    style_content: str
    prompt: str
    target_words: int
    max_tokens: int


def _normalize_partial_selection(
    *,
    chapter_content: str,
    partial_request: PartialRegenerateRequest,
) -> tuple[int, int, str]:
    content_length = len(chapter_content)
    start_position = partial_request.start_position
    end_position = partial_request.end_position

    if start_position >= content_length:
        raise HTTPException(status_code=400, detail="??????????")
    if end_position > content_length:
        raise HTTPException(status_code=400, detail="??????????")
    if start_position >= end_position:
        raise HTTPException(status_code=400, detail="????????????")

    actual_selected = chapter_content[start_position:end_position]
    selected_text = partial_request.selected_text
    if actual_selected == selected_text:
        return start_position, end_position, selected_text

    search_start = max(0, start_position - 50)
    search_end = min(content_length, end_position + 50)
    search_area = chapter_content[search_start:search_end]
    if selected_text not in search_area:
        raise HTTPException(
            status_code=400,
            detail="??????????????????????",
        )

    offset = search_area.find(selected_text)
    corrected_start = search_start + offset
    corrected_end = corrected_start + len(selected_text)
    logger.info(f"????????: {corrected_start}-{corrected_end}")
    return corrected_start, corrected_end, selected_text


async def _resolve_style_content(
    db_session: AsyncSession,
    *,
    project_id: str,
    requested_style_id: Optional[int],
    user_id: str,
) -> tuple[Optional[int], str]:
    await sync_low_ai_presets(db_session)

    style_id = requested_style_id
    if not style_id:
        default_style_result = await db_session.execute(
            select(ProjectDefaultStyle.style_id)
            .where(ProjectDefaultStyle.project_id == project_id)
        )
        default_style_id = default_style_result.scalar_one_or_none()
        if default_style_id:
            style_id = default_style_id
            logger.info(f"???? - ??????????: {style_id}")

    if not style_id:
        return None, ""

    style_result = await db_session.execute(
        select(WritingStyle).where(WritingStyle.id == style_id)
    )
    style = style_result.scalar_one_or_none()
    if style is None:
        return style_id, ""
    if style.user_id is not None and style.user_id != user_id:
        logger.warning(f"?? {style_id} ??????????")
        return style_id, ""

    style_content = style.prompt_content or ""
    style_type = "????" if style.user_id is None else "?????"
    logger.info(f"???? - ??????: {style.name} ({style_type})")
    return style_id, style_content


def _build_length_requirement(
    *,
    length_mode: Optional[str],
    target_word_count: Optional[int],
    original_word_count: int,
) -> str:
    if length_mode == "similar":
        min_words = int(original_word_count * 0.8)
        max_words = int(original_word_count * 1.2)
        return f"????????????{original_word_count}????{min_words}-{max_words}????"
    if length_mode == "expand":
        min_words = int(original_word_count * 1.2)
        max_words = int(original_word_count * 2.0)
        return f"?????????{min_words}-{max_words}??"
    if length_mode == "condense":
        min_words = int(original_word_count * 0.5)
        max_words = int(original_word_count * 0.8)
        return f"?????????{min_words}-{max_words}??"
    if length_mode == "custom" and target_word_count:
        return f"??????{target_word_count}?????20%???"
    return f"????????????{original_word_count}??"


def _calculate_target_words(
    *,
    length_mode: Optional[str],
    target_word_count: Optional[int],
    original_word_count: int,
) -> int:
    if length_mode == "expand":
        return int(original_word_count * 2.0)
    if length_mode == "custom" and target_word_count:
        return target_word_count
    return int(original_word_count * 1.5)


async def prepare_partial_regeneration(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
    partial_request: PartialRegenerateRequest,
    user_id: str,
) -> PartialRegenerationPreparation:
    chapter_content = chapter.content or ""
    if not chapter_content.strip():
        raise HTTPException(status_code=400, detail="??????")

    start_position, end_position, selected_text = _normalize_partial_selection(
        chapter_content=chapter_content,
        partial_request=partial_request,
    )
    original_word_count = len(selected_text)

    context_chars = partial_request.context_chars
    context_before_start = max(0, start_position - context_chars)
    context_before = chapter_content[context_before_start:start_position]
    context_after_end = min(len(chapter_content), end_position + context_chars)
    context_after = chapter_content[end_position:context_after_end]
    logger.info(
        f"???? - ??: {original_word_count}?, ??: {len(context_before)}?, ??: {len(context_after)}?"
    )

    style_id, style_content = await _resolve_style_content(
        db_session,
        project_id=chapter.project_id,
        requested_style_id=partial_request.style_id,
        user_id=user_id,
    )

    length_requirement = _build_length_requirement(
        length_mode=partial_request.length_mode,
        target_word_count=partial_request.target_word_count,
        original_word_count=original_word_count,
    )
    template = await PromptService.get_template("PARTIAL_REGENERATE", user_id, db_session)
    if not template:
        template = PromptService.PARTIAL_REGENERATE

    prompt = PromptService.format_prompt(
        template,
        context_before=context_before if context_before else "????????",
        original_word_count=original_word_count,
        selected_text=selected_text,
        context_after=context_after if context_after else "????????",
        user_instructions=partial_request.user_instructions,
        length_requirement=length_requirement,
        style_content=style_content if style_content else "????????????",
    )

    target_words = _calculate_target_words(
        length_mode=partial_request.length_mode,
        target_word_count=partial_request.target_word_count,
        original_word_count=original_word_count,
    )
    max_tokens = max(500, min(int(target_words * 3), 8000))

    return PartialRegenerationPreparation(
        start_position=start_position,
        end_position=end_position,
        original_text=selected_text,
        original_word_count=original_word_count,
        style_id=style_id,
        style_content=style_content,
        prompt=prompt,
        target_words=target_words,
        max_tokens=max_tokens,
    )
