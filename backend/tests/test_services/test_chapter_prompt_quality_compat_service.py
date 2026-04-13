from types import SimpleNamespace

from app.services import chapter_prompt_quality_compat_service as compat_service


def test_should_delegate_compute_story_quality_metrics(monkeypatch):
    captured = {}

    def fake_service(**kwargs):
        captured.update(kwargs)
        return {'overall_score': 88.0}

    monkeypatch.setattr(compat_service, '_compute_story_quality_metrics_service', fake_service)

    result = compat_service.compute_story_quality_metrics(
        content='story',
        chapter_outline='outline',
        world_rules='rules',
        quality_runtime_context={'chapter_number': 1},
    )

    assert result == {'overall_score': 88.0}
    assert captured['content'] == 'story'
    assert captured['chapter_outline'] == 'outline'
    assert captured['world_rules'] == 'rules'
    assert captured['quality_runtime_context'] == {'chapter_number': 1}


def test_should_delegate_prompt_runtime_helpers(monkeypatch):
    captured = {}

    def fake_detect(**kwargs):
        captured['detect'] = kwargs
        return 'low_ai_serial'

    def fake_build(**kwargs):
        captured['build'] = kwargs
        return 'prompt'

    monkeypatch.setattr(compat_service, '_detect_style_profile_service', fake_detect)
    monkeypatch.setattr(compat_service, '_build_chapter_runtime_system_prompt_service', fake_build)

    profile = compat_service.detect_style_profile('serial', 'preset', 'content')
    prompt = compat_service.build_chapter_runtime_system_prompt(
        project=SimpleNamespace(),
        style_content='style',
        chapter_outline='outline',
        previous_summary='summary',
        style_name='serial',
        style_preset_id='preset',
        target_word_count=1200,
        story_runtime_contract={'guardrails': True},
    )

    assert profile == 'low_ai_serial'
    assert prompt == 'prompt'
    assert captured['detect']['style_name'] == 'serial'
    assert captured['build']['style_content'] == 'style'
    assert captured['build']['chapter_outline'] == 'outline'
    assert captured['build']['target_word_count'] == 1200


def test_should_delegate_resolve_generation_temperature(monkeypatch):
    monkeypatch.setattr(
        compat_service,
        '_resolve_generation_temperature_service',
        lambda profile: 0.66 if profile == 'x' else 0.1,
    )

    assert compat_service.resolve_generation_temperature('x') == 0.66
