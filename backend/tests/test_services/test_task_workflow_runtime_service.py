import pytest

from app.services import task_workflow_runtime_service as runtime_service
from app.services import task_quality_snapshot_service


@pytest.mark.asyncio
async def test_should_clear_task_runtime_caches(monkeypatch):
    calls = []

    async def fake_clear_quality(task_id):
        calls.append(('quality', task_id))

    async def fake_clear_workflow(task_id):
        calls.append(('workflow', task_id))

    monkeypatch.setattr(task_quality_snapshot_service, 'clear_task_quality_metrics_cache', fake_clear_quality)
    monkeypatch.setattr(runtime_service, 'clear_task_workflow_runtime_cache', fake_clear_workflow)

    await runtime_service.clear_task_runtime_caches('task-1')

    assert calls == [('quality', 'task-1'), ('workflow', 'task-1')]


def test_should_expose_snapshot_unset_sentinel():
    assert runtime_service.SNAPSHOT_UNSET is runtime_service._SNAPSHOT_UNSET
