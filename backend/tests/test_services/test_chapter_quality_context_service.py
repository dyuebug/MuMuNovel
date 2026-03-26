from app.models.project import Project
from app.services.story_repair_payload_service import StoryRepairPayload
from app.services.chapter_quality_context_service import (
    StoryGenerationGuidance,
    StoryPacket,
    build_analysis_quality_kwargs,
    build_chapter_generation_intent,
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



def test_should_round_trip_story_packet_runtime_contract():
    packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="hook",
            story_focus="advance_plot",
            plot_stage="development",
            story_creation_brief="keep the vault threat visible",
            quality_preset="tight_prose",
            quality_notes="trim explanation",
        ),
        request_overrides={
            "creative_mode": "hook",
            "quality_notes": "trim explanation",
            "story_focus": " ",
        },
        source="chapter-generate-request",
    ).with_blueprint(
        long_term_goal="Protect the hidden key before the guild closes in.",
        chapter_count=12,
        current_chapter_number=5,
        target_word_count=2600,
        character_focus_source=["Lin", "Bo"],
        foreshadow_payoff_source=["recover the hidden key"],
        character_state_source=["Lin: trust strained after the failed ambush"],
        relationship_state_source=["Lin/Bo: alliance tested by secrecy"],
        foreshadow_state_source=["hidden key: still missing after the archive raid"],
        organization_state_source=["ShadowGuild: control tightened around the docks"],
        career_state_source=["Lin/Strategist: stage 3 with supply-chain pressure"],
    )

    contract = packet.to_runtime_contract()
    restored = StoryPacket.from_runtime_contract(contract)

    assert contract["version"] == 1
    assert restored == packet



def test_should_build_story_runtime_contract_from_intent_snapshot():
    packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="hook",
            story_focus="advance_plot",
            plot_stage="development",
            story_creation_brief="keep the pressure visible",
        ),
        source="chapter-generate-request",
    )
    project = Project(title="runtime-contract", user_id="user-1", chapter_count=16)
    chapter = type("ChapterStub", (), {"chapter_number": 4})()

    intent = build_chapter_generation_intent(
        story_packet=packet,
        quality_profile={"genre": "mystery"},
        project=project,
        chapter=chapter,
        chapter_context=None,
        target_word_count=2800,
        character_focus_source=["Lin"],
        foreshadow_payoff_source=["recover the hidden key"],
        relationship_state_source=["Lin/Bo: alliance tested by secrecy"],
        organization_state_source=["ShadowGuild: control tightened around the docks"],
        career_state_source=["Lin/Strategist: stage 3 with supply-chain pressure"],
    )

    contract = intent.build_story_runtime_contract()
    restored = StoryPacket.from_runtime_contract(contract)

    assert restored.guidance.creative_mode == "hook"
    assert restored.blueprint.chapter_count == 16
    assert restored.blueprint.current_chapter_number == 4
    assert restored.blueprint.target_word_count == 2800
    assert restored.blueprint.character_focus_names == ("Lin",)
    assert restored.blueprint.foreshadow_payoff_plan == ("recover the hidden key",)
    assert restored.blueprint.relationship_state_ledger == ("Lin/Bo: alliance tested by secrecy",)
    assert restored.blueprint.organization_state_ledger == ("ShadowGuild: control tightened around the docks",)
    assert restored.blueprint.career_state_ledger == ("Lin/Strategist: stage 3 with supply-chain pressure",)



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


def test_should_build_story_packet_from_legacy_guidance_and_preserve_analysis_contract():
    guidance = StoryGenerationGuidance(
        creative_mode="hook",
        story_focus="advance_plot",
        plot_stage="development",
        story_creation_brief="强调冲突推进",
        quality_preset="tight_prose",
        quality_notes="减少解释性旁白",
    )

    packet = StoryPacket.from_guidance(
        guidance,
        request_overrides={
            "creative_mode": " hook ",
            "quality_notes": " 减少解释性旁白 ",
            "story_focus": "   ",
        },
        source="legacy-analysis-guidance",
    )

    kwargs = packet.build_analysis_quality_kwargs({"genre": "悬疑"})

    assert packet.source == "legacy-analysis-guidance"
    assert packet.guidance == guidance
    assert packet.request_overrides == {
        "creative_mode": "hook",
        "quality_notes": "减少解释性旁白",
    }
    assert kwargs["genre"] == "悬疑"
    assert kwargs["creative_mode"] == "hook"
    assert kwargs["story_focus"] == "advance_plot"
    assert kwargs["quality_notes"] == "减少解释性旁白"



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



def test_should_build_story_packet_blueprint_from_project_and_source():
    project = Project(
        title="test-project",
        user_id="user-1",
        theme="Power and cost",
        description="A young lead is dragged into the capital struggle.",
        chapter_count=12,
        target_words=240000,
        default_story_creation_brief="keep the pressure visible",
    )

    packet = build_story_generation_packet(
        project,
        source={
            "character_focus": ["Lin", "Su"],
            "foreshadow_payoff_plan": ["recover the hidden key", "pay off the banquet ambush"],
        },
        source_label="chapter-generate-request",
    )

    assert "Power and cost" in (packet.blueprint.long_term_goal or "")
    assert packet.blueprint.chapter_count == 12
    assert packet.blueprint.target_word_count == 240000
    assert packet.blueprint.character_focus_names == ("Lin", "Su")
    assert packet.blueprint.foreshadow_payoff_plan == (
        "recover the hidden key",
        "pay off the banquet ambush",
    )


def test_should_build_prompt_quality_kwargs_with_story_blueprint_runtime_blocks():
    packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="hook",
            story_focus="advance_plot",
            plot_stage="climax",
            story_creation_brief="keep the pressure visible",
        ),
        source="chapter-generate-request",
    ).with_blueprint(
        long_term_goal="The lead must seize the capital before the enemy closes in.",
        chapter_count=12,
        current_chapter_number=5,
        target_word_count=2600,
        character_focus_source=["Lin", "Su"],
        foreshadow_payoff_source=["recover the hidden key", "pay off the banquet ambush"],
        character_state_source={
            "story_character_state_ledger": ["Lin: distrust remains visible"],
        },
        relationship_state_source={
            "story_relationship_state_ledger": ["Lin/Su: uneasy alliance under tension"],
        },
        foreshadow_state_source={
            "story_foreshadow_state_ledger": ["hidden key: still missing from the archive"],
        },
        organization_state_source={
            "story_organization_state_ledger": ["Guild: control tightened around the docks"],
        },
        career_state_source={
            "story_career_state_ledger": ["Lin/Strategist: stalled at stage 3"],
        },
    )

    kwargs = packet.build_prompt_quality_kwargs({"genre": "mystery"})

    assert kwargs["story_long_term_goal"] == "The lead must seize the capital before the enemy closes in."
    assert kwargs["story_character_focus"] == ["Lin", "Su"]
    assert kwargs["story_foreshadow_payoff_plan"] == [
        "recover the hidden key",
        "pay off the banquet ambush",
    ]
    assert kwargs["story_character_state_ledger"] == ["Lin: distrust remains visible"]
    assert kwargs["story_relationship_state_ledger"] == ["Lin/Su: uneasy alliance under tension"]
    assert kwargs["story_foreshadow_state_ledger"] == ["hidden key: still missing from the archive"]
    assert kwargs["story_organization_state_ledger"] == ["Guild: control tightened around the docks"]
    assert kwargs["story_career_state_ledger"] == ["Lin/Strategist: stalled at stage 3"]
    assert "The lead must seize the capital before the enemy closes in." in kwargs["story_long_term_goal_block"]
    assert "Lin" in kwargs["story_character_focus_anchor_block"]
    assert "recover the hidden key" in kwargs["story_foreshadow_payoff_plan_block"]
    assert "Lin: distrust remains visible" in kwargs["story_character_state_ledger_block"]
    assert "Lin/Su: uneasy alliance under tension" in kwargs["story_relationship_state_ledger_block"]
    assert "hidden key: still missing from the archive" in kwargs["story_foreshadow_state_ledger_block"]
    assert "Guild: control tightened around the docks" in kwargs["story_organization_state_ledger_block"]
    assert "Lin/Strategist: stalled at stage 3" in kwargs["story_career_state_ledger_block"]
    assert "2600" in kwargs["story_pacing_budget_block"]


def test_should_build_quality_runtime_context_with_story_ledgers():
    packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="hook",
            story_focus="advance_plot",
            plot_stage="development",
            story_creation_brief="keep the pressure visible",
        ),
        source="chapter-generate-request",
    ).with_blueprint(
        long_term_goal="Keep pushing toward the capital.",
        chapter_count=12,
        current_chapter_number=6,
        target_word_count=2800,
        character_focus_source=["Lin", "Su"],
        foreshadow_payoff_source=["recover the hidden key"],
        character_state_source={
            "story_character_state_ledger": ["Lin: distrust remains visible"],
        },
        relationship_state_source={
            "story_relationship_state_ledger": ["Lin/Su: uneasy alliance under tension"],
        },
        foreshadow_state_source={
            "story_foreshadow_state_ledger": ["hidden key: still missing from the archive"],
        },
        organization_state_source={
            "story_organization_state_ledger": ["Guild: control tightened around the docks"],
        },
        career_state_source={
            "story_career_state_ledger": ["Lin/Strategist: stalled at stage 3"],
        },
    )

    runtime_context = packet.build_quality_runtime_context(
        chapter_count=12,
        current_chapter_number=6,
        target_word_count=2800,
        character_focus_source=["Lin", "Su"],
        foreshadow_payoff_source=["recover the hidden key"],
        character_state_source={
            "story_character_state_ledger": ["Lin: distrust remains visible"],
        },
        relationship_state_source={
            "story_relationship_state_ledger": ["Lin/Su: uneasy alliance under tension"],
        },
        foreshadow_state_source={
            "story_foreshadow_state_ledger": ["hidden key: still missing from the archive"],
        },
        organization_state_source={
            "story_organization_state_ledger": ["Guild: control tightened around the docks"],
        },
        career_state_source={
            "story_career_state_ledger": ["Lin/Strategist: stalled at stage 3"],
        },
    )

    assert runtime_context["plot_stage"] == "development"
    assert runtime_context["chapter_count"] == 12
    assert runtime_context["current_chapter_number"] == 6
    assert runtime_context["target_word_count"] == 2800
    assert runtime_context["character_focus"] == ["Lin", "Su"]
    assert runtime_context["foreshadow_payoff_plan"] == ["recover the hidden key"]
    assert runtime_context["character_state_ledger"] == ["Lin: distrust remains visible"]
    assert runtime_context["relationship_state_ledger"] == ["Lin/Su: uneasy alliance under tension"]
    assert runtime_context["foreshadow_state_ledger"] == ["hidden key: still missing from the archive"]
    assert runtime_context["organization_state_ledger"] == ["Guild: control tightened around the docks"]
    assert runtime_context["career_state_ledger"] == ["Lin/Strategist: stalled at stage 3"]



def test_should_build_prompt_quality_kwargs_from_story_repair_payload_object():
    packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="hook",
            story_focus="advance_plot",
            plot_stage="development",
            story_creation_brief="keep pressure visible",
        ),
        source="chapter-generate-request",
    )
    payload = StoryRepairPayload(
        summary="优先补强冲突折返与兑现节奏",
        targets=("升级代价", "兑现伏笔"),
        strengths=("保留对白辨识度",),
    )

    kwargs = packet.build_prompt_quality_kwargs(
        {"genre": "mystery"},
        story_repair_payload=payload,
    )

    assert kwargs["story_repair_summary"] == "优先补强冲突折返与兑现节奏"
    assert kwargs["story_repair_targets"] == ["升级代价", "兑现伏笔"]
    assert kwargs["story_preserve_strengths"] == ["保留对白辨识度"]
    assert "优先补强冲突折返与兑现节奏" in kwargs["story_repair_target_block"]
    assert "升级代价" in kwargs["story_repair_target_block"]
    assert "保留对白辨识度" in kwargs["story_repair_target_block"]



def test_should_build_chapter_generation_intent_with_quality_history_context_fallback():
    packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="hook",
            story_focus="advance_plot",
            plot_stage="development",
            story_creation_brief="keep pressure visible",
        ),
        source="chapter-generate-request",
    )
    project = Project(title="history-fallback", user_id="user-1", chapter_count=12)
    chapter = type("ChapterStub", (), {"chapter_number": 7})()

    intent = build_chapter_generation_intent(
        story_packet=packet,
        quality_profile={"genre": "mystery"},
        project=project,
        chapter=chapter,
        chapter_context=None,
        target_word_count=2600,
        quality_history_context={
            "foreshadow_payoff_plan": ["recover the hidden key"],
            "character_state_ledger": ["Lin: injured hand still limits movement"],
            "relationship_state_ledger": ["Lin/Su: uneasy alliance under tension"],
            "foreshadow_state_ledger": ["hidden key: still missing after the archive raid"],
            "organization_state_ledger": ["ShadowGuild: control tightened around the docks"],
            "career_state_ledger": ["Lin/Strategist: stage 3 with supply-chain pressure"],
        },
    )

    prompt_kwargs = intent.build_prompt_quality_kwargs()
    runtime_context = intent.build_quality_runtime_context()

    assert "recover the hidden key" in prompt_kwargs["story_foreshadow_payoff_plan_block"]
    assert "Lin: injured hand still limits movement" in prompt_kwargs["story_character_state_ledger_block"]
    assert "Lin/Su: uneasy alliance under tension" in prompt_kwargs["story_relationship_state_ledger_block"]
    assert "hidden key: still missing after the archive raid" in prompt_kwargs["story_foreshadow_state_ledger_block"]
    assert "ShadowGuild: control tightened around the docks" in prompt_kwargs["story_organization_state_ledger_block"]
    assert "Lin/Strategist: stage 3 with supply-chain pressure" in prompt_kwargs["story_career_state_ledger_block"]
    assert runtime_context["foreshadow_payoff_plan"] == ["recover the hidden key"]
    assert runtime_context["organization_state_ledger"] == ["ShadowGuild: control tightened around the docks"]
    assert runtime_context["career_state_ledger"] == ["Lin/Strategist: stage 3 with supply-chain pressure"]


def test_should_forward_quality_metrics_summary_into_intent_prompt_kwargs():
    packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="hook",
            story_focus="advance_plot",
            plot_stage="development",
            story_creation_brief="keep pressure visible",
        ),
        source="chapter-generate-request",
    )
    project = Project(title="quality-summary", user_id="user-1", chapter_count=12)
    chapter = type("ChapterStub", (), {"chapter_number": 7})()

    intent = build_chapter_generation_intent(
        story_packet=packet,
        quality_profile={"genre": "mystery"},
        project=project,
        chapter=chapter,
        chapter_context=None,
        target_word_count=2600,
        quality_metrics_summary={
            "chapter_count": 3,
            "avg_pacing_score": 7.8,
            "avg_payoff_chain_rate": 73.0,
            "avg_cliffhanger_rate": 81.0,
            "recent_focus_areas": ["payoff", "continuity"],
            "continuity_preflight": {
                "summary": "Recent chapters show 1 continuity handoff gaps.",
                "repair_targets": ["Carry forward the hidden-key pressure."],
            },
        },
    )

    prompt_kwargs = intent.build_prompt_quality_kwargs()

    assert prompt_kwargs["quality_metrics_summary"]["avg_pacing_score"] == 7.8
    assert "【章节近期质量趋势】" in prompt_kwargs["story_quality_trend_block"]
    assert "最近节奏稳定度均值：7.8/10" in prompt_kwargs["story_quality_trend_block"]
    assert "Carry forward the hidden-key pressure." in prompt_kwargs["story_quality_trend_block"]


def test_should_apply_story_repair_guidance_defaults_when_request_has_no_explicit_overrides():
    project = Project(
        title="repair-defaults",
        user_id="user-1",
        default_creative_mode="balanced",
        default_story_focus="deepen_character",
        default_quality_preset="clean_prose",
    )
    packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="balanced",
            story_focus="deepen_character",
            quality_preset="clean_prose",
        ),
        source="chapter-generate-request",
    )
    chapter = type("ChapterStub", (), {"chapter_number": 3})()

    intent = build_chapter_generation_intent(
        story_packet=packet,
        quality_profile={"genre": "mystery"},
        project=project,
        chapter=chapter,
        chapter_context=None,
        target_word_count=2400,
        active_story_repair_payload={
            "recommended_action": "patch_payoff",
            "recommended_action_mode": "payoff",
            "recommended_focus_area": "payoff",
            "weakest_metric_label": "回报兑现",
            "summary": "需要尽快补强伏笔兑现。",
        },
    )

    prompt_kwargs = intent.build_prompt_quality_kwargs()

    assert prompt_kwargs["creative_mode"] == "payoff"
    assert prompt_kwargs["story_focus"] == "foreshadow_payoff"
    assert prompt_kwargs["quality_preset"] == "plot_drive"
    assert "回报兑现" in prompt_kwargs["quality_notes"]


def test_should_keep_explicit_request_overrides_above_story_repair_defaults():
    project = Project(
        title="repair-defaults",
        user_id="user-1",
        default_creative_mode="balanced",
        default_story_focus="advance_plot",
        default_quality_preset="plot_drive",
    )
    packet = build_story_generation_packet(
        project,
        source={
            "creative_mode": "emotion",
            "story_focus": "deepen_character",
            "quality_preset": "emotion_drama",
        },
        source_label="chapter-regenerate-request",
    )
    chapter = type("ChapterStub", (), {"chapter_number": 5})()

    intent = build_chapter_generation_intent(
        story_packet=packet,
        quality_profile={"genre": "mystery"},
        project=project,
        chapter=chapter,
        chapter_context=None,
        target_word_count=2600,
        active_story_repair_payload={
            "recommended_action": "bridge_scene",
            "recommended_action_mode": "bridge",
            "recommended_focus_area": "pacing",
        },
    )

    prompt_kwargs = intent.build_prompt_quality_kwargs()

    assert prompt_kwargs["creative_mode"] == "emotion"
    assert prompt_kwargs["story_focus"] == "deepen_character"
    assert prompt_kwargs["quality_preset"] == "emotion_drama"


def test_should_include_genre_and_style_profile_in_quality_runtime_context():
    project = Project(
        title="仙朝风云",
        user_id="user-1",
        genre="仙侠权谋",
        chapter_count=12,
    )
    packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="hook",
            story_focus="advance_plot",
            plot_stage="development",
            quality_preset="plot_drive",
        ),
        source="chapter-generate-request",
    )
    chapter = type("ChapterStub", (), {"chapter_number": 6})()

    intent = build_chapter_generation_intent(
        story_packet=packet,
        quality_profile={
            "genre": "仙侠权谋",
            "style_name": "低AI连载",
            "style_preset_id": "low_ai_serial",
            "style_profile": "low_ai_serial",
        },
        project=project,
        chapter=chapter,
        chapter_context=None,
        target_word_count=2600,
    )

    runtime_context = intent.build_quality_runtime_context()

    assert runtime_context["genre"] == "仙侠权谋"
    assert "xianxia_fantasy" in runtime_context["genre_profiles"]
    assert "history_power" in runtime_context["genre_profiles"]
    assert runtime_context["style_name"] == "低AI连载"
    assert runtime_context["style_preset_id"] == "low_ai_serial"
    assert runtime_context["style_profile"] == "low_ai_serial"
    assert runtime_context["quality_preset"] == "plot_drive"
