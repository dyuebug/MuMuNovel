import pytest
from types import SimpleNamespace

from app.schemas.regeneration import ChapterRegenerateRequest
from app.services.chapter_regenerator import ChapterRegenerator
from app.services.prompt_service import PromptService


def test_should_merge_story_repair_guidance_and_auto_focus_areas():
    regenerator = ChapterRegenerator(ai_service=None)
    analysis = SimpleNamespace(
        suggestions=[
            "???????????????",
            "??????????????",
        ],
        conflict_level=4,
        pacing_score=5.8,
        coherence_score=5.9,
        dialogue_ratio=0.1,
    )
    request = ChapterRegenerateRequest(
        modification_source="mixed",
        selected_suggestion_indices=[0, 1],
        custom_instructions="???????????????",
        target_word_count=2200,
        focus_areas=[],
        story_repair_summary="????????????",
        story_repair_targets=["????????????", "??????????"],
        story_preserve_strengths=["??????????"],
    )

    instructions = regenerator._build_modification_instructions(analysis, request)

    assert "????????" in instructions
    assert "???????????" in instructions
    assert "??????????" in instructions
    assert "????" in instructions
    assert "????" in instructions
    assert "????" in instructions


@pytest.mark.asyncio
async def test_should_inject_story_generation_guidance_into_regeneration_prompt():
    prompt = await PromptService.get_chapter_regeneration_prompt(
        chapter_number=7,
        title="试炼夜",
        word_count=2800,
        content="原始章节内容",
        modification_instructions="请补强冲突推进与章尾牵引。",
        project_context={
            "project_title": "测试项目",
            "genre": "悬疑",
            "theme": "代价与真相",
            "narrative_perspective": "第三人称",
            "time_period": "近未来",
            "location": "海港城",
            "atmosphere": "压抑",
            "characters_info": "主角、反派、线人",
            "chapter_outline": "调查失控，主角被迫提前摊牌。",
            "prompt_quality_kwargs": {
                "genre": "悬疑",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
                "plot_stage": "climax",
                "story_creation_brief": "让关键选择立刻带来代价和反馈。",
                "quality_preset": "plot_drive",
                "quality_notes": "减少解释句；强化章尾牵引",
                "quality_preference_block": "【质量预设】当前采用“剧情推进优先”\n- 补充偏好：\n  - 减少解释句\n  - 强化章尾牵引",
                "story_creation_brief_block": "【本轮创作总控】\n- 执行摘要：让关键选择立刻带来代价和反馈。",
                "creative_mode_block": "【创作模式】当前采用“钩子优先”",
                "story_focus_block": "【结构侧重点】当前优先“推进主线”",
                "narrative_blueprint_block": "【章节叙事蓝图】优先推进冲突升级与信息揭示。",
                "story_repair_target_block": "【修复目标卡】\n- 当前问题：冲突爆点不足",
                "story_repair_diagnostic_block": "【诊断优先级卡】\n- 当前最弱项：章尾牵引（当前值：61）",
                "style_name": "冷峻",
                "style_preset_id": "preset-1",
                "style_content": "短句、压迫感、少解释",
                "external_assets": [],
                "reference_assets": [],
                "mcp_guard": "禁止暴露检索来源",
                "mcp_references": "案件档案摘要",
            },
        },
        style_content="短句、压迫感、少解释",
        target_word_count=3200,
    )

    assert "让关键选择立刻带来代价和反馈。" in prompt
    assert "强化章尾牵引" in prompt
    assert "当前最弱项：章尾牵引（当前值：61）" in prompt