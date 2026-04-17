from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Awaitable, Callable, Dict, Optional

from sqlalchemy.ext.asyncio import AsyncSession

from app.logger import get_logger
from app.models.chapter import Chapter
from app.models.project import Project


logger = get_logger(__name__)


@dataclass(frozen=True)
class BatchGenerationPrompt:
    chapter_perspective: str
    base_prompt: str
    prompt: str


@dataclass(frozen=True)
class BatchGenerationRequestPayload:
    system_prompt: str
    max_tokens: int
    generate_kwargs: Dict[str, Any]


@dataclass(frozen=True)
class BatchGenerationPromptStageResult:
    batch_prompt: BatchGenerationPrompt
    request_payload: BatchGenerationRequestPayload
    prompt: str
    system_prompt: str
    max_tokens: int
    generate_kwargs: Dict[str, Any]


def _clip_research_text(value: Any, limit: int) -> str:
    if value is None:
        return ''
    text = ' '.join(str(value).split())
    if not text:
        return ''
    if len(text) <= limit:
        return text
    return f"{text[: max(limit - 1, 1)].rstrip()}…"


def _pick_research_asset_text(asset: Dict[str, Any], *keys: str, limit: int) -> str:
    for key in keys:
        value = asset.get(key)
        clipped = _clip_research_text(value, limit)
        if clipped:
            return clipped
    return ''


def _build_web_research_grounding_block(research_assets: Optional[list[Any]]) -> str:
    if not research_assets:
        return ''

    lines = ['【🌐 联网检索事实锚点】']
    included_count = 0

    for raw_asset in research_assets:
        asset = raw_asset if isinstance(raw_asset, dict) else {'summary': raw_asset}
        title = _pick_research_asset_text(asset, 'title', 'name', 'query', 'topic', limit=40)
        source = _pick_research_asset_text(asset, 'source', 'provider', 'domain', 'url', limit=60)
        summary = _pick_research_asset_text(asset, 'summary', 'snippet', 'text', 'content', 'excerpt', limit=150)
        usage_hint = _pick_research_asset_text(asset, 'usage_hint', 'focus', 'purpose', 'reasoning', limit=80)

        entry_parts = []
        if title:
            entry_parts.append(f'主题：{title}')
        if source:
            entry_parts.append(f'来源：{source}')
        if summary:
            entry_parts.append(f'摘要：{summary}')
        if usage_hint:
            entry_parts.append(f'可用点：{usage_hint}')

        if not entry_parts:
            continue

        included_count += 1
        lines.append(f"- 条目 {included_count}：{'；'.join(entry_parts)}")
        if included_count >= 4:
            break

    if included_count == 0:
        return ''

    lines.extend(
        [
            '- 外部信息仅用于补强职业细节、时代氛围、场景常识、社会情绪与行动逻辑。',
            '- 若与既有设定、本章大纲、上章回执或角色状态冲突，以项目 canon 为准。',
            '- 不要照抄来源原文，不要写“根据资料显示 / 据搜索结果 / 某网站指出”等暴露检索过程的话。',
            '',
        ]
    )
    return '\n'.join(lines) + '\n\n'


async def build_batch_generation_prompt(
    *,
    db_session: AsyncSession,
    chapter: Chapter,
    project: Project,
    chapter_context: Any,
    outline_mode: str,
    current_user_id: str,
    target_word_count: int,
    temp_narrative_perspective: Optional[str],
    previous_summary_context: Optional[str],
    prompt_quality_kwargs: Dict[str, Any],
    style_content: str,
    get_template_fn: Callable[[str, str, AsyncSession], Awaitable[str]],
    format_prompt_fn: Callable[..., str],
    apply_style_to_prompt_fn: Callable[[str, str], str],
) -> BatchGenerationPrompt:
    chapter_perspective = (
        temp_narrative_perspective
        or project.narrative_perspective
        or '????'
    )
    logger.info(f'Batch prompt stage perspective: {chapter_perspective}')

    common_kwargs = {
        'project_title': project.title,
        'chapter_number': chapter.chapter_number,
        'chapter_title': chapter.title,
        'chapter_outline': chapter_context.chapter_outline,
        'target_word_count': target_word_count,
        'narrative_perspective': chapter_perspective,
        'world_time_period': project.world_time_period or '???',
        'world_location': project.world_location or '???',
        'world_atmosphere': project.world_atmosphere or '???',
        'world_rules': project.world_rules or '???',
        'characters_info': chapter_context.chapter_characters or '??????',
        'chapter_careers': chapter_context.chapter_careers or '??????',
        'foreshadow_reminders': chapter_context.foreshadow_reminders or '?????????',
        **prompt_quality_kwargs,
    }

    if outline_mode == 'one-to-one':
        if chapter_context.continuation_point:
            template_key = 'CHAPTER_GENERATION_ONE_TO_ONE_NEXT'
            template = await get_template_fn(template_key, current_user_id, db_session)
            base_prompt = format_prompt_fn(
                template,
                previous_chapter_content=chapter_context.continuation_point,
                previous_chapter_summary=chapter_context.previous_chapter_summary or '',
                relevant_memories=chapter_context.relevant_memories or '??????',
                **common_kwargs,
            )
        else:
            template_key = 'CHAPTER_GENERATION_ONE_TO_ONE'
            template = await get_template_fn(template_key, current_user_id, db_session)
            base_prompt = format_prompt_fn(
                template,
                relevant_memories=chapter_context.relevant_memories or '??????',
                **common_kwargs,
            )
    else:
        if chapter_context.continuation_point:
            final_prev_summary = '??????????????????'
            if chapter_context.previous_chapter_summary:
                final_prev_summary = chapter_context.previous_chapter_summary
            elif previous_summary_context:
                final_prev_summary = previous_summary_context
            template_key = 'CHAPTER_GENERATION_ONE_TO_MANY_NEXT'
            template = await get_template_fn(template_key, current_user_id, db_session)
            base_prompt = format_prompt_fn(
                template,
                continuation_point=chapter_context.continuation_point,
                previous_chapter_summary=final_prev_summary,
                recent_chapters_context=chapter_context.recent_chapters_context or '',
                relevant_memories=chapter_context.relevant_memories or '',
                **common_kwargs,
            )
        else:
            template_key = 'CHAPTER_GENERATION_ONE_TO_MANY'
            template = await get_template_fn(template_key, current_user_id, db_session)
            base_prompt = format_prompt_fn(
                template,
                relevant_memories=chapter_context.relevant_memories or '??????',
                **common_kwargs,
            )

    prompt = (
        apply_style_to_prompt_fn(base_prompt, style_content)
        if style_content
        else base_prompt
    )
    return BatchGenerationPrompt(
        chapter_perspective=chapter_perspective,
        base_prompt=base_prompt,
        prompt=prompt,
    )


def build_batch_generation_request_payload(
    *,
    prompt: str,
    project: Project,
    chapter_context: Any,
    style_content: str,
    style_name: str,
    style_preset_id: Any,
    target_word_count: int,
    ai_service: Any,
    custom_model: Optional[str],
    story_runtime_contract: Optional[Dict[str, Any]],
    research_assets: Optional[list[Any]] = None,
    build_runtime_system_prompt_fn: Callable[..., str],
    calculate_max_tokens_fn: Callable[[int], int],
    build_request_options_fn: Callable[[Any], Optional[Dict[str, Any]]],
    detect_style_profile_fn: Callable[..., str],
    resolve_generation_temperature_fn: Callable[[str], float],
) -> BatchGenerationRequestPayload:
    web_research_grounding_block = _build_web_research_grounding_block(research_assets)
    system_prompt = build_runtime_system_prompt_fn(
        project=project,
        style_content=style_content,
        chapter_outline=chapter_context.chapter_outline,
        previous_summary=chapter_context.previous_chapter_summary,
        style_name=style_name,
        style_preset_id=style_preset_id,
        target_word_count=target_word_count,
        story_runtime_contract=story_runtime_contract,
        web_research_grounding_block=web_research_grounding_block,
    )
    max_tokens = calculate_max_tokens_fn(target_word_count)
    style_profile = detect_style_profile_fn(
        style_name=style_name,
        style_preset_id=style_preset_id,
        style_content=style_content,
    )
    generate_kwargs: Dict[str, Any] = {
        'prompt': prompt,
        'system_prompt': system_prompt,
        'tool_choice': 'auto',
        'max_tokens': max_tokens,
        'temperature': resolve_generation_temperature_fn(style_profile),
    }
    request_options = build_request_options_fn(ai_service)
    if request_options is not None:
        generate_kwargs['request_options'] = request_options
    if custom_model:
        generate_kwargs['model'] = custom_model
        logger.info(f'  Batch generation uses custom model: {custom_model}')
    return BatchGenerationRequestPayload(
        system_prompt=system_prompt,
        max_tokens=max_tokens,
        generate_kwargs=generate_kwargs,
    )


async def execute_batch_generation_prompt_stage(
    *,
    db_session: AsyncSession,
    chapter: Chapter,
    project: Project,
    chapter_context: Any,
    outline_mode: str,
    current_user_id: str,
    target_word_count: int,
    temp_narrative_perspective: Optional[str],
    previous_summary_context: Optional[str],
    prompt_quality_kwargs: Dict[str, Any],
    style_content: str,
    style_name: str,
    style_preset_id: Any,
    ai_service: Any,
    custom_model: Optional[str],
    story_runtime_contract: Optional[Dict[str, Any]],
    research_assets: Optional[list[Any]] = None,
    get_template_fn: Callable[[str, str, AsyncSession], Awaitable[str]],
    format_prompt_fn: Callable[..., str],
    apply_style_to_prompt_fn: Callable[[str, str], str],
    build_runtime_system_prompt_fn: Callable[..., str],
    calculate_max_tokens_fn: Callable[[int], int],
    build_request_options_fn: Callable[[Any], Optional[Dict[str, Any]]],
    detect_style_profile_fn: Callable[..., Any],
    resolve_generation_temperature_fn: Callable[[Any], float],
    build_prompt_fn: Callable[..., Awaitable[BatchGenerationPrompt]] = build_batch_generation_prompt,
    build_request_payload_fn: Callable[..., BatchGenerationRequestPayload] = build_batch_generation_request_payload,
) -> BatchGenerationPromptStageResult:
    batch_prompt = await build_prompt_fn(
        db_session=db_session,
        chapter=chapter,
        project=project,
        chapter_context=chapter_context,
        outline_mode=outline_mode,
        current_user_id=current_user_id,
        target_word_count=target_word_count,
        temp_narrative_perspective=temp_narrative_perspective,
        previous_summary_context=previous_summary_context,
        prompt_quality_kwargs=prompt_quality_kwargs,
        style_content=style_content,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        apply_style_to_prompt_fn=apply_style_to_prompt_fn,
    )
    prompt = batch_prompt.prompt
    request_payload = build_request_payload_fn(
        prompt=prompt,
        project=project,
        chapter_context=chapter_context,
        style_content=style_content,
        style_name=style_name,
        style_preset_id=style_preset_id,
        target_word_count=target_word_count,
        ai_service=ai_service,
        custom_model=custom_model,
        story_runtime_contract=story_runtime_contract,
        research_assets=research_assets,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=calculate_max_tokens_fn,
        build_request_options_fn=build_request_options_fn,
        detect_style_profile_fn=detect_style_profile_fn,
        resolve_generation_temperature_fn=resolve_generation_temperature_fn,
    )
    if style_content:
        logger.info(f'Batch prompt stage style content length: {len(style_content)}')
    logger.info(
        f'Batch prompt stage request built: target_word_count={target_word_count}, '
        f'max_tokens={request_payload.max_tokens}'
    )
    return BatchGenerationPromptStageResult(
        batch_prompt=batch_prompt,
        request_payload=request_payload,
        prompt=prompt,
        system_prompt=request_payload.system_prompt,
        max_tokens=request_payload.max_tokens,
        generate_kwargs=dict(request_payload.generate_kwargs),
    )
