"""Prompt template render owner for marker prep, formatting, and MCP test prompts."""

from __future__ import annotations

import re
from typing import Any, Callable, Dict, Optional, Tuple

from tests.test_support.story_prompt_template_support_test_support import (
    compact_prompt_text as _story_compact_prompt_text,
)


QUALITY_TEMPLATE_MARKER_PATTERN = re.compile(
    r'^<prompt_template_key value="(?P<key>[A-Z0-9_]+)" />\n?',
    re.MULTILINE,
)


def prepare_template_content(template_key: Optional[str], template: Optional[str]) -> Optional[str]:
    """Ensure the template marker is prepended exactly once when a key is known."""

    if not template:
        return template
    marker = f'<prompt_template_key value="{template_key}" />\n' if template_key else ""
    prepared = template
    if marker and not prepared.startswith(marker):
        prepared = f"{marker}{prepared}"
    return prepared


def _append_prompt_block(template: str, block: str, *, after_tag: Optional[str] = None) -> str:
    cleaned_block = _story_compact_prompt_text(block)
    if not cleaned_block:
        return template
    if cleaned_block in template:
        return template
    if after_tag and after_tag in template:
        return template.replace(after_tag, f"{after_tag}\n\n{cleaned_block}", 1)
    return f"{template.rstrip()}\n\n{cleaned_block}".strip()


def extract_template_key_marker(template: str) -> Tuple[Optional[str], str]:
    """Return the embedded template key marker and stripped template body."""

    if not template:
        return None, template
    match = QUALITY_TEMPLATE_MARKER_PATTERN.match(template)
    if not match:
        return None, template
    return match.group("key"), template[match.end():]


def inject_quality_contract(
    *,
    template: str,
    template_key: Optional[str],
    build_quality_runtime_blocks: Callable[..., Dict[str, str]],
    **kwargs,
) -> str:
    """Inject runtime quality blocks into one rendered prompt."""

    blocks = build_quality_runtime_blocks(template_key, **kwargs)
    injected = _append_prompt_block(
        template,
        blocks.get("quality_contract_block"),
        after_tag="</fusion_contract>",
    )
    injected = _append_prompt_block(
        injected,
        blocks.get("quality_mcp_references_block"),
        after_tag="</fusion_contract>",
    )
    return injected


def format_prompt(
    template: str,
    *,
    template_prepare: Callable[[Optional[str], Optional[str]], Optional[str]],
    build_quality_runtime_blocks: Callable[..., Dict[str, str]],
    **kwargs,
) -> str:
    """Render one prompt template and inject the runtime quality contract."""

    template_key = kwargs.pop("_template_key", None)
    try:
        extracted_template_key, prepared_template = extract_template_key_marker(template)
        if not template_key:
            template_key = extracted_template_key
        prepared_template = template_prepare(template_key, prepared_template)
        rendered = prepared_template.format(**kwargs)
        return inject_quality_contract(
            template=rendered,
            template_key=template_key,
            build_quality_runtime_blocks=build_quality_runtime_blocks,
            **kwargs,
        )
    except KeyError as exc:
        raise ValueError(f"缺少必需的参数: {exc}") from exc


async def get_mcp_tool_test_prompts(
    *,
    plugin_name: str,
    user_id: Optional[str],
    db: Any,
    get_template: Callable[[str, str, Any], Any],
    template_prepare: Callable[[Optional[str], Optional[str]], Optional[str]],
    format_prompt_fn: Callable[..., str],
    user_template_default: str,
    system_template_default: str,
) -> Dict[str, str]:
    """Build MCP tool-test prompt bundle with optional user template overrides."""

    if user_id and db:
        user_template = await get_template("MCP_TOOL_TEST", user_id, db)
    else:
        user_template = template_prepare("MCP_TOOL_TEST", user_template_default)

    if user_id and db:
        system_template = await get_template("MCP_TOOL_TEST_SYSTEM", user_id, db)
    else:
        system_template = template_prepare("MCP_TOOL_TEST_SYSTEM", system_template_default)

    return {
        "user": format_prompt_fn(
            user_template,
            plugin_name=plugin_name,
            _template_key="MCP_TOOL_TEST",
        ),
        "system": system_template,
    }
