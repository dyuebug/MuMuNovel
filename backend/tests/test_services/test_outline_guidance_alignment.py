from app.api.outlines import _build_outline_memory_guidance, _merge_outline_requirements
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
            story_creation_brief="????????",
            quality_preset="tight_prose",
            quality_notes="???????",
        ),
        source="outline-create-request",
    )

    merged = _merge_outline_requirements(
        "??????",
        chapter_count=4,
        memory_guidance="???????????\n1. ????????",
        story_packet=story_packet,
    )

    assert "??????" in merged
    assert "????????" in merged
    assert "???????" in merged
    assert "????????" in merged


def test_should_merge_wizard_outline_requirements_from_story_packet():
    story_packet = StoryPacket.from_guidance(
        StoryGenerationGuidance(
            creative_mode="hook",
            story_focus="advance_plot",
            plot_stage="development",
            story_creation_brief="???????",
            quality_preset="plot_drive",
            quality_notes="?????",
        ),
        source="wizard-outline-request",
    )

    merged = _merge_wizard_outline_requirements(
        "??????",
        outline_count=3,
        creative_mode=None,
        story_focus=None,
        plot_stage=None,
        story_creation_brief=None,
        quality_preset=None,
        quality_notes=None,
        story_packet=story_packet,
    )

    assert "??????" in merged
    assert "???????" in merged
    assert "?????" in merged
    assert "????" in merged
    assert "????" in merged
    assert "????" in merged



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
        character_state_source={"chapter_characters": "????????\n- Lin????????"},
        relationship_state_source={"chapter_characters": "????????\n- Lin/Su??????????"},
        foreshadow_state_source={"foreshadow_reminders": "????????\n- hidden key????????????"},
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
        character_state_source={"chapter_characters": "????????\n- Lin????????"},
        relationship_state_source={"chapter_characters": "????????\n- Lin/Su??????????"},
        foreshadow_state_source={"foreshadow_reminders": "????????\n- hidden key????????????"},
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
