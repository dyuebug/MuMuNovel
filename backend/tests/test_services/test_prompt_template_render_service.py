import pytest
import sys

from tests.test_support.prompt_service_test_support import (
    MCP_TOOL_TEST,
    MCP_TOOL_TEST_SYSTEM,
    get_template,
)
from tests.test_support.prompt_template_facade_test_support import (
    format_prompt as facade_format_prompt,
    get_mcp_tool_test_prompts as facade_get_mcp_tool_test_prompts,
)
from tests.test_support.story_prompt_block_test_support import build_quality_runtime_blocks
from tests.test_support.prompt_template_render_test_support import (
    format_prompt,
    inject_quality_contract,
    prepare_template_content,
)


pytestmark = pytest.mark.asyncio


def test_should_prepare_template_marker_once():
    prepared = prepare_template_content("MCP_TOOL_TEST", "正文")

    assert prepared.startswith('<prompt_template_key value="MCP_TOOL_TEST" />')
    assert prepare_template_content("MCP_TOOL_TEST", prepared) == prepared


def test_facade_test_support_import_should_not_eagerly_load_story_prompt_source_map():
    sys.modules.pop("app.services.prompt_template_facade_service", None)
    sys.modules.pop("tests.test_support.prompt_template_facade_test_support", None)
    sys.modules.pop("app.services.story_prompt_block_service", None)
    sys.modules.pop("tests.test_support.story_prompt_block_test_support", None)

    __import__("tests.test_support.prompt_template_facade_test_support")

    assert "tests.test_support.prompt_template_facade_test_support" in sys.modules
    assert "app.services.prompt_template_facade_service" not in sys.modules
    assert "app.services.story_prompt_block_service" not in sys.modules
    assert "tests.test_support.story_prompt_block_test_support" not in sys.modules


def test_prompt_service_test_support_import_should_not_eagerly_load_story_prompt_source_map():
    sys.modules.pop("app.services.prompt_service", None)
    sys.modules.pop("tests.test_support.prompt_service_test_support", None)
    sys.modules.pop("app.services.story_prompt_block_service", None)
    sys.modules.pop("tests.test_support.story_prompt_block_test_support", None)

    __import__("tests.test_support.prompt_service_test_support")

    assert "tests.test_support.prompt_service_test_support" in sys.modules
    assert "app.services.prompt_service" not in sys.modules
    assert "app.services.story_prompt_block_service" not in sys.modules
    assert "tests.test_support.story_prompt_block_test_support" not in sys.modules


def test_prompt_service_should_lazy_export_story_prompt_constants():
    sys.modules.pop("app.services.prompt_service", None)
    sys.modules.pop("tests.test_support.prompt_service_test_support", None)
    sys.modules.pop("app.services.story_prompt_block_service", None)
    sys.modules.pop("tests.test_support.story_prompt_block_test_support", None)

    from tests.test_support.prompt_service_test_support import CREATIVE_MODE_SPECS

    assert CREATIVE_MODE_SPECS["balanced"]["label"] == "均衡推进"
    assert "app.services.story_prompt_block_service" not in sys.modules
    assert "tests.test_support.story_prompt_block_test_support" in sys.modules


def test_should_render_prompt_and_inject_quality_contract():
    prompt = format_prompt(
        "请输出结果。",
        template_prepare=prepare_template_content,
        build_quality_runtime_blocks=build_quality_runtime_blocks,
        _template_key="PLOT_ANALYSIS",
        genre="悬疑",
        style_name="冷峻",
        style_preset_id="preset-1",
        style_content="保留锋利节奏",
        story_creation_brief="突出代价和抉择",
        quality_notes="减少说明句",
        external_assets=[{"title": "案件档案"}],
        mcp_references="案件档案摘要",
    )

    assert "突出代价和抉择" in prompt
    assert "减少说明句" in prompt
    assert "案件档案摘要" in prompt


async def test_should_build_mcp_tool_test_prompts_with_defaults():
    prompts = await facade_get_mcp_tool_test_prompts(
        plugin_name="demo-plugin",
        user_id=None,
        db=None,
        get_template=get_template,
        format_prompt_fn=facade_format_prompt,
        user_template_default=MCP_TOOL_TEST,
        system_template_default=MCP_TOOL_TEST_SYSTEM,
    )

    assert "demo-plugin" in prompts["user"]
    assert prompts["system"].startswith('<prompt_template_key value="MCP_TOOL_TEST_SYSTEM" />')
