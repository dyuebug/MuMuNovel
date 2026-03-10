from app.api.outlines import _merge_outline_requirements
from app.services.prompt_service import PromptService


def test_should_inject_creative_mode_block_into_chapter_quality_contract():
    blocks = PromptService._build_quality_runtime_blocks(
        "CHAPTER_GENERATION_ONE_TO_ONE",
        creative_mode="hook",
        story_focus="advance_plot",
    )

    assert "钩子优先" in blocks["creative_mode_block"]
    assert "钩子优先" in blocks["quality_contract_block"]
    assert "主线推进" in blocks["story_focus_block"]
    assert "主线推进" in blocks["quality_contract_block"]


def test_should_merge_outline_requirements_with_creative_mode_block():
    merged = _merge_outline_requirements("保留双线叙事", "suspense", "relationship_shift")

    assert "保留双线叙事" in merged
    assert "【创作模式】当前采用“悬念拉满”" in merged
    assert "【结构侧重点】当前优先“关系转折”" in merged
