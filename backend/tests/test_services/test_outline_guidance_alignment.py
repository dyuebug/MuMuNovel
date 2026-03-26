from app.api.outlines import (
    _build_outline_memory_guidance,
    _build_outline_story_quality_trend_guidance_from_summary,
    _merge_outline_requirements,
)
from app.api.wizard_stream import _merge_wizard_outline_requirements
from app.schemas.outline import OutlineGenerateRequest
from app.services.chapter_quality_context_service import StoryGenerationGuidance, StoryPacket


def test_should_normalize_story_creation_brief_in_outline_generate_request():
    request = OutlineGenerateRequest(
        project_id="project-1",
        theme="命运与抉择",
        chapter_count=8,
        narrative_perspective="第三人称",
        story_creation_brief="  突出代价和抉择  ",
    )

    assert request.story_creation_brief == "突出代价和抉择"


def test_should_merge_outline_requirements_with_story_creation_brief_and_memory_guidance():
    merged = _merge_outline_requirements(
        "保留双线并进",
        "hook",
        "advance_plot",
        "climax",
        6,
        "突出代价和抉择",
        "plot_drive",
        "减少说明句",
        "【连载记忆与伏笔约束】\n【未完结伏笔】\n1. 神秘钥匙尚未回收",
    )

    assert "保留双线并进" in merged
    assert "突出代价和抉择" in merged
    assert "减少说明句" in merged
    assert "神秘钥匙尚未回收" in merged


def test_should_build_outline_memory_guidance_from_memory_context():
    guidance = _build_outline_memory_guidance(
        {
            "recent_context": "【最近章节记忆】\n1. 主角已决定潜入敌营",
            "character_states": "【角色相关记忆】\n1. 女主对主角仍存戒备",
            "foreshadows": "【未完结伏笔】\n1. 神秘钥匙尚未回收",
            "plot_points": "【重要情节点】\n1. 皇城宴会将成为转折点",
        }
    )

    assert guidance.startswith("【连载记忆与伏笔约束】")
    assert "主角已决定潜入敌营" in guidance
    assert "女主对主角仍存戒备" in guidance
    assert "神秘钥匙尚未回收" in guidance
    assert "皇城宴会将成为转折点" in guidance

def test_should_merge_wizard_outline_requirements_with_generation_guidance():
    merged = _merge_wizard_outline_requirements(
        "保留双线并进",
        outline_count=3,
        creative_mode="hook",
        story_focus="advance_plot",
        plot_stage="development",
        story_creation_brief="突出代价和抉择",
        quality_preset="plot_drive",
        quality_notes="减少说明句",
    )

    assert "保留双线并进" in merged
    assert "突出代价和抉择" in merged
    assert "减少说明句" in merged
    assert "钩子优先" in merged
    assert "主线推进" in merged
    assert "发展阶段" in merged
    assert "开局部分" in merged



def test_should_merge_outline_requirements_from_story_packet():
    story_packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="payoff",
            story_focus="foreshadow_payoff",
            plot_stage="ending",
            story_creation_brief="highlight the trade-off",
            quality_preset="tight_prose",
            quality_notes="cut exposition",
        ),
        source="outline-create-request",
    )

    merged = _merge_outline_requirements(
        "keep dual threads",
        chapter_count=4,
        memory_guidance="[serial memory]\n1. hidden key unresolved",
        story_packet=story_packet,
    )

    assert "keep dual threads" in merged
    assert "highlight the trade-off" in merged
    assert "cut exposition" in merged
    assert "hidden key unresolved" in merged


def test_should_merge_wizard_outline_requirements_from_story_packet():
    story_packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="hook",
            story_focus="advance_plot",
            plot_stage="development",
            story_creation_brief="front-load the pressure",
            quality_preset="plot_drive",
            quality_notes="trim filler",
        ),
        source="wizard-outline-request",
    )

    merged = _merge_wizard_outline_requirements(
        "keep dual threads",
        outline_count=3,
        creative_mode=None,
        story_focus=None,
        plot_stage=None,
        story_creation_brief=None,
        quality_preset=None,
        quality_notes=None,
        story_packet=story_packet,
    )

    assert "keep dual threads" in merged
    assert "front-load the pressure" in merged
    assert "trim filler" in merged
    assert "\u3010\u5f00\u5c40\u5927\u7eb2\u7ea6\u675f\u3011" in merged
    assert "钩子优先" in merged
    assert "主线推进" in merged
    assert "发展阶段" in merged



def test_should_merge_outline_requirements_with_story_packet_blueprint_blocks():
    story_packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="payoff",
            story_focus="foreshadow_payoff",
            plot_stage="ending",
            story_creation_brief="keep the payoff grounded",
        ),
        source="outline-create-request",
    ).with_blueprint(
        long_term_goal="The lead must seize the capital before the enemy closes in.",
        chapter_count=10,
        character_focus_source=["Lin", "Su"],
        foreshadow_payoff_source=["recover the hidden key"],
        character_state_source={"chapter_characters": "【Lin】\n当前状态：remains under pressure"},
        relationship_state_source={"chapter_characters": "【Lin / Su】\n关系网络：remain in conflict"},
        foreshadow_state_source={"foreshadow_reminders": "【伏笔状态】\n- hidden key still needs payoff"},
    )

    merged = _merge_outline_requirements(
        "keep the dual-thread structure",
        chapter_count=10,
        story_packet=story_packet,
    )

    assert "【长线目标锚点】" in merged
    assert "【大纲角色焦点锚点】" in merged
    assert "【大纲伏笔兑现计划】" in merged
    assert "【大纲节奏预算】" in merged


def test_should_merge_wizard_outline_requirements_with_story_packet_blueprint_blocks():
    story_packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="hook",
            story_focus="advance_plot",
            plot_stage="development",
            story_creation_brief="front-load the pressure",
        ),
        source="wizard-outline-request",
    ).with_blueprint(
        long_term_goal="The lead must seize the capital before the enemy closes in.",
        chapter_count=8,
        character_focus_source=["Lin", "Su"],
        foreshadow_payoff_source=["recover the hidden key"],
        character_state_source={"chapter_characters": "【Lin】\n当前状态：remains under pressure"},
        relationship_state_source={"chapter_characters": "【Lin / Su】\n关系网络：remain in conflict"},
        foreshadow_state_source={"foreshadow_reminders": "【伏笔状态】\n- hidden key still needs payoff"},
    )

    merged = _merge_wizard_outline_requirements(
        "keep the dual-thread structure",
        outline_count=3,
        creative_mode=None,
        story_focus=None,
        plot_stage=None,
        story_creation_brief=None,
        quality_preset=None,
        quality_notes=None,
        story_packet=story_packet,
    )

    assert "【长线目标锚点】" in merged
    assert "【大纲角色焦点锚点】" in merged
    assert "【大纲伏笔兑现计划】" in merged
    assert "【卷级节奏】" in merged



def test_should_build_outline_story_quality_trend_guidance_from_summary():
    guidance = _build_outline_story_quality_trend_guidance_from_summary(
        {
            "chapter_count": 3,
            "overall_score_trend": "falling",
            "avg_payoff_chain_rate": 61.0,
            "avg_cliffhanger_rate": 58.0,
        }
    )

    assert "\u3010\u5927\u7eb2\u8fd1\u671f\u8d28\u91cf\u8d8b\u52bf\u3011" in guidance
    assert "\u6700\u8fd1 3 \u7ae0" in guidance
    assert "\u540e\u7eed\u7ae0\u8282" in guidance
    assert "\u672c\u7ae0" not in guidance


def test_should_merge_outline_requirements_with_quality_trend_guidance():
    merged = _merge_outline_requirements(
        "\u4fdd\u7559\u53cc\u7ebf\u5e76\u8fdb",
        "hook",
        story_focus="advance_plot",
        quality_trend_guidance="\u3010\u5927\u7eb2\u8fd1\u671f\u8d28\u91cf\u8d8b\u52bf\u3011\n- \u540e\u7eed\u7ae0\u8282\u8981\u4f18\u5148\u56de\u6536\u65e7\u627f\u8bfa",
    )

    assert "\u4fdd\u7559\u53cc\u7ebf\u5e76\u8fdb" in merged
    assert "\u3010\u5927\u7eb2\u8fd1\u671f\u8d28\u91cf\u8d8b\u52bf\u3011" in merged
    assert "\u540e\u7eed\u7ae0\u8282\u8981\u4f18\u5148\u56de\u6536\u65e7\u627f\u8bfa" in merged


def test_should_merge_wizard_outline_requirements_with_quality_trend_guidance():
    merged = _merge_wizard_outline_requirements(
        "\u4fdd\u7559\u53cc\u7ebf\u5e76\u8fdb",
        outline_count=3,
        creative_mode="hook",
        story_focus="advance_plot",
        plot_stage="development",
        story_creation_brief="\u7a81\u51fa\u4ee3\u4ef7\u548c\u6292\u62e9",
        quality_preset="plot_drive",
        quality_notes="\u51cf\u5c11\u8bf4\u660e\u53e5",
        quality_trend_guidance="\u3010\u5927\u7eb2\u8fd1\u671f\u8d28\u91cf\u8d8b\u52bf\u3011\n- \u540e\u7eed\u7ae0\u8282\u8981\u4f18\u5148\u56de\u6536\u65e7\u627f\u8bfa",
    )

    assert "\u4fdd\u7559\u53cc\u7ebf\u5e76\u8fdb" in merged
    assert "\u3010\u5927\u7eb2\u8fd1\u671f\u8d28\u91cf\u8d8b\u52bf\u3011" in merged
    assert "\u540e\u7eed\u7ae0\u8282\u8981\u4f18\u5148\u56de\u6536\u65e7\u627f\u8bfa" in merged
