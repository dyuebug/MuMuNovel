"""Test-support prompt template facade for legacy Python prompt tests."""

from __future__ import annotations

from typing import Any, Callable, Dict, Optional

from tests.test_support.prompt_template_access_test_support import (
    get_template as _access_get_template,
    get_template_with_fallback as _access_get_template_with_fallback,
)
from tests.test_support.prompt_template_catalog_test_support import (
    build_system_template_catalog as _catalog_build_system_template_catalog,
    get_system_template_info as _catalog_get_system_template_info,
)
from tests.test_support.prompt_template_render_test_support import (
    format_prompt as _render_format_prompt,
    get_mcp_tool_test_prompts as _render_get_mcp_tool_test_prompts,
    prepare_template_content as _render_prepare_template_content,
)

TemplateLookupFn = Callable[[str], Optional[str]]
TemplateInfoFn = Callable[[str], dict]


def _build_quality_runtime_blocks(*args: Any, **kwargs: Any) -> Dict[str, str]:
    """Load the legacy story prompt source-map only when rendering needs it."""

    from tests.test_support.story_prompt_block_test_support import build_quality_runtime_blocks

    return build_quality_runtime_blocks(*args, **kwargs)


async def get_template_for_owner(
    template_key: str,
    user_id: str,
    db: Any,
    *,
    template_lookup: TemplateLookupFn,
) -> Optional[str]:
    """Convenience wrapper for callers that only need one concrete template owner."""

    return await get_template(
        template_key=template_key,
        user_id=user_id,
        db=db,
        template_lookup=template_lookup,
        get_system_template_info=lambda key: get_system_template_info(
            template_key=key,
            template_lookup=template_lookup,
        ),
    )


def format_prompt(template: str, **kwargs) -> str:
    """Render one template with the default prompt-template owner chain."""

    return _render_format_prompt(
        template,
        template_prepare=_render_prepare_template_content,
        build_quality_runtime_blocks=_build_quality_runtime_blocks,
        **kwargs,
    )


async def get_mcp_tool_test_prompts(
    *,
    plugin_name: str,
    user_id: str | None = None,
    db: Any = None,
    get_template: Callable[[str, str, Any], Any],
    user_template_default: str,
    system_template_default: str,
    format_prompt_fn: Callable[..., str] = format_prompt,
) -> Dict[str, str]:
    """Build MCP tool-test prompts with the default render/prepare wiring."""

    return await _render_get_mcp_tool_test_prompts(
        plugin_name=plugin_name,
        user_id=user_id,
        db=db,
        get_template=get_template,
        template_prepare=_render_prepare_template_content,
        format_prompt_fn=format_prompt_fn,
        user_template_default=user_template_default,
        system_template_default=system_template_default,
    )


async def get_template_with_fallback(
    *,
    template_key: str,
    user_id: str | None = None,
    db: Any = None,
    template_lookup: TemplateLookupFn,
    get_system_template_info: TemplateInfoFn,
) -> Optional[str]:
    """Load one template with fallback to the system default."""

    return await _access_get_template_with_fallback(
        template_key=template_key,
        user_id=user_id,
        db=db,
        template_lookup=template_lookup,
        template_prepare=_render_prepare_template_content,
        get_system_template_info=get_system_template_info,
    )


async def get_template(
    *,
    template_key: str,
    user_id: str,
    db: Any,
    template_lookup: TemplateLookupFn,
    get_system_template_info: TemplateInfoFn,
) -> Optional[str]:
    """Load one effective template for the given user/template pair."""

    return await _access_get_template(
        template_key=template_key,
        user_id=user_id,
        db=db,
        template_lookup=template_lookup,
        template_prepare=_render_prepare_template_content,
        get_system_template_info=get_system_template_info,
    )


def get_all_system_templates(*, template_lookup: TemplateLookupFn) -> list:
    """Build the current system template catalog for one template owner."""

    return _catalog_build_system_template_catalog(
        template_lookup=template_lookup,
        template_prepare=_render_prepare_template_content,
    )


def get_system_template_info(
    *,
    template_key: str,
    template_lookup: TemplateLookupFn,
) -> dict:
    """Return one system template info record from the current template owner."""

    return _catalog_get_system_template_info(
        template_key,
        get_all_system_templates(template_lookup=template_lookup),
    )
