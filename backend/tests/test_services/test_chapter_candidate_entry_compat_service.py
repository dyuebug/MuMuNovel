import pytest

from app.services import chapter_candidate_entry_compat_service as compat_service


@pytest.mark.asyncio
async def test_should_delegate_generate_best_ranked_candidate_with_cached_dependencies(monkeypatch):
    captured = {}

    def fake_dependencies(**kwargs):
        captured['dependency_kwargs'] = kwargs
        return {'deps': True}

    async def fake_workflow(**kwargs):
        captured['workflow_kwargs'] = kwargs
        return {'winner': 'ok'}

    monkeypatch.setattr(compat_service, 'get_chapter_candidate_executor_dependencies', fake_dependencies)
    monkeypatch.setattr(compat_service, 'generate_best_ranked_candidate_workflow', fake_workflow)

    result = await compat_service.generate_best_ranked_candidate(
        ai_service='ai',
        base_generate_kwargs={'prompt': 'hello'},
        target_word_count=1200,
        source='chapter',
        generation_label='label',
        quality_evaluator='quality',
        quality_gate_plan_builder='gate',
        max_candidates=3,
        runtime_state={'candidate_total': 3},
        resolve_generation_attempt_labels_fn='resolve',
        sync_generation_runtime_state_fn='sync',
        collect_generation_candidate_output_fn='collect',
        build_generation_candidate_record_fn='record',
    )

    assert result == {'winner': 'ok'}
    assert captured['dependency_kwargs'] == {
        'resolve_generation_attempt_labels_fn': 'resolve',
        'sync_generation_runtime_state_fn': 'sync',
        'collect_generation_candidate_output_fn': 'collect',
        'build_generation_candidate_record_fn': 'record',
    }
    assert captured['workflow_kwargs']['dependencies'] == {'deps': True}
    assert captured['workflow_kwargs']['ai_service'] == 'ai'
    assert captured['workflow_kwargs']['base_generate_kwargs'] == {'prompt': 'hello'}


def test_should_cache_candidate_executor_dependencies(monkeypatch):
    calls = []

    def fake_builder(**kwargs):
        calls.append(kwargs)
        return {'deps': len(calls)}

    compat_service.get_chapter_candidate_executor_dependencies.cache_clear()
    monkeypatch.setattr(
        compat_service,
        '_build_default_chapter_candidate_executor_dependencies_compat_service',
        fake_builder,
    )

    first = compat_service.get_chapter_candidate_executor_dependencies(
        resolve_generation_attempt_labels_fn='resolve',
        sync_generation_runtime_state_fn='sync',
        collect_generation_candidate_output_fn='collect',
        build_generation_candidate_record_fn='record',
    )
    second = compat_service.get_chapter_candidate_executor_dependencies(
        resolve_generation_attempt_labels_fn='resolve',
        sync_generation_runtime_state_fn='sync',
        collect_generation_candidate_output_fn='collect',
        build_generation_candidate_record_fn='record',
    )

    assert first == second == {'deps': 1}
    assert len(calls) == 1
