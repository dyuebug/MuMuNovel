from __future__ import annotations

from dataclasses import dataclass
from typing import Optional

from fastapi import HTTPException
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.chapter import Chapter
from app.models.outline import Outline
from app.models.project import Project
from app.models.project_default_style import ProjectDefaultStyle
from app.models.writing_style import WritingStyle
from app.schemas.chapter import PartialRegenerateRequest
from app.services.prompt_service import PromptService
from app.services.chapter_web_research_service import chapter_web_research_service
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


def _build_partial_web_research_grounding_block(assets: list[dict]) -> str:
    newline = "\n"
    lines: list[str] = []
    for index, asset in enumerate(assets or [], start=1):
        title = str(asset.get("title") or asset.get("source") or f"Reference {index}").strip()
        summary = str(
            asset.get("summary")
            or asset.get("snippet")
            or asset.get("text")
            or asset.get("raw_content")
            or ""
        ).strip()
        usage_hint = str(asset.get("usage_hint") or "").strip()
        url = str(asset.get("url") or "").strip()
        item_lines = [f"{index}. {title}"]
        if summary:
            item_lines.append(f"   - Summary: {summary}")
        if usage_hint:
            item_lines.append(f"   - Usage: {usage_hint}")
        if url:
            item_lines.append(f"   - Link: {url}")
        lines.append(newline.join(item_lines))
    if not lines:
        return ""
    return (
        f"{newline}{newline}[Web Research References]{newline}"
        "Use the following references to improve factual texture and scene grounding, but integrate them naturally:\n"
        + newline.join(lines)
    )


async def _load_partial_regeneration_project_bundle(
    db_session: AsyncSession,
    *,
    chapter: Chapter,
) -> tuple[Project, Optional[Outline]]:
    project_result = await db_session.execute(
        select(Project).where(Project.id == chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if project is None:
        raise HTTPException(status_code=404, detail="章节不存在")

    outline = None
    if chapter.outline_id:
        outline_result = await db_session.execute(
            select(Outline).where(Outline.id == chapter.outline_id)
        )
        outline = outline_result.scalar_one_or_none()
    else:
        outline_result = await db_session.execute(
            select(Outline)
            .where(Outline.project_id == chapter.project_id)
            .where(Outline.order_index == chapter.chapter_number)
        )
        outline = outline_result.scalar_one_or_none()
    return project, outline


def _normalize_partial_selection(
    *,
    chapter_content: str,
    partial_request: PartialRegenerateRequest,
) -> tuple[int, int, str]:
    content_length = len(chapter_content)
    start_position = partial_request.start_position
    end_position = partial_request.end_position

    if start_position >= content_length:
        raise HTTPException(status_code=400, detail="请先选中需要重写的内容")
    if end_position > content_length:
        raise HTTPException(status_code=400, detail="请提供有效的选中文本")
    if start_position >= end_position:
        raise HTTPException(status_code=400, detail="选中文本与原文不匹配，请重试")

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
            detail="未找到对应章节的大纲上下文，无法执行局部重写",
        )

    offset = search_area.find(selected_text)
    corrected_start = search_start + offset
    corrected_end = corrected_start + len(selected_text)
    logger.info(f"局部重写选区已校正: {corrected_start}-{corrected_end}")
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
            logger.info(f"局部重写 - 使用项目默认风格ID: {style_id}")

    if not style_id:
        return None, ""

    style_result = await db_session.execute(
        select(WritingStyle).where(WritingStyle.id == style_id)
    )
    style = style_result.scalar_one_or_none()
    if style is None:
        return style_id, ""
    if style.user_id is not None and style.user_id != user_id:
        logger.warning(f"风格 {style_id} 不属于当前用户，已忽略")
        return style_id, ""

    style_content = style.prompt_content or ""
    style_type = "系统风格" if style.user_id is None else "用户风格"
    logger.info(f"局部重写 - 使用风格: {style.name} ({style_type})")
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
        return f"尽量保持与原文接近，原文约 {original_word_count} 字，目标 {min_words}-{max_words} 字"
    if length_mode == "expand":
        min_words = int(original_word_count * 1.2)
        max_words = int(original_word_count * 2.0)
        return f"建议扩写至 {min_words}-{max_words} 字"
    if length_mode == "condense":
        min_words = int(original_word_count * 0.5)
        max_words = int(original_word_count * 0.8)
        return f"建议压缩至 {min_words}-{max_words} 字"
    if length_mode == "custom" and target_word_count:
        return f"目标长度约 {target_word_count} 字，允许上下浮动 20%"
        return f"默认按接近原文长度处理，原文约 {original_word_count} 字"


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
        raise HTTPException(status_code=400, detail="章节内容为空")

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
        f"局部重写上下文 - 原文: {original_word_count}字, 前文: {len(context_before)}字, 后文: {len(context_after)}字"
    )

    style_id, style_content = await _resolve_style_content(
        db_session,
        project_id=chapter.project_id,
        requested_style_id=partial_request.style_id,
        user_id=user_id,
    )
    project, outline = await _load_partial_regeneration_project_bundle(
        db_session,
        chapter=chapter,
    )
    web_research_bundle = await chapter_web_research_service.collect_for_chapter(
        user_id=user_id,
        db_session=db_session,
        project=project,
        chapter=chapter,
        outline=outline,
        story_creation_brief=None,
        enable_web_research=partial_request.enable_web_research,
        web_research_query=partial_request.web_research_query,
    )
    web_research_grounding_block = _build_partial_web_research_grounding_block(
        list(web_research_bundle.get("assets") or [])
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
        context_before=context_before if context_before else "（无前文上下文）",
        original_word_count=original_word_count,
        selected_text=selected_text,
        context_after=context_after if context_after else "（无后文上下文）",
        user_instructions=(partial_request.user_instructions or "") + web_research_grounding_block,
        length_requirement=length_requirement,
        style_content=style_content if style_content else "（未提供风格约束）",
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
