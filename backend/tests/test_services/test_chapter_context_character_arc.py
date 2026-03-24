from app.models.character import Character
from app.models.memory import StoryMemory
from app.services.chapter_context_service import build_character_arc_snapshot


def test_should_build_character_arc_snapshot_from_state_and_recent_memory():
    focused_character = Character(
        id="char-1",
        project_id="project-1",
        name="苏离",
        role_type="protagonist",
        current_state="因议会背叛而变得警惕克制",
        state_updated_chapter=12,
        status="active",
    )
    background_character = Character(
        id="char-2",
        project_id="project-1",
        name="林澈",
        role_type="supporting",
        current_state="",
        status="active",
    )
    memories = [
        StoryMemory(
            id="memory-1",
            project_id="project-1",
            memory_type="character_event",
            title="苏离的变化",
            content="苏离决定不再轻信任何人，并开始主动试探盟友。",
            related_characters=["苏离"],
            story_timeline=11,
            importance_score=0.95,
        )
    ]

    snapshot = build_character_arc_snapshot(
        characters=[focused_character, background_character],
        memories=memories,
        current_chapter=13,
    )

    assert snapshot is not None
    assert "【角色弧光快照】" in snapshot
    assert "苏离" in snapshot
    assert "当前状态" in snapshot
    assert "第12章" in snapshot
    assert "第11章" in snapshot
    assert "林澈" not in snapshot
