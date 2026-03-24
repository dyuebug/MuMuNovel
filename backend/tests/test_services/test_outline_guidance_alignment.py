from app.api.outlines import _build_outline_memory_guidance, _merge_outline_requirements
from app.schemas.outline import OutlineGenerateRequest


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
