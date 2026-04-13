import pytest

from app.services import chapter_candidate_executor_compat_service as compat_service
from app.services.chapter_candidate_executor_compat_service import (
    build_default_chapter_candidate_executor_dependencies,
    build_generation_candidate_record,
    collect_generation_candidate_output,
    resolve_generation_attempt_labels,
    sync_generation_runtime_state,
)


pytestmark = pytest.mark.asyncio


class StubAIService:
    def __init__(self, chunks: list[str]):
        self._chunks = list(chunks)
        self.calls: list[dict] = []

    async def generate_text_stream(self, **kwargs):
        self.calls.append(dict(kwargs))
        for chunk in self._chunks:
            yield chunk


async def test_should_collect_generation_candidate_output_via_compat_service():
    runtime_state = {'candidate_total': 2}
    ai_service = StubAIService(['alpha', 'beta'])

    full_content, chunks = await collect_generation_candidate_output(
        ai_service=ai_service,
        generate_kwargs={'prompt': 'hello'},
        candidate_index=2,
        runtime_state=runtime_state,
    )

    assert full_content == 'alphabeta'
    assert chunks == ['alpha', 'beta']
    assert ai_service.calls == [{'prompt': 'hello'}]
    assert runtime_state['candidate_index'] == 2
    assert runtime_state['candidate_total'] == 2
    assert runtime_state['chunk_count'] == 2


def test_should_resolve_generation_attempt_labels_via_compat_service():
    assert resolve_generation_attempt_labels(1) == ('single_pass', 'initial_candidate')
    assert resolve_generation_attempt_labels(2) == ('rerank_retry', 'rerank_candidate')
    assert resolve_generation_attempt_labels(1, is_word_budget_repair=True) == (
        'word_budget_repair',
        'word_budget_repair',
    )


def test_should_sync_generation_runtime_state_via_compat_service():
    runtime_state = {}

    sync_generation_runtime_state(
        runtime_state,
        candidate_index=3,
        candidate_total=4,
        current_chars=128,
        chunk_count=5,
        generation_path='rerank_retry',
        attempt_kind='rerank_candidate',
        rerank_used=True,
        winner_candidate_index=2,
    )

    assert runtime_state['candidate_index'] == 3
    assert runtime_state['candidate_total'] == 4
    assert runtime_state['current_chars'] == 128
    assert runtime_state['chunk_count'] == 5
    assert runtime_state['generation_path'] == 'rerank_retry'
    assert runtime_state['attempt_kind'] == 'rerank_candidate'
    assert runtime_state['rerank_used'] is True
    assert runtime_state['winner_candidate_index'] == 2


def test_should_build_generation_candidate_record_via_compat_service():
    warnings = []

    def quality_evaluator(content: str) -> dict:
        return {
            'overall_score': 88.0,
            'quality_gate': {
                'decision': 'allow_save',
                'status': 'pass',
            },
        }

    def quality_gate_plan_builder(metrics: dict, attempt_offset: int) -> dict:
        return {
            'action': 'continue',
            'quality_gate': metrics.get('quality_gate') or {
                'decision': 'allow_save',
                'status': 'pass',
            },
            'attempt_offset': attempt_offset,
        }

    result = build_generation_candidate_record(
        full_content='First paragraph.\nSecond paragraph.',
        candidate_chunks=['First paragraph.', 'Second paragraph.'],
        target_word_count=1200,
        source='chapter',
        generation_label='compat-test',
        candidate_index=2,
        candidate_offset=1,
        quality_evaluator=quality_evaluator,
        quality_gate_plan_builder=quality_gate_plan_builder,
        generation_path='rerank_retry',
        attempt_kind='rerank_candidate',
        log_warning_fn=warnings.append,
    )

    assert result['candidate_index'] == 2
    assert result['generation_path'] == 'rerank_retry'
    assert result['attempt_kind'] == 'rerank_candidate'
    assert result['quality_gate_plan']['attempt_offset'] == 1
    assert warnings == []


def test_should_build_default_candidate_executor_dependencies_via_compat_service(monkeypatch):
    captured = {}

    def fake_builder(**kwargs):
        captured.update(kwargs)
        return {'ok': True}

    monkeypatch.setattr(
        compat_service,
        '_build_default_chapter_candidate_executor_dependencies_service',
        fake_builder,
    )

    result = build_default_chapter_candidate_executor_dependencies(
        resolve_generation_attempt_labels_fn='resolve',
        sync_generation_runtime_state_fn='sync',
        collect_generation_candidate_output_fn='collect',
        build_generation_candidate_record_fn='record',
    )

    assert result == {'ok': True}
    assert captured == {
        'resolve_generation_attempt_labels_fn': 'resolve',
        'sync_generation_runtime_state_fn': 'sync',
        'collect_generation_candidate_output_fn': 'collect',
        'build_generation_candidate_record_fn': 'record',
    }
