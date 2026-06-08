import pytest

from app.services import project_quality_trend_service


@pytest.mark.asyncio
async def test_should_resolve_project_quality_trend_snapshot_with_default_wiring(monkeypatch):
    captured = {}

    async def fake_service(**kwargs):
        captured.update(kwargs)
        return {'ok': True}

    monkeypatch.setattr(
        project_quality_trend_service,
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

    monkeypatch.setattr(project_quality_trend_service, 'build_quality_metrics_summary_state', build_state)
    monkeypatch.setattr(project_quality_trend_service, 'advance_quality_metrics_summary_state', advance_state)
    monkeypatch.setattr(project_quality_trend_service, 'build_quality_metrics_summary_from_state', summary_from_state)
    monkeypatch.setattr(project_quality_trend_service, 'load_project_quality_trend_snapshot', load_snapshot)
    monkeypatch.setattr(project_quality_trend_service, 'persist_project_quality_trend_snapshot', persist_snapshot)

    result = await project_quality_trend_service.get_project_quality_trend_snapshot_with_default_wiring(
        project_id='project-1',
        limit=12,
        items=[{'chapter_id': 'chapter-1'}],
        metrics_history=[{'overall_score': 88.0}],
        total_chapters=3,
        analyzed_chapters=2,
        last_generated_at=None,
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
