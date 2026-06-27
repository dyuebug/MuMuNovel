import pytest
from tests.test_support import (
    single_generation_stream_orchestration_test_adapter as stream_execution_service,
)


def test_should_build_chapter_generation_stream_request_payload():
    runtime_context = stream_execution_service.ChapterGenerationStreamRuntimeContext(
        chapter=object(),
        project=object(),
        outline=None,
        outline_mode='one-to-many',
        quality_profile={},
        story_packet=object(),
        generation_guidance=None,
        story_repair_state={},
        story_repair_payload=None,
        resolved_style_id=7,
        style_content='style-content',
        style_name='style-name',
        style_preset_id='preset-1',
    )
    built_context = stream_execution_service.ChapterGenerationStreamBuiltContext(
        chapter_context=type(
            'ChapterContext',
            (),
            {
                'chapter_outline': 'outline',
                'previous_chapter_summary': 'summary',
            },
        )(),
        generation_intent={'intent': True},
        prompt_quality_kwargs={'quality': 'high'},
        story_runtime_contract={'runtime': True},
    )
    stream_prompt = stream_execution_service.ChapterGenerationStreamPrompt(
        chapter_perspective='第一人称',
        base_prompt='base prompt',
        prompt='styled prompt',
    )
    captured = {}

    def fake_build_runtime_system_prompt(**kwargs):
        captured['system_prompt_kwargs'] = kwargs
        return 'system prompt'

    def fake_build_request_options(ai_service):
        captured['request_options_ai_service'] = ai_service
        return {'transport_max_retries': 2}

    def fake_detect_style_profile(**kwargs):
        captured['style_profile_kwargs'] = kwargs
        return 'stylized'

    def fake_resolve_generation_temperature(style_profile):
        captured['temperature_style_profile'] = style_profile
        return 0.72

    payload = stream_execution_service.build_chapter_generation_stream_request_payload(
        runtime_context=runtime_context,
        built_context=built_context,
        stream_prompt=stream_prompt,
        project=type('Project', (), {})(),
        target_word_count=2400,
        enable_mcp=True,
        custom_model='gpt-test',
        ai_service='ai-service',
        build_runtime_system_prompt_fn=fake_build_runtime_system_prompt,
        calculate_max_tokens_fn=lambda count: count // 2,
        build_request_options_fn=fake_build_request_options,
        detect_style_profile_fn=fake_detect_style_profile,
        resolve_generation_temperature_fn=fake_resolve_generation_temperature,
    )

    assert payload.system_prompt == 'system prompt'
    assert payload.max_tokens == 1200
    assert payload.generate_kwargs['prompt'] == 'styled prompt'
    assert payload.generate_kwargs['system_prompt'] == 'system prompt'
    assert payload.generate_kwargs['auto_mcp'] is True
    assert payload.generate_kwargs['max_tokens'] == 1200
    assert payload.generate_kwargs['temperature'] == 0.72
    assert payload.generate_kwargs['request_options'] == {'transport_max_retries': 2}
    assert payload.generate_kwargs['model'] == 'gpt-test'
    assert captured['system_prompt_kwargs']['style_content'] == 'style-content'
    assert captured['system_prompt_kwargs']['story_runtime_contract'] == {'runtime': True}
    assert captured['request_options_ai_service'] == 'ai-service'
    assert captured['style_profile_kwargs']['style_name'] == 'style-name'
    assert captured['temperature_style_profile'] == 'stylized'


@pytest.mark.asyncio
async def test_should_resolve_default_quality_profile_from_quality_context_owner(monkeypatch):
    captured = {}

    async def fake_resolve_chapter_quality_profile(*args, **kwargs):
        captured["args"] = args
        captured["kwargs"] = kwargs
        return {"resolved_style_id": 9}

    monkeypatch.setattr(
        "tests.test_support.chapter_prompt_quality_test_support.resolve_chapter_quality_profile",
        fake_resolve_chapter_quality_profile,
    )

    result = await stream_execution_service._default_resolve_quality_profile(
        "db-session",
        user_id="user-1",
        project="project-1",
    )

    assert result == {"resolved_style_id": 9}
    assert captured["args"] == ()
    assert captured["kwargs"] == {
        "db_session": "db-session",
        "user_id": "user-1",
        "project": "project-1",
    }


@pytest.mark.asyncio
async def test_should_build_default_story_packet_from_quality_context_owner(monkeypatch):
    captured = {}

    async def fake_build_story_generation_packet_with_project_continuity(*args, **kwargs):
        captured["args"] = args
        captured["kwargs"] = kwargs
        return {"guidance": "packet"}

    monkeypatch.setattr(
        "tests.test_support.story_continuity_ledger_test_support.build_story_generation_packet_with_project_continuity",
        fake_build_story_generation_packet_with_project_continuity,
    )

    result = await stream_execution_service._default_build_story_packet(
        "db-session",
        "project-1",
        source="request",
    )

    assert result == {"guidance": "packet"}
    assert captured["args"] == ("db-session", "project-1")
    assert captured["kwargs"] == {
        "source": "request",
    }
