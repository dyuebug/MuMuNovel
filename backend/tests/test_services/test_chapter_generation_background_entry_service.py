import pytest
from fastapi import HTTPException

from app.services.chapter_generation import background_entry_service as entry_service


class _ScalarResult:
    def __init__(self, value):
        self._value = value

    def scalar_one_or_none(self):
        return self._value


@pytest.mark.asyncio
async def test_should_generate_chapter_content_background_with_default_wiring():
    captured = {}
    chapter = type('Chapter', (), {'project_id': 'project-1'})()
    project = object()

    async def fake_load_chapter(**kwargs):
        captured['load_kwargs'] = kwargs
        return chapter

    async def fake_orchestrate(*args, **kwargs):
        captured['orchestrate_args'] = args
        captured['orchestrate_kwargs'] = kwargs
        return {'ok': True}

    class FakeDbSession:
        async def execute(self, _stmt):
            return _ScalarResult(project)

    monkeypatch_target = entry_service.orchestrate_single_chapter_background_generation
    entry_service.orchestrate_single_chapter_background_generation = fake_orchestrate
    try:
        result = await entry_service.generate_chapter_content_background_with_default_wiring(
            db_session=FakeDbSession(),
            chapter_id='chapter-1',
            user_id='user-1',
            generate_request='request',
            background_tasks='bg',
            ai_service='ai',
            load_accessible_chapter_or_404_fn=fake_load_chapter,
            check_prerequisites_fn='check',
            build_workflow_snapshot_fn='snapshot',
            resolve_story_repair_state_fn='repair-state',
            sync_task_story_repair_state_fn='sync-repair',
            execution_callable='execute',
        )
    finally:
        entry_service.orchestrate_single_chapter_background_generation = monkeypatch_target

    assert result == {'ok': True}
    assert captured['load_kwargs']['chapter_id'] == 'chapter-1'
    assert captured['load_kwargs']['user_id'] == 'user-1'
    assert captured['orchestrate_args']
    assert captured['orchestrate_kwargs']['chapter_id'] == 'chapter-1'
    assert captured['orchestrate_kwargs']['chapter'] is chapter
    assert captured['orchestrate_kwargs']['project'] is project
    assert captured['orchestrate_kwargs']['user_id'] == 'user-1'
    assert captured['orchestrate_kwargs']['generate_request'] == 'request'
    assert captured['orchestrate_kwargs']['background_tasks'] == 'bg'
    assert captured['orchestrate_kwargs']['ai_service'] == 'ai'
    assert captured['orchestrate_kwargs']['check_prerequisites_fn'] == 'check'
    assert captured['orchestrate_kwargs']['build_workflow_snapshot_fn'] == 'snapshot'
    assert captured['orchestrate_kwargs']['resolve_story_repair_state_fn'] == 'repair-state'
    assert captured['orchestrate_kwargs']['sync_task_story_repair_state_fn'] == 'sync-repair'
    assert captured['orchestrate_kwargs']['execution_callable'] == 'execute'


@pytest.mark.asyncio
async def test_should_raise_when_project_missing_during_background_generation_entry():
    chapter = type('Chapter', (), {'project_id': 'project-1'})()

    async def fake_load_chapter(**_kwargs):
        return chapter

    class FakeDbSession:
        async def execute(self, _stmt):
            return _ScalarResult(None)

    with pytest.raises(HTTPException) as exc_info:
        await entry_service.generate_chapter_content_background_with_default_wiring(
            db_session=FakeDbSession(),
            chapter_id='chapter-1',
            user_id='user-1',
            generate_request='request',
            background_tasks='bg',
            ai_service='ai',
            load_accessible_chapter_or_404_fn=fake_load_chapter,
            check_prerequisites_fn='check',
            build_workflow_snapshot_fn='snapshot',
            resolve_story_repair_state_fn='repair-state',
            sync_task_story_repair_state_fn='sync-repair',
            execution_callable='execute',
        )

    assert exc_info.value.status_code == 404
    assert exc_info.value.detail == 'Project not found'
