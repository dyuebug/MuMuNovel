from app.services.chapter_candidate_record_service import (
    ChapterCandidateRecordRequest,
    build_generation_candidate_record,
)


def test_should_build_generation_candidate_record_with_enriched_selection_metadata():
    builder_calls = []

    def quality_evaluator(content: str) -> dict:
        return {
            'overall_score': 82.5,
            'quality_gate': {
                'decision': 'manual_review',
                'status': 'blocked',
            },
        }

    def quality_gate_plan_builder(metrics: dict, attempt_offset: int) -> dict:
        builder_calls.append(
            {
                'attempt_offset': attempt_offset,
                'has_selection': 'candidate_selection' in metrics,
            }
        )
        if 'candidate_selection' in metrics:
            return {
                'action': 'retry',
                'quality_gate': {
                    'decision': 'manual_review',
                    'status': 'blocked',
                },
                'saw_selection': True,
            }
        return {
            'action': 'retry',
            'quality_gate': {
                'decision': 'manual_review',
                'status': 'blocked',
            },
            'saw_selection': False,
        }

    result = build_generation_candidate_record(
        request=ChapterCandidateRecordRequest(
            full_content='First paragraph.\nSecond paragraph.',
            candidate_chunks=['First paragraph.', 'Second paragraph.'],
            target_word_count=1200,
            source='chapter',
            generation_label='test-candidate',
            candidate_index=2,
            candidate_offset=1,
            quality_evaluator=quality_evaluator,
            quality_gate_plan_builder=quality_gate_plan_builder,
            generation_path='rerank_retry',
            attempt_kind='rerank_candidate',
        )
    )

    assert len(builder_calls) == 2
    assert builder_calls[0]['has_selection'] is False
    assert builder_calls[1]['has_selection'] is True
    assert result['candidate_index'] == 2
    assert result['candidate_count'] == 2
    assert result['generation_path'] == 'rerank_retry'
    assert result['attempt_kind'] == 'rerank_candidate'
    assert result['quality_gate_plan']['saw_selection'] is True
    assert result['quality_metrics']['candidate_selection']['candidate_index'] == 2
    assert result['quality_metrics']['candidate_selection']['rerank_used'] is True


def test_should_fallback_to_initial_quality_gate_plan_when_enriched_plan_is_empty():
    def quality_evaluator(content: str) -> dict:
        return {
            'overall_score': 91.0,
            'quality_gate': {
                'decision': 'allow_save',
                'status': 'pass',
            },
        }

    def quality_gate_plan_builder(metrics: dict, attempt_offset: int) -> dict:
        if 'candidate_selection' in metrics:
            return {}
        return {
            'action': 'continue',
            'quality_gate': {
                'decision': 'allow_save',
                'status': 'pass',
            },
        }

    result = build_generation_candidate_record(
        request=ChapterCandidateRecordRequest(
            full_content='Valid chapter content.',
            candidate_chunks=['Valid chapter content.'],
            target_word_count=1200,
            source='chapter',
            generation_label='test-fallback-plan',
            candidate_index=1,
            candidate_offset=0,
            quality_evaluator=quality_evaluator,
            quality_gate_plan_builder=quality_gate_plan_builder,
            generation_path='single_pass',
            attempt_kind='initial_candidate',
        )
    )

    assert result['quality_gate_plan']['action'] == 'continue'
    assert isinstance(result['quality_metrics']['quality_gate'], dict)
    assert result['quality_metrics']['quality_gate'] == result['quality_gate_plan']['quality_gate']


def test_should_raise_when_sanitized_generation_is_empty_and_log_removed_meta_lines():
    warnings = []

    def quality_evaluator(content: str) -> dict:
        return {'overall_score': 50.0}

    def quality_gate_plan_builder(metrics: dict, attempt_offset: int) -> dict:
        return {'action': 'retry'}

    try:
        build_generation_candidate_record(
            request=ChapterCandidateRecordRequest(
                full_content='step 1\nstep 2',
                candidate_chunks=['step 1', 'step 2'],
                target_word_count=1200,
                source='chapter',
                generation_label='test-empty-after-sanitize',
                candidate_index=1,
                candidate_offset=0,
                quality_evaluator=quality_evaluator,
                quality_gate_plan_builder=quality_gate_plan_builder,
                generation_path='single_pass',
                attempt_kind='initial_candidate',
            ),
            log_warning_fn=warnings.append,
        )
    except ValueError as exc:
        assert 'generated empty narrative after sanitization' in str(exc)
    else:
        raise AssertionError('expected ValueError when sanitized candidate becomes empty')

    assert len(warnings) == 1
    assert 'Sanitized 2 workflow/meta lines' in warnings[0]
