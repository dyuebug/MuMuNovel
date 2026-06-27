"""AI dependency helper test support for retired Python API routes."""
from __future__ import annotations

import json
from typing import Any, Dict, Optional

from fastapi import Depends, Request
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from tests.test_support.retired_runtime_test_support import settings as app_settings
from tests.test_support.database_test_support import get_db
from tests.test_support.retired_runtime_test_support import get_logger
from migrator_app.models import MCPPlugin, Settings, User
from tests.test_support.ai_gateway.ai_service import AIService, create_user_ai_service_with_mcp
from tests.test_support.api_common_test_support import require_request_user

logger = get_logger(__name__)

PLACEHOLDER_API_KEYS = {
    "your_openai_api_key_here",
    "your_anthropic_api_key_here",
    "your_gemini_api_key_here",
    "your_api_key_here",
}


def normalize_env_api_key(api_key: Optional[str]) -> str:
    """Treat example API keys as empty values."""
    if not api_key:
        return ""

    normalized = api_key.strip()
    if normalized.lower() in PLACEHOLDER_API_KEYS:
        return ""

    return normalized


def read_env_defaults() -> Dict[str, Any]:
    """Read default AI settings from environment-backed app config."""
    return {
        "api_provider": app_settings.default_ai_provider,
        "api_key": (
            normalize_env_api_key(app_settings.openai_api_key)
            or normalize_env_api_key(app_settings.anthropic_api_key)
            or normalize_env_api_key(app_settings.gemini_api_key)
            or ""
        ),
        "api_base_url": app_settings.openai_base_url or app_settings.anthropic_base_url or "",
        "llm_model": app_settings.default_model,
        "temperature": app_settings.default_temperature,
        "max_tokens": app_settings.default_max_tokens,
    }


async def load_or_create_user_settings(
    db_session: AsyncSession,
    user_id: str,
    defaults: Dict[str, Any],
) -> tuple[Settings, bool]:
    """Load persisted settings or create the first row from environment defaults."""
    result = await db_session.execute(select(Settings).where(Settings.user_id == user_id))
    settings = result.scalar_one_or_none()
    if settings is not None:
        return settings, False

    settings = Settings(user_id=user_id, **defaults)
    db_session.add(settings)
    await db_session.commit()
    await db_session.refresh(settings)
    return settings, True


async def list_user_mcp_plugins(
    db_session: AsyncSession,
    user_id: str,
) -> list[MCPPlugin]:
    """Load all MCP plugins for a user."""
    result = await db_session.execute(select(MCPPlugin).where(MCPPlugin.user_id == user_id))
    return list(result.scalars().all())


def require_login(request: Request) -> User:
    """依赖：要求用户已登录。"""
    return require_request_user(request, "需要登录")


async def get_user_ai_service(
    user: User = Depends(require_login),
    db: AsyncSession = Depends(get_db),
) -> AIService:
    """
    依赖：获取当前用户的 AI 服务实例（支持 MCP 工具自动加载）。

    保留给历史 Python route adapter 测试使用；生产 API 路径由 Rust 接管。
    """
    settings, created = await load_or_create_user_settings(
        db,
        user.user_id,
        read_env_defaults(),
    )
    if created:
        logger.info("用户 %s 首次使用AI服务，已从.env同步设置到数据库", user.user_id)

    mcp_plugins = await list_user_mcp_plugins(db, user.user_id)
    enable_mcp = any(plugin.enabled for plugin in mcp_plugins) if mcp_plugins else False

    if mcp_plugins:
        enabled_count = sum(1 for plugin in mcp_plugins if plugin.enabled)
        logger.info(
            "用户 %s 有 %s 个MCP插件，%s 个启用，%s 决定使用MCP",
            user.user_id,
            len(mcp_plugins),
            enabled_count,
            enable_mcp,
        )
    else:
        logger.debug("用户 %s 没有配置MCP插件，禁用MCP", user.user_id)

    backup_urls = None
    if settings.api_backup_urls:
        try:
            if isinstance(settings.api_backup_urls, str):
                backup_urls = json.loads(settings.api_backup_urls)
            else:
                backup_urls = settings.api_backup_urls
        except (json.JSONDecodeError, TypeError):
            logger.warning("用户 %s 的 api_backup_urls 解析失败，忽略备用地址", user.user_id)

    return create_user_ai_service_with_mcp(
        api_provider=settings.api_provider,
        api_key=settings.api_key,
        api_base_url=settings.api_base_url or "",
        model_name=settings.llm_model,
        temperature=settings.temperature,
        max_tokens=settings.max_tokens,
        user_id=user.user_id,
        db_session=db,
        system_prompt=settings.system_prompt,
        enable_mcp=enable_mcp,
        backup_urls=backup_urls,
        fallback_strategy=settings.fallback_strategy,
    )



