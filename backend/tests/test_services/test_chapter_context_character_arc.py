import json
from types import SimpleNamespace
from app.models.character import Character
from app.models.memory import StoryMemory
from app.services.chapter_context_service import (
    _build_outline_structure_character_fallback,
    build_character_arc_snapshot,
)


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


import pytest

from app.services.chapter_context_service import OneToManyContextBuilder, OneToOneContextBuilder


@pytest.mark.asyncio
async def test_one_to_one_context_builder_should_delegate_character_arc_snapshot(monkeypatch):
    expected = "【角色弧光快照】\n- 苏离：当前状态「警惕」"

    async def fake_delegate(self, project_id, chapter_number, db, filter_character_names=None):
        assert project_id == "project-1"
        assert chapter_number == 3
        assert filter_character_names == ["苏离"]
        return expected

    monkeypatch.setattr(
        OneToManyContextBuilder,
        "_get_character_arc_snapshot",
        fake_delegate,
    )

    builder = OneToOneContextBuilder()
    result = await builder._get_character_arc_snapshot(
        project_id="project-1",
        chapter_number=3,
        db=object(),
        filter_character_names=["苏离"],
    )

    assert result == expected

def test_should_build_outline_structure_character_fallback_with_organizations():
    outline = SimpleNamespace(
        structure=json.dumps(
            {
                "characters": [
                    {"name": "??", "type": "character", "role": "protagonist", "summary": "???????????????"},
                    {"name": "????????", "type": "organization", "description": "???????????"},
                ]
            },
            ensure_ascii=False,
        )
    )

    result = _build_outline_structure_character_fallback(
        outline,
        filter_character_names=["??", "????????"],
    )

    assert "角色/组织参考：" in result
    assert "角色：" in result
    assert "组织：" in result
    assert "???????????" in result
    assert "定位=protagonist" in result



class _ScalarListResult:
    def __init__(self, values):
        self._values = list(values)

    def scalars(self):
        return self

    def all(self):
        return list(self._values)


class _RowsResult:
    def __init__(self, rows):
        self._rows = list(rows)

    def all(self):
        return list(self._rows)


class _StubDB:
    def __init__(self, responses):
        self._responses = list(responses)
        self.calls = 0

    async def execute(self, *_args, **_kwargs):
        if self.calls >= len(self._responses):
            raise AssertionError(f"unexpected execute call index={self.calls}")
        response = self._responses[self.calls]
        self.calls += 1
        return response


@pytest.mark.asyncio
async def test_one_to_many_builder_should_fallback_to_outline_structure_when_db_has_no_characters():
    outline = SimpleNamespace(
        structure=json.dumps(
            {
                "characters": [
                    {"name": "??", "type": "character", "role": "protagonist", "summary": "????????????"},
                    {"name": "????????", "type": "organization", "description": "????????????"},
                ]
            },
            ensure_ascii=False,
        )
    )
    chapter = SimpleNamespace(expansion_plan=None)
    project = SimpleNamespace(id="project-1")
    db = _StubDB([_ScalarListResult([])])

    builder = OneToManyContextBuilder()
    characters_info, careers_info = await builder._build_chapter_characters_1n(
        chapter=chapter,
        project=project,
        outline=outline,
        db=db,
    )

    assert careers_info is None
    assert "角色/组织参考：" in characters_info
    assert "??" in characters_info
    assert "????????" in characters_info


@pytest.mark.asyncio
async def test_one_to_many_builder_should_append_outline_organization_fallback_when_db_is_partial():
    outline = SimpleNamespace(
        structure=json.dumps(
            {
                "characters": [
                    {"name": "??", "type": "character", "role": "protagonist", "summary": "????????????"},
                    {"name": "????????", "type": "organization", "description": "????????????"},
                ]
            },
            ensure_ascii=False,
        )
    )
    chapter = SimpleNamespace(expansion_plan=None)
    project = SimpleNamespace(id="project-1")
    db_character = Character(
        id="char-1",
        project_id="project-1",
        name="??",
        role_type="protagonist",
        status="active",
        is_organization=False,
    )
    db = _StubDB([
        _ScalarListResult([db_character]),
        _ScalarListResult([]),
        _RowsResult([]),
        _ScalarListResult([]),
    ])

    builder = OneToManyContextBuilder()
    characters_info, careers_info = await builder._build_chapter_characters_1n(
        chapter=chapter,
        project=project,
        outline=outline,
        db=db,
    )

    assert careers_info is None
    assert "??" in characters_info
    assert "角色/组织参考：" in characters_info
    assert "????????" in characters_info


@pytest.mark.asyncio
async def test_one_to_one_builder_should_fallback_to_outline_structure_when_no_db_character_matches(monkeypatch):
    outline = SimpleNamespace(
        structure=json.dumps(
            {
                "summary": "????????????????????",
                "characters": [
                    {"name": "??", "type": "character", "role": "protagonist", "summary": "????????????"},
                    {"name": "????????", "type": "organization", "description": "????????????"},
                ],
            },
            ensure_ascii=False,
        )
    )
    chapter = SimpleNamespace(
        id="chapter-1",
        project_id="project-1",
        chapter_number=1,
        title="?1?",
        summary=None,
        expansion_plan=None,
    )
    project = SimpleNamespace(
        id="project-1",
        title="?????",
        genre="????",
        theme="????",
        narrative_perspective="????",
    )
    db = _StubDB([_ScalarListResult([])])

    async def fake_arc_snapshot(self, project_id, chapter_number, db, filter_character_names=None):
        return None

    monkeypatch.setattr(OneToOneContextBuilder, "_get_character_arc_snapshot", fake_arc_snapshot)

    builder = OneToOneContextBuilder()
    context = await builder.build(
        chapter=chapter,
        project=project,
        outline=outline,
        user_id="user-1",
        db=db,
        target_word_count=1200,
    )

    assert "角色/组织参考：" in context.chapter_characters
    assert "??" in context.chapter_characters
    assert "????????" in context.chapter_characters
