import pytest

from tests.test_support import (
    single_generation_stream_entry_test_adapter as stream_entry_service,
)


@pytest.mark.asyncio
async def test_should_prepare_chapter_generation_stream_request():
    chapter = type(
        'Chapter',
        (),
        {
            'id': 'chapter-1',
            'chapter_number': 2,
            'title': 'Chapter',
            'content': 'current',
            'project_id': 'project-1',
        },
    )()
    previous_chapter = type(
        'Chapter',
        (),
        {
            'id': 'chapter-0',
            'chapter_number': 1,
            'title': 'Previous',
            'content': 'previous content',
        },
    )()

    async def fake_check_prerequisites(db_session, current_chapter):
        assert current_chapter is chapter
        return True, '', [previous_chapter]

    async def fake_load_chapter(_db_session, chapter_id):
        assert chapter_id == 'chapter-1'
        return chapter

    result = await stream_entry_service.prepare_chapter_generation_stream_request(
        object(),
        chapter_id='chapter-1',
        check_prerequisites_fn=fake_check_prerequisites,
        load_chapter_fn=fake_load_chapter,
    )

    assert result.chapter is chapter
    assert result.previous_chapters_data == [
        {
            'id': 'chapter-0',
            'chapter_number': 1,
            'title': 'Previous',
            'content': 'previous content',
        }
    ]


@pytest.mark.asyncio
async def test_should_raise_when_prepare_stream_request_chapter_missing():
    async def fake_check_prerequisites(*_args, **_kwargs):
        raise AssertionError('should not be called')

    async def fake_load_chapter(_db_session, chapter_id):
        assert chapter_id == 'missing'
        return None

    with pytest.raises(ValueError, match='章节不存在'):
        await stream_entry_service.prepare_chapter_generation_stream_request(
            object(),
            chapter_id='missing',
            check_prerequisites_fn=fake_check_prerequisites,
            load_chapter_fn=fake_load_chapter,
        )


@pytest.mark.asyncio
async def test_should_raise_when_prepare_stream_request_prerequisites_fail():
    chapter = type(
        'Chapter',
        (),
        {
            'id': 'chapter-1',
            'chapter_number': 2,
            'title': 'Chapter',
            'content': 'current',
            'project_id': 'project-1',
        },
    )()

    async def fake_check_prerequisites(_db_session, _chapter):
        return False, '前置章节尚未完成', []

    async def fake_load_chapter(_db_session, chapter_id):
        assert chapter_id == 'chapter-1'
        return chapter

    with pytest.raises(RuntimeError, match='前置章节尚未完成'):
        await stream_entry_service.prepare_chapter_generation_stream_request(
            object(),
            chapter_id='chapter-1',
            check_prerequisites_fn=fake_check_prerequisites,
            load_chapter_fn=fake_load_chapter,
        )


@pytest.mark.asyncio
async def test_should_generate_chapter_content_stream_with_explicit_wiring():
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

    result = await stream_entry_service.generate_chapter_content_stream_with_explicit_wiring(
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
        build_default_stream_dependencies_fn=fake_build_dependencies,
        prepare_stream_request_fn=fake_prepare,
        build_event_stream_fn=fake_event_stream,
        create_sse_response_fn=fake_create_sse_response,
        cancel_outline_postprocess_tasks_fn='cancel-fn',
        candidate_generator_fn='candidate-fn',
        candidate_rerank_limit=4,
        one_to_one_builder_cls='one-builder',
        one_to_many_builder_cls='many-builder',
        get_template_fn='get-template-fn',
        format_prompt_fn='format-prompt-fn',
        apply_style_to_prompt_fn='apply-style-fn',
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
    assert captured['dependency_kwargs']['get_template_fn'] == 'get-template-fn'
    assert captured['dependency_kwargs']['format_prompt_fn'] == 'format-prompt-fn'
    assert captured['dependency_kwargs']['apply_style_to_prompt_fn'] == 'apply-style-fn'
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


@pytest.mark.asyncio
async def test_should_generate_chapter_content_stream_with_default_wiring(monkeypatch):
    captured = {}

    async def fake_generate_with_explicit_wiring(**kwargs):
        captured.update(kwargs)
        return 'stream-response'

    monkeypatch.setattr(
        stream_entry_service,
        'generate_chapter_content_stream_with_explicit_wiring',
        fake_generate_with_explicit_wiring,
    )
    monkeypatch.setattr(
        stream_entry_service,
        'prepare_chapter_generation_stream_request',
        'prepare-fn',
    )
    monkeypatch.setattr(
        'tests.test_support.utils.sse_response.create_sse_response',
        'sse-fn',
    )
    monkeypatch.setattr(stream_entry_service, 'CHAPTER_CANDIDATE_RERANK_LIMIT', 4)
    monkeypatch.setattr(stream_entry_service, 'CHAPTER_STREAM_HEARTBEAT_INTERVAL_SECONDS', 0.25)
    monkeypatch.setattr(stream_entry_service, 'OneToOneContextBuilder', 'one-builder')
    monkeypatch.setattr(stream_entry_service, 'OneToManyContextBuilder', 'many-builder')
    monkeypatch.setattr(stream_entry_service, '_generate_best_ranked_candidate', 'candidate-fn')
    monkeypatch.setattr(stream_entry_service, 'build_chapter_generation_event_stream', 'event-stream-fn')
    monkeypatch.setattr(stream_entry_service, 'build_chapter_runtime_system_prompt', 'system-prompt-fn')
    monkeypatch.setattr(stream_entry_service, 'build_default_chapter_generation_stream_dependencies', 'deps-fn')
    monkeypatch.setattr(stream_entry_service, 'check_chapter_generation_prerequisites', 'check-fn')
    monkeypatch.setattr(stream_entry_service, 'compute_story_quality_metrics', 'metrics-fn')
    monkeypatch.setattr(stream_entry_service, 'detect_style_profile', 'detect-style-fn')
    monkeypatch.setattr(stream_entry_service, 'execute_chapter_analysis_background', 'analysis-fn')
    monkeypatch.setattr(stream_entry_service, 'format_prompt', 'format-prompt-fn')
    monkeypatch.setattr(stream_entry_service, 'get_db', 'db-fn')
    monkeypatch.setattr(stream_entry_service, 'get_template', 'get-template-fn')
    monkeypatch.setattr(stream_entry_service, 'resolve_generation_temperature', 'temperature-fn')
    monkeypatch.setattr(stream_entry_service, 'resolve_quality_gate_execution_plan', 'quality-plan-fn')
    monkeypatch.setattr(stream_entry_service, 'apply_style_to_prompt', 'apply-style-fn')
    monkeypatch.setattr(
        'tests.test_support.outlines_route_test_adapter.cancel_outline_postprocess_tasks',
        'cancel-fn',
    )

    result = await stream_entry_service.generate_chapter_content_stream_with_default_wiring(
        chapter_id='chapter-1',
        request='request',
        background_tasks='bg',
        generate_request='generate-request',
        user_ai_service='ai-service',
    )

    assert result == 'stream-response'
    assert captured['chapter_id'] == 'chapter-1'
    assert captured['request'] == 'request'
    assert captured['background_tasks'] == 'bg'
    assert captured['generate_request'] == 'generate-request'
    assert captured['user_ai_service'] == 'ai-service'
    assert captured['get_db_fn'] == 'db-fn'
    assert captured['check_prerequisites_fn'] == 'check-fn'
    assert captured['build_default_stream_dependencies_fn'] == 'deps-fn'
    assert captured['prepare_stream_request_fn'] == 'prepare-fn'
    assert captured['build_event_stream_fn'] == 'event-stream-fn'
    assert captured['create_sse_response_fn'] == 'sse-fn'
    assert captured['cancel_outline_postprocess_tasks_fn'] == 'cancel-fn'
    assert captured['candidate_generator_fn'] == 'candidate-fn'
    assert captured['candidate_rerank_limit'] == 4
    assert captured['heartbeat_interval_seconds'] == 0.25
