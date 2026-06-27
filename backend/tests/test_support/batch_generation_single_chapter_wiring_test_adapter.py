from __future__ import annotations

import asyncio
from asyncio import Lock
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
import re
from typing import TYPE_CHECKING, Any, Awaitable, Callable, Dict, Mapping, Optional

SOURCE_MAP_FREEZE_STATUS = "frozen_source_map_rollback_only"
SOURCE_MAP_FREEZE_REASON = (
    "Rust owns the active batch-generation route/read/runtime chain while this "
    "Python wiring file remains the consolidated rollback/source-map host for "
    "legacy batch single-chapter candidate runtime orchestration."
)
SOURCE_MAP_RUST_OWNER = (
    "backend-rs/src/api/health.rs; "
    "backend-rs/src/services/chapter_single_generation_prepare_service/research_payload_owner.rs; "
    "backend-rs/src/services/chapter_batch_generation_runtime_state_service.rs; "
    "backend-rs/src/services/chapter_batch_generation_read_context_service.rs; "
    "backend-rs/src/services/chapter_candidate_event_service.rs"
)
SOURCE_MAP_ROLLBACK_FLAG = "batch_generation_route_flag_retired_test_only_source_map"
SOURCE_MAP_PHYSICAL_CLOSEOUT_ACTION = "freeze"

from tests.test_support.retired_runtime_test_support import get_logger
from tests.test_support.prompt_template_facade_test_support import (
    format_prompt as _facade_format_prompt,
    get_template_for_owner,
)

if TYPE_CHECKING:
    from sqlalchemy.ext.asyncio import AsyncSession

    from migrator_app.models.chapter import Chapter
    from tests.test_support.ai_gateway.ai_service import AIService
    from tests.test_support.story_packet_test_support import StoryPacket
    from tests.test_support.story_repair_payload_test_support import StoryRepairPayload

CHAPTER_CANDIDATE_RERANK_LIMIT = 2
CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS = 10.0
RESPONSES_TEXT_GENERATION_PROVIDERS = {"sub2api", "openai_responses"}
CHAPTER_GENERATION_TRANSPORT_RETRY_CAP = 2
CHAPTER_GENERATION_FIRST_CHUNK_TIMEOUT = 20.0
_PROMPT_SERVICE_SOURCE_PATH = (
    Path(__file__).resolve().parent / "prompt_service_test_support.py"
)
logger = get_logger(__name__)


@lru_cache(maxsize=1)
def _load_batch_generation_prompt_template_map() -> dict[str, str]:
    source = _PROMPT_SERVICE_SOURCE_PATH.read_text(encoding="utf-8")
    template_keys = (
        "CHAPTER_GENERATION_ONE_TO_ONE",
        "CHAPTER_GENERATION_ONE_TO_ONE_NEXT",
        "CHAPTER_GENERATION_ONE_TO_MANY",
        "CHAPTER_GENERATION_ONE_TO_MANY_NEXT",
    )
    templates: dict[str, str] = {}
    for template_key in template_keys:
        match = re.search(
            rf'^\s*{template_key}\s*=\s*"""(.*?)"""',
            source,
            flags=re.MULTILINE | re.DOTALL,
        )
        if not match:
            raise RuntimeError(
                f"batch generation test adapter 未找到模板常量: {template_key}"
            )
        templates[template_key] = match.group(1)
    return templates


def _batch_generation_template_lookup(template_key: str) -> Optional[str]:
    return _load_batch_generation_prompt_template_map().get(template_key)


async def _default_get_batch_generation_template(
    template_key: str,
    user_id: str,
    db_session: "AsyncSession",
):
    return await get_template_for_owner(
        template_key,
        user_id,
        db_session,
        template_lookup=_batch_generation_template_lookup,
    )


def _default_format_batch_generation_prompt(template: str, **kwargs) -> str:
    return _facade_format_prompt(template, **kwargs)


def _chapter_generation_runtime_prompt_service():
    from tests.test_support import (
        chapter_generation_runtime_prompt_test_support
        as chapter_generation_runtime_prompt_service,
    )

    return chapter_generation_runtime_prompt_service


def _foreshadow_service_instance():
    from tests.test_support.foreshadow_test_support import foreshadow_service

    return foreshadow_service


def _memory_service_instance():
    from tests.test_support.memory_service_test_support import memory_service

    return memory_service


def _outline_runtime_source_test_support():
    from tests.test_support import outline_runtime_source_test_support

    return outline_runtime_source_test_support


def _chapter_candidate_result_service():
    from tests.test_support import chapter_candidate_result_test_support

    return chapter_candidate_result_test_support


def _chapter_candidate_finalize_service():
    from tests.test_support import chapter_candidate_finalize_test_support

    return chapter_candidate_finalize_test_support


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
        summary = _pick_research_asset_text(
            asset,
            'summary',
            'snippet',
            'text',
            'content',
            'excerpt',
            limit=150,
        )
        usage_hint = _pick_research_asset_text(
            asset,
            'usage_hint',
            'focus',
            'purpose',
            'reasoning',
            limit=80,
        )

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
    db_session: "AsyncSession",
    chapter: "Chapter",
    project: Any,
    chapter_context: Any,
    outline_mode: str,
    current_user_id: str,
    target_word_count: int,
    temp_narrative_perspective: Optional[str],
    previous_summary_context: Optional[str],
    prompt_quality_kwargs: Dict[str, Any],
    style_content: str,
    get_template_fn: Callable[[str, str, "AsyncSession"], Awaitable[str]],
    format_prompt_fn: Callable[..., str],
    apply_style_to_prompt_fn: Callable[[str, str], str],
) -> BatchGenerationPrompt:
    chapter_perspective = (
        temp_narrative_perspective
        or project.narrative_perspective
        or '第三人称'
    )
    logger.info(f'Batch prompt stage perspective: {chapter_perspective}')

    common_kwargs = {
        'project_title': project.title,
        'chapter_number': chapter.chapter_number,
        'chapter_title': chapter.title,
        'chapter_outline': chapter_context.chapter_outline,
        'target_word_count': target_word_count,
        'narrative_perspective': chapter_perspective,
        'world_time_period': project.world_time_period or '未提供',
        'world_location': project.world_location or '未提供',
        'world_atmosphere': project.world_atmosphere or '未提供',
        'world_rules': project.world_rules or '未提供',
        'characters_info': chapter_context.chapter_characters or '暂无角色信息',
        'chapter_careers': chapter_context.chapter_careers or '暂无职业信息',
        'foreshadow_reminders': chapter_context.foreshadow_reminders or '暂无伏笔提醒',
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
                relevant_memories=chapter_context.relevant_memories or '暂无相关记忆',
                **common_kwargs,
            )
        else:
            template_key = 'CHAPTER_GENERATION_ONE_TO_ONE'
            template = await get_template_fn(template_key, current_user_id, db_session)
            base_prompt = format_prompt_fn(
                template,
                relevant_memories=chapter_context.relevant_memories or '暂无相关记忆',
                **common_kwargs,
            )
    else:
        if chapter_context.continuation_point:
            final_prev_summary = '承接上一章剧情继续推进。'
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
                relevant_memories=chapter_context.relevant_memories or '暂无相关记忆',
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
    project: Any,
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
    db_session: "AsyncSession",
    chapter: "Chapter",
    project: Any,
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
    get_template_fn: Optional[Callable[[str, str, "AsyncSession"], Awaitable[str]]] = None,
    format_prompt_fn: Optional[Callable[..., str]] = None,
    apply_style_to_prompt_fn: Optional[Callable[[str, str], str]] = None,
    build_runtime_system_prompt_fn: Optional[Callable[..., str]] = None,
    calculate_max_tokens_fn: Optional[Callable[[int], int]] = None,
    build_request_options_fn: Optional[Callable[[Any], Optional[Dict[str, Any]]]] = None,
    detect_style_profile_fn: Optional[Callable[..., Any]] = None,
    resolve_generation_temperature_fn: Optional[Callable[[Any], float]] = None,
    build_prompt_fn: Callable[..., Awaitable[BatchGenerationPrompt]] = build_batch_generation_prompt,
    build_request_payload_fn: Callable[..., BatchGenerationRequestPayload] = build_batch_generation_request_payload,
) -> BatchGenerationPromptStageResult:
    if get_template_fn is None:
        get_template_fn = get_template
    if format_prompt_fn is None:
        format_prompt_fn = format_prompt
    if apply_style_to_prompt_fn is None:
        apply_style_to_prompt_fn = WritingStyleManager.apply_style_to_prompt
    if build_runtime_system_prompt_fn is None:
        build_runtime_system_prompt_fn = build_chapter_runtime_system_prompt
    if calculate_max_tokens_fn is None:
        calculate_max_tokens_fn = _calculate_chapter_generation_max_tokens
    if build_request_options_fn is None:
        build_request_options_fn = _build_chapter_generation_request_options
    if detect_style_profile_fn is None:
        detect_style_profile_fn = detect_style_profile
    if resolve_generation_temperature_fn is None:
        resolve_generation_temperature_fn = resolve_generation_temperature

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


def detect_style_profile(*args, **kwargs):
    return _chapter_generation_runtime_prompt_service().detect_style_profile(*args, **kwargs)


def resolve_generation_temperature(*args, **kwargs):
    return _chapter_generation_runtime_prompt_service().resolve_generation_temperature(*args, **kwargs)


def build_chapter_generation_runtime_bundle(*args, **kwargs):
    from tests.test_support.story_packet_test_support import (
        build_chapter_generation_runtime_bundle as impl,
    )

    return impl(*args, **kwargs)


def build_chapter_quality_runtime_context(*args, **kwargs):
    from tests.test_support.story_packet_test_support import (
        build_chapter_quality_runtime_context as impl,
    )

    return impl(*args, **kwargs)


def _calculate_chapter_generation_max_tokens(target_word_count: int) -> int:
    safe_target = max(200, int(target_word_count or 0))
    calculated_max_tokens = int(safe_target * 0.6)
    return max(700, min(calculated_max_tokens, 8000))


def _build_chapter_generation_request_options(ai_service: Any) -> Optional[Dict[str, Any]]:
    normalized_provider = str(getattr(ai_service, "api_provider", "") or "").strip().lower()
    if normalized_provider not in RESPONSES_TEXT_GENERATION_PROVIDERS:
        return None

    retry_cfg = getattr(getattr(ai_service, "config", None), "retry", None)
    configured_retry_budget = int(
        getattr(retry_cfg, "max_retries", CHAPTER_GENERATION_TRANSPORT_RETRY_CAP)
        or CHAPTER_GENERATION_TRANSPORT_RETRY_CAP
    )
    transport_max_retries = max(
        1,
        min(configured_retry_budget, CHAPTER_GENERATION_TRANSPORT_RETRY_CAP),
    )
    return {
        "prefer_chat_completions": True,
        "transport_max_retries": transport_max_retries,
        "first_chunk_timeout": CHAPTER_GENERATION_FIRST_CHUNK_TIMEOUT,
        "allow_non_stream_fallback": False,
    }


def build_story_generation_packet_with_project_continuity(*args, **kwargs):
    from tests.test_support.story_continuity_ledger_test_support import (
        build_story_generation_packet_with_project_continuity as impl,
    )

    return impl(*args, **kwargs)


def clone_chapter_quality_profile(*args, **kwargs):
    from tests.test_support.chapter_prompt_quality_test_support import (
        clone_chapter_quality_profile as impl,
    )

    return impl(*args, **kwargs)


class _LazyForeshadowService:
    def __getattr__(self, name: str):
        return getattr(_foreshadow_service_instance(), name)


class _LazyMemoryService:
    def __getattr__(self, name: str):
        return getattr(_memory_service_instance(), name)


_foreshadow_service = _LazyForeshadowService()
_memory_service = _LazyMemoryService()


def build_outline_structure_runtime_sources(*args, **kwargs):
    return _outline_runtime_source_test_support().build_outline_structure_runtime_sources(
        *args,
        **kwargs,
    )


class WritingStyleManager:
    @staticmethod
    def apply_style_to_prompt(*args, **kwargs):
        from tests.test_support.story_writing_style_test_support import (
            WritingStyleManager as WritingStyleManagerImpl,
        )

        return WritingStyleManagerImpl.apply_style_to_prompt(*args, **kwargs)


def attach_story_runtime_contract(*args, **kwargs):
    from tests.test_support.schemas.generation_payload import attach_story_runtime_contract as impl

    return impl(*args, **kwargs)


async def _publish_task_stream_event(*args, **kwargs):
    from tests.test_support.task_system import publish_task_stream_event as impl

    return await impl(*args, **kwargs)


def build_chapter_candidate_runtime_state(*args, **kwargs):
    from tests.test_support import chapter_candidate_runtime_state_test_support

    return chapter_candidate_runtime_state_test_support.build_chapter_candidate_runtime_state(
        *args,
        **kwargs,
    )


def snapshot_chapter_candidate_runtime_state(*args, **kwargs):
    from tests.test_support import chapter_candidate_runtime_state_test_support

    return chapter_candidate_runtime_state_test_support.snapshot_chapter_candidate_runtime_state(
        *args,
        **kwargs,
    )


def build_batch_generation_start_progress_event(*, chapter: "Chapter") -> Dict[str, Any]:
    return {
        "type": "progress",
        "chapter_id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "message": f"Generating chapter {chapter.chapter_number}",
        "progress": 35,
        "status": "running",
        "phase": "generating",
    }


def build_batch_generation_candidate_progress_event(
    *,
    chapter: "Chapter",
    runtime_snapshot,
    target_word_count: int,
) -> Dict[str, Any]:
    progress = 35 + int(
        min(runtime_snapshot.current_chars / max(target_word_count, 1), 1.0) * 25
    )
    if runtime_snapshot.candidate_index > 1:
        progress = max(progress, 40 + (runtime_snapshot.candidate_index - 1) * 5)
    progress = min(progress, 70)
    return {
        "type": "progress",
        "chapter_id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "message": (
            f"Generating chapter {chapter.chapter_number} candidate "
            f"{runtime_snapshot.candidate_index}/{runtime_snapshot.candidate_total} "
            f"({runtime_snapshot.current_chars} chars)"
        ),
        "progress": progress,
        "status": "running",
        "phase": "generating",
        "candidate_index": runtime_snapshot.candidate_index,
        "candidate_count": runtime_snapshot.candidate_count,
        "word_count": runtime_snapshot.current_chars,
        "generation_path": runtime_snapshot.generation_path,
        "attempt_kind": runtime_snapshot.attempt_kind,
        "rerank_used": runtime_snapshot.rerank_used,
        "word_budget_repair_used": runtime_snapshot.word_budget_repair_used,
    }


def build_batch_generation_selected_candidate_progress_event(
    *,
    chapter: "Chapter",
    selected_candidate_view,
    candidate_word_count: int,
    chapter_context_stats: Mapping[str, Any],
) -> Dict[str, Any]:
    winner_candidate_index = selected_candidate_view.winner_candidate_index
    return {
        "type": "progress",
        "chapter_id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "message": (
            f"Selected chapter {chapter.chapter_number} candidate "
            f"{winner_candidate_index}/{selected_candidate_view.candidate_count} "
            f"({candidate_word_count} chars)"
        ),
        "progress": 70,
        "status": "running",
        "phase": "generating",
        "candidate_index": selected_candidate_view.candidate_index,
        "candidate_count": selected_candidate_view.candidate_count,
        "word_count": candidate_word_count,
        "generation_path": selected_candidate_view.generation_path,
        "attempt_kind": selected_candidate_view.attempt_kind,
        "rerank_used": selected_candidate_view.rerank_used,
        "word_budget_repair_used": selected_candidate_view.word_budget_repair_used,
        "winner_candidate_index": winner_candidate_index,
        "pre_compaction_total_length": chapter_context_stats.get(
            "pre_compaction_total_length"
        ),
        "context_budget_limit": chapter_context_stats.get("context_budget_limit"),
        "compaction_applied": chapter_context_stats.get("compaction_applied"),
        "compaction_details": chapter_context_stats.get("compaction_details"),
    }


def build_batch_generation_chunk_event(*, chapter: "Chapter", chunk: str) -> Dict[str, Any]:
    return {
        "type": "chunk",
        "chapter_id": chapter.id,
        "chapter_number": chapter.chapter_number,
        "content": chunk,
    }


def build_chapter_generation_progress_kwargs(
    *,
    runtime_snapshot,
    target_word_count: int,
) -> Dict[str, Any]:
    return {
        "current_chars": runtime_snapshot.current_chars,
        "estimated_total": target_word_count,
        "message": (
            f"候选草稿生成 {runtime_snapshot.candidate_index}/{runtime_snapshot.candidate_total} ... "
            f"({runtime_snapshot.current_chars}字)"
        ),
        "retry_count": max(runtime_snapshot.candidate_index - 1, 0),
        "max_retries": max(runtime_snapshot.candidate_total - 1, 1),
    }


def normalize_selected_candidate_result(*args, **kwargs):
    return _chapter_candidate_result_service().normalize_selected_candidate_result(*args, **kwargs)


def snapshot_chapter_candidate(*args, **kwargs):
    return _chapter_candidate_finalize_service().snapshot_chapter_candidate(*args, **kwargs)


@dataclass(frozen=True)
class BatchGenerationCandidateQualityHooks:
    quality_evaluator: Callable[[str], Dict[str, Any]]
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]]


@dataclass(frozen=True)
class BatchGenerationCandidateExecution:
    runtime_state: Dict[str, Any]
    selected_candidate_task: asyncio.Task


@dataclass(frozen=True)
class BatchGenerationCandidateFlowResult:
    selected_candidate: Dict[str, Any]
    selected_candidate_result: Dict[str, Any]


class PromptService:
    @staticmethod
    async def get_template(*args, **kwargs):
        return await _default_get_batch_generation_template(*args, **kwargs)

    @staticmethod
    def format_prompt(*args, **kwargs):
        return _default_format_batch_generation_prompt(*args, **kwargs)


async def get_template(*args, **kwargs):
    return await _default_get_batch_generation_template(*args, **kwargs)


def format_prompt(*args, **kwargs):
    return _default_format_batch_generation_prompt(*args, **kwargs)


class _LazyChapterWebResearchService:
    def is_enabled(self, *args, **kwargs):
        from tests.test_support.chapter_web_research_test_support import (
            chapter_web_research_service,
        )

        return chapter_web_research_service.is_enabled(*args, **kwargs)

    async def collect_for_chapter(self, *args, **kwargs):
        from tests.test_support.chapter_web_research_test_support import (
            chapter_web_research_service,
        )

        return await chapter_web_research_service.collect_for_chapter(*args, **kwargs)

    async def replace_chapter_memories(self, *args, **kwargs):
        from tests.test_support.chapter_web_research_test_support import (
            chapter_web_research_service,
        )

        return await chapter_web_research_service.replace_chapter_memories(*args, **kwargs)


chapter_web_research_service = _LazyChapterWebResearchService()
_chapter_web_research_service = chapter_web_research_service


class _LazyOneToOneContextBuilder:
    def __new__(cls, *args, **kwargs):
        from tests.test_support.chapter_context_test_support import OneToOneContextBuilder

        return OneToOneContextBuilder(*args, **kwargs)


class _LazyOneToManyContextBuilder:
    def __new__(cls, *args, **kwargs):
        from tests.test_support.chapter_context_test_support import OneToManyContextBuilder

        return OneToManyContextBuilder(*args, **kwargs)


OneToOneContextBuilder = _LazyOneToOneContextBuilder
OneToManyContextBuilder = _LazyOneToManyContextBuilder


def build_chapter_runtime_system_prompt(*args, **kwargs):
    from tests.test_support.chapter_generation_runtime_prompt_test_support import (
        build_chapter_runtime_system_prompt as build_chapter_runtime_system_prompt_service,
    )

    return build_chapter_runtime_system_prompt_service(*args, **kwargs)


def compute_story_quality_metrics(*args, **kwargs):
    from tests.test_support.story_quality_metrics_aggregation_test_support import (
        compute_story_quality_metrics,
    )

    return compute_story_quality_metrics(*args, **kwargs)


async def resolve_chapter_quality_profile(*args, **kwargs):
    from tests.test_support.chapter_prompt_quality_test_support import (
        resolve_chapter_quality_profile,
    )

    return await resolve_chapter_quality_profile(*args, **kwargs)


@dataclass(frozen=True)
class BatchGenerationRuntimePreparation:
    effective_story_packet: StoryPacket
    generation_guidance: Any
    quality_profile: Dict[str, Any]
    style_id: Any
    style_content: str
    style_name: str
    style_preset_id: Any
    generation_runtime: Optional[Any]


@dataclass(frozen=True)
class BatchGenerationResolvedRuntime:
    generation_runtime: Any
    generation_intent: Any
    prompt_quality_kwargs: Dict[str, Any]
    story_runtime_contract: Any


@dataclass(frozen=True)
class BatchGenerationBuiltContext:
    chapter_context: Any
    outline_runtime_sources: Any


@dataclass(frozen=True)
class BatchGenerationProjectOutlineContext:
    project: Any
    outline: Any
    outline_mode: str
    research_assets: list[Any]


@dataclass(frozen=True)
class BatchGenerationChapterRuntimeArtifacts:
    effective_story_packet: StoryPacket
    generation_guidance: Any
    quality_profile: Dict[str, Any]
    style_id: Any
    style_content: str
    style_name: str
    style_preset_id: Any
    chapter_context: Any
    outline_runtime_sources: Any
    generation_runtime: Any
    generation_intent: Any
    prompt_quality_kwargs: Dict[str, Any]
    story_runtime_contract: Any


async def load_batch_generation_project_and_outline(
    *,
    db_session: "AsyncSession",
    chapter: "Chapter",
) -> tuple[Any, Any, str]:
    from sqlalchemy import select
    from migrator_app.models.outline import Outline
    from migrator_app.models.project import Project

    project_result = await db_session.execute(
        select(Project).where(Project.id == chapter.project_id)
    )
    project = project_result.scalar_one_or_none()
    if not project:
        raise Exception("项目不存在")

    outline_mode = project.outline_mode if project else "one-to-many"
    logger.info(f"批量生成单章 - 大纲模式: {outline_mode}")

    if chapter.outline_id:
        outline_result = await db_session.execute(
            select(Outline).where(Outline.id == chapter.outline_id)
        )
    else:
        outline_result = await db_session.execute(
            select(Outline)
            .where(Outline.project_id == chapter.project_id)
            .where(Outline.order_index == chapter.chapter_number)
        )

    return project, outline_result.scalar_one_or_none(), outline_mode


async def collect_batch_generation_research_assets(
    *,
    db_session: "AsyncSession",
    chapter: "Chapter",
    project: Any,
    outline: Any,
    user_id: str,
    story_creation_brief: Optional[str],
    enable_web_research: Optional[bool],
    web_research_query: Optional[str],
    stream_task_id: Optional[str],
    write_lock: Any,
    chapter_web_research_service: Any,
    publish_task_stream_event_fn: Callable[..., Any],
) -> list[Dict[str, str]]:
    if not chapter_web_research_service.is_enabled(enable_web_research):
        return []

    if stream_task_id:
        await publish_task_stream_event_fn(
            stream_task_id,
            {
                "type": "progress",
                "message": f"第{chapter.chapter_number}章正在联网检索",
                "progress": 18,
                "status": "running",
                "phase": "researching",
            },
            db_session=db_session,
        )

    research_bundle = await chapter_web_research_service.collect_for_chapter(
        project=project,
        chapter=chapter,
        outline=outline,
        story_creation_brief=story_creation_brief,
        enable_web_research=enable_web_research,
        web_research_query=web_research_query,
    )
    research_assets = list(research_bundle.get("assets") or [])
    research_query = str(research_bundle.get("query") or "")
    if not research_assets:
        return research_assets

    async with write_lock:
        saved_memory_ids = await chapter_web_research_service.replace_chapter_memories(
            db_session=db_session,
            user_id=user_id,
            project=project,
            chapter=chapter,
            query=research_query,
            archive_path=str(research_bundle.get("archive_path") or ""),
            assets=research_assets,
        )

    logger.info(
        "联网检索 - 第%s章获得 %s 条资料，归档 %s 条记忆",
        chapter.chapter_number,
        len(research_assets),
        len(saved_memory_ids),
    )
    if stream_task_id:
        await publish_task_stream_event_fn(
            stream_task_id,
            {
                "type": "progress",
                "message": f"第{chapter.chapter_number}章已检索到 {len(research_assets)} 条资料",
                "progress": 22,
                "status": "running",
                "phase": "researching",
            },
            db_session=db_session,
        )

    return research_assets


async def prepare_batch_generation_project_outline_context(
    *,
    db_session: "AsyncSession",
    chapter: "Chapter",
    user_id: str,
    story_creation_brief: Optional[str],
    enable_web_research: Optional[bool],
    web_research_query: Optional[str],
    stream_task_id: Optional[str],
    write_lock: Any,
    chapter_web_research_service: Any,
    publish_task_stream_event_fn: Callable[..., Any],
    load_project_outline_fn: Callable[..., Awaitable[tuple[Any, Any, str]]] = load_batch_generation_project_and_outline,
    collect_research_assets_fn: Callable[..., Awaitable[list[Dict[str, str]]]] = collect_batch_generation_research_assets,
) -> BatchGenerationProjectOutlineContext:
    project, outline, outline_mode = await load_project_outline_fn(
        db_session=db_session,
        chapter=chapter,
    )
    research_assets = await collect_research_assets_fn(
        db_session=db_session,
        chapter=chapter,
        project=project,
        outline=outline,
        user_id=user_id,
        story_creation_brief=story_creation_brief,
        enable_web_research=enable_web_research,
        web_research_query=web_research_query,
        stream_task_id=stream_task_id,
        write_lock=write_lock,
        chapter_web_research_service=chapter_web_research_service,
        publish_task_stream_event_fn=publish_task_stream_event_fn,
    )
    return BatchGenerationProjectOutlineContext(
        project=project,
        outline=outline,
        outline_mode=outline_mode,
        research_assets=research_assets,
    )


async def prepare_batch_generation_runtime(
    *,
    db_session: "AsyncSession",
    user_id: str,
    project: Any,
    chapter: "Chapter",
    target_word_count: int,
    style_id: Optional[int],
    story_packet: Optional["StoryPacket"],
    base_quality_profile: Optional[Dict[str, Any]],
    research_assets: list[Any],
    creative_mode: Optional[str],
    story_focus: Optional[str],
    plot_stage: Optional[str],
    story_creation_brief: Optional[str],
    quality_preset: Optional[str],
    quality_notes: Optional[str],
    chapter_context: Any = None,
    outline_runtime_sources: Any = None,
    story_repair_state: Optional[Dict[str, Any]] = None,
    story_repair_payload: Optional["StoryRepairPayload"] = None,
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None,
    build_story_packet_fn: Callable[..., Any] = build_story_generation_packet_with_project_continuity,
    clone_quality_profile_fn: Callable[..., Dict[str, Any]] = clone_chapter_quality_profile,
    resolve_quality_profile_fn: Callable[..., Any] = resolve_chapter_quality_profile,
    build_generation_runtime_bundle_fn: Optional[Callable[..., Any]] = None,
) -> BatchGenerationRuntimePreparation:
    effective_story_packet = (
        story_packet
        if story_packet is not None
        else await build_story_packet_fn(
            db_session,
            project,
            creative_mode=creative_mode,
            story_focus=story_focus,
            plot_stage=plot_stage,
            story_creation_brief=story_creation_brief,
            quality_preset=quality_preset,
            quality_notes=quality_notes,
            source_label='batch-single-chapter-generate',
        )
    )
    generation_guidance = effective_story_packet.guidance

    if isinstance(base_quality_profile, dict) and base_quality_profile:
        quality_profile = clone_quality_profile_fn(
            base_quality_profile,
            external_assets=research_assets,
            reference_assets=research_assets,
        )
    else:
        quality_profile = await resolve_quality_profile_fn(
            db_session=db_session,
            user_id=user_id,
            project=project,
            style_id=style_id,
            enable_mcp=True,
            external_assets=research_assets,
            reference_assets=research_assets,
            prefer_project_default_style=not bool(style_id),
            log_prefix='批量生成',
        )

    resolved_style_id = quality_profile.get('resolved_style_id')
    style_content = quality_profile.get('style_content') or ''
    style_name = quality_profile.get('style_name') or ''
    style_preset_id = quality_profile.get('style_preset_id') or ''

    generation_runtime = None
    if build_generation_runtime_bundle_fn is not None and chapter_context is not None:
        generation_runtime = build_generation_runtime_bundle_fn(
            story_packet=effective_story_packet,
            quality_profile=quality_profile,
            project=project,
            chapter=chapter,
            chapter_context=chapter_context,
            target_word_count=target_word_count,
            story_repair_state=story_repair_state,
            story_repair_payload=story_repair_payload,
            active_story_repair_payload=active_story_repair_snapshot,
            character_focus_source=outline_runtime_sources or None,
            character_state_source=outline_runtime_sources or None,
            organization_state_source=outline_runtime_sources or None,
        )
    return BatchGenerationRuntimePreparation(
        effective_story_packet=effective_story_packet,
        generation_guidance=generation_guidance,
        quality_profile=(dict(quality_profile) if isinstance(quality_profile, dict) else {}),
        style_id=resolved_style_id,
        style_content=style_content,
        style_name=style_name,
        style_preset_id=style_preset_id,
        generation_runtime=generation_runtime,
    )


def finalize_batch_generation_runtime(
    *,
    runtime_preparation: BatchGenerationRuntimePreparation,
    project: Any,
    chapter: "Chapter",
    chapter_context: Any,
    target_word_count: int,
    outline_runtime_sources: Any,
    story_repair_state: Optional[Dict[str, Any]],
    story_repair_payload: Optional["StoryRepairPayload"],
    active_story_repair_snapshot: Optional[Dict[str, Any]],
    build_generation_runtime_bundle_fn: Callable[..., Any],
) -> BatchGenerationResolvedRuntime:
    generation_runtime = build_generation_runtime_bundle_fn(
        story_packet=runtime_preparation.effective_story_packet,
        quality_profile=runtime_preparation.quality_profile,
        project=project,
        chapter=chapter,
        chapter_context=chapter_context,
        target_word_count=target_word_count,
        story_repair_state=story_repair_state,
        story_repair_payload=story_repair_payload,
        active_story_repair_payload=active_story_repair_snapshot,
        character_focus_source=outline_runtime_sources or None,
        character_state_source=outline_runtime_sources or None,
        organization_state_source=outline_runtime_sources or None,
    )
    return BatchGenerationResolvedRuntime(
        generation_runtime=generation_runtime,
        generation_intent=generation_runtime.generation_intent,
        prompt_quality_kwargs=generation_runtime.prompt_quality_kwargs,
        story_runtime_contract=generation_runtime.story_runtime_contract,
    )


async def build_batch_generation_context(
    *,
    db_session: "AsyncSession",
    chapter: "Chapter",
    project: Any,
    outline: Any,
    outline_mode: str,
    user_id: str,
    target_word_count: int,
    style_content: str,
    memory_service: Any,
    foreshadow_service: Any,
    one_to_one_builder_cls: Callable[..., Any],
    one_to_many_builder_cls: Callable[..., Any],
    build_outline_structure_runtime_sources_fn: Callable[[Any], Any],
) -> BatchGenerationBuiltContext:
    if outline_mode == 'one-to-one':
        logger.info(f'构建上下文 - [1-1模式] 使用 {one_to_one_builder_cls.__name__}')
        context_builder = one_to_one_builder_cls(
            memory_service=memory_service,
            foreshadow_service=foreshadow_service,
        )
        chapter_context = await context_builder.build(
            chapter=chapter,
            project=project,
            outline=outline,
            user_id=user_id,
            db=db_session,
            target_word_count=target_word_count,
        )
    else:
        logger.info(f'构建上下文 - [1-N模式] 使用 {one_to_many_builder_cls.__name__}')
        context_builder = one_to_many_builder_cls(
            memory_service=memory_service,
            foreshadow_service=foreshadow_service,
        )
        chapter_context = await context_builder.build(
            chapter=chapter,
            project=project,
            outline=outline,
            user_id=user_id,
            db=db_session,
            style_content=style_content,
            target_word_count=target_word_count,
        )

    context_stats = (
        dict(chapter_context.context_stats)
        if isinstance(getattr(chapter_context, 'context_stats', None), dict)
        else {}
    )
    logger.info('批量生成 - 上下文摘要')
    logger.info(f'  - 章节号: {chapter.chapter_number}')
    logger.info(f"  - 续写点长度: {len(getattr(chapter_context, 'continuation_point', '') or '')} 字")
    logger.info(f"  - 记忆数: {context_stats.get('memory_count', 0)} 条")
    logger.info(f"  - 上下文总长: {context_stats.get('total_length', 0)} 字")

    outline_runtime_sources = build_outline_structure_runtime_sources_fn(outline)
    return BatchGenerationBuiltContext(
        chapter_context=chapter_context,
        outline_runtime_sources=outline_runtime_sources,
    )


async def resolve_batch_generation_chapter_runtime(
    *,
    db_session: "AsyncSession",
    user_id: str,
    project: Any,
    chapter: "Chapter",
    outline: Any,
    outline_mode: str,
    target_word_count: int,
    style_id: Optional[int],
    story_packet: Optional["StoryPacket"],
    base_quality_profile: Optional[Dict[str, Any]],
    research_assets: list[Any],
    creative_mode: Optional[str],
    story_focus: Optional[str],
    plot_stage: Optional[str],
    story_creation_brief: Optional[str],
    quality_preset: Optional[str],
    quality_notes: Optional[str],
    memory_service: Any,
    foreshadow_service: Any,
    story_repair_state: Optional[Dict[str, Any]],
    story_repair_payload: Optional["StoryRepairPayload"],
    active_story_repair_snapshot: Optional[Dict[str, Any]],
    build_generation_runtime_bundle_fn: Callable[..., Any],
    build_story_packet_fn: Callable[..., Any] = build_story_generation_packet_with_project_continuity,
    clone_quality_profile_fn: Callable[..., Dict[str, Any]] = clone_chapter_quality_profile,
    resolve_quality_profile_fn: Callable[..., Any] = resolve_chapter_quality_profile,
    one_to_one_builder_cls: Callable[..., Any] = OneToOneContextBuilder,
    one_to_many_builder_cls: Callable[..., Any] = OneToManyContextBuilder,
    build_outline_structure_runtime_sources_fn: Callable[[Any], Any] = build_outline_structure_runtime_sources,
    prepare_runtime_fn: Callable[..., Awaitable[BatchGenerationRuntimePreparation]] = prepare_batch_generation_runtime,
    build_context_fn: Callable[..., Awaitable[BatchGenerationBuiltContext]] = build_batch_generation_context,
    finalize_runtime_fn: Callable[..., BatchGenerationResolvedRuntime] = finalize_batch_generation_runtime,
) -> BatchGenerationChapterRuntimeArtifacts:
    runtime_preparation = await prepare_runtime_fn(
        db_session=db_session,
        user_id=user_id,
        project=project,
        chapter=chapter,
        target_word_count=target_word_count,
        style_id=style_id,
        story_packet=story_packet,
        base_quality_profile=base_quality_profile,
        research_assets=research_assets,
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
        build_story_packet_fn=build_story_packet_fn,
        clone_quality_profile_fn=clone_quality_profile_fn,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
    )
    built_context = await build_context_fn(
        db_session=db_session,
        chapter=chapter,
        project=project,
        outline=outline,
        outline_mode=outline_mode,
        user_id=user_id,
        target_word_count=target_word_count,
        style_content=runtime_preparation.style_content,
        memory_service=memory_service,
        foreshadow_service=foreshadow_service,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=build_outline_structure_runtime_sources_fn,
    )
    resolved_runtime = finalize_runtime_fn(
        runtime_preparation=runtime_preparation,
        project=project,
        chapter=chapter,
        chapter_context=built_context.chapter_context,
        target_word_count=target_word_count,
        outline_runtime_sources=built_context.outline_runtime_sources,
        story_repair_state=story_repair_state,
        story_repair_payload=story_repair_payload,
        active_story_repair_snapshot=active_story_repair_snapshot,
        build_generation_runtime_bundle_fn=build_generation_runtime_bundle_fn,
    )
    return BatchGenerationChapterRuntimeArtifacts(
        effective_story_packet=runtime_preparation.effective_story_packet,
        generation_guidance=runtime_preparation.generation_guidance,
        quality_profile=runtime_preparation.quality_profile,
        style_id=runtime_preparation.style_id,
        style_content=runtime_preparation.style_content,
        style_name=runtime_preparation.style_name,
        style_preset_id=runtime_preparation.style_preset_id,
        chapter_context=built_context.chapter_context,
        outline_runtime_sources=built_context.outline_runtime_sources,
        generation_runtime=resolved_runtime.generation_runtime,
        generation_intent=resolved_runtime.generation_intent,
        prompt_quality_kwargs=resolved_runtime.prompt_quality_kwargs,
        story_runtime_contract=resolved_runtime.story_runtime_contract,
    )


def resolve_quality_gate_execution_plan(*args, **kwargs):
    from tests.test_support.story_repair_payload_test_support import (
        resolve_quality_gate_execution_plan as resolve_quality_gate_execution_plan_impl,
    )

    return resolve_quality_gate_execution_plan_impl(*args, **kwargs)


async def collect_batch_single_chapter_generation_candidate_output(
    ai_service: AIService,
    generate_kwargs: Dict[str, Any],
    *,
    candidate_index: int = 1,
    max_output_chars: Optional[int] = None,
    runtime_state: Optional[Dict[str, Any]] = None,
) -> tuple[str, list[str]]:
    from tests.test_support.chapter_candidate_executor_test_support import (
        ChapterCandidateOutputRequest,
        collect_generation_candidate_output,
    )

    return await collect_generation_candidate_output(
        request=ChapterCandidateOutputRequest(
            ai_service=ai_service,
            generate_kwargs=generate_kwargs,
            candidate_index=candidate_index,
            max_output_chars=max_output_chars,
            runtime_state=runtime_state,
        ),
    )


def resolve_batch_single_chapter_generation_attempt_labels(
    candidate_index: int,
    *,
    is_word_budget_repair: bool = False,
) -> tuple[str, str]:
    from tests.test_support.chapter_candidate_executor_test_support import (
        resolve_generation_attempt_labels,
    )

    return resolve_generation_attempt_labels(
        candidate_index,
        is_word_budget_repair=is_word_budget_repair,
    )


def sync_batch_single_chapter_generation_runtime_state(
    runtime_state: Optional[Dict[str, Any]],
    *,
    candidate_index: int,
    candidate_total: int,
    current_chars: Optional[int] = None,
    chunk_count: Optional[int] = None,
    generation_path: Optional[str] = None,
    attempt_kind: Optional[str] = None,
    rerank_used: Optional[bool] = None,
    word_budget_repair_used: Optional[bool] = None,
    winner_candidate_index: Optional[int] = None,
) -> None:
    from tests.test_support.chapter_candidate_runtime_state_test_support import (
        sync_chapter_candidate_runtime_state,
    )

    sync_chapter_candidate_runtime_state(
        runtime_state,
        candidate_index=candidate_index,
        candidate_total=candidate_total,
        current_chars=current_chars,
        chunk_count=chunk_count,
        generation_path=generation_path,
        attempt_kind=attempt_kind,
        rerank_used=rerank_used,
        word_budget_repair_used=word_budget_repair_used,
        winner_candidate_index=winner_candidate_index,
    )


def build_batch_single_chapter_generation_candidate_record(
    *,
    full_content: str,
    candidate_chunks: list[str],
    target_word_count: int,
    source: str,
    generation_label: str,
    candidate_index: int,
    candidate_offset: int,
    quality_evaluator,
    quality_gate_plan_builder,
    generation_path: str,
    attempt_kind: str,
    log_warning_fn,
):
    from tests.test_support.chapter_candidate_executor_test_support import (
        ChapterCandidateRecordRequest,
        build_generation_candidate_record,
    )

    return build_generation_candidate_record(
        request=ChapterCandidateRecordRequest(
            full_content=full_content,
            candidate_chunks=candidate_chunks,
            target_word_count=target_word_count,
            source=source,
            generation_label=generation_label,
            candidate_index=candidate_index,
            candidate_offset=candidate_offset,
            quality_evaluator=quality_evaluator,
            quality_gate_plan_builder=quality_gate_plan_builder,
            generation_path=generation_path,
            attempt_kind=attempt_kind,
        ),
        log_warning_fn=log_warning_fn,
    )


def build_batch_single_chapter_generation_candidate_record_with_default_logging(**kwargs):
    from tests.test_support.retired_runtime_test_support import get_logger

    logger = get_logger(__name__)
    return build_batch_single_chapter_generation_candidate_record(
        **kwargs,
        log_warning_fn=logger.warning,
    )


def build_batch_generation_candidate_quality_hooks(
    *,
    story_packet: "StoryPacket",
    project,
    chapter,
    chapter_context: Any,
    target_word_count: int,
    generation_intent: Any,
    retry_count: int,
    max_retries: int,
    current_story_repair_payload: Optional["StoryRepairPayload"],
    build_quality_runtime_context_fn: Callable[..., Dict[str, Any]],
    compute_story_quality_metrics_fn: Callable[..., Dict[str, Any]],
    resolve_quality_gate_execution_plan_fn: Callable[..., Dict[str, Any]],
    log_prefix: str = "Batch",
) -> BatchGenerationCandidateQualityHooks:
    def quality_evaluator(generated_content: str) -> Dict[str, Any]:
        quality_runtime_context = build_quality_runtime_context_fn(
            story_packet=story_packet,
            project=project,
            chapter=chapter,
            chapter_context=chapter_context,
            target_word_count=target_word_count,
            generation_intent=generation_intent,
        )
        metrics = compute_story_quality_metrics_fn(
            content=generated_content,
            chapter_outline=chapter_context.chapter_outline,
            world_rules=project.world_rules,
            quality_runtime_context=quality_runtime_context,
        )
        logger.info(
            f'{log_prefix} candidate metrics - overall={metrics["overall_score"]}, '
            f'conflict={metrics["conflict_chain_hit_rate"]}, '
            f'rule={metrics["rule_grounding_hit_rate"]}'
        )
        return metrics

    def quality_gate_plan_builder(
        candidate_metrics: Dict[str, Any],
        attempt_offset: int,
    ) -> Dict[str, Any]:
        return resolve_quality_gate_execution_plan_fn(
            candidate_metrics if isinstance(candidate_metrics, dict) else None,
            retry_count=retry_count,
            max_retries=max_retries,
            current_story_repair_payload=current_story_repair_payload,
            scope="batch",
        )

    return BatchGenerationCandidateQualityHooks(
        quality_evaluator=quality_evaluator,
        quality_gate_plan_builder=quality_gate_plan_builder,
    )


def build_batch_generation_candidate_runtime_state(*, max_candidates: int) -> Dict[str, Any]:
    return build_chapter_candidate_runtime_state(max_candidates=max_candidates)


def create_batch_generation_candidate_execution(
    *,
    ai_service: Any,
    generate_kwargs: Dict[str, Any],
    target_word_count: int,
    chapter_number: int,
    quality_evaluator: Callable[[str], Dict[str, Any]],
    quality_gate_plan_builder: Callable[[Dict[str, Any], int], Dict[str, Any]],
    max_candidates: int,
    candidate_generator_fn: Callable[..., Any],
) -> BatchGenerationCandidateExecution:
    runtime_state = build_batch_generation_candidate_runtime_state(max_candidates=max_candidates)
    selected_candidate_task = asyncio.create_task(
        candidate_generator_fn(
            ai_service=ai_service,
            base_generate_kwargs=generate_kwargs,
            target_word_count=target_word_count,
            source="batch",
            generation_label=f"chapter={chapter_number}",
            quality_evaluator=quality_evaluator,
            quality_gate_plan_builder=quality_gate_plan_builder,
            max_candidates=max_candidates,
            runtime_state=runtime_state,
        )
    )
    return BatchGenerationCandidateExecution(
        runtime_state=runtime_state,
        selected_candidate_task=selected_candidate_task,
    )


async def wait_for_batch_generation_candidate(
    *,
    selected_candidate_task: asyncio.Task,
    runtime_state: Dict[str, Any],
    stream_task_id: Optional[str],
    chapter: "Chapter",
    target_word_count: int,
    heartbeat_interval_seconds: float,
    db_session: "AsyncSession",
    publish_stream_event_fn: Callable[..., Awaitable[None]] = _publish_task_stream_event,
) -> Dict[str, Any]:
    try:
        while True:
            try:
                return await asyncio.wait_for(
                    asyncio.shield(selected_candidate_task),
                    timeout=heartbeat_interval_seconds,
                )
            except asyncio.TimeoutError:
                runtime_snapshot = snapshot_chapter_candidate_runtime_state(runtime_state)
                if stream_task_id:
                    await publish_stream_event_fn(
                        stream_task_id,
                        build_batch_generation_candidate_progress_event(
                            chapter=chapter,
                            runtime_snapshot=runtime_snapshot,
                            target_word_count=target_word_count,
                        ),
                        db_session=db_session,
                    )
    finally:
        if not selected_candidate_task.done():
            selected_candidate_task.cancel()


async def emit_batch_generation_selected_candidate_events(
    *,
    stream_task_id: Optional[str],
    stream_chunks: bool,
    chapter: "Chapter",
    selected_candidate: Dict[str, Any],
    candidate_word_count: int,
    quality_gate_plan: Dict[str, Any],
    chapter_context_stats: Dict[str, Any],
    db_session: "AsyncSession",
    publish_stream_event_fn: Callable[..., Awaitable[None]] = _publish_task_stream_event,
) -> None:
    selected_candidate_view = snapshot_chapter_candidate(selected_candidate)
    if stream_task_id:
        await publish_stream_event_fn(
            stream_task_id,
            build_batch_generation_selected_candidate_progress_event(
                chapter=chapter,
                selected_candidate_view=selected_candidate_view,
                candidate_word_count=candidate_word_count,
                chapter_context_stats=chapter_context_stats,
            ),
            db_session=db_session,
        )

    if stream_task_id and stream_chunks and str(quality_gate_plan.get("action") or "continue") == "continue":
        for chunk in selected_candidate_view.candidate_chunks:
            await publish_stream_event_fn(
                stream_task_id,
                build_batch_generation_chunk_event(chapter=chapter, chunk=chunk),
            )


def build_batch_generation_selected_candidate_result(
    *,
    chapter: "Chapter",
    selected_candidate: Dict[str, Any],
    story_runtime_contract: Optional[Dict[str, Any]],
    attach_story_runtime_contract_fn: Callable[[Any, Optional[Dict[str, Any]]], Any] = attach_story_runtime_contract,
) -> Dict[str, Any]:
    normalized_result = normalize_selected_candidate_result(
        selected_candidate=selected_candidate,
        story_runtime_contract=story_runtime_contract,
        attach_story_runtime_contract_fn=attach_story_runtime_contract_fn,
    )
    full_content = normalized_result.full_content
    candidate_word_count = normalized_result.candidate_word_count
    quality_metrics = dict(normalized_result.quality_metrics)
    quality_gate_plan = dict(normalized_result.quality_gate_plan)
    candidate_count = normalized_result.candidate_count

    logger.info(f"Batch candidate ready: chapter={chapter.chapter_number}, word_count={candidate_word_count}")
    if candidate_count > 1:
        logger.info(
            f"Batch candidate rerank winner: chapter={chapter.chapter_number}, "
            f"candidate_count={candidate_count}, "
            f"winner={normalized_result.candidate_index}"
        )

    summary_preview = full_content[:300].replace("\n", " ") if full_content else ""
    return {
        "full_content": full_content,
        "word_count": candidate_word_count,
        "summary_preview": summary_preview,
        "quality_metrics": quality_metrics,
        "quality_gate_plan": quality_gate_plan,
        "candidate_count": candidate_count,
        "story_runtime_contract": story_runtime_contract,
    }


async def execute_batch_generation_candidate_flow(
    *,
    stream_task_id: Optional[str],
    stream_chunks: bool,
    chapter: "Chapter",
    effective_story_packet: "StoryPacket",
    project,
    chapter_context: Any,
    target_word_count: int,
    generation_intent: Any,
    current_story_repair_payload: Optional["StoryRepairPayload"],
    retry_count: int,
    max_retries: int,
    default_candidate_limit: int,
    ai_service: Any,
    generate_kwargs: Dict[str, Any],
    story_runtime_contract: Optional[Dict[str, Any]],
    db_session: "AsyncSession",
    heartbeat_interval_seconds: float,
    build_quality_runtime_context_fn: Callable[..., Dict[str, Any]],
    compute_story_quality_metrics_fn: Callable[..., Dict[str, Any]],
    resolve_quality_gate_execution_plan_fn: Callable[..., Dict[str, Any]],
    candidate_generator_fn: Callable[..., Any],
    attach_story_runtime_contract_fn: Callable[[Any, Optional[Dict[str, Any]]], Any],
    build_candidate_quality_hooks_fn: Callable[..., BatchGenerationCandidateQualityHooks] = build_batch_generation_candidate_quality_hooks,
    create_candidate_execution_fn: Callable[..., BatchGenerationCandidateExecution] = create_batch_generation_candidate_execution,
    wait_for_candidate_fn: Callable[..., Awaitable[Dict[str, Any]]] = wait_for_batch_generation_candidate,
    emit_selected_candidate_events_fn: Callable[..., Awaitable[None]] = emit_batch_generation_selected_candidate_events,
    build_selected_candidate_result_fn: Callable[..., Dict[str, Any]] = build_batch_generation_selected_candidate_result,
) -> BatchGenerationCandidateFlowResult:
    max_candidates = max(1, int(default_candidate_limit or 1)) if retry_count <= 0 else 1
    candidate_quality_hooks = build_candidate_quality_hooks_fn(
        story_packet=effective_story_packet,
        project=project,
        chapter=chapter,
        chapter_context=chapter_context,
        target_word_count=target_word_count,
        generation_intent=generation_intent,
        retry_count=retry_count,
        max_retries=max_retries,
        current_story_repair_payload=current_story_repair_payload,
        build_quality_runtime_context_fn=build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        log_prefix="Batch",
    )
    evaluate_candidate_quality = candidate_quality_hooks.quality_evaluator
    build_candidate_quality_gate_plan = candidate_quality_hooks.quality_gate_plan_builder

    if stream_task_id and stream_chunks:
        candidate_execution = create_candidate_execution_fn(
            ai_service=ai_service,
            generate_kwargs=generate_kwargs,
            target_word_count=target_word_count,
            chapter_number=chapter.chapter_number,
            quality_evaluator=evaluate_candidate_quality,
            quality_gate_plan_builder=build_candidate_quality_gate_plan,
            max_candidates=max_candidates,
            candidate_generator_fn=candidate_generator_fn,
        )
        selected_candidate = await wait_for_candidate_fn(
            selected_candidate_task=candidate_execution.selected_candidate_task,
            runtime_state=candidate_execution.runtime_state,
            stream_task_id=stream_task_id,
            chapter=chapter,
            target_word_count=target_word_count,
            heartbeat_interval_seconds=heartbeat_interval_seconds,
            db_session=db_session,
        )
    else:
        selected_candidate = await candidate_generator_fn(
            ai_service=ai_service,
            base_generate_kwargs=generate_kwargs,
            target_word_count=target_word_count,
            source="batch",
            generation_label=f"chapter={chapter.chapter_number}",
            quality_evaluator=evaluate_candidate_quality,
            quality_gate_plan_builder=build_candidate_quality_gate_plan,
            max_candidates=max_candidates,
        )

    chapter_context_stats = (
        dict(chapter_context.context_stats)
        if isinstance(getattr(chapter_context, "context_stats", None), dict)
        else {}
    )
    selected_candidate_result = build_selected_candidate_result_fn(
        chapter=chapter,
        selected_candidate=selected_candidate,
        story_runtime_contract=story_runtime_contract,
        attach_story_runtime_contract_fn=attach_story_runtime_contract_fn,
    )
    await emit_selected_candidate_events_fn(
        stream_task_id=stream_task_id,
        stream_chunks=stream_chunks,
        chapter=chapter,
        selected_candidate=selected_candidate,
        candidate_word_count=int(selected_candidate_result.get("word_count") or 0),
        quality_gate_plan=selected_candidate_result.get("quality_gate_plan") or {},
        chapter_context_stats=chapter_context_stats,
        db_session=db_session,
    )
    return BatchGenerationCandidateFlowResult(
        selected_candidate=selected_candidate,
        selected_candidate_result=selected_candidate_result,
    )


async def execute_batch_generation_generation_stage(
    *,
    stream_task_id: Optional[str],
    stream_chunks: bool,
    chapter: "Chapter",
    effective_story_packet: "StoryPacket",
    project,
    chapter_context: Any,
    target_word_count: int,
    generation_intent: Any,
    current_story_repair_payload: Optional["StoryRepairPayload"],
    retry_count: int,
    max_retries: int,
    default_candidate_limit: int,
    ai_service: Any,
    generate_kwargs: Dict[str, Any],
    story_runtime_contract: Optional[Dict[str, Any]],
    db_session: "AsyncSession",
    heartbeat_interval_seconds: float,
    build_quality_runtime_context_fn: Callable[..., Dict[str, Any]],
    compute_story_quality_metrics_fn: Callable[..., Dict[str, Any]],
    resolve_quality_gate_execution_plan_fn: Callable[..., Dict[str, Any]],
    candidate_generator_fn: Callable[..., Any],
    attach_story_runtime_contract_fn: Callable[[Any, Optional[Dict[str, Any]]], Any],
    publish_stream_event_fn: Callable[..., Awaitable[None]] = _publish_task_stream_event,
    execute_candidate_flow_fn: Callable[..., Awaitable[BatchGenerationCandidateFlowResult]] = execute_batch_generation_candidate_flow,
) -> BatchGenerationCandidateFlowResult:
    if stream_task_id and stream_chunks:
        await publish_stream_event_fn(
            stream_task_id,
            build_batch_generation_start_progress_event(chapter=chapter),
            db_session=db_session,
        )

    return await execute_candidate_flow_fn(
        stream_task_id=stream_task_id,
        stream_chunks=stream_chunks,
        chapter=chapter,
        effective_story_packet=effective_story_packet,
        project=project,
        chapter_context=chapter_context,
        target_word_count=target_word_count,
        generation_intent=generation_intent,
        current_story_repair_payload=current_story_repair_payload,
        retry_count=retry_count,
        max_retries=max_retries,
        default_candidate_limit=default_candidate_limit,
        ai_service=ai_service,
        generate_kwargs=generate_kwargs,
        story_runtime_contract=story_runtime_contract,
        db_session=db_session,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
        build_quality_runtime_context_fn=build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        candidate_generator_fn=candidate_generator_fn,
        attach_story_runtime_contract_fn=attach_story_runtime_contract_fn,
    )
async def generate_best_ranked_batch_single_chapter_candidate(
    *,
    ai_service,
    base_generate_kwargs: Dict[str, Any],
    target_word_count: int,
    source: str,
    generation_label: str,
    quality_evaluator,
    quality_gate_plan_builder,
    max_candidates: int = CHAPTER_CANDIDATE_RERANK_LIMIT,
    runtime_state: Optional[Dict[str, Any]] = None,
):
    from tests.test_support.chapter_candidate_executor_test_support import generate_best_ranked_candidate

    return await generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs=base_generate_kwargs,
        target_word_count=target_word_count,
        source=source,
        generation_label=generation_label,
        quality_evaluator=quality_evaluator,
        quality_gate_plan_builder=quality_gate_plan_builder,
        max_candidates=max_candidates,
        runtime_state=runtime_state,
        resolve_generation_attempt_labels_fn=resolve_batch_single_chapter_generation_attempt_labels,
        sync_generation_runtime_state_fn=sync_batch_single_chapter_generation_runtime_state,
        collect_generation_candidate_output_fn=collect_batch_single_chapter_generation_candidate_output,
        build_generation_candidate_record_fn=build_batch_single_chapter_generation_candidate_record_with_default_logging,
    )


@dataclass(slots=True)
class BatchGenerationSingleChapterRequest:
    db_session: AsyncSession
    chapter: Chapter
    user_id: str
    style_id: Optional[int]
    target_word_count: int
    ai_service: AIService
    write_lock: Lock
    story_packet: Optional[StoryPacket] = None
    base_quality_profile: Optional[Dict[str, Any]] = None
    custom_model: Optional[str] = None
    previous_summary_context: Optional[str] = None
    temp_narrative_perspective: Optional[str] = None
    creative_mode: Optional[str] = None
    story_focus: Optional[str] = None
    plot_stage: Optional[str] = None
    story_creation_brief: Optional[str] = None
    quality_preset: Optional[str] = None
    quality_notes: Optional[str] = None
    enable_web_research: Optional[bool] = None
    web_research_query: Optional[str] = None
    story_repair_summary: Optional[str] = None
    story_repair_targets: Optional[list[str]] = None
    story_preserve_strengths: Optional[list[str]] = None
    story_repair_payload: Optional[StoryRepairPayload] = None
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None
    story_repair_state: Optional[Dict[str, Any]] = None
    stream_task_id: Optional[str] = None
    stream_chunks: bool = False
    retry_count: int = 0
    max_retries: int = 1


@dataclass(slots=True)
class BatchGenerationSingleChapterDependencies:
    chapter_web_research_service: Any
    publish_task_stream_event_fn: Callable[..., Any]
    prepare_project_outline_context_fn: Callable[..., Any]
    resolve_batch_generation_chapter_runtime_fn: Callable[..., Any]
    build_generation_runtime_bundle_fn: Callable[..., Any]
    build_story_packet_fn: Callable[..., Any]
    clone_quality_profile_fn: Callable[..., Any]
    resolve_quality_profile_fn: Callable[..., Any]
    one_to_one_builder_cls: Any
    one_to_many_builder_cls: Any
    build_outline_structure_runtime_sources_fn: Callable[..., Any]
    execute_prompt_stage_fn: Callable[..., Any]
    get_template_fn: Callable[..., Any]
    format_prompt_fn: Callable[..., Any]
    apply_style_to_prompt_fn: Callable[..., Any]
    build_runtime_system_prompt_fn: Callable[..., Any]
    calculate_max_tokens_fn: Callable[..., Any]
    build_request_options_fn: Callable[..., Any]
    detect_style_profile_fn: Callable[..., Any]
    resolve_generation_temperature_fn: Callable[..., Any]
    execute_generation_stage_fn: Callable[..., Any]
    build_quality_runtime_context_fn: Callable[..., Any]
    compute_story_quality_metrics_fn: Callable[..., Any]
    resolve_quality_gate_execution_plan_fn: Callable[..., Any]
    candidate_generator_fn: Callable[..., Any]
    attach_story_runtime_contract_fn: Callable[..., Any]
    memory_service: Any
    foreshadow_service: Any
    default_candidate_limit: int
    heartbeat_interval_seconds: float


def build_batch_generation_single_chapter_request(
    *,
    db_session: AsyncSession,
    chapter: Chapter,
    user_id: str,
    style_id: Optional[int],
    target_word_count: int,
    ai_service: AIService,
    write_lock: Lock,
    story_packet: Optional[StoryPacket] = None,
    base_quality_profile: Optional[Dict[str, Any]] = None,
    custom_model: Optional[str] = None,
    previous_summary_context: Optional[str] = None,
    temp_narrative_perspective: Optional[str] = None,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
    enable_web_research: Optional[bool] = None,
    web_research_query: Optional[str] = None,
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[list[str]] = None,
    story_preserve_strengths: Optional[list[str]] = None,
    story_repair_payload: Optional[StoryRepairPayload] = None,
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None,
    story_repair_state: Optional[Dict[str, Any]] = None,
    stream_task_id: Optional[str] = None,
    stream_chunks: bool = False,
    retry_count: int = 0,
    max_retries: int = 1,
) -> BatchGenerationSingleChapterRequest:
    return BatchGenerationSingleChapterRequest(
        db_session=db_session,
        chapter=chapter,
        user_id=user_id,
        style_id=style_id,
        target_word_count=target_word_count,
        ai_service=ai_service,
        write_lock=write_lock,
        story_packet=story_packet,
        base_quality_profile=base_quality_profile,
        custom_model=custom_model,
        previous_summary_context=previous_summary_context,
        temp_narrative_perspective=temp_narrative_perspective,
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
        enable_web_research=enable_web_research,
        web_research_query=web_research_query,
        story_repair_summary=story_repair_summary,
        story_repair_targets=story_repair_targets,
        story_preserve_strengths=story_preserve_strengths,
        story_repair_payload=story_repair_payload,
        active_story_repair_snapshot=active_story_repair_snapshot,
        story_repair_state=story_repair_state,
        stream_task_id=stream_task_id,
        stream_chunks=stream_chunks,
        retry_count=retry_count,
        max_retries=max_retries,
    )


def build_batch_generation_single_chapter_dependencies(
    *,
    chapter_web_research_service: Any,
    publish_task_stream_event_fn: Callable[..., Any],
    prepare_project_outline_context_fn: Callable[..., Any],
    resolve_batch_generation_chapter_runtime_fn: Callable[..., Any],
    build_generation_runtime_bundle_fn: Callable[..., Any],
    build_story_packet_fn: Callable[..., Any],
    clone_quality_profile_fn: Callable[..., Any],
    resolve_quality_profile_fn: Callable[..., Any],
    one_to_one_builder_cls: Any,
    one_to_many_builder_cls: Any,
    build_outline_structure_runtime_sources_fn: Callable[..., Any],
    execute_prompt_stage_fn: Callable[..., Any],
    get_template_fn: Callable[..., Any],
    format_prompt_fn: Callable[..., Any],
    apply_style_to_prompt_fn: Callable[..., Any],
    build_runtime_system_prompt_fn: Callable[..., Any],
    calculate_max_tokens_fn: Callable[..., Any],
    build_request_options_fn: Callable[..., Any],
    detect_style_profile_fn: Callable[..., Any],
    resolve_generation_temperature_fn: Callable[..., Any],
    execute_generation_stage_fn: Callable[..., Any],
    build_quality_runtime_context_fn: Callable[..., Any],
    compute_story_quality_metrics_fn: Callable[..., Any],
    resolve_quality_gate_execution_plan_fn: Callable[..., Any],
    candidate_generator_fn: Callable[..., Any],
    attach_story_runtime_contract_fn: Callable[..., Any],
    memory_service: Any,
    foreshadow_service: Any,
    default_candidate_limit: int,
    heartbeat_interval_seconds: float,
) -> BatchGenerationSingleChapterDependencies:
    return BatchGenerationSingleChapterDependencies(
        chapter_web_research_service=chapter_web_research_service,
        publish_task_stream_event_fn=publish_task_stream_event_fn,
        prepare_project_outline_context_fn=prepare_project_outline_context_fn,
        resolve_batch_generation_chapter_runtime_fn=resolve_batch_generation_chapter_runtime_fn,
        build_generation_runtime_bundle_fn=build_generation_runtime_bundle_fn,
        build_story_packet_fn=build_story_packet_fn,
        clone_quality_profile_fn=clone_quality_profile_fn,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=build_outline_structure_runtime_sources_fn,
        execute_prompt_stage_fn=execute_prompt_stage_fn,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        apply_style_to_prompt_fn=apply_style_to_prompt_fn,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=calculate_max_tokens_fn,
        build_request_options_fn=build_request_options_fn,
        detect_style_profile_fn=detect_style_profile_fn,
        resolve_generation_temperature_fn=resolve_generation_temperature_fn,
        execute_generation_stage_fn=execute_generation_stage_fn,
        build_quality_runtime_context_fn=build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        candidate_generator_fn=candidate_generator_fn,
        attach_story_runtime_contract_fn=attach_story_runtime_contract_fn,
        memory_service=memory_service,
        foreshadow_service=foreshadow_service,
        default_candidate_limit=default_candidate_limit,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
    )


async def generate_single_chapter_for_batch_workflow(
    *,
    request: BatchGenerationSingleChapterRequest,
    dependencies: BatchGenerationSingleChapterDependencies,
) -> Dict[str, Any]:
    project_outline_context = await dependencies.prepare_project_outline_context_fn(
        db_session=request.db_session,
        chapter=request.chapter,
        user_id=request.user_id,
        story_creation_brief=request.story_creation_brief,
        enable_web_research=request.enable_web_research,
        web_research_query=request.web_research_query,
        stream_task_id=request.stream_task_id,
        write_lock=request.write_lock,
        chapter_web_research_service=dependencies.chapter_web_research_service,
        publish_task_stream_event_fn=dependencies.publish_task_stream_event_fn,
    )

    resolved_chapter_runtime = await dependencies.resolve_batch_generation_chapter_runtime_fn(
        db_session=request.db_session,
        user_id=request.user_id,
        project=project_outline_context.project,
        chapter=request.chapter,
        outline=project_outline_context.outline,
        outline_mode=project_outline_context.outline_mode,
        target_word_count=request.target_word_count,
        style_id=request.style_id,
        story_packet=request.story_packet,
        base_quality_profile=request.base_quality_profile,
        research_assets=project_outline_context.research_assets,
        creative_mode=request.creative_mode,
        story_focus=request.story_focus,
        plot_stage=request.plot_stage,
        story_creation_brief=request.story_creation_brief,
        quality_preset=request.quality_preset,
        quality_notes=request.quality_notes,
        memory_service=dependencies.memory_service,
        foreshadow_service=dependencies.foreshadow_service,
        story_repair_state=request.story_repair_state,
        story_repair_payload=request.story_repair_payload,
        active_story_repair_snapshot=request.active_story_repair_snapshot,
        build_generation_runtime_bundle_fn=dependencies.build_generation_runtime_bundle_fn,
        build_story_packet_fn=dependencies.build_story_packet_fn,
        clone_quality_profile_fn=dependencies.clone_quality_profile_fn,
        resolve_quality_profile_fn=dependencies.resolve_quality_profile_fn,
        one_to_one_builder_cls=dependencies.one_to_one_builder_cls,
        one_to_many_builder_cls=dependencies.one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=dependencies.build_outline_structure_runtime_sources_fn,
    )
    effective_story_packet = resolved_chapter_runtime.effective_story_packet
    chapter_context = resolved_chapter_runtime.chapter_context
    generation_intent = resolved_chapter_runtime.generation_intent
    prompt_quality_kwargs = resolved_chapter_runtime.prompt_quality_kwargs
    story_runtime_contract = resolved_chapter_runtime.story_runtime_contract

    prompt_stage_result = await dependencies.execute_prompt_stage_fn(
        db_session=request.db_session,
        chapter=request.chapter,
        project=project_outline_context.project,
        chapter_context=chapter_context,
        outline_mode=project_outline_context.outline_mode,
        current_user_id=request.user_id,
        target_word_count=request.target_word_count,
        temp_narrative_perspective=request.temp_narrative_perspective,
        previous_summary_context=request.previous_summary_context,
        prompt_quality_kwargs=prompt_quality_kwargs,
        style_content=resolved_chapter_runtime.style_content,
        style_name=resolved_chapter_runtime.style_name,
        style_preset_id=resolved_chapter_runtime.style_preset_id,
        ai_service=request.ai_service,
        custom_model=request.custom_model,
        story_runtime_contract=story_runtime_contract,
        research_assets=project_outline_context.research_assets,
        get_template_fn=dependencies.get_template_fn,
        format_prompt_fn=dependencies.format_prompt_fn,
        apply_style_to_prompt_fn=dependencies.apply_style_to_prompt_fn,
        build_runtime_system_prompt_fn=dependencies.build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=dependencies.calculate_max_tokens_fn,
        build_request_options_fn=dependencies.build_request_options_fn,
        detect_style_profile_fn=dependencies.detect_style_profile_fn,
        resolve_generation_temperature_fn=dependencies.resolve_generation_temperature_fn,
    )

    candidate_flow_result = await dependencies.execute_generation_stage_fn(
        stream_task_id=request.stream_task_id,
        stream_chunks=request.stream_chunks,
        chapter=request.chapter,
        effective_story_packet=effective_story_packet,
        project=project_outline_context.project,
        chapter_context=chapter_context,
        target_word_count=request.target_word_count,
        generation_intent=generation_intent,
        current_story_repair_payload=request.story_repair_payload,
        retry_count=request.retry_count,
        max_retries=request.max_retries,
        default_candidate_limit=dependencies.default_candidate_limit,
        ai_service=request.ai_service,
        generate_kwargs=prompt_stage_result.generate_kwargs,
        story_runtime_contract=story_runtime_contract,
        db_session=request.db_session,
        heartbeat_interval_seconds=dependencies.heartbeat_interval_seconds,
        build_quality_runtime_context_fn=dependencies.build_quality_runtime_context_fn,
        compute_story_quality_metrics_fn=dependencies.compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=dependencies.resolve_quality_gate_execution_plan_fn,
        candidate_generator_fn=dependencies.candidate_generator_fn,
        attach_story_runtime_contract_fn=dependencies.attach_story_runtime_contract_fn,
    )

    return candidate_flow_result.selected_candidate_result


def build_default_batch_generation_single_chapter_dependencies(
    *,
    candidate_generator_fn: Callable[..., Any],
    default_candidate_limit: int,
    heartbeat_interval_seconds: float,
    chapter_web_research_service: Any = chapter_web_research_service,
    publish_task_stream_event_fn: Callable[..., Any] = _publish_task_stream_event,
    resolve_quality_profile_fn: Callable[..., Any] = resolve_chapter_quality_profile,
    one_to_one_builder_cls: Any = OneToOneContextBuilder,
    one_to_many_builder_cls: Any = OneToManyContextBuilder,
    get_template_fn: Callable[..., Any] = get_template,
    format_prompt_fn: Callable[..., Any] = format_prompt,
    build_runtime_system_prompt_fn: Callable[..., Any] = build_chapter_runtime_system_prompt,
    compute_story_quality_metrics_fn: Callable[..., Any] = compute_story_quality_metrics,
    resolve_quality_gate_execution_plan_fn: Callable[..., Any] = resolve_quality_gate_execution_plan,
):
    return build_batch_generation_single_chapter_dependencies(
        chapter_web_research_service=chapter_web_research_service,
        publish_task_stream_event_fn=publish_task_stream_event_fn,
        prepare_project_outline_context_fn=prepare_batch_generation_project_outline_context,
        resolve_batch_generation_chapter_runtime_fn=resolve_batch_generation_chapter_runtime,
        build_generation_runtime_bundle_fn=build_chapter_generation_runtime_bundle,
        build_story_packet_fn=build_story_generation_packet_with_project_continuity,
        clone_quality_profile_fn=clone_chapter_quality_profile,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        build_outline_structure_runtime_sources_fn=build_outline_structure_runtime_sources,
        execute_prompt_stage_fn=execute_batch_generation_prompt_stage,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        apply_style_to_prompt_fn=WritingStyleManager.apply_style_to_prompt,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        calculate_max_tokens_fn=_calculate_chapter_generation_max_tokens,
        build_request_options_fn=_build_chapter_generation_request_options,
        detect_style_profile_fn=detect_style_profile,
        resolve_generation_temperature_fn=resolve_generation_temperature,
        execute_generation_stage_fn=execute_batch_generation_generation_stage,
        build_quality_runtime_context_fn=build_chapter_quality_runtime_context,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
        candidate_generator_fn=candidate_generator_fn,
        attach_story_runtime_contract_fn=attach_story_runtime_contract,
        memory_service=_memory_service,
        foreshadow_service=_foreshadow_service,
        default_candidate_limit=default_candidate_limit,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
    )


async def _generate_best_ranked_candidate(*args, **kwargs):
    return await generate_best_ranked_batch_single_chapter_candidate(*args, **kwargs)


async def generate_single_chapter_for_batch_with_default_wiring(
    *,
    db_session: AsyncSession,
    chapter: Chapter,
    user_id: str,
    style_id: Optional[int],
    target_word_count: int,
    ai_service: AIService,
    write_lock: Lock,
    story_packet: Optional[StoryPacket] = None,
    base_quality_profile: Optional[Dict[str, Any]] = None,
    custom_model: Optional[str] = None,
    previous_summary_context: Optional[str] = None,
    temp_narrative_perspective: Optional[str] = None,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
    enable_web_research: Optional[bool] = None,
    web_research_query: Optional[str] = None,
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[list[str]] = None,
    story_preserve_strengths: Optional[list[str]] = None,
    story_repair_payload: Optional[StoryRepairPayload] = None,
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None,
    story_repair_state: Optional[Dict[str, Any]] = None,
    stream_task_id: Optional[str] = None,
    stream_chunks: bool = False,
    retry_count: int = 0,
    max_retries: int = 1,
    candidate_generator_fn: Callable[..., Any],
    default_candidate_limit: int,
    heartbeat_interval_seconds: float,
    chapter_web_research_service: Any = chapter_web_research_service,
    publish_task_stream_event_fn: Callable[..., Any] = _publish_task_stream_event,
    resolve_quality_profile_fn: Callable[..., Any] = resolve_chapter_quality_profile,
    one_to_one_builder_cls: Any = OneToOneContextBuilder,
    one_to_many_builder_cls: Any = OneToManyContextBuilder,
    get_template_fn: Callable[..., Any] = get_template,
    format_prompt_fn: Callable[..., Any] = format_prompt,
    build_runtime_system_prompt_fn: Callable[..., Any] = build_chapter_runtime_system_prompt,
    compute_story_quality_metrics_fn: Callable[..., Any] = compute_story_quality_metrics,
    resolve_quality_gate_execution_plan_fn: Callable[..., Any] = resolve_quality_gate_execution_plan,
) -> Dict[str, Any]:
    workflow_request = build_batch_generation_single_chapter_request(
        db_session=db_session,
        chapter=chapter,
        user_id=user_id,
        style_id=style_id,
        target_word_count=target_word_count,
        ai_service=ai_service,
        write_lock=write_lock,
        story_packet=story_packet,
        base_quality_profile=base_quality_profile,
        custom_model=custom_model,
        previous_summary_context=previous_summary_context,
        temp_narrative_perspective=temp_narrative_perspective,
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
        enable_web_research=enable_web_research,
        web_research_query=web_research_query,
        story_repair_summary=story_repair_summary,
        story_repair_targets=story_repair_targets,
        story_preserve_strengths=story_preserve_strengths,
        story_repair_payload=story_repair_payload,
        active_story_repair_snapshot=active_story_repair_snapshot,
        story_repair_state=story_repair_state,
        stream_task_id=stream_task_id,
        stream_chunks=stream_chunks,
        retry_count=retry_count,
        max_retries=max_retries,
    )
    workflow_dependencies = build_default_batch_generation_single_chapter_dependencies(
        candidate_generator_fn=candidate_generator_fn,
        default_candidate_limit=default_candidate_limit,
        heartbeat_interval_seconds=heartbeat_interval_seconds,
        chapter_web_research_service=chapter_web_research_service,
        publish_task_stream_event_fn=publish_task_stream_event_fn,
        resolve_quality_profile_fn=resolve_quality_profile_fn,
        one_to_one_builder_cls=one_to_one_builder_cls,
        one_to_many_builder_cls=one_to_many_builder_cls,
        get_template_fn=get_template_fn,
        format_prompt_fn=format_prompt_fn,
        build_runtime_system_prompt_fn=build_runtime_system_prompt_fn,
        compute_story_quality_metrics_fn=compute_story_quality_metrics_fn,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan_fn,
    )
    return await generate_single_chapter_for_batch_workflow(
        request=workflow_request,
        dependencies=workflow_dependencies,
    )


async def generate_single_chapter_for_batch(
    *,
    db_session: AsyncSession,
    chapter: Chapter,
    user_id: str,
    style_id: Optional[int],
    target_word_count: int,
    ai_service: AIService,
    write_lock: Lock,
    story_packet: Optional[StoryPacket] = None,
    base_quality_profile: Optional[Dict[str, Any]] = None,
    custom_model: Optional[str] = None,
    previous_summary_context: Optional[str] = None,
    temp_narrative_perspective: Optional[str] = None,
    creative_mode: Optional[str] = None,
    story_focus: Optional[str] = None,
    plot_stage: Optional[str] = None,
    story_creation_brief: Optional[str] = None,
    quality_preset: Optional[str] = None,
    quality_notes: Optional[str] = None,
    enable_web_research: Optional[bool] = None,
    web_research_query: Optional[str] = None,
    story_repair_summary: Optional[str] = None,
    story_repair_targets: Optional[list[str]] = None,
    story_preserve_strengths: Optional[list[str]] = None,
    story_repair_payload: Optional[StoryRepairPayload] = None,
    active_story_repair_snapshot: Optional[Dict[str, Any]] = None,
    story_repair_state: Optional[Dict[str, Any]] = None,
    stream_task_id: Optional[str] = None,
    stream_chunks: bool = False,
    retry_count: int = 0,
    max_retries: int = 1,
) -> Dict[str, Any]:
    return await generate_single_chapter_for_batch_with_default_wiring(
        db_session=db_session,
        chapter=chapter,
        user_id=user_id,
        style_id=style_id,
        target_word_count=target_word_count,
        ai_service=ai_service,
        write_lock=write_lock,
        story_packet=story_packet,
        base_quality_profile=base_quality_profile,
        custom_model=custom_model,
        previous_summary_context=previous_summary_context,
        temp_narrative_perspective=temp_narrative_perspective,
        creative_mode=creative_mode,
        story_focus=story_focus,
        plot_stage=plot_stage,
        story_creation_brief=story_creation_brief,
        quality_preset=quality_preset,
        quality_notes=quality_notes,
        enable_web_research=enable_web_research,
        web_research_query=web_research_query,
        story_repair_summary=story_repair_summary,
        story_repair_targets=story_repair_targets,
        story_preserve_strengths=story_preserve_strengths,
        story_repair_payload=story_repair_payload,
        active_story_repair_snapshot=active_story_repair_snapshot,
        story_repair_state=story_repair_state,
        stream_task_id=stream_task_id,
        stream_chunks=stream_chunks,
        retry_count=retry_count,
        max_retries=max_retries,
        candidate_generator_fn=_generate_best_ranked_candidate,
        default_candidate_limit=CHAPTER_CANDIDATE_RERANK_LIMIT,
        heartbeat_interval_seconds=CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS,
        chapter_web_research_service=chapter_web_research_service,
        publish_task_stream_event_fn=_publish_task_stream_event,
        resolve_quality_profile_fn=resolve_chapter_quality_profile,
        one_to_one_builder_cls=OneToOneContextBuilder,
        one_to_many_builder_cls=OneToManyContextBuilder,
        get_template_fn=get_template,
        format_prompt_fn=format_prompt,
        build_runtime_system_prompt_fn=build_chapter_runtime_system_prompt,
        compute_story_quality_metrics_fn=compute_story_quality_metrics,
        resolve_quality_gate_execution_plan_fn=resolve_quality_gate_execution_plan,
    )




