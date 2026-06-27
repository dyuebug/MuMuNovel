"""Prompt template access owner for user override lookup and managed sync."""

from __future__ import annotations

from typing import Any, Callable, Optional

from sqlalchemy import select

from tests.test_support.retired_runtime_test_support import get_logger
from migrator_app.models import PromptTemplate
from tests.test_support.prompt_template_sync_test_support import sync_managed_template_if_legacy


logger = get_logger(__name__)


TemplatePrepareFn = Callable[[Optional[str], Optional[str]], Optional[str]]
TemplateInfoFn = Callable[[str], dict]


async def get_template_with_fallback(
    *,
    template_key: str,
    user_id: Optional[str],
    db: Any,
    template_lookup: Callable[[str], Optional[str]],
    template_prepare: TemplatePrepareFn,
    get_system_template_info: TemplateInfoFn,
) -> Optional[str]:
    """Load template content, preferring user override and falling back to system default."""

    if not user_id or not db:
        return template_lookup(template_key)

    return await get_template(
        template_key=template_key,
        user_id=user_id,
        db=db,
        template_lookup=template_lookup,
        template_prepare=template_prepare,
        get_system_template_info=get_system_template_info,
    )


async def get_template(
    *,
    template_key: str,
    user_id: str,
    db: Any,
    template_lookup: Callable[[str], Optional[str]],
    template_prepare: TemplatePrepareFn,
    get_system_template_info: TemplateInfoFn,
) -> Optional[str]:
    """Load the effective template content for one user/template key pair."""

    template_content = template_lookup(template_key)
    template_content = template_prepare(template_key, template_content)
    template_info = get_system_template_info(template_key)

    try:
        await sync_managed_template_if_legacy(
            db=db,
            user_id=user_id,
            template_key=template_key,
            system_template_content=template_content,
            system_template_info=template_info,
        )
    except Exception as sync_error:
        logger.warning(
            "Managed template sync failed, fallback to normal flow: user_id=%s, template_key=%s, error=%s",
            user_id,
            template_key,
            sync_error,
        )

    result = await db.execute(
        select(PromptTemplate).where(
            PromptTemplate.user_id == user_id,
            PromptTemplate.template_key == template_key,
            PromptTemplate.is_active == True,
        )
    )
    custom_template = result.scalar_one_or_none()

    if custom_template:
        logger.info(
            "✅ 使用用户自定义提示词: user_id=%s, template_key=%s, template_name=%s",
            user_id,
            template_key,
            custom_template.template_name,
        )
        return template_prepare(template_key, custom_template.template_content)

    logger.info(
        "⚪ 使用系统默认提示词: user_id=%s, template_key=%s (未找到自定义模板)",
        user_id,
        template_key,
    )
    if template_content is None:
        logger.warning("⚠️ 未找到系统默认模板: %s", template_key)

    return template_content



