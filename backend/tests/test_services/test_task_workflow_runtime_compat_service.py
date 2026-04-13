import pytest

from app.services import task_workflow_runtime_compat_service as compat_service
from app.services import task_workflow_runtime_service as runtime_service


@pytest.mark.asyncio
async def test_should_delegate_clear_task_runtime_caches(monkeypatch):
    calls = []

    async def fake_clear_quality(task_id):
        calls.append(('quality', task_id))

    async def fake_clear_workflow(task_id):
        calls.append(('workflow', task_id))

    monkeypatch.setattr(compat_service, 'clear_task_quality_metrics_cache', fake_clear_quality)
    monkeypatch.setattr(compat_service, 'clear_task_workflow_runtime_cache', fake_clear_workflow)

    await compat_service.clear_task_runtime_caches('task-1')

    assert calls == [('quality', 'task-1'), ('workflow', 'task-1')]


@pytest.mark.asyncio
async def test_should_delegate_upsert_batch_generation_snapshot(monkeypatch):
    captured = {}

    async def fake_upsert(db_session, task_id, **kwargs):
        captured['db_session'] = db_session
        captured['task_id'] = task_id
        captured.update(kwargs)
        return {'ok': True}

    monkeypatch.setattr(compat_service, '_upsert_batch_generation_snapshot_service', fake_upsert)

    result = await compat_service.upsert_batch_generation_snapshot(
        'db',
        'task-1',
        workflow_runtime_state={'phase': 'loading'},
    )

    assert result == {'ok': True}
    assert captured['db_session'] == 'db'
    assert captured['task_id'] == 'task-1'
    assert captured['workflow_runtime_state'] == {'phase': 'loading'}
    assert captured['latest_quality_metrics'] is compat_service.SNAPSHOT_UNSET
    assert captured['quality_metrics_history'] is compat_service.SNAPSHOT_UNSET
    assert captured['quality_metrics_summary'] is compat_service.SNAPSHOT_UNSET


def test_should_share_snapshot_unset_with_runtime_service():
    assert compat_service.SNAPSHOT_UNSET is runtime_service.SNAPSHOT_UNSET
