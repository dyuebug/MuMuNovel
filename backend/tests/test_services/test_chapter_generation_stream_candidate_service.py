from types import SimpleNamespace

from tests.test_support import (
    single_generation_stream_candidate_test_adapter as stream_candidate_service,
)


def test_should_build_selected_candidate_outcome_for_quality_gate_retry():
    captured = {}

    def fake_build_draft_attempt(**kwargs):
        captured.update(kwargs)
        return {'draft': True, 'state': kwargs['attempt_state']}

    def fake_attach_story_runtime_contract(payload, contract):
        return payload

    original_normalize = stream_candidate_service.build_chapter_generation_selected_candidate_outcome.__globals__[
        '__builtins__'
    ]
    from tests.test_support import chapter_candidate_result_test_support

    original_normalize_fn = (
        chapter_candidate_result_test_support.normalize_selected_candidate_result
    )
    chapter_candidate_result_test_support.normalize_selected_candidate_result = (
        lambda **_kwargs: SimpleNamespace(
            full_content='generated content',
            candidate_word_count=321,
            candidate_chunks=['chunk-a', 'chunk-b'],
            quality_metrics={'overall_score': 87},
            quality_gate_plan={
                'message': 'need follow-up',
                'active_story_repair_payload': {'repair_mode': 'retry'},
            },
            quality_gate_action='retry',
            quality_gate_snapshot={'decision': 'retry'},
        )
    )
    try:
        outcome = stream_candidate_service.build_chapter_generation_selected_candidate_outcome(
            selected_candidate={'ignored': True},
            story_runtime_contract={'runtime': True},
            previous_content='old content',
            previous_word_count=111,
            project_id='project-1',
            chapter_id='chapter-1',
            build_draft_attempt_fn=fake_build_draft_attempt,
            attach_story_runtime_contract_fn=fake_attach_story_runtime_contract,
        )
    finally:
        chapter_candidate_result_test_support.normalize_selected_candidate_result = original_normalize_fn

    assert outcome.full_content == 'generated content'
    assert outcome.candidate_word_count == 321
    assert outcome.quality_gate_requires_followup is True
    assert outcome.content_applied is False
    assert outcome.provisional_draft_allowed is True
    assert outcome.attempt_state == 'retry'
    assert outcome.draft_attempt == {'draft': True, 'state': 'retry'}
    assert captured['project_id'] == 'project-1'
    assert captured['chapter_id'] == 'chapter-1'
    assert captured['repair_payload'] == {'repair_mode': 'retry'}
    assert captured['previous_content'] == 'old content'
    assert captured['previous_word_count'] == 111


def test_should_build_chapter_stream_draft_attempt_with_previous_content_defaults(monkeypatch):
    captured = {}

    def fake_build_batch_chapter_draft_attempt(**kwargs):
        captured.update(kwargs)
        return {"draft": True}

    monkeypatch.setattr(
        "tests.test_support.batch_generation_retry_test_adapter.build_batch_chapter_draft_attempt",
        fake_build_batch_chapter_draft_attempt,
    )

    result = stream_candidate_service.build_chapter_stream_draft_attempt(
        project_id="project-1",
        chapter_id="chapter-1",
        source="chapter",
        attempt_state="retry",
        quality_gate_action="retry",
        quality_gate_decision="retry",
        full_content="generated content",
        repair_payload={"repair_mode": "retry"},
        previous_content="old content",
        previous_word_count=123,
    )

    assert result == {"draft": True}
    assert captured["project_id"] == "project-1"
    assert captured["chapter_id"] == "chapter-1"
    assert captured["source"] == "chapter"
    assert captured["attempt_state"] == "retry"
    assert captured["repair_payload"] == {
        "repair_mode": "retry",
        "previous_content": "old content",
        "previous_word_count": 123,
    }


def test_should_build_candidate_quality_hooks_with_runtime_context():
    captured = {}
    runtime_context = type(
        'RuntimeContext',
        (),
        {
            'chapter': type('Chapter', (), {'id': 'chapter-1'})(),
            'project': type('Project', (), {'world_rules': 'rules'})(),
            'story_packet': {'packet': True},
            'story_repair_payload': {'repair': True},
        },
    )()
    built_context = type(
        'BuiltContext',
        (),
        {
            'chapter_context': type('ChapterContext', (), {'chapter_outline': 'outline'})(),
            'generation_intent': {'intent': True},
        },
    )()

    def fake_build_quality_runtime_context(**kwargs):
        captured['quality_runtime_context_kwargs'] = kwargs
        return {'ctx': True}

    def fake_compute_story_quality_metrics(**kwargs):
        captured['metrics_kwargs'] = kwargs
        return {
            'overall_score': 91,
            'conflict_chain_hit_rate': 0.7,
            'rule_grounding_hit_rate': 0.8,
        }

    def fake_resolve_quality_gate_execution_plan(metrics, **kwargs):
        captured['quality_plan_metrics'] = metrics
        captured['quality_plan_kwargs'] = kwargs
        return {'decision': 'continue'}

    hooks = stream_candidate_service.build_chapter_generation_candidate_quality_hooks(
        runtime_context=runtime_context,
        built_context=built_context,
        target_word_count=2000,
        build_quality_runtime_context_fn=fake_build_quality_runtime_context,
        compute_story_quality_metrics_fn=fake_compute_story_quality_metrics,
        resolve_quality_gate_execution_plan_fn=fake_resolve_quality_gate_execution_plan,
        retry_count=1,
        max_retries=2,
        scope='chapter',
        log_prefix='Chapter',
    )

    metrics = hooks.quality_evaluator('generated text')
    plan = hooks.quality_gate_plan_builder(metrics, 0)

    assert metrics['overall_score'] == 91
    assert plan == {'decision': 'continue'}
    assert captured['quality_runtime_context_kwargs']['story_packet'] == {'packet': True}
    assert captured['quality_runtime_context_kwargs']['target_word_count'] == 2000
    assert captured['metrics_kwargs']['content'] == 'generated text'
    assert captured['metrics_kwargs']['chapter_outline'] == 'outline'
    assert captured['quality_plan_metrics']['overall_score'] == 91
    assert captured['quality_plan_kwargs']['retry_count'] == 1
    assert captured['quality_plan_kwargs']['max_retries'] == 2
    assert captured['quality_plan_kwargs']['current_story_repair_payload'] == {'repair': True}
