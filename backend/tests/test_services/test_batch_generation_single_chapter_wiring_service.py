from asyncio import Lock

import pytest

from app.services import batch_generation_single_chapter_wiring_service as wiring_service


def test_should_build_default_batch_generation_single_chapter_dependencies(monkeypatch):
    captured = {}
    expected = object()

    def fake_build_dependencies(**kwargs):
        captured.update(kwargs)
        return expected

    def fake_publish_task_stream_event(*_args, **_kwargs):
        return None

    def fake_resolve_quality_profile(*_args, **_kwargs):
        return None

    candidate_generator = object()

    monkeypatch.setattr(
        wiring_service,
        'build_batch_generation_single_chapter_dependencies',
        fake_build_dependencies,
    )

    result = wiring_service.build_default_batch_generation_single_chapter_dependencies(
        candidate_generator_fn=candidate_generator,
        default_candidate_limit=4,
        heartbeat_interval_seconds=0.25,
        publish_task_stream_event_fn=fake_publish_task_stream_event,
        resolve_quality_profile_fn=fake_resolve_quality_profile,
    )

    assert result is expected
    assert captured['candidate_generator_fn'] is candidate_generator
    assert captured['default_candidate_limit'] == 4
    assert captured['heartbeat_interval_seconds'] == 0.25
    assert captured['publish_task_stream_event_fn'] is fake_publish_task_stream_event
    assert captured['resolve_quality_profile_fn'] is fake_resolve_quality_profile
    assert captured['resolve_batch_generation_chapter_runtime_fn'] is wiring_service.resolve_batch_generation_chapter_runtime
    assert captured['build_generation_runtime_bundle_fn'] is wiring_service.build_chapter_generation_runtime_bundle
    assert captured['build_story_packet_fn'] is wiring_service.build_story_generation_packet_with_project_continuity
    assert captured['clone_quality_profile_fn'] is wiring_service.clone_chapter_quality_profile
    assert captured['build_outline_structure_runtime_sources_fn'] is wiring_service.build_outline_structure_runtime_sources
    assert captured['execute_prompt_stage_fn'] is wiring_service.execute_batch_generation_prompt_stage
    assert captured['get_template_fn'].__qualname__ == wiring_service.PromptService.get_template.__qualname__
    assert captured['format_prompt_fn'] is wiring_service.PromptService.format_prompt
    assert captured['apply_style_to_prompt_fn'] is wiring_service.WritingStyleManager.apply_style_to_prompt
    assert captured['build_runtime_system_prompt_fn'] is wiring_service.build_chapter_runtime_system_prompt
    assert captured['calculate_max_tokens_fn'] is wiring_service._calculate_chapter_generation_max_tokens
    assert captured['build_request_options_fn'] is wiring_service._build_chapter_generation_request_options
    assert captured['detect_style_profile_fn'] is wiring_service.detect_style_profile
    assert captured['resolve_generation_temperature_fn'] is wiring_service.resolve_generation_temperature
    assert captured['execute_generation_stage_fn'] is wiring_service.execute_batch_generation_generation_stage
    assert captured['build_quality_runtime_context_fn'] is wiring_service.build_chapter_quality_runtime_context
    assert captured['compute_story_quality_metrics_fn'] is wiring_service.compute_story_quality_metrics
    assert captured['resolve_quality_gate_execution_plan_fn'] is wiring_service.resolve_quality_gate_execution_plan
    assert captured['attach_story_runtime_contract_fn'] is wiring_service.attach_story_runtime_contract
    assert captured['memory_service'] is wiring_service._memory_service
    assert captured['foreshadow_service'] is wiring_service._foreshadow_service


@pytest.mark.asyncio
async def test_should_generate_single_chapter_for_batch_with_default_wiring(monkeypatch):
    captured = {}

    def fake_build_request(**kwargs):
        captured['request_kwargs'] = kwargs
        return 'request-object'

    def fake_build_default_dependencies(**kwargs):
        captured['dependency_kwargs'] = kwargs
        return 'dependency-object'

    async def fake_workflow(**kwargs):
        captured['workflow_kwargs'] = kwargs
        return {'ok': True}

    monkeypatch.setattr(
        wiring_service,
        'build_batch_generation_single_chapter_request',
        fake_build_request,
    )
    monkeypatch.setattr(
        wiring_service,
        'build_default_batch_generation_single_chapter_dependencies',
        fake_build_default_dependencies,
    )
    monkeypatch.setattr(
        wiring_service,
        'generate_single_chapter_for_batch_workflow',
        fake_workflow,
    )

    chapter = object()
    db_session = object()
    ai_service = object()
    candidate_generator = object()
    result = await wiring_service.generate_single_chapter_for_batch_with_default_wiring(
        db_session=db_session,
        chapter=chapter,
        user_id='user-1',
        style_id=None,
        target_word_count=1800,
        ai_service=ai_service,
        write_lock=Lock(),
        story_creation_brief='brief',
        retry_count=1,
        max_retries=2,
        candidate_generator_fn=candidate_generator,
        default_candidate_limit=6,
        heartbeat_interval_seconds=0.5,
    )

    assert result == {'ok': True}
    assert captured['request_kwargs']['db_session'] is db_session
    assert captured['request_kwargs']['chapter'] is chapter
    assert captured['request_kwargs']['user_id'] == 'user-1'
    assert captured['request_kwargs']['target_word_count'] == 1800
    assert captured['request_kwargs']['ai_service'] is ai_service
    assert captured['request_kwargs']['story_creation_brief'] == 'brief'
    assert captured['request_kwargs']['retry_count'] == 1
    assert captured['request_kwargs']['max_retries'] == 2
    assert captured['dependency_kwargs']['candidate_generator_fn'] is candidate_generator
    assert captured['dependency_kwargs']['default_candidate_limit'] == 6
    assert captured['dependency_kwargs']['heartbeat_interval_seconds'] == 0.5
    assert captured['dependency_kwargs']['chapter_web_research_service'] is wiring_service._chapter_web_research_service
    assert captured['dependency_kwargs']['publish_task_stream_event_fn'] is wiring_service._publish_task_stream_event
    assert captured['dependency_kwargs']['resolve_quality_profile_fn'] is wiring_service.resolve_chapter_quality_profile
    assert captured['dependency_kwargs']['one_to_one_builder_cls'] is wiring_service.OneToOneContextBuilder
    assert captured['dependency_kwargs']['one_to_many_builder_cls'] is wiring_service.OneToManyContextBuilder
    assert captured['dependency_kwargs']['get_template_fn'].__qualname__ == wiring_service.PromptService.get_template.__qualname__
    assert captured['dependency_kwargs']['format_prompt_fn'] is wiring_service.PromptService.format_prompt
    assert captured['dependency_kwargs']['build_runtime_system_prompt_fn'] is wiring_service.build_chapter_runtime_system_prompt
    assert captured['dependency_kwargs']['compute_story_quality_metrics_fn'] is wiring_service.compute_story_quality_metrics
    assert captured['dependency_kwargs']['resolve_quality_gate_execution_plan_fn'] is wiring_service.resolve_quality_gate_execution_plan
    assert captured['workflow_kwargs'] == {
        'request': 'request-object',
        'dependencies': 'dependency-object',
    }
