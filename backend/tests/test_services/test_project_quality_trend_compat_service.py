import pytest

from app.services import project_quality_trend_compat_service as compat_service
from app.services.project_quality_trend_compat_service import (
    get_project_quality_trend_snapshot,
)


@pytest.mark.asyncio
async def test_should_delegate_project_quality_trend_snapshot_to_service(monkeypatch):
    captured = {}

    async def fake_service(**kwargs):
        captured.update(kwargs)
        return {'ok': True}

    monkeypatch.setattr(
        compat_service,
        '_get_project_quality_trend_snapshot_service',
        fake_service,
    )

    def build_state(*_args, **_kwargs):
        return {'state': True}

    def advance_state(*_args, **_kwargs):
        return {'state': 'advanced'}

    def summary_from_state(*_args, **_kwargs):
        return {'summary': True}

    def load_snapshot(*_args, **_kwargs):
        return {'cached': True}

    def persist_snapshot(*_args, **_kwargs):
        return None

    result = await get_project_quality_trend_snapshot(
        project_id='project-1',
        limit=12,
        items=[{'chapter_id': 'chapter-1'}],
        metrics_history=[{'overall_score': 88.0}],
        total_chapters=3,
        analyzed_chapters=2,
        last_generated_at=None,
        build_summary_state_fn=build_state,
        advance_summary_state_fn=advance_state,
        summary_from_state_fn=summary_from_state,
        load_snapshot_fn=load_snapshot,
        persist_snapshot_fn=persist_snapshot,
    )

    assert result == {'ok': True}
    assert captured['project_id'] == 'project-1'
    assert captured['limit'] == 12
    assert captured['items'] == [{'chapter_id': 'chapter-1'}]
    assert captured['metrics_history'] == [{'overall_score': 88.0}]
    assert captured['total_chapters'] == 3
    assert captured['analyzed_chapters'] == 2
    assert captured['build_summary_state_fn'] is build_state
    assert captured['advance_summary_state_fn'] is advance_state
    assert captured['summary_from_state_fn'] is summary_from_state
    assert captured['load_snapshot_fn'] is load_snapshot
    assert captured['persist_snapshot_fn'] is persist_snapshot
