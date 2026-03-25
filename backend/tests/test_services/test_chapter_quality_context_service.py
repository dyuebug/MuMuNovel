from app.models.project import Project
from app.services.chapter_quality_context_service import (
    StoryGenerationGuidance,
    StoryPacket,
    build_analysis_quality_kwargs,
    build_prompt_quality_kwargs,
    build_story_generation_packet,
    build_story_repair_diagnostic_context,
    resolve_story_generation_guidance,
)


def test_should_resolve_story_generation_guidance_from_project_defaults():
    project = Project(
        title="测试项目",
        user_id="user-1",
        default_creative_mode="hook",
        default_story_focus="advance_plot",
        default_plot_stage="development",
        default_story_creation_brief=" 强调代价与选择 ",
        default_quality_preset="plot_drive",
        default_quality_notes=" 减少说明句 ",
    )

    guidance = resolve_story_generation_guidance(project)

    assert guidance == StoryGenerationGuidance(
        creative_mode="hook",
        story_focus="advance_plot",
        plot_stage="development",
        story_creation_brief="强调代价与选择",
        quality_preset="plot_drive",
        quality_notes="减少说明句",
    )


def test_should_build_story_packet_from_mapping_and_project_defaults():
    project = Project(
        title="test-project",
        user_id="user-1",
        default_creative_mode="hook",
        default_story_focus="advance_plot",
        default_plot_stage="development",
        default_story_creation_brief=" emphasize cost choices ",
        default_quality_preset="plot_drive",
        default_quality_notes=" reduce exposition ",
    )

    packet = build_story_generation_packet(
        project,
        source={
            "creative_mode": " payoff ",
            "quality_notes": " trim explanation ",
            "story_focus": "   ",
        },
        source_label="outline-create-request",
    )

    assert isinstance(packet, StoryPacket)
    assert packet.source == "outline-create-request"
    assert packet.request_overrides == {
        "creative_mode": "payoff",
        "quality_notes": "trim explanation",
    }
    assert packet.guidance == StoryGenerationGuidance(
        creative_mode="payoff",
        story_focus="advance_plot",
        plot_stage="development",
        story_creation_brief="emphasize cost choices",
        quality_preset="plot_drive",
        quality_notes="trim explanation",
    )
    assert packet.to_generation_kwargs() == {
        "creative_mode": "payoff",
        "story_focus": "advance_plot",
        "plot_stage": "development",
        "story_creation_brief": "emphasize cost choices",
        "quality_preset": "plot_drive",
        "quality_notes": "trim explanation",
    }



def test_should_build_story_packet_from_request_like_object_and_reuse_prompt_kwargs():
    request_like = type(
        "RequestLike",
        (),
        {
            "creative_mode": " suspense ",
            "story_focus": "reveal_mystery",
            "plot_stage": " climax ",
            "story_creation_brief": " move the cost to the foreground ",
            "quality_preset": "tight_prose",
            "quality_notes": " trim explanation ",
        },
    )()

    packet = build_story_generation_packet(
        None,
        source=request_like,
        source_label="chapter-regenerate-request",
    )
    kwargs = packet.build_prompt_quality_kwargs({"genre": "mystery"})

    assert packet.source == "chapter-regenerate-request"
    assert packet.request_overrides == {
        "creative_mode": "suspense",
        "story_focus": "reveal_mystery",
        "plot_stage": "climax",
        "story_creation_brief": "move the cost to the foreground",
        "quality_preset": "tight_prose",
        "quality_notes": "trim explanation",
    }
    assert kwargs["genre"] == "mystery"
    assert kwargs["creative_mode"] == "suspense"
    assert kwargs["story_focus"] == "reveal_mystery"
    assert kwargs["plot_stage"] == "climax"
    assert kwargs["story_creation_brief"] == "move the cost to the foreground"
    assert kwargs["quality_preset"] == "tight_prose"
    assert kwargs["quality_notes"] == "trim explanation"



def test_should_build_prompt_quality_kwargs_with_guidance_fields():
    profile = {
        "genre": "悬疑",
        "style_name": "冷峻",
        "style_preset_id": "preset-1",
        "style_content": "保留紧绷语气",
        "external_assets": [{"title": "案件档案"}],
        "reference_assets": [{"title": "案件档案"}],
        "mcp_guard": "禁止暴露检索来源",
        "mcp_references": "案件档案摘要",
    }
    guidance = StoryGenerationGuidance(
        creative_mode="hook",
        story_focus="advance_plot",
        plot_stage="climax",
        story_creation_brief="突出代价和抉择",
        quality_preset="tight_prose",
        quality_notes="减少说明句",
    )

    kwargs = build_prompt_quality_kwargs(
        profile,
        guidance=guidance,
        story_repair_summary="补强冲突爆点",
        story_repair_targets=["冲突升级"],
        story_preserve_strengths=["人物张力"],
    )

    assert kwargs["genre"] == "悬疑"
    assert kwargs["style_name"] == "冷峻"
    assert kwargs["creative_mode"] == "hook"
    assert kwargs["story_focus"] == "advance_plot"
    assert kwargs["plot_stage"] == "climax"
    assert kwargs["story_creation_brief"] == "突出代价和抉择"
    assert kwargs["quality_preset"] == "tight_prose"
    assert kwargs["quality_notes"] == "减少说明句"
    assert kwargs["story_repair_targets"] == ["冲突升级"]
    assert kwargs["story_preserve_strengths"] == ["人物张力"]
    assert "突出代价和抉择" in kwargs["story_creation_brief_block"]
    assert "减少说明句" in kwargs["quality_preference_block"]


def test_should_build_analysis_quality_kwargs_with_guidance_fields():
    profile = {
        "genre": "奇幻",
        "style_name": "史诗",
        "style_preset_id": "preset-2",
        "style_content": "保留宏大感",
        "external_assets": [{"title": "世界观卡"}],
        "mcp_references": "世界观摘要",
    }
    guidance = StoryGenerationGuidance(
        creative_mode="payoff",
        story_focus="foreshadow_payoff",
        plot_stage="ending",
        story_creation_brief="优先兑现前文承诺",
        quality_preset="clean_prose",
        quality_notes="减少重复抒情",
    )

    kwargs = build_analysis_quality_kwargs(profile, guidance=guidance)

    assert kwargs["genre"] == "奇幻"
    assert kwargs["style_name"] == "史诗"
    assert kwargs["creative_mode"] == "payoff"
    assert kwargs["story_focus"] == "foreshadow_payoff"
    assert kwargs["plot_stage"] == "ending"
    assert kwargs["story_creation_brief"] == "优先兑现前文承诺"
    assert kwargs["quality_preset"] == "clean_prose"
    assert kwargs["quality_notes"] == "减少重复抒情"
    assert kwargs["mcp_references"] == "世界观摘要"





def test_should_build_prompt_quality_kwargs_with_story_repair_diagnostic_context():
    kwargs = build_prompt_quality_kwargs(
        {"genre": "玄幻"},
        guidance=StoryGenerationGuidance(
            creative_mode="hook",
            story_focus="advance_plot",
            plot_stage="climax",
            story_creation_brief="优先兑现前文埋下的回报点",
            quality_preset="tight_prose",
            quality_notes="减少解释性旁白",
        ),
        story_repair_summary="本章需要优先补强冲突升级与回报兑现。",
        story_repair_targets=["把主冲突推到不可回避的阶段"],
        story_preserve_strengths=["保留角色对峙时的压迫感"],
        active_story_repair_payload={
            "source": "manual_plus_current_chapter_quality",
            "summary": "当前章节的回报兑现不足，冲突升级也不够扎实。",
            "focus_areas": ["conflict", "payoff"],
            "weakest_metric_label": "回报兑现",
            "weakest_metric_value": 58.0,
        },
    )

    assert kwargs["story_repair_source"] == "manual_plus_current_chapter_quality"
    assert kwargs["story_repair_source_label"] == "手动要求 + 当前章节质量"
    assert kwargs["story_repair_focus_areas"] == ["冲突链推进", "回报兑现"]
    assert kwargs["story_repair_weakest_metric_label"] == "回报兑现"
    assert kwargs["story_repair_weakest_metric_value"] == 58.0
    assert "【诊断优先级卡】" in kwargs["story_repair_diagnostic_block"]
    assert "当前最弱项：回报兑现（当前值：58）" in kwargs["story_repair_diagnostic_block"]
    assert "优先修复维度：冲突链推进 / 回报兑现" in kwargs["story_repair_diagnostic_block"]


def test_should_build_outline_story_repair_diagnostic_context():
    diagnostic = build_story_repair_diagnostic_context(
        {
            "source": "recent_chapter_quality_summary",
            "source_label": "最近3章质量汇总",
            "summary": "最近章节优先修复「章尾牵引 / 大纲贴合」，先让推进、约束与结果真正落地，再做表面润色。",
            "focus_areas": ["cliffhanger", "outline"],
            "weakest_metric_label": "章尾牵引",
            "weakest_metric_value": 61.5,
            "quality_gate_label": "需人工介入",
            "quality_gate_decision": "manual_review",
            "quality_gate_summary": "最近章节质量风险较高，建议先人工介入或重写关键桥段，再继续后续生成。",
            "quality_gate_failed_metrics": ["章尾牵引", "大纲贴合"],
        },
        scene="outline",
    )

    assert diagnostic["story_repair_source_label"] == "最近3章质量汇总"
    assert diagnostic["story_repair_focus_areas"] == ["章尾牵引", "大纲贴合"]
    assert diagnostic["story_repair_quality_gate_label"] == "需人工介入"
    assert diagnostic["story_repair_quality_gate_failed_metrics"] == ["章尾牵引", "大纲贴合"]
    assert "质量门禁：需人工介入" in diagnostic["story_repair_diagnostic_block"]
    assert "门禁失败维度：章尾牵引 / 大纲贴合" in diagnostic["story_repair_diagnostic_block"]
    assert "当前最弱项：章尾牵引（当前值：61.5）" in diagnostic["story_repair_diagnostic_block"]
    assert "先把最弱项拆成每章的目标、阻力、回报与章尾牵引，再统一分配节拍。" in diagnostic["story_repair_diagnostic_block"]
