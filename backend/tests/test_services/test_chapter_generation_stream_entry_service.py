import pytest

from app.services import chapter_generation_stream_entry_service as entry_service


@pytest.mark.asyncio
async def test_should_generate_chapter_content_stream_with_default_wiring(monkeypatch):
    captured = {}

    class FakeRequest:
        class state:
            user_id = 'user-1'

    async def fake_prepare(_db, **kwargs):
        captured['prepare_kwargs'] = kwargs

    def fake_build_dependencies(**kwargs):
        captured['dependency_kwargs'] = kwargs
        return 'deps'

    def fake_event_stream(**kwargs):
        captured['stream_kwargs'] = kwargs

        async def _generator():
            yield 'data: {}\n\n'

        return _generator()

    def fake_create_sse_response(generator):
        captured['generator'] = generator
        return 'sse-response'

    async def fake_db_source(_request):
        class FakeDb:
            async def close(self):
                captured['db_closed'] = True

        yield FakeDb()

    monkeypatch.setattr(entry_service, 'prepare_chapter_generation_stream_request', fake_prepare)
    monkeypatch.setattr(entry_service, 'build_default_chapter_generation_stream_dependencies', fake_build_dependencies)
    monkeypatch.setattr(entry_service, 'build_chapter_generation_event_stream', fake_event_stream)
    monkeypatch.setattr(entry_service, 'create_sse_response', fake_create_sse_response)

    result = await entry_service.generate_chapter_content_stream_with_default_wiring(
        chapter_id='chapter-1',
        request=FakeRequest(),
        background_tasks=object(),
        generate_request=type('Req', (), {
            'style_id': 7,
            'target_word_count': 1800,
            'enable_analysis': True,
            'model': 'gpt-test',
            'narrative_perspective': 'first_person',
        })(),
        user_ai_service=object(),
        get_db_fn=fake_db_source,
        check_prerequisites_fn='check-fn',
        cancel_outline_postprocess_tasks_fn='cancel-fn',
        candidate_generator_fn='candidate-fn',
        candidate_rerank_limit=4,
        one_to_one_builder_cls='one-builder',
        one_to_many_builder_cls='many-builder',
        build_runtime_system_prompt_fn='system-prompt-fn',
        detect_style_profile_fn='detect-style-fn',
        resolve_generation_temperature_fn='temperature-fn',
        compute_story_quality_metrics_fn='metrics-fn',
        resolve_quality_gate_execution_plan_fn='quality-plan-fn',
        analyze_chapter_background_fn='analysis-fn',
        heartbeat_interval_seconds=0.25,
    )

    assert result == 'sse-response'
    assert captured['prepare_kwargs']['chapter_id'] == 'chapter-1'
    assert captured['prepare_kwargs']['check_prerequisites_fn'] == 'check-fn'
    assert captured['dependency_kwargs']['cancel_outline_postprocess_tasks_fn'] == 'cancel-fn'
    assert captured['dependency_kwargs']['candidate_generator_fn'] == 'candidate-fn'
    assert captured['dependency_kwargs']['candidate_rerank_limit'] == 4
    assert captured['dependency_kwargs']['one_to_one_builder_cls'] == 'one-builder'
    assert captured['dependency_kwargs']['one_to_many_builder_cls'] == 'many-builder'
    assert captured['dependency_kwargs']['build_runtime_system_prompt_fn'] == 'system-prompt-fn'
    assert captured['dependency_kwargs']['detect_style_profile_fn'] == 'detect-style-fn'
    assert captured['dependency_kwargs']['resolve_generation_temperature_fn'] == 'temperature-fn'
    assert captured['dependency_kwargs']['compute_story_quality_metrics_fn'] == 'metrics-fn'
    assert captured['dependency_kwargs']['resolve_quality_gate_execution_plan_fn'] == 'quality-plan-fn'
    assert captured['dependency_kwargs']['analyze_chapter_background_fn'] == 'analysis-fn'
    assert captured['stream_kwargs']['chapter_id'] == 'chapter-1'
    assert captured['stream_kwargs']['current_user_id'] == 'user-1'
    assert captured['stream_kwargs']['target_word_count'] == 1800
    assert captured['stream_kwargs']['enable_analysis'] is True
    assert captured['stream_kwargs']['heartbeat_interval_seconds'] == 0.25
    assert captured['stream_kwargs']['custom_model'] == 'gpt-test'
    assert captured['stream_kwargs']['temp_narrative_perspective'] == 'first_person'
    assert captured['stream_kwargs']['style_id'] == 7
    assert captured['stream_kwargs']['dependencies'] == 'deps'
    assert captured['db_closed'] is True
