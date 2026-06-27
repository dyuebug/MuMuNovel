from types import SimpleNamespace

from tests.test_support.chapter_generation_runtime_prompt_test_support import (
    build_chapter_runtime_system_prompt,
    resolve_generation_temperature,
)
from tests.test_support.schemas.novel_quality_rules import detect_style_profile


def test_should_detect_low_ai_serial_style_profile():
    assert detect_style_profile("低AI连载感", None, None) == "low_ai_serial"
    assert detect_style_profile(None, "low_ai_serial", None) == "low_ai_serial"
    assert detect_style_profile(None, None, "需要连载感") == "low_ai_serial"


def test_should_resolve_generation_temperature_by_style_profile():
    assert resolve_generation_temperature("low_ai_serial") == 0.82
    assert resolve_generation_temperature("low_ai_life") == 0.78
    assert resolve_generation_temperature("default") == 0.72


def test_should_build_chapter_runtime_system_prompt_with_runtime_contract():
    project = SimpleNamespace(
        world_time_period="近未来",
        world_location="海边城市",
        world_atmosphere="冷峻",
        world_rules="组织封锁港口",
    )

    prompt = build_chapter_runtime_system_prompt(
        project=project,
        style_content="低AI连载感",
        chapter_outline="1. 角色抵达港口\n2. 组织下达封锁命令",
        previous_summary="上一章角色刚刚获得线索",
        target_word_count=1200,
        story_runtime_contract={
            "blueprint": {
                "organization_state_ledger": ["巡查队控制码头", "商会切断补给"],
            },
        },
        web_research_grounding_block="【资料锚点】潮汐会改变登陆窗口\n\n",
    )

    assert "低AI连载感" in prompt
    assert "组织封锁港口" in prompt
    assert "巡查队控制码头" in prompt
    assert "商会切断补给" in prompt
    assert "潮汐会改变登陆窗口" in prompt
    assert "目标约1200字" in prompt
