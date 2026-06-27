import pytest

from tests.test_support.story_packet_test_support import StoryGenerationGuidance
from tests.test_support.story_packet_test_support import (
    build_analysis_quality_kwargs,
)
from tests.test_support.prompt_service_test_support import format_prompt


@pytest.mark.parametrize(
    "template_key",
    ["PLOT_ANALYSIS", "CHAPTER_TEXT_CHECKER", "CHAPTER_TEXT_REVISER"],
)
def test_should_align_analysis_checker_and_reviser_with_story_guidance(template_key: str):
    guidance = StoryGenerationGuidance(
        creative_mode="hook",
        story_focus="advance_plot",
        plot_stage="climax",
        story_creation_brief="突出代价和抉择",
        quality_preset="tight_prose",
        quality_notes="减少说明句",
    )
    kwargs = build_analysis_quality_kwargs(
        {
            "genre": "悬疑",
            "style_name": "冷峻",
            "style_preset_id": "preset-1",
            "style_content": "保留锋利节奏",
            "external_assets": [{"title": "案件档案"}],
            "mcp_references": "案件档案摘要",
        },
        guidance=guidance,
    )

    prompt = format_prompt(
        "请输出结果。",
        _template_key=template_key,
        **kwargs,
    )

    assert "突出代价和抉择" in prompt
    assert "减少说明句" in prompt
    assert "案件档案摘要" in prompt
