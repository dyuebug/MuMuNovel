from __future__ import annotations

import json
import re
from datetime import datetime
from functools import lru_cache
from pathlib import Path
from typing import Any, AsyncGenerator, Dict, Optional

from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.retired_runtime_test_support import get_logger
from migrator_app.models import (
    Career,
    CharacterCareer,
    CharacterRelationship,
    Organization,
    OrganizationMember,
    ProjectDefaultStyle,
    RelationshipType,
    WritingStyle,
)
from migrator_app.models.character import Character
from migrator_app.models.project import Project
from tests.test_support.ai_gateway.ai_service import AIService
from tests.test_support.prompt_template_facade_test_support import (
    format_prompt as _facade_format_prompt,
    get_template_for_owner,
)
from tests.test_support.chapter_web_research_test_support import (
    chapter_web_research_service,
)
from tests.test_support.utils.sse_response import WizardProgressTracker


logger = get_logger(__name__)

WIZARD_RESPONSES_TEXT_GENERATION_PROVIDERS = {"sub2api", "openai_responses"}
WIZARD_GENERATION_FIRST_CHUNK_TIMEOUT = 20.0
_PROMPT_SERVICE_SOURCE_PATH = (
    Path(__file__).resolve().parent / "prompt_service_test_support.py"
)


@lru_cache(maxsize=1)
def _load_wizard_prompt_template_map() -> dict[str, str]:
    source = _PROMPT_SERVICE_SOURCE_PATH.read_text(encoding="utf-8")
    template_keys = ("WORLD_BUILDING", "CAREER_SYSTEM_GENERATION")
    templates: dict[str, str] = {}
    for template_key in template_keys:
        match = re.search(
            rf'^\s*{template_key}\s*=\s*"""(.*?)"""',
            source,
            flags=re.MULTILINE | re.DOTALL,
        )
        if not match:
            raise RuntimeError(f"wizard test support 未找到模板常量: {template_key}")
        templates[template_key] = match.group(1)
    return templates


def _wizard_template_lookup(template_key: str) -> Optional[str]:
    return _load_wizard_prompt_template_map().get(template_key)


async def _default_get_wizard_template(
    template_key: str,
    user_id: str,
    db: AsyncSession,
):
    return await get_template_for_owner(
        template_key,
        user_id,
        db,
        template_lookup=_wizard_template_lookup,
    )


def _default_format_wizard_prompt(template: str, **kwargs) -> str:
    return _facade_format_prompt(template, **kwargs)


class PromptService:
    get_template = staticmethod(_default_get_wizard_template)
    format_prompt = staticmethod(_default_format_wizard_prompt)


async def get_template(*args, **kwargs):
    return await _default_get_wizard_template(*args, **kwargs)


def format_prompt(*args, **kwargs):
    return _default_format_wizard_prompt(*args, **kwargs)


_ORIGINAL_PROMPTSERVICE_GET_TEMPLATE = PromptService.get_template
_ORIGINAL_PROMPTSERVICE_FORMAT_PROMPT = PromptService.format_prompt


async def _get_wizard_template(template_key: str, user_id: str, db: AsyncSession):
    patched_impl = globals().get("get_template")
    if patched_impl is None:
        raise RuntimeError("wizard get_template 未定义")
    return await patched_impl(template_key, user_id, db)


def _format_wizard_prompt(template: str, **kwargs) -> str:
    patched_impl = globals().get("format_prompt")
    if patched_impl is None:
        raise RuntimeError("wizard format_prompt 未定义")
    return patched_impl(template, **kwargs)


def _build_wizard_generation_request_options(
    ai_service: AIService,
    provider: Optional[str] = None,
) -> Optional[Dict[str, Any]]:
    normalized_provider = (
        str(provider or getattr(ai_service, "api_provider", "") or "").strip().lower()
    )
    if normalized_provider not in WIZARD_RESPONSES_TEXT_GENERATION_PROVIDERS:
        return None

    retry_cfg = getattr(getattr(ai_service, "config", None), "retry", None)
    configured_retry_budget = int(getattr(retry_cfg, "max_retries", 2) or 2)
    transport_max_retries = max(1, min(configured_retry_budget, 2))
    return {
        "prefer_chat_completions": True,
        "prefer_normalized_v1_candidate": True,
        "transport_max_retries": transport_max_retries,
        "first_chunk_timeout": WIZARD_GENERATION_FIRST_CHUNK_TIMEOUT,
        "allow_non_stream_fallback": False,
    }


def _normalize_research_text(value: Any, limit: int = 180) -> str:
    text = " ".join(str(value or "").replace("\r", " ").replace("\n", " ").split()).strip()
    if len(text) <= limit:
        return text
    return text[: limit - 3].rstrip() + "..."


def _compose_research_seed(*values: Any, limit: int = 320) -> str:
    cleaned = [
        _normalize_research_text(value, 160)
        for value in values
        if _normalize_research_text(value, 160)
    ]
    seed = " | ".join(cleaned[:5])
    return seed[:limit]


def _normalize_reference_research_assets(
    value: Any,
    limit: int = 6,
) -> list[Dict[str, str]]:
    if not isinstance(value, list):
        return []

    normalized: list[Dict[str, str]] = []
    for item in value[:limit]:
        if not isinstance(item, dict):
            continue
        title = _normalize_research_text(
            item.get("title") or item.get("source") or "参考资料",
            120,
        )
        source = _normalize_research_text(item.get("source"), 300)
        summary = _normalize_research_text(
            item.get("summary") or item.get("raw_content") or item.get("title"),
            220,
        )
        if not title and not source and not summary:
            continue
        normalized.append(
            {
                "title": title or source or "参考资料",
                "source": source,
                "summary": summary,
            }
        )
    return normalized


def _merge_reference_research_assets(
    *asset_groups: list[Dict[str, str]],
    limit: int = 8,
) -> list[Dict[str, str]]:
    merged: list[Dict[str, str]] = []
    seen: set[str] = set()
    for group in asset_groups:
        for asset in group or []:
            title = _normalize_research_text(asset.get("title"), 120)
            source = _normalize_research_text(asset.get("source"), 300)
            summary = _normalize_research_text(asset.get("summary"), 220)
            if not title and not source and not summary:
                continue

            dedupe_key = f"{title}|{source}|{summary}"
            if dedupe_key in seen:
                continue
            seen.add(dedupe_key)
            merged.append(
                {
                    "title": title or source or "参考资料",
                    "source": source,
                    "summary": summary,
                }
            )
            if len(merged) >= limit:
                return merged
    return merged


async def _save_project_research_assets(
    *,
    db: AsyncSession,
    user_id: Optional[str],
    project_id: str,
    query: str,
    archive_path: str,
    assets: list[Dict[str, str]],
    memory_type: str,
    title_prefix: str,
) -> None:
    if not user_id or not assets:
        return
    try:
        await chapter_web_research_service.replace_memories(
            db_session=db,
            user_id=user_id,
            project_id=project_id,
            query=query,
            archive_path=archive_path,
            assets=assets,
            memory_type=memory_type,
            title_prefix=title_prefix,
            story_timeline=0,
            chapter_id=None,
        )
    except Exception as error:
        logger.warning(f"⚠️ 保存项目级研究资料失败: {error}")


def _normalize_optional_text(value: Any) -> Optional[str]:
    if value is None:
        return None

    normalized = str(value).strip()
    return normalized or None


async def world_building_generator(
    data: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> AsyncGenerator[str, None]:
    """世界构建流式生成器 - 支持MCP工具增强"""
    db_committed = False
    tracker = WizardProgressTracker("世界观")

    try:
        yield await tracker.start()

        title = data.get("title")
        description = data.get("description")
        theme = data.get("theme")
        genre = data.get("genre")
        narrative_perspective = data.get("narrative_perspective")
        target_words = data.get("target_words")
        chapter_count = data.get("chapter_count")
        character_count = data.get("character_count")
        outline_mode = data.get("outline_mode", "one-to-many")
        default_creative_mode = data.get("default_creative_mode")
        default_story_focus = data.get("default_story_focus")
        default_plot_stage = data.get("default_plot_stage")
        default_story_creation_brief = data.get("default_story_creation_brief")
        default_quality_preset = data.get("default_quality_preset")
        default_quality_notes = data.get("default_quality_notes")
        provider = data.get("provider")
        model = data.get("model")
        enable_mcp = data.get("enable_mcp", True)
        enable_web_research = data.get("enable_web_research")
        web_research_query = data.get("web_research_query")
        user_id = data.get("user_id")
        reference_research_assets = _normalize_reference_research_assets(
            data.get("reference_research_assets")
        )
        request_options = _build_wizard_generation_request_options(
            user_ai_service,
            provider,
        )

        if not title or not description or not theme or not genre:
            yield await tracker.error("title、description、theme 和 genre 是必需的参数", 400)
            return

        preparing_message = "Preparing AI generation..."
        if chapter_web_research_service.is_enabled(enable_web_research) and web_research_query:
            preparing_message = (
                f"Preparing AI generation with web research: {web_research_query}..."
            )
        yield await tracker.preparing(preparing_message)

        world_research_seed = web_research_query or _compose_research_seed(
            title,
            theme,
            genre,
            description,
        )
        world_research_context = web_research_query or _compose_research_seed(
            title,
            theme,
            genre,
            description,
            limit=260,
        )
        world_research_bundle = await chapter_web_research_service.collect_assets(
            user_id=user_id,
            db_session=db,
            exa_query=world_research_seed,
            grok_query=(
                "Collect world-building references, genre conventions, cultural details, "
                "historical signals, and setting inspirations relevant to: "
                f"{world_research_context}"
            )
            if world_research_context
            else "",
            enable_web_research=enable_web_research,
            archive_scope=f"wizard_{user_id or 'anonymous'}",
            archive_id=f"world_building_{datetime.now().strftime('%Y%m%d%H%M%S')}",
            metadata={"title": title, "theme": theme, "genre": genre},
        )
        world_research_assets = _merge_reference_research_assets(
            reference_research_assets,
            list(world_research_bundle.get("assets") or []),
        )
        template = await _get_wizard_template("WORLD_BUILDING", user_id, db)
        base_prompt = _format_wizard_prompt(
            template,
            title=title,
            theme=theme,
            genre=genre or "通用类型",
            description=description or "暂无简介",
            external_assets=world_research_assets,
            reference_assets=world_research_assets,
        )

        if user_id:
            user_ai_service.user_id = user_id
            user_ai_service.db_session = db

        max_world_retries = 3
        world_retry_count = 0
        world_generation_success = False
        world_data: Dict[str, Any] = {}
        estimated_total = 1000

        while world_retry_count < max_world_retries and not world_generation_success:
            try:
                if world_retry_count > 0:
                    tracker.reset_generating_progress()

                yield await tracker.generating(
                    current_chars=0,
                    estimated_total=estimated_total,
                    retry_count=world_retry_count,
                    max_retries=max_world_retries,
                )

                accumulated_text = ""
                chunk_count = 0

                async for chunk in user_ai_service.generate_text_stream(
                    prompt=base_prompt,
                    provider=provider,
                    model=model,
                    tool_choice="required",
                    auto_mcp=enable_mcp,
                    request_options=request_options,
                ):
                    chunk_count += 1
                    accumulated_text += chunk
                    yield await tracker.generating_chunk(chunk)

                    current_len = len(accumulated_text)
                    if chunk_count % 10 == 0:
                        yield await tracker.generating(
                            current_chars=current_len,
                            estimated_total=estimated_total,
                            retry_count=world_retry_count,
                            max_retries=max_world_retries,
                        )
                    if chunk_count % 20 == 0:
                        yield await tracker.heartbeat()

                if not accumulated_text or not accumulated_text.strip():
                    logger.warning(
                        f"⚠️ AI返回空世界观（尝试{world_retry_count + 1}/{max_world_retries}）"
                    )
                    world_retry_count += 1
                    if world_retry_count < max_world_retries:
                        yield await tracker.retry(world_retry_count, max_world_retries, "AI返回为空")
                        continue
                    world_data = {
                        "time_period": "AI多次返回为空，请稍后重试",
                        "location": "AI多次返回为空，请稍后重试",
                        "atmosphere": "AI多次返回为空，请稍后重试",
                        "rules": "AI多次返回为空，请稍后重试",
                    }
                    world_generation_success = True
                    break

                yield await tracker.parsing("解析世界观数据...")

                try:
                    cleaned_text = user_ai_service._clean_json_response(accumulated_text)
                    world_data = json.loads(cleaned_text)
                    logger.info(
                        f"✅ 世界观JSON解析成功（尝试{world_retry_count + 1}/{max_world_retries}）"
                    )
                    world_generation_success = True
                except json.JSONDecodeError as error:
                    logger.error(
                        f"❌ 世界构建JSON解析失败（尝试{world_retry_count + 1}/{max_world_retries}）: {error}"
                    )
                    logger.error(f"   原始内容长度: {len(accumulated_text)}")
                    logger.error(f"   原始内容预览: {accumulated_text[:200]}")
                    world_retry_count += 1
                    if world_retry_count < max_world_retries:
                        yield await tracker.retry(world_retry_count, max_world_retries, "JSON解析失败")
                        continue
                    world_data = {
                        "time_period": "AI返回格式错误，请重试",
                        "location": "AI返回格式错误，请重试",
                        "atmosphere": "AI返回格式错误，请重试",
                        "rules": "AI返回格式错误，请重试",
                    }
                    world_generation_success = True

            except Exception as error:
                logger.error(
                    f"❌ 世界构建生成异常（尝试{world_retry_count + 1}/{max_world_retries}）: "
                    f"{type(error).__name__}: {error}"
                )
                world_retry_count += 1
                if world_retry_count < max_world_retries:
                    yield await tracker.retry(world_retry_count, max_world_retries, "生成异常")
                    continue
                raise

        yield await tracker.saving("保存世界观到数据库...")

        if not user_id:
            yield await tracker.error("用户ID缺失，无法创建项目", 401)
            return

        project = Project(
            user_id=user_id,
            title=title,
            description=description,
            theme=theme,
            genre=genre,
            world_time_period=world_data.get("time_period"),
            world_location=world_data.get("location"),
            world_atmosphere=world_data.get("atmosphere"),
            world_rules=world_data.get("rules"),
            narrative_perspective=narrative_perspective,
            target_words=target_words,
            chapter_count=chapter_count,
            character_count=character_count,
            outline_mode=outline_mode,
            default_creative_mode=default_creative_mode,
            default_story_focus=default_story_focus,
            default_plot_stage=default_plot_stage,
            default_story_creation_brief=default_story_creation_brief,
            default_quality_preset=default_quality_preset,
            default_quality_notes=default_quality_notes,
            wizard_status="incomplete",
            wizard_step=1,
            status="planning",
        )
        db.add(project)
        await db.commit()
        await db.refresh(project)

        yield await tracker.saving("保存默认写作风格...", 0.6)

        try:
            result = await db.execute(
                select(WritingStyle)
                .where(
                    WritingStyle.user_id.is_(None),
                    WritingStyle.order_index == 1,
                )
                .limit(1)
            )
            first_style = result.scalar_one_or_none()
            if first_style:
                default_style = ProjectDefaultStyle(
                    project_id=project.id,
                    style_id=first_style.id,
                )
                db.add(default_style)
                await db.commit()
                logger.info(f"为项目 {project.id} 自动设置默认风格: {first_style.name}")
        except Exception as error:
            logger.warning(f"设置默认写作风格失败: {error}，不影响项目创建")

        project.wizard_step = 1
        await db.commit()

        await _save_project_research_assets(
            db=db,
            user_id=user_id,
            project_id=project.id,
            query=str(world_research_bundle.get("query") or ""),
            archive_path=str(world_research_bundle.get("archive_path") or ""),
            assets=world_research_assets,
            memory_type=chapter_web_research_service.WORLD_MEMORY_TYPE,
            title_prefix="世界观外部资料",
        )

        db_committed = True

        yield await tracker.complete()
        yield await tracker.result(
            {
                "project_id": project.id,
                "time_period": world_data.get("time_period"),
                "location": world_data.get("location"),
                "atmosphere": world_data.get("atmosphere"),
                "rules": world_data.get("rules"),
                "research_query": str(world_research_bundle.get("query") or ""),
                "research_assets": world_research_assets,
            }
        )
        yield await tracker.done()
        logger.info(f"✅ 世界观生成完成，项目ID: {project.id}")

    except GeneratorExit:
        logger.warning("世界构建生成器被提前关闭")
        if not db_committed and db.in_transaction():
            await db.rollback()
            logger.info("世界构建事务已回滚（GeneratorExit）")
    except Exception as error:
        logger.error(f"世界构建流式生成失败: {str(error)}")
        if not db_committed and db.in_transaction():
            await db.rollback()
            logger.info("世界构建事务已回滚（异常）")
        yield await tracker.error(f"生成失败: {str(error)}")


async def world_building_regenerate_generator(
    project_id: str,
    data: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> AsyncGenerator[str, None]:
    """世界观重新生成流式生成器"""
    db_committed = False
    tracker = WizardProgressTracker("世界观")

    try:
        yield await tracker.start("开始重新生成世界观...")
        yield await tracker.loading("加载项目信息...")

        result = await db.execute(select(Project).where(Project.id == project_id))
        project = result.scalar_one_or_none()
        if not project:
            yield await tracker.error("项目不存在", 404)
            return

        provider = data.get("provider")
        model = data.get("model")
        enable_mcp = data.get("enable_mcp", True)
        user_id = data.get("user_id")
        request_options = _build_wizard_generation_request_options(user_ai_service, provider)

        yield await tracker.preparing("准备AI提示词...")
        template = await _get_wizard_template("WORLD_BUILDING", user_id, db)
        base_prompt = _format_wizard_prompt(
            template,
            title=project.title,
            theme=project.theme or "未设定",
            genre=project.genre or "通用",
            description=project.description or "暂无简介",
        )

        if user_id:
            user_ai_service.user_id = user_id
            user_ai_service.db_session = db

        max_world_retries = 3
        world_retry_count = 0
        world_generation_success = False
        world_data: Dict[str, Any] = {}
        estimated_total = 1000

        while world_retry_count < max_world_retries and not world_generation_success:
            try:
                if world_retry_count > 0:
                    tracker.reset_generating_progress()

                yield await tracker.generating(
                    current_chars=0,
                    estimated_total=estimated_total,
                    message="重新生成世界观",
                    retry_count=world_retry_count,
                    max_retries=max_world_retries,
                )

                accumulated_text = ""
                chunk_count = 0

                async for chunk in user_ai_service.generate_text_stream(
                    prompt=base_prompt,
                    provider=provider,
                    model=model,
                    tool_choice="required",
                    auto_mcp=enable_mcp,
                    request_options=request_options,
                ):
                    chunk_count += 1
                    accumulated_text += chunk
                    yield await tracker.generating_chunk(chunk)

                    current_len = len(accumulated_text)
                    if chunk_count % 10 == 0:
                        yield await tracker.generating(
                            current_chars=current_len,
                            estimated_total=estimated_total,
                            message="重新生成世界观",
                            retry_count=world_retry_count,
                            max_retries=max_world_retries,
                        )
                    if chunk_count % 20 == 0:
                        yield await tracker.heartbeat()

                if not accumulated_text or not accumulated_text.strip():
                    logger.warning(
                        f"⚠️ AI返回空世界观（尝试{world_retry_count + 1}/{max_world_retries}）"
                    )
                    world_retry_count += 1
                    if world_retry_count < max_world_retries:
                        yield await tracker.retry(world_retry_count, max_world_retries, "AI返回为空")
                        continue
                    world_data = {
                        "time_period": "AI多次返回为空，请稍后重试",
                        "location": "AI多次返回为空，请稍后重试",
                        "atmosphere": "AI多次返回为空，请稍后重试",
                        "rules": "AI多次返回为空，请稍后重试",
                    }
                    world_generation_success = True
                    break

                yield await tracker.parsing("解析AI返回结果...")
                try:
                    cleaned_text = user_ai_service._clean_json_response(accumulated_text)
                    world_data = json.loads(cleaned_text)
                    logger.info(
                        f"✅ 世界观重新生成JSON解析成功（尝试{world_retry_count + 1}/{max_world_retries}）"
                    )
                    world_generation_success = True
                except json.JSONDecodeError as error:
                    logger.error(
                        f"❌ 世界构建JSON解析失败（尝试{world_retry_count + 1}/{max_world_retries}）: {error}"
                    )
                    logger.error(f"   原始内容长度: {len(accumulated_text)}")
                    logger.error(f"   原始内容预览: {accumulated_text[:200]}")
                    world_retry_count += 1
                    if world_retry_count < max_world_retries:
                        yield await tracker.retry(world_retry_count, max_world_retries, "JSON解析失败")
                        continue
                    world_data = {
                        "time_period": "AI返回格式错误，请重试",
                        "location": "AI返回格式错误，请重试",
                        "atmosphere": "AI返回格式错误，请重试",
                        "rules": "AI返回格式错误，请重试",
                    }
                    world_generation_success = True

            except Exception as error:
                logger.error(
                    f"❌ 世界观重新生成异常（尝试{world_retry_count + 1}/{max_world_retries}）: "
                    f"{type(error).__name__}: {error}"
                )
                world_retry_count += 1
                if world_retry_count < max_world_retries:
                    yield await tracker.retry(world_retry_count, max_world_retries, "生成异常")
                    continue
                raise

        yield await tracker.saving("生成完成，等待用户确认...", 0.5)
        yield await tracker.complete()
        yield await tracker.result(
            {
                "time_period": world_data.get("time_period"),
                "location": world_data.get("location"),
                "atmosphere": world_data.get("atmosphere"),
                "rules": world_data.get("rules"),
            }
        )
        yield await tracker.done()

    except GeneratorExit:
        logger.warning("世界观重新生成器被提前关闭")
        if not db_committed and db.in_transaction():
            await db.rollback()
            logger.info("世界观重新生成事务已回滚（GeneratorExit）")
    except Exception as error:
        logger.error(f"世界观重新生成失败: {str(error)}")
        if not db_committed and db.in_transaction():
            await db.rollback()
            logger.info("世界观重新生成事务已回滚（异常）")
        yield await tracker.error(f"生成失败: {str(error)}")


async def career_system_generator(
    data: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> AsyncGenerator[str, None]:
    """职业体系生成流式生成器。"""
    db_committed = False
    tracker = WizardProgressTracker("职业体系")

    try:
        yield await tracker.start()

        project_id = data.get("project_id")
        provider = data.get("provider")
        model = data.get("model")
        enable_mcp = data.get("enable_mcp", True)
        enable_web_research = data.get("enable_web_research")
        web_research_query = data.get("web_research_query")
        user_id = data.get("user_id")
        reference_research_assets = _normalize_reference_research_assets(
            data.get("reference_research_assets")
        )

        if not project_id:
            yield await tracker.error("project_id 是必需的参数", 400)
            return

        yield await tracker.loading("加载项目信息...")
        result = await db.execute(select(Project).where(Project.id == project_id))
        project = result.scalar_one_or_none()
        if not project:
            yield await tracker.error("项目不存在", 404)
            return

        if user_id:
            user_ai_service.user_id = user_id
            user_ai_service.db_session = db

        world_data = {
            "time_period": project.world_time_period or "未设定",
            "location": project.world_location or "未设定",
            "atmosphere": project.world_atmosphere or "未设定",
            "rules": project.world_rules or "未设定",
        }

        yield await tracker.preparing("准备AI提示词...")
        careers_research_context = _compose_research_seed(
            project.title,
            project.theme,
            project.genre,
            world_data.get("time_period"),
            world_data.get("location"),
            world_data.get("rules"),
            limit=260,
        )
        careers_research_seed = web_research_query or careers_research_context
        careers_research_bundle = await chapter_web_research_service.collect_assets(
            user_id=user_id,
            db_session=db,
            exa_query=careers_research_seed,
            grok_query=(
                "请为小说职业体系设计做实时网络研究，提炼职业分层、晋升逻辑、能力体系、社会分工与可借鉴设定，并给出来源。"
                f"背景：{careers_research_context}"
            )
            if careers_research_context
            else "",
            enable_web_research=enable_web_research,
            archive_scope=project.id,
            archive_id="wizard_careers",
            metadata={"project_id": project.id, "context": "careers"},
        )
        careers_research_assets = _merge_reference_research_assets(
            reference_research_assets,
            list(careers_research_bundle.get("assets") or []),
        )
        template = await _get_wizard_template("CAREER_SYSTEM_GENERATION", user_id, db)
        career_prompt = _format_wizard_prompt(
            template,
            title=project.title,
            genre=project.genre or "未设定",
            theme=project.theme or "未设定",
            description=project.description or "暂无简介",
            time_period=world_data.get("time_period", "未设定"),
            location=world_data.get("location", "未设定"),
            atmosphere=world_data.get("atmosphere", "未设定"),
            rules=world_data.get("rules", "未设定"),
            external_assets=careers_research_assets,
            reference_assets=careers_research_assets,
        )

        request_options = _build_wizard_generation_request_options(
            user_ai_service,
            provider,
        )
        estimated_total = 5000
        max_career_retries = 3
        career_retry_count = 0
        career_generation_success = False

        while career_retry_count < max_career_retries and not career_generation_success:
            try:
                if career_retry_count > 0:
                    tracker.reset_generating_progress()

                yield await tracker.generating(
                    current_chars=0,
                    estimated_total=estimated_total,
                    retry_count=career_retry_count,
                    max_retries=max_career_retries,
                )

                career_response = ""
                chunk_count = 0

                async for chunk in user_ai_service.generate_text_stream(
                    prompt=career_prompt,
                    provider=provider,
                    model=model,
                    auto_mcp=enable_mcp,
                    request_options=request_options,
                ):
                    chunk_count += 1
                    career_response += chunk
                    yield await tracker.generating_chunk(chunk)

                    current_len = len(career_response)
                    if chunk_count % 10 == 0:
                        yield await tracker.generating(
                            current_chars=current_len,
                            estimated_total=estimated_total,
                            retry_count=career_retry_count,
                            max_retries=max_career_retries,
                        )
                    if chunk_count % 20 == 0:
                        yield await tracker.heartbeat()

                if not career_response or not career_response.strip():
                    logger.warning(
                        f"⚠️ AI返回空职业体系（尝试{career_retry_count + 1}/{max_career_retries}）"
                    )
                    career_retry_count += 1
                    if career_retry_count < max_career_retries:
                        yield await tracker.retry(career_retry_count, max_career_retries, "AI返回为空")
                        continue
                    yield await tracker.error("职业体系生成失败（AI多次返回为空）")
                    return

                yield await tracker.parsing("解析职业体系数据...")

                try:
                    cleaned_response = user_ai_service._clean_json_response(career_response)
                    career_data = json.loads(cleaned_response)
                    logger.info(
                        f"✅ 职业体系JSON解析成功（尝试{career_retry_count + 1}/{max_career_retries}）"
                    )

                    yield await tracker.saving("保存职业数据...")

                    main_careers_created = []
                    for idx, career_info in enumerate(career_data.get("main_careers", [])):
                        try:
                            stages_json = json.dumps(
                                career_info.get("stages", []),
                                ensure_ascii=False,
                            )
                            attribute_bonuses = career_info.get("attribute_bonuses")
                            attribute_bonuses_json = (
                                json.dumps(attribute_bonuses, ensure_ascii=False)
                                if attribute_bonuses
                                else None
                            )

                            career = Career(
                                project_id=project.id,
                                name=career_info.get("name", f"未命名主职业{idx + 1}"),
                                type="main",
                                description=career_info.get("description"),
                                category=career_info.get("category"),
                                stages=stages_json,
                                max_stage=career_info.get("max_stage", 10),
                                requirements=career_info.get("requirements"),
                                special_abilities=career_info.get("special_abilities"),
                                worldview_rules=career_info.get("worldview_rules"),
                                attribute_bonuses=attribute_bonuses_json,
                                source="ai",
                            )
                            db.add(career)
                            await db.flush()
                            main_careers_created.append(career.name)
                            logger.info(f"  ✅ 创建主职业：{career.name}")
                        except Exception as error:
                            logger.error(f"  ❌ 创建主职业失败：{error}")
                            continue

                    sub_careers_created = []
                    for idx, career_info in enumerate(career_data.get("sub_careers", [])):
                        try:
                            stages_json = json.dumps(
                                career_info.get("stages", []),
                                ensure_ascii=False,
                            )
                            attribute_bonuses = career_info.get("attribute_bonuses")
                            attribute_bonuses_json = (
                                json.dumps(attribute_bonuses, ensure_ascii=False)
                                if attribute_bonuses
                                else None
                            )

                            career = Career(
                                project_id=project.id,
                                name=career_info.get("name", f"未命名副职业{idx + 1}"),
                                type="sub",
                                description=career_info.get("description"),
                                category=career_info.get("category"),
                                stages=stages_json,
                                max_stage=career_info.get("max_stage", 5),
                                requirements=career_info.get("requirements"),
                                special_abilities=career_info.get("special_abilities"),
                                worldview_rules=career_info.get("worldview_rules"),
                                attribute_bonuses=attribute_bonuses_json,
                                source="ai",
                            )
                            db.add(career)
                            await db.flush()
                            sub_careers_created.append(career.name)
                            logger.info(f"  ✅ 创建副职业：{career.name}")
                        except Exception as error:
                            logger.error(f"  ❌ 创建副职业失败：{error}")
                            continue

                    project.wizard_step = 2
                    await _save_project_research_assets(
                        db=db,
                        user_id=user_id,
                        project_id=project.id,
                        query=str(careers_research_bundle.get("query") or ""),
                        archive_path=str(careers_research_bundle.get("archive_path") or ""),
                        assets=careers_research_assets,
                        memory_type=chapter_web_research_service.CAREERS_MEMORY_TYPE,
                        title_prefix="职业体系外部资料",
                    )

                    await db.commit()
                    db_committed = True
                    career_generation_success = True
                    logger.info(
                        "🎉 职业体系生成完成：主职业%s个，副职业%s个",
                        len(main_careers_created),
                        len(sub_careers_created),
                    )

                    yield await tracker.complete()
                    yield await tracker.result(
                        {
                            "project_id": project.id,
                            "main_careers_count": len(main_careers_created),
                            "sub_careers_count": len(sub_careers_created),
                            "main_careers": main_careers_created,
                            "sub_careers": sub_careers_created,
                            "research_query": str(careers_research_bundle.get("query") or ""),
                            "research_assets": careers_research_assets,
                        }
                    )
                    yield await tracker.done()

                except json.JSONDecodeError as error:
                    logger.error(
                        f"❌ 职业体系JSON解析失败（尝试{career_retry_count + 1}/{max_career_retries}）: {error}"
                    )
                    career_retry_count += 1
                    if career_retry_count < max_career_retries:
                        yield await tracker.retry(career_retry_count, max_career_retries, "JSON解析失败")
                        continue
                    yield await tracker.error("职业体系解析失败（已达最大重试次数）")
                    return
                except Exception as error:
                    logger.error(
                        f"❌ 职业体系保存失败（尝试{career_retry_count + 1}/{max_career_retries}）: {error}"
                    )
                    career_retry_count += 1
                    if career_retry_count < max_career_retries:
                        yield await tracker.retry(career_retry_count, max_career_retries, "保存失败")
                        continue
                    yield await tracker.error("职业体系保存失败（已达最大重试次数）")
                    return

            except Exception as error:
                logger.error(
                    f"❌ 职业体系生成异常（尝试{career_retry_count + 1}/{max_career_retries}）: {error}"
                )
                career_retry_count += 1
                if career_retry_count < max_career_retries:
                    yield await tracker.retry(career_retry_count, max_career_retries, "生成异常")
                    continue
                yield await tracker.error(f"职业体系生成失败: {error}")
                return

    except GeneratorExit:
        logger.warning("职业体系生成器被提前关闭")
        if not db_committed and db.in_transaction():
            await db.rollback()
            logger.info("职业体系事务已回滚（GeneratorExit）")
    except Exception as error:
        logger.error(f"职业体系流式生成失败: {error}")
        if not db_committed and db.in_transaction():
            await db.rollback()
            logger.info("职业体系事务已回滚（异常）")
        yield await tracker.error(f"生成失败: {error}")


async def characters_generator(
    data: Dict[str, Any],
    db: AsyncSession,
    user_ai_service: AIService,
) -> AsyncGenerator[str, None]:
    """角色批量生成流式生成器。"""
    db_committed = False
    tracker = WizardProgressTracker("角色")

    try:
        yield await tracker.start()

        project_id = data.get("project_id")
        count = data.get("count", 5)
        world_context = data.get("world_context")
        theme = data.get("theme", "")
        genre = data.get("genre", "")
        requirements = _normalize_optional_text(data.get("requirements")) or ""
        provider = data.get("provider")
        model = data.get("model")
        enable_mcp = data.get("enable_mcp", True)
        enable_web_research = data.get("enable_web_research")
        web_research_query = data.get("web_research_query")
        user_id = data.get("user_id")
        reference_research_assets = _normalize_reference_research_assets(
            data.get("reference_research_assets")
        )

        yield await tracker.loading("验证项目...", 0.3)
        result = await db.execute(select(Project).where(Project.id == project_id))
        project = result.scalar_one_or_none()
        if not project:
            yield await tracker.error("项目不存在", 404)
            return

        project.wizard_step = 2
        world_context = world_context or {
            "time_period": project.world_time_period or "未设定",
            "location": project.world_location or "未设定",
            "atmosphere": project.world_atmosphere or "未设定",
            "rules": project.world_rules or "未设定",
        }

        if user_id:
            user_ai_service.user_id = user_id
            user_ai_service.db_session = db

        characters_research_context = _compose_research_seed(
            project.title,
            project.theme,
            project.genre,
            world_context.get("location"),
            requirements,
            limit=260,
        )
        characters_research_bundle = await chapter_web_research_service.collect_assets(
            user_id=user_id,
            db_session=db,
            exa_query=web_research_query
            or _compose_research_seed(
                project.title,
                project.theme,
                project.genre,
                world_context.get("location"),
                world_context.get("rules"),
                requirements,
            ),
            grok_query=(
                "请为小说角色设计做实时网络研究，提炼人物原型、职业细节、社会语境、表达习惯与可借鉴素材，并给出来源。"
                f"背景：{characters_research_context}"
            )
            if characters_research_context
            else "",
            enable_web_research=enable_web_research,
            archive_scope=project.id,
            archive_id="wizard_characters",
            metadata={"project_id": project.id, "context": "characters"},
        )
        characters_research_assets = _merge_reference_research_assets(
            reference_research_assets,
            list(characters_research_bundle.get("assets") or []),
        )

        yield await tracker.loading("加载职业体系...", 0.8)
        career_result = await db.execute(
            select(Career).where(Career.project_id == project_id).order_by(Career.type, Career.id)
        )
        careers = career_result.scalars().all()
        main_careers = [career for career in careers if career.type == "main"]
        sub_careers = [career for career in careers if career.type == "sub"]

        careers_context = ""
        if main_careers or sub_careers:
            careers_context = "\n\n【职业体系】\n"
            if main_careers:
                careers_context += "主职业：\n"
                for career in main_careers:
                    careers_context += f"- {career.name}: {career.description or '暂无描述'}\n"
            if sub_careers:
                careers_context += "\n副职业：\n"
                for career in sub_careers:
                    careers_context += f"- {career.name}: {career.description or '暂无描述'}\n"

            careers_context += "\n请为每个角色分配职业：\n"
            careers_context += "- 每个角色必须有1个主职业（从上述主职业中选择）\n"
            careers_context += "- 每个角色可以有0-2个副职业（从上述副职业中选择，可选）\n"
            careers_context += "- 主职业初始阶段建议为1-3\n"
            careers_context += "- 副职业初始阶段建议为1-2\n"
            careers_context += "- 请在返回的JSON中包含 career_assignment 字段：\n"
            careers_context += (
                '  {"main_career": "职业名称", "main_stage": 2, '
                '"sub_careers": [{"career": "副职业名称", "stage": 1}]}\n'
            )
            logger.info(f"✅ 加载了{len(main_careers)}个主职业和{len(sub_careers)}个副职业")
        else:
            logger.warning("⚠️ 项目没有职业体系，跳过职业分配")

        batch_size = 5
        max_retries = 3
        all_characters = []
        request_options = _build_wizard_generation_request_options(
            user_ai_service,
            provider,
        )
        total_batches = (count + batch_size - 1) // batch_size

        for batch_idx in range(total_batches):
            remaining = count - len(all_characters)
            current_batch_size = min(batch_size, remaining)
            if current_batch_size <= 0:
                logger.info(f"已生成{len(all_characters)}个角色,达到目标数量{count}")
                break

            retry_count = 0
            batch_success = False
            batch_error_message = ""

            while retry_count < max_retries and not batch_success:
                try:
                    if retry_count > 0:
                        tracker.reset_generating_progress()

                    yield await tracker.generating(
                        current_chars=0,
                        estimated_total=batch_size * 800,
                        message=f"生成第{batch_idx + 1}/{total_batches}批角色 ({current_batch_size}个)",
                        retry_count=retry_count,
                        max_retries=max_retries,
                    )

                    existing_chars_context = ""
                    if all_characters:
                        existing_chars_context = "\n\n【已生成的角色】:\n"
                        for char in all_characters:
                            existing_chars_context += (
                                f"- {char.get('name')}: {char.get('role_type', '未知')}, "
                                f"{char.get('personality', '暂无')[:50]}...\n"
                            )
                        existing_chars_context += "\n请确保新角色与已有角色形成合理的关系网络和互动。\n"

                    if batch_idx == 0:
                        if current_batch_size == 1:
                            batch_requirements = f"{requirements}\n请生成1个主角(protagonist)"
                        else:
                            batch_requirements = (
                                f"{requirements}\n请精确生成{current_batch_size}个角色:"
                                f"1个主角(protagonist)和{current_batch_size - 1}个核心配角(supporting)"
                            )
                    else:
                        batch_requirements = (
                            f"{requirements}\n请精确生成{current_batch_size}个角色{existing_chars_context}"
                        )
                        if batch_idx == total_batches - 1:
                            batch_requirements += "\n可以包含组织或反派(antagonist)"
                        else:
                            batch_requirements += "\n主要是配角(supporting)和反派(antagonist)"

                    template = await _get_wizard_template(
                        "CHARACTERS_BATCH_GENERATION",
                        user_id,
                        db,
                    )
                    prompt = _format_wizard_prompt(
                        template,
                        count=current_batch_size,
                        time_period=world_context.get("time_period", ""),
                        location=world_context.get("location", ""),
                        atmosphere=world_context.get("atmosphere", ""),
                        rules=world_context.get("rules", ""),
                        theme=theme or project.theme or "",
                        genre=genre or project.genre or "",
                        requirements=batch_requirements + careers_context,
                        external_assets=characters_research_assets,
                        reference_assets=characters_research_assets,
                    )

                    accumulated_text = ""
                    chunk_count = 0
                    estimated_total = batch_size * 800

                    async for chunk in user_ai_service.generate_text_stream(
                        prompt=prompt,
                        provider=provider,
                        model=model,
                        tool_choice="required",
                        auto_mcp=enable_mcp,
                        request_options=request_options,
                    ):
                        chunk_count += 1
                        accumulated_text += chunk
                        yield await tracker.generating_chunk(chunk)

                        current_len = len(accumulated_text)
                        if chunk_count % 10 == 0:
                            yield await tracker.generating(
                                current_chars=current_len,
                                estimated_total=estimated_total,
                                message=f"生成第{batch_idx + 1}/{total_batches}批角色中",
                                retry_count=retry_count,
                                max_retries=max_retries,
                            )
                        if chunk_count % 20 == 0:
                            yield await tracker.heartbeat()

                    cleaned_text = user_ai_service._clean_json_response(accumulated_text)
                    characters_data = json.loads(cleaned_text)
                    if not isinstance(characters_data, list):
                        characters_data = [characters_data]

                    if len(characters_data) != current_batch_size:
                        error_msg = (
                            f"批次{batch_idx + 1}生成数量不正确: "
                            f"期望{current_batch_size}个, 实际{len(characters_data)}个"
                        )
                        logger.error(error_msg)
                        if retry_count < max_retries - 1:
                            retry_count += 1
                            yield await tracker.retry(retry_count, max_retries, error_msg)
                            continue
                        yield await tracker.error(error_msg)
                        return

                    all_characters.extend(characters_data)
                    batch_success = True
                    logger.info(
                        f"批次{batch_idx + 1}成功添加{len(characters_data)}个角色,"
                        f"当前总数{len(all_characters)}/{count}"
                    )
                except json.JSONDecodeError as error:
                    logger.error(
                        f"批次{batch_idx + 1}解析失败(尝试{retry_count + 1}/{max_retries}): {error}"
                    )
                    batch_error_message = f"JSON解析失败: {error}"
                    retry_count += 1
                    if retry_count < max_retries:
                        yield await tracker.retry(retry_count, max_retries, "JSON解析失败")
                except Exception as error:
                    logger.error(
                        f"批次{batch_idx + 1}生成异常(尝试{retry_count + 1}/{max_retries}): {error}"
                    )
                    batch_error_message = f"生成异常: {error}"
                    retry_count += 1
                    if retry_count < max_retries:
                        yield await tracker.retry(retry_count, max_retries, "生成异常")

            if not batch_success:
                error_msg = f"批次{batch_idx + 1}在{max_retries}次重试后仍然失败"
                if batch_error_message:
                    error_msg += f": {batch_error_message}"
                logger.error(error_msg)
                yield await tracker.error(error_msg)
                return

        yield await tracker.parsing("验证角色数据...")
        valid_entity_names = set()
        valid_organization_names = set()

        for char_data in all_characters:
            entity_name = char_data.get("name", "")
            if entity_name:
                valid_entity_names.add(entity_name)
                if char_data.get("is_organization", False):
                    valid_organization_names.add(entity_name)

        cleaned_count = 0
        for char_data in all_characters:
            if "relationships_array" in char_data and isinstance(
                char_data["relationships_array"],
                list,
            ):
                original_rels = char_data["relationships_array"]
                valid_rels = []
                for rel in original_rels:
                    target_name = rel.get("target_character_name", "")
                    if target_name in valid_entity_names:
                        valid_rels.append(rel)
                    else:
                        cleaned_count += 1
                        logger.debug(
                            f"  🧹 清理无效关系引用：{char_data.get('name')} -> {target_name}"
                        )
                char_data["relationships_array"] = valid_rels

            if "organization_memberships" in char_data and isinstance(
                char_data["organization_memberships"],
                list,
            ):
                original_orgs = char_data["organization_memberships"]
                valid_orgs = []
                for org_mem in original_orgs:
                    org_name = org_mem.get("organization_name", "")
                    if org_name in valid_organization_names:
                        valid_orgs.append(org_mem)
                    else:
                        cleaned_count += 1
                        logger.debug(
                            f"  🧹 清理无效组织引用：{char_data.get('name')} -> {org_name}"
                        )
                char_data["organization_memberships"] = valid_orgs

        if cleaned_count > 0:
            logger.info(f"✨ 清理了{cleaned_count}个AI幻觉引用")
            yield await tracker.parsing(f"已清理{cleaned_count}个无效引用", 0.7)

        yield await tracker.saving("保存角色到数据库...")
        created_characters = []
        character_name_to_obj = {}

        for char_data in all_characters:
            relationships_text = ""
            relationships_array = char_data.get("relationships_array", [])
            if relationships_array and isinstance(relationships_array, list):
                rel_descriptions = []
                for rel in relationships_array:
                    target = rel.get("target_character_name", "未知")
                    rel_type = rel.get("relationship_type", "关系")
                    desc = rel.get("description", "")
                    rel_descriptions.append(f"{target}({rel_type}): {desc}")
                relationships_text = "; ".join(rel_descriptions)
            elif isinstance(char_data.get("relationships"), dict):
                relationships_text = json.dumps(
                    char_data.get("relationships"),
                    ensure_ascii=False,
                )
            elif isinstance(char_data.get("relationships"), str):
                relationships_text = char_data.get("relationships")

            is_organization = char_data.get("is_organization", False)
            character = Character(
                project_id=project_id,
                name=char_data.get("name", "未命名角色"),
                age=str(char_data.get("age", "")) if not is_organization else None,
                gender=char_data.get("gender") if not is_organization else None,
                is_organization=is_organization,
                role_type=char_data.get("role_type", "supporting"),
                personality=char_data.get("personality", ""),
                background=char_data.get("background", ""),
                appearance=char_data.get("appearance", ""),
                relationships=relationships_text,
                organization_type=char_data.get("organization_type")
                if is_organization
                else None,
                organization_purpose=char_data.get("organization_purpose")
                if is_organization
                else None,
                traits=json.dumps(char_data.get("traits", []), ensure_ascii=False)
                if char_data.get("traits")
                else None,
            )
            db.add(character)
            created_characters.append((character, char_data))

        await db.flush()

        if main_careers or sub_careers:
            yield await tracker.saving("分配角色职业...", 0.3)
            careers_assigned = 0
            career_name_to_obj = {career.name: career for career in careers}

            for character, char_data in created_characters:
                if character.is_organization:
                    continue

                try:
                    career_assignment = char_data.get("career_assignment", {})
                    main_career_name = career_assignment.get("main_career")
                    main_career_stage = career_assignment.get("main_stage", 1)

                    if main_career_name and main_career_name in career_name_to_obj:
                        main_career = career_name_to_obj[main_career_name]
                        char_career = CharacterCareer(
                            character_id=character.id,
                            career_id=main_career.id,
                            career_type="main",
                            current_stage=min(main_career_stage, main_career.max_stage),
                            stage_progress=0,
                        )
                        db.add(char_career)
                        character.main_career_id = main_career.id
                        character.main_career_stage = char_career.current_stage
                        careers_assigned += 1
                        logger.info(
                            f"  ✅ 分配主职业：{character.name} -> {main_career.name} "
                            f"(阶段{char_career.current_stage})"
                        )
                    elif main_career_name:
                        logger.warning(f"  ⚠️ 主职业不存在：{character.name} -> {main_career_name}")

                    sub_career_assignments = career_assignment.get("sub_careers", [])
                    sub_career_list = []
                    for sub_assign in sub_career_assignments[:2]:
                        sub_career_name = sub_assign.get("career")
                        sub_career_stage = sub_assign.get("stage", 1)
                        if sub_career_name and sub_career_name in career_name_to_obj:
                            sub_career = career_name_to_obj[sub_career_name]
                            char_career = CharacterCareer(
                                character_id=character.id,
                                career_id=sub_career.id,
                                career_type="sub",
                                current_stage=min(sub_career_stage, sub_career.max_stage),
                                stage_progress=0,
                            )
                            db.add(char_career)
                            sub_career_list.append(
                                {
                                    "career_id": sub_career.id,
                                    "stage": char_career.current_stage,
                                }
                            )
                            careers_assigned += 1
                            logger.info(
                                f"  ✅ 分配副职业：{character.name} -> {sub_career.name} "
                                f"(阶段{char_career.current_stage})"
                            )
                        elif sub_career_name:
                            logger.warning(f"  ⚠️ 副职业不存在：{character.name} -> {sub_career_name}")

                    if sub_career_list:
                        character.sub_careers = json.dumps(sub_career_list, ensure_ascii=False)
                except Exception as error:
                    logger.warning(f"  ❌ 分配职业失败：{character.name} - {error}")
                    continue

            await db.flush()
            logger.info(f"💼 职业分配完成：共分配{careers_assigned}个职业")
            yield await tracker.saving(f"已分配{careers_assigned}个职业", 0.4)

        for character, _ in created_characters:
            await db.refresh(character)
            character_name_to_obj[character.name] = character
            logger.info(
                f"向导创建角色：{character.name} (ID: {character.id}, 是否组织: {character.is_organization})"
            )

        yield await tracker.saving("创建组织记录...", 0.5)
        organization_name_to_obj = {}
        for character, char_data in created_characters:
            if character.is_organization:
                org_check = await db.execute(
                    select(Organization).where(Organization.character_id == character.id)
                )
                existing_org = org_check.scalar_one_or_none()
                if not existing_org:
                    org = Organization(
                        character_id=character.id,
                        project_id=project_id,
                        member_count=0,
                        power_level=char_data.get("power_level", 50),
                        location=char_data.get("location"),
                        motto=char_data.get("motto"),
                        color=char_data.get("color"),
                    )
                    db.add(org)
                    logger.info(f"向导创建组织记录：{character.name}")
                else:
                    org = existing_org
                organization_name_to_obj[character.name] = org

        await db.flush()
        for character, _ in created_characters:
            await db.refresh(character)

        yield await tracker.saving("创建角色关系...", 0.7)
        relationships_created = 0
        for character, char_data in created_characters:
            if character.is_organization:
                continue

            relationships_data = char_data.get("relationships_array", [])
            if not relationships_data and isinstance(char_data.get("relationships"), list):
                relationships_data = char_data.get("relationships")

            if relationships_data and isinstance(relationships_data, list):
                for rel in relationships_data:
                    try:
                        target_name = rel.get("target_character_name")
                        if not target_name:
                            logger.debug(
                                f"  ⚠️  {character.name}的关系缺少target_character_name，跳过"
                            )
                            continue

                        target_char = character_name_to_obj.get(target_name)
                        if target_char:
                            existing_rel = await db.execute(
                                select(CharacterRelationship).where(
                                    CharacterRelationship.project_id == project_id,
                                    CharacterRelationship.character_from_id == character.id,
                                    CharacterRelationship.character_to_id == target_char.id,
                                )
                            )
                            if existing_rel.scalar_one_or_none():
                                logger.debug(f"  ℹ️  关系已存在：{character.name} -> {target_name}")
                                continue

                            relationship = CharacterRelationship(
                                project_id=project_id,
                                character_from_id=character.id,
                                character_to_id=target_char.id,
                                relationship_name=rel.get("relationship_type", "未知关系"),
                                intimacy_level=rel.get("intimacy_level", 50),
                                description=rel.get("description", ""),
                                started_at=rel.get("started_at"),
                                source="ai",
                            )
                            rel_type_result = await db.execute(
                                select(RelationshipType).where(
                                    RelationshipType.name == rel.get("relationship_type")
                                )
                            )
                            rel_type = rel_type_result.scalar_one_or_none()
                            if rel_type:
                                relationship.relationship_type_id = rel_type.id

                            db.add(relationship)
                            relationships_created += 1
                            logger.info(
                                f"  ✅ 向导创建关系：{character.name} -> {target_name} "
                                f"({rel.get('relationship_type')})"
                            )
                        else:
                            logger.warning(
                                f"  ⚠️  目标角色不存在：{character.name} -> {target_name}（可能是AI幻觉）"
                            )
                    except Exception as error:
                        logger.warning(f"  ❌ 向导创建关系失败：{character.name} - {error}")
                        continue

        yield await tracker.saving("创建组织成员关系...", 0.9)
        members_created = 0
        for character, char_data in created_characters:
            if character.is_organization:
                continue

            org_memberships = char_data.get("organization_memberships", [])
            if org_memberships and isinstance(org_memberships, list):
                for membership in org_memberships:
                    try:
                        org_name = membership.get("organization_name")
                        if not org_name:
                            logger.debug(
                                f"  ⚠️  {character.name}的组织成员关系缺少organization_name，跳过"
                            )
                            continue

                        org = organization_name_to_obj.get(org_name)
                        if org:
                            existing_member = await db.execute(
                                select(OrganizationMember).where(
                                    OrganizationMember.organization_id == org.id,
                                    OrganizationMember.character_id == character.id,
                                )
                            )
                            if existing_member.scalar_one_or_none():
                                logger.debug(f"  ℹ️  成员关系已存在：{character.name} -> {org_name}")
                                continue

                            member = OrganizationMember(
                                organization_id=org.id,
                                character_id=character.id,
                                position=membership.get("position", "成员"),
                                rank=membership.get("rank", 0),
                                loyalty=membership.get("loyalty", 50),
                                joined_at=membership.get("joined_at"),
                                status=membership.get("status", "active"),
                                source="ai",
                            )
                            db.add(member)
                            org.member_count += 1
                            members_created += 1
                            logger.info(
                                f"  ✅ 向导添加成员：{character.name} -> {org_name} "
                                f"({membership.get('position')})"
                            )
                        else:
                            logger.debug(f"  ℹ️  组织引用已被清理：{character.name} -> {org_name}")
                    except Exception as error:
                        logger.warning(f"  ❌ 向导添加组织成员失败：{character.name} - {error}")
                        continue

        logger.info("📊 向导数据统计：")
        logger.info(f"  - 创建角色/组织：{len(created_characters)} 个")
        logger.info(f"  - 创建组织详情：{len(organization_name_to_obj)} 个")
        logger.info(f"  - 创建角色关系：{relationships_created} 条")
        logger.info(f"  - 创建组织成员：{members_created} 条")

        project.character_count = len(created_characters)
        project.wizard_step = 3
        logger.info(f"✅ 更新项目角色数量: {project.character_count}")

        await _save_project_research_assets(
            db=db,
            user_id=user_id,
            project_id=project_id,
            query=str(characters_research_bundle.get("query") or ""),
            archive_path=str(characters_research_bundle.get("archive_path") or ""),
            assets=characters_research_assets,
            memory_type=chapter_web_research_service.CHARACTERS_MEMORY_TYPE,
            title_prefix="角色外部资料",
        )

        await db.commit()
        db_committed = True
        created_characters = [char for char, _ in created_characters]

        yield await tracker.complete()
        yield await tracker.result(
            {
                "message": f"成功生成{len(created_characters)}个角色/组织（分{total_batches}批完成）",
                "count": len(created_characters),
                "batches": total_batches,
                "research_query": str(characters_research_bundle.get("query") or ""),
                "research_assets": characters_research_assets,
                "characters": [
                    {
                        "id": char.id,
                        "project_id": char.project_id,
                        "name": char.name,
                        "age": char.age,
                        "gender": char.gender,
                        "is_organization": char.is_organization,
                        "role_type": char.role_type,
                        "personality": char.personality,
                        "background": char.background,
                        "appearance": char.appearance,
                        "relationships": "",
                        "organization_type": char.organization_type,
                        "organization_purpose": char.organization_purpose,
                        "organization_members": "",
                        "traits": char.traits,
                        "created_at": char.created_at.isoformat() if char.created_at else None,
                        "updated_at": char.updated_at.isoformat() if char.updated_at else None,
                    }
                    for char in created_characters
                ],
            }
        )
        yield await tracker.done()
    except GeneratorExit:
        logger.warning("角色生成器被提前关闭")
        if not db_committed and db.in_transaction():
            await db.rollback()
            logger.info("角色生成事务已回滚（GeneratorExit）")
    except Exception as error:
        logger.error(f"角色生成失败: {error}")
        if not db_committed and db.in_transaction():
            await db.rollback()
            logger.info("角色生成事务已回滚（异常）")
        yield await tracker.error(f"生成失败: {error}")




