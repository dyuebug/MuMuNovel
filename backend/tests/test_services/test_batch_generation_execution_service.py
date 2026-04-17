import asyncio
from types import SimpleNamespace

import pytest

from app.models.chapter import Chapter
from app.models.project import Project
from app.services import batch_generation_execution_service as batch_execution_service


def test_should_build_batch_generation_request_payload_with_request_options_and_custom_model():
    project = Project(id='project-1', title='??', user_id='user-1')
    chapter_context = SimpleNamespace(
        chapter_outline='????',
        previous_chapter_summary='?????',
    )
    ai_service = SimpleNamespace(api_provider='openai_responses')

    captured_runtime_prompt_kwargs = {}

    def fake_build_runtime_system_prompt(**kwargs):
        captured_runtime_prompt_kwargs.update(kwargs)
        return f"sys:{kwargs['style_name']}:{kwargs['target_word_count']}"

    payload = batch_execution_service.build_batch_generation_request_payload(
        prompt='?????',
        project=project,
        chapter_context=chapter_context,
        style_content='??',
        style_name='??',
        style_preset_id='preset-1',
        target_word_count=1600,
        ai_service=ai_service,
        custom_model='gpt-custom',
        story_runtime_contract={'contract': True},
        research_assets=[
            {
                'title': 'night market',
                'source': 'mock-source',
                'summary': 'used for scene atmosphere and crowd texture.',
                'usage_hint': 'improve environment details',
            }
        ],
        build_runtime_system_prompt_fn=fake_build_runtime_system_prompt,
        calculate_max_tokens_fn=lambda target_word_count: 960,
        build_request_options_fn=lambda _ai_service: {'transport_max_retries': 2},
        detect_style_profile_fn=lambda **kwargs: 'profile-a',
        resolve_generation_temperature_fn=lambda style_profile: 0.75,
    )

    assert payload.system_prompt == 'sys:??:1600'
    assert captured_runtime_prompt_kwargs['web_research_grounding_block']
    assert 'night market' in captured_runtime_prompt_kwargs['web_research_grounding_block']
    assert payload.max_tokens == 960
    assert payload.generate_kwargs == {
        'prompt': '?????',
        'system_prompt': 'sys:??:1600',
        'tool_choice': 'auto',
        'max_tokens': 960,
        'temperature': 0.75,
        'request_options': {'transport_max_retries': 2},
        'model': 'gpt-custom',
    }


def test_should_build_batch_generation_request_payload_without_optional_overrides():
    project = Project(id='project-1', title='??', user_id='user-1')
    chapter_context = SimpleNamespace(
        chapter_outline='????',
        previous_chapter_summary='?????',
    )

    payload = batch_execution_service.build_batch_generation_request_payload(
        prompt='?????',
        project=project,
        chapter_context=chapter_context,
        style_content='',
        style_name='',
        style_preset_id=None,
        target_word_count=1200,
        ai_service=SimpleNamespace(api_provider='openai'),
        custom_model=None,
        story_runtime_contract=None,
        research_assets=None,
        build_runtime_system_prompt_fn=lambda **kwargs: 'system',
        calculate_max_tokens_fn=lambda target_word_count: 800,
        build_request_options_fn=lambda _ai_service: None,
        detect_style_profile_fn=lambda **kwargs: 'profile-b',
        resolve_generation_temperature_fn=lambda style_profile: 0.55,
    )

    assert payload.generate_kwargs == {
        'prompt': '?????',
        'system_prompt': 'system',
        'tool_choice': 'auto',
        'max_tokens': 800,
        'temperature': 0.55,
    }


def test_should_calculate_estimated_time():
    estimated = batch_execution_service.calculate_estimated_time(
        chapter_count=3,
        target_word_count=3000,
        enable_analysis=True,
    )

    assert estimated == 9


class _FakeBackgroundTasks:
    def __init__(self):
        self.calls = []

    def add_task(self, callable_, **kwargs):
        self.calls.append((callable_, kwargs))


def test_should_enqueue_batch_generation_execution():
    background_tasks = _FakeBackgroundTasks()

    def fake_execution(**kwargs):
        return None

    batch_execution_service.enqueue_batch_generation_execution(
        background_tasks,
        fake_execution,
        batch_id='batch-1',
        user_id='user-1',
        ai_service=SimpleNamespace(name='ai'),
        custom_model='gpt-x',
        temp_narrative_perspective='????',
        story_packet=SimpleNamespace(source='packet'),
        base_quality_profile={'style': 'x'},
        enable_web_research=True,
        web_research_query='query',
        story_repair_payload=None,
    )

    assert background_tasks.calls[0][0] is fake_execution
    assert background_tasks.calls[0][1]['batch_id'] == 'batch-1'
    assert background_tasks.calls[0][1]['custom_model'] == 'gpt-x'
    assert background_tasks.calls[0][1]['web_research_query'] == 'query'


def test_should_build_batch_generation_candidate_quality_hooks():
    project = Project(id='project-1', title='??', user_id='user-1', world_rules='??')
    chapter = Chapter(id='chapter-1', project_id='project-1', chapter_number=2, title='???')
    chapter_context = SimpleNamespace(chapter_outline='????')

    hooks = batch_execution_service.build_batch_generation_candidate_quality_hooks(
        story_packet=SimpleNamespace(packet='story'),
        project=project,
        chapter=chapter,
        chapter_context=chapter_context,
        target_word_count=1800,
        generation_intent={'intent': '??'},
        retry_count=1,
        max_retries=3,
        current_story_repair_payload={'repair': True},
        build_quality_runtime_context_fn=lambda **kwargs: {
            'target_word_count': kwargs['target_word_count'],
            'generation_intent': kwargs['generation_intent'],
        },
        compute_story_quality_metrics_fn=lambda **kwargs: {
            'overall_score': 88,
            'conflict_chain_hit_rate': 72,
            'rule_grounding_hit_rate': 69,
            'runtime_context': kwargs['quality_runtime_context'],
        },
        resolve_quality_gate_execution_plan_fn=lambda metrics, **kwargs: {
            'action': 'retry',
            'metrics': metrics,
            'retry_count': kwargs['retry_count'],
            'max_retries': kwargs['max_retries'],
            'payload': kwargs['current_story_repair_payload'],
            'scope': kwargs['scope'],
        },
    )

    metrics = hooks.quality_evaluator('????')
    plan = hooks.quality_gate_plan_builder({'overall_score': 60}, 0)

    assert metrics['runtime_context'] == {
        'target_word_count': 1800,
        'generation_intent': {'intent': '??'},
    }
    assert plan == {
        'action': 'retry',
        'metrics': {'overall_score': 60},
        'retry_count': 1,
        'max_retries': 3,
        'payload': {'repair': True},
        'scope': 'batch',
    }


def test_should_build_batch_generation_candidate_runtime_state():
    state = batch_execution_service.build_batch_generation_candidate_runtime_state(max_candidates=2)

    assert state == {
        'candidate_total': 2,
        'candidate_count': 2,
        'candidate_index': 1,
        'current_chars': 0,
        'word_count': 0,
        'chunk_count': 0,
        'generation_path': 'single_pass',
        'attempt_kind': 'initial_candidate',
        'rerank_used': False,
        'word_budget_repair_used': False,
        'winner_candidate_index': None,
    }



@pytest.mark.asyncio
async def test_should_create_batch_generation_candidate_execution():
    async def fake_candidate_generator(**kwargs):
        kwargs['runtime_state']['current_chars'] = 1280
        return {'full_content': '??', 'candidate_count': kwargs['max_candidates']}

    execution = batch_execution_service.create_batch_generation_candidate_execution(
        ai_service=SimpleNamespace(api_provider='openai'),
        generate_kwargs={'prompt': '?????'},
        target_word_count=1500,
        chapter_number=6,
        quality_evaluator=lambda content: {'ok': True},
        quality_gate_plan_builder=lambda metrics, attempt_offset: {'action': 'continue'},
        max_candidates=2,
        candidate_generator_fn=fake_candidate_generator,
    )

    result = await execution.selected_candidate_task

    assert execution.runtime_state['candidate_total'] == 2
    assert result['candidate_count'] == 2


@pytest.mark.asyncio
async def test_should_wait_for_batch_generation_candidate_and_emit_progress(monkeypatch):
    events = []

    async def fake_publish(task_id, payload, db_session=None):
        events.append((task_id, payload))

    async def delayed_result():
        await asyncio.sleep(0.02)
        return {'full_content': '??', 'candidate_count': 2}

    monkeypatch.setattr(batch_execution_service, 'publish_task_stream_event', fake_publish)
    runtime_state = {
        'candidate_total': 2,
        'candidate_count': 2,
        'candidate_index': 2,
        'current_chars': 900,
        'word_count': 900,
        'chunk_count': 3,
        'generation_path': 'rerank',
        'attempt_kind': 'rerank_candidate',
        'rerank_used': True,
        'word_budget_repair_used': False,
        'winner_candidate_index': None,
    }

    selected_candidate = await batch_execution_service.wait_for_batch_generation_candidate(
        selected_candidate_task=asyncio.create_task(delayed_result()),
        runtime_state=runtime_state,
        stream_task_id='task-1',
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=3, title='???'),
        target_word_count=1800,
        heartbeat_interval_seconds=0.01,
        db_session=None,
    )

    assert selected_candidate['full_content'] == '??'
    assert events
    assert events[0][0] == 'task-1'
    assert events[0][1]['type'] == 'progress'
    assert events[0][1]['candidate_index'] == 2
    assert events[0][1]['generation_path'] == 'rerank'


@pytest.mark.asyncio
async def test_should_emit_batch_generation_selected_candidate_events(monkeypatch):
    events = []

    async def fake_publish(task_id, payload, db_session=None):
        events.append((task_id, payload))

    monkeypatch.setattr(batch_execution_service, 'publish_task_stream_event', fake_publish)

    await batch_execution_service.emit_batch_generation_selected_candidate_events(
        stream_task_id='task-1',
        stream_chunks=True,
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=4, title='???'),
        selected_candidate={
            'candidate_index': 2,
            'candidate_count': 3,
            'generation_path': 'rerank',
            'attempt_kind': 'rerank_candidate',
            'rerank_used': True,
            'word_budget_repair_used': False,
            'winner_candidate_index': 2,
            'candidate_chunks': ['??1', '??2'],
        },
        candidate_word_count=1450,
        quality_gate_plan={'action': 'continue'},
        chapter_context_stats={
            'pre_compaction_total_length': 5000,
            'context_budget_limit': 4000,
            'compaction_applied': True,
            'compaction_details': {'removed': 1000},
        },
        db_session=None,
    )

    assert len(events) == 3
    assert events[0][1]['type'] == 'progress'
    assert events[0][1]['winner_candidate_index'] == 2
    assert events[1][1] == {
        'type': 'chunk',
        'chapter_id': 'chapter-1',
        'chapter_number': 4,
        'content': '??1',
    }
    assert events[2][1]['content'] == '??2'



def test_should_build_batch_generation_selected_candidate_result():
    chapter = Chapter(id='chapter-1', project_id='project-1', chapter_number=5, title='???')

    result = batch_execution_service.build_batch_generation_selected_candidate_result(
        chapter=chapter,
        selected_candidate={
            'full_content': '???\n???',
            'word_count': 1234,
            'quality_metrics': {'overall_score': 91},
            'quality_gate_plan': {'action': 'continue'},
            'candidate_count': 2,
            'candidate_index': 2,
        },
        story_runtime_contract={'contract': True},
        attach_story_runtime_contract_fn=lambda metrics, contract: {
            **(metrics or {}),
            'story_runtime_contract': contract,
        },
    )

    assert result == {
        'full_content': '???\n???',
        'word_count': 1234,
        'summary_preview': '??? ???',
        'quality_metrics': {
            'overall_score': 91,
            'story_runtime_contract': {'contract': True},
        },
        'quality_gate_plan': {'action': 'continue'},
        'candidate_count': 2,
        'story_runtime_contract': {'contract': True},
    }



@pytest.mark.asyncio
async def test_should_build_batch_generation_prompt_for_one_to_one_next_with_style():
    template_calls = []
    format_calls = []

    async def fake_get_template(template_key, user_id, db_session):
        template_calls.append(template_key)
        return f'template:{template_key}'

    def fake_format_prompt(template, **kwargs):
        format_calls.append({'template': template, **kwargs})
        return f'formatted:{template}'

    result = await batch_execution_service.build_batch_generation_prompt(
        db_session=None,
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=3, title='???'),
        project=Project(id='project-1', title='??', user_id='user-1', outline_mode='one-to-one'),
        chapter_context=SimpleNamespace(
            chapter_outline='??',
            continuation_point='?????',
            previous_chapter_summary='?????',
            chapter_characters='??A',
            chapter_careers='??A',
            foreshadow_reminders='??A',
            relevant_memories='??A',
            recent_chapters_context='',
        ),
        outline_mode='one-to-one',
        current_user_id='user-1',
        target_word_count=1800,
        temp_narrative_perspective='????',
        previous_summary_context=None,
        prompt_quality_kwargs={'quality_preset': 'plot_drive'},
        style_content='????',
        get_template_fn=fake_get_template,
        format_prompt_fn=fake_format_prompt,
        apply_style_to_prompt_fn=lambda prompt, style: f'styled::{style}::{prompt}',
    )

    assert template_calls == ['CHAPTER_GENERATION_ONE_TO_ONE_NEXT']
    assert result.chapter_perspective == '????'
    assert result.base_prompt == 'formatted:template:CHAPTER_GENERATION_ONE_TO_ONE_NEXT'
    assert result.prompt == 'styled::????::formatted:template:CHAPTER_GENERATION_ONE_TO_ONE_NEXT'
    assert format_calls[0]['previous_chapter_content'] == '?????'
    assert format_calls[0]['previous_chapter_summary'] == '?????'
    assert format_calls[0]['narrative_perspective'] == '????'


@pytest.mark.asyncio
async def test_should_build_batch_generation_prompt_for_one_to_many_next_with_previous_summary_fallback():
    template_calls = []
    format_calls = []

    async def fake_get_template(template_key, user_id, db_session):
        template_calls.append(template_key)
        return f'template:{template_key}'

    def fake_format_prompt(template, **kwargs):
        format_calls.append({'template': template, **kwargs})
        return f'formatted:{template}'

    result = await batch_execution_service.build_batch_generation_prompt(
        db_session=None,
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=6, title='???'),
        project=Project(id='project-1', title='??', user_id='user-1', outline_mode='one-to-many'),
        chapter_context=SimpleNamespace(
            chapter_outline='??',
            continuation_point='????',
            previous_chapter_summary='',
            chapter_characters='??A',
            chapter_careers='??A',
            foreshadow_reminders='??A',
            relevant_memories='??A',
            recent_chapters_context='??????',
        ),
        outline_mode='one-to-many',
        current_user_id='user-1',
        target_word_count=2000,
        temp_narrative_perspective=None,
        previous_summary_context='??????',
        prompt_quality_kwargs={'quality_notes': '????'},
        style_content='',
        get_template_fn=fake_get_template,
        format_prompt_fn=fake_format_prompt,
        apply_style_to_prompt_fn=lambda prompt, style: f'styled::{style}::{prompt}',
    )

    assert template_calls == ['CHAPTER_GENERATION_ONE_TO_MANY_NEXT']
    assert result.base_prompt == 'formatted:template:CHAPTER_GENERATION_ONE_TO_MANY_NEXT'
    assert result.prompt == result.base_prompt
    assert format_calls[0]['previous_chapter_summary'] == '??????'
    assert format_calls[0]['recent_chapters_context'] == '??????'



@pytest.mark.asyncio
async def test_should_build_batch_generation_context_for_one_to_many_with_stats_and_outline_sources():
    build_calls = []

    class FakeOneToManyBuilder:
        def __init__(self, *args, **kwargs):
            self.kwargs = kwargs

        async def build(self, **kwargs):
            build_calls.append(kwargs)
            return SimpleNamespace(
                continuation_point='????',
                context_stats={'memory_count': 3, 'total_length': 2048},
            )

    result = await batch_execution_service.build_batch_generation_context(
        db_session=None,
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=4, title='???'),
        project=Project(id='project-1', title='??', user_id='user-1', outline_mode='one-to-many'),
        outline=SimpleNamespace(id='outline-1'),
        outline_mode='one-to-many',
        user_id='user-1',
        target_word_count=1800,
        style_content='????',
        memory_service=object(),
        foreshadow_service=object(),
        one_to_one_builder_cls=lambda **kwargs: None,
        one_to_many_builder_cls=FakeOneToManyBuilder,
        build_outline_structure_runtime_sources_fn=lambda outline: {'outline_id': outline.id},
    )

    assert build_calls[0]['style_content'] == '????'
    assert build_calls[0]['target_word_count'] == 1800
    assert result.chapter_context.continuation_point == '????'
    assert result.outline_runtime_sources == {'outline_id': 'outline-1'}


@pytest.mark.asyncio
async def test_should_build_batch_generation_context_for_one_to_one_without_style_content():
    build_calls = []

    class FakeOneToOneBuilder:
        def __init__(self, *args, **kwargs):
            self.kwargs = kwargs

        async def build(self, **kwargs):
            build_calls.append(kwargs)
            return SimpleNamespace(
                continuation_point='',
                context_stats={'memory_count': 1, 'total_length': 512},
            )

    result = await batch_execution_service.build_batch_generation_context(
        db_session=None,
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=1, title='???'),
        project=Project(id='project-1', title='??', user_id='user-1', outline_mode='one-to-one'),
        outline=SimpleNamespace(id='outline-1'),
        outline_mode='one-to-one',
        user_id='user-1',
        target_word_count=1200,
        style_content='????',
        memory_service=object(),
        foreshadow_service=object(),
        one_to_one_builder_cls=FakeOneToOneBuilder,
        one_to_many_builder_cls=lambda **kwargs: None,
        build_outline_structure_runtime_sources_fn=lambda outline: {'outline_id': outline.id},
    )

    assert 'style_content' not in build_calls[0]
    assert build_calls[0]['target_word_count'] == 1200
    assert result.outline_runtime_sources == {'outline_id': 'outline-1'}


@pytest.mark.asyncio
async def test_should_prepare_batch_generation_runtime_with_base_quality_profile():
    clone_calls = []
    bundle_calls = []

    def fake_clone_quality_profile(profile, **kwargs):
        clone_calls.append(kwargs)
        return {
            **profile,
            'cloned': kwargs['external_assets'],
        }

    async def fail_if_called(**kwargs):
        raise AssertionError('resolve_quality_profile_fn should not be called')

    def fake_build_generation_runtime_bundle(**kwargs):
        bundle_calls.append(kwargs)
        return SimpleNamespace(
            generation_intent={'intent': 'batch'},
            prompt_quality_kwargs={'quality': 'strict'},
            story_runtime_contract={'contract': True},
        )

    result = await batch_execution_service.prepare_batch_generation_runtime(
        db_session=None,
        user_id='user-1',
        project=Project(id='project-1', title='??', user_id='user-1'),
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=3, title='???'),
        target_word_count=1800,
        style_id=5,
        story_packet=SimpleNamespace(guidance={'mode': 'hook'}, source='prebuilt'),
        base_quality_profile={
            'resolved_style_id': 7,
            'style_content': '????',
            'style_name': '??',
            'style_preset_id': 'preset-7',
        },
        research_assets=['asset-1'],
        creative_mode='fast',
        story_focus='??',
        plot_stage='??',
        story_creation_brief='brief',
        quality_preset='balanced',
        quality_notes='notes',
        chapter_context=SimpleNamespace(outline='ctx'),
        outline_runtime_sources={'outline_id': 'outline-1'},
        story_repair_state={'repair': True},
        story_repair_payload=SimpleNamespace(payload='repair'),
        active_story_repair_snapshot={'snapshot': True},
        build_story_packet_fn=lambda *args, **kwargs: (_ for _ in ()).throw(AssertionError('build_story_packet_fn should not be called')),
        clone_quality_profile_fn=fake_clone_quality_profile,
        resolve_quality_profile_fn=fail_if_called,
        build_generation_runtime_bundle_fn=fake_build_generation_runtime_bundle,
    )

    assert result.effective_story_packet.source == 'prebuilt'
    assert result.generation_guidance == {'mode': 'hook'}
    assert result.quality_profile['cloned'] == ['asset-1']
    assert result.style_id == 7
    assert result.style_content == '????'
    assert clone_calls == [{'external_assets': ['asset-1'], 'reference_assets': ['asset-1']}]
    assert bundle_calls[0]['quality_profile']['cloned'] == ['asset-1']
    assert bundle_calls[0]['character_focus_source'] == {'outline_id': 'outline-1'}
    assert result.generation_runtime.story_runtime_contract == {'contract': True}


@pytest.mark.asyncio
async def test_should_prepare_batch_generation_runtime_by_building_story_packet_and_resolving_quality_profile():
    packet_calls = []
    resolve_calls = []
    bundle_calls = []

    async def fake_build_story_packet(db_session, project, **kwargs):
        packet_calls.append({'project_id': project.id, **kwargs})
        return SimpleNamespace(guidance={'mode': 'resolved'}, source='built')

    def fail_if_called(profile, **kwargs):
        raise AssertionError('clone_quality_profile_fn should not be called')

    async def fake_resolve_quality_profile(**kwargs):
        resolve_calls.append(kwargs)
        return {
            'resolved_style_id': 11,
            'style_content': '????',
            'style_name': '??',
            'style_preset_id': 'preset-11',
        }

    def fake_build_generation_runtime_bundle(**kwargs):
        bundle_calls.append(kwargs)
        return SimpleNamespace(
            generation_intent={'intent': 'resolved'},
            prompt_quality_kwargs={'quality': 'balanced'},
            story_runtime_contract={'contract': 'runtime'},
        )

    result = await batch_execution_service.prepare_batch_generation_runtime(
        db_session=None,
        user_id='user-1',
        project=Project(id='project-1', title='??', user_id='user-1'),
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=5, title='???'),
        target_word_count=2200,
        style_id=None,
        story_packet=None,
        base_quality_profile=None,
        research_assets=['asset-x'],
        creative_mode='steady',
        story_focus='??',
        plot_stage='???',
        story_creation_brief='summary',
        quality_preset='cinematic',
        quality_notes='keep pace',
        chapter_context=SimpleNamespace(outline='ctx'),
        outline_runtime_sources=None,
        story_repair_state=None,
        story_repair_payload=None,
        active_story_repair_snapshot=None,
        build_story_packet_fn=fake_build_story_packet,
        clone_quality_profile_fn=fail_if_called,
        resolve_quality_profile_fn=fake_resolve_quality_profile,
        build_generation_runtime_bundle_fn=fake_build_generation_runtime_bundle,
    )

    assert packet_calls[0]['source_label'] == 'batch-single-chapter-generate'
    assert packet_calls[0]['project_id'] == 'project-1'
    assert packet_calls[0]['quality_notes'] == 'keep pace'
    assert resolve_calls[0]['external_assets'] == ['asset-x']
    assert resolve_calls[0]['reference_assets'] == ['asset-x']
    assert resolve_calls[0]['prefer_project_default_style'] is True
    assert result.effective_story_packet.source == 'built'
    assert result.generation_guidance == {'mode': 'resolved'}
    assert result.style_name == '??'
    assert result.style_id == 11
    assert bundle_calls[0]['story_packet'].source == 'built'
    assert result.generation_runtime.prompt_quality_kwargs == {'quality': 'balanced'}




@pytest.mark.asyncio
async def test_should_resolve_batch_generation_chapter_runtime():
    prepare_calls = []
    build_calls = []
    finalize_calls = []
    runtime_preparation = batch_execution_service.BatchGenerationRuntimePreparation(
        effective_story_packet=SimpleNamespace(source='prepared'),
        generation_guidance={'mode': 'prepared'},
        quality_profile={'style_name': '??'},
        style_id=9,
        style_content='style-content',
        style_name='??',
        style_preset_id='preset-9',
        generation_runtime=None,
    )
    built_context = batch_execution_service.BatchGenerationBuiltContext(
        chapter_context=SimpleNamespace(chapter_outline='outline'),
        outline_runtime_sources={'outline_id': 'outline-1'},
    )
    resolved_runtime = batch_execution_service.BatchGenerationResolvedRuntime(
        generation_runtime=SimpleNamespace(runtime='ok'),
        generation_intent={'intent': 'batch'},
        prompt_quality_kwargs={'quality': 'strict'},
        story_runtime_contract={'contract': True},
    )

    async def fake_prepare_runtime(**kwargs):
        prepare_calls.append(kwargs)
        return runtime_preparation

    async def fake_build_context(**kwargs):
        build_calls.append(kwargs)
        return built_context

    def fake_finalize_runtime(**kwargs):
        finalize_calls.append(kwargs)
        return resolved_runtime

    result = await batch_execution_service.resolve_batch_generation_chapter_runtime(
        db_session=None,
        user_id='user-1',
        project=Project(id='project-1', title='??', user_id='user-1'),
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=2, title='???'),
        outline=SimpleNamespace(id='outline-1'),
        outline_mode='one-to-many',
        target_word_count=2000,
        style_id=5,
        story_packet=None,
        base_quality_profile=None,
        research_assets=['asset-1'],
        creative_mode='balanced',
        story_focus='focus',
        plot_stage='mid',
        story_creation_brief='brief',
        quality_preset='cinematic',
        quality_notes='keep pace',
        memory_service='memory-service',
        foreshadow_service='foreshadow-service',
        story_repair_state={'repair': True},
        story_repair_payload=SimpleNamespace(payload='repair'),
        active_story_repair_snapshot={'snapshot': 'yes'},
        build_generation_runtime_bundle_fn=lambda **kwargs: None,
        build_story_packet_fn=lambda **kwargs: None,
        clone_quality_profile_fn=lambda *args, **kwargs: {},
        resolve_quality_profile_fn=lambda **kwargs: None,
        one_to_one_builder_cls=lambda **kwargs: None,
        one_to_many_builder_cls=lambda **kwargs: None,
        build_outline_structure_runtime_sources_fn=lambda outline: {'outline_id': outline.id},
        prepare_runtime_fn=fake_prepare_runtime,
        build_context_fn=fake_build_context,
        finalize_runtime_fn=fake_finalize_runtime,
    )

    assert prepare_calls[0]['research_assets'] == ['asset-1']
    assert build_calls[0]['style_content'] == 'style-content'
    assert finalize_calls[0]['outline_runtime_sources'] == {'outline_id': 'outline-1'}
    assert result.effective_story_packet.source == 'prepared'
    assert result.chapter_context.chapter_outline == 'outline'
    assert result.prompt_quality_kwargs == {'quality': 'strict'}
    assert result.story_runtime_contract == {'contract': True}


def test_should_finalize_batch_generation_runtime():
    bundle_calls = []
    runtime_preparation = batch_execution_service.BatchGenerationRuntimePreparation(
        effective_story_packet=SimpleNamespace(source='prepared'),
        generation_guidance={'mode': 'prepared'},
        quality_profile={'resolved_style_id': 3, 'style_name': '??'},
        style_id=3,
        style_content='????',
        style_name='??',
        style_preset_id='preset-3',
        generation_runtime=None,
    )

    def fake_build_generation_runtime_bundle(**kwargs):
        bundle_calls.append(kwargs)
        return SimpleNamespace(
            generation_intent={'intent': 'batch-runtime'},
            prompt_quality_kwargs={'quality_notes': 'keep tension'},
            story_runtime_contract={'contract': 'story'},
        )

    result = batch_execution_service.finalize_batch_generation_runtime(
        runtime_preparation=runtime_preparation,
        project=Project(id='project-1', title='??', user_id='user-1'),
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=8, title='???'),
        chapter_context=SimpleNamespace(chapter_outline='outline'),
        target_word_count=2600,
        outline_runtime_sources={'outline_id': 'outline-8'},
        story_repair_state={'repair': True},
        story_repair_payload=SimpleNamespace(payload='repair'),
        active_story_repair_snapshot={'snapshot': 'yes'},
        build_generation_runtime_bundle_fn=fake_build_generation_runtime_bundle,
    )

    assert bundle_calls[0]['story_packet'].source == 'prepared'
    assert bundle_calls[0]['quality_profile']['style_name'] == '??'
    assert bundle_calls[0]['character_focus_source'] == {'outline_id': 'outline-8'}
    assert result.generation_runtime.story_runtime_contract == {'contract': 'story'}
    assert result.generation_intent == {'intent': 'batch-runtime'}
    assert result.prompt_quality_kwargs == {'quality_notes': 'keep tension'}
    assert result.story_runtime_contract == {'contract': 'story'}



@pytest.mark.asyncio
async def test_should_execute_batch_generation_candidate_flow_without_stream():
    hook_calls = []
    generator_calls = []
    result_calls = []
    emit_calls = []

    def fake_build_candidate_quality_hooks(**kwargs):
        hook_calls.append(kwargs)
        return batch_execution_service.BatchGenerationCandidateQualityHooks(
            quality_evaluator=lambda content: {'score': len(content)},
            quality_gate_plan_builder=lambda metrics, attempt_offset: {'action': 'continue', 'attempt_offset': attempt_offset},
        )

    async def fake_candidate_generator(**kwargs):
        generator_calls.append(kwargs)
        metrics = kwargs['quality_evaluator']('????')
        gate_plan = kwargs['quality_gate_plan_builder'](metrics, 0)
        return {
            'full_content': '????',
            'word_count': 4,
            'quality_metrics': metrics,
            'quality_gate_plan': gate_plan,
            'candidate_count': kwargs['max_candidates'],
            'candidate_index': 1,
        }

    def fake_build_selected_candidate_result(**kwargs):
        result_calls.append(kwargs)
        return {
            'word_count': 4,
            'quality_gate_plan': {'action': 'continue'},
            'story_runtime_contract': kwargs['story_runtime_contract'],
        }

    async def fake_emit_selected_candidate_events(**kwargs):
        emit_calls.append(kwargs)

    flow_result = await batch_execution_service.execute_batch_generation_candidate_flow(
        stream_task_id=None,
        stream_chunks=False,
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=2, title='???'),
        effective_story_packet=SimpleNamespace(source='prepared'),
        project=Project(id='project-1', title='??', user_id='user-1', world_rules='??'),
        chapter_context=SimpleNamespace(chapter_outline='outline', context_stats={'memory_count': 2}),
        target_word_count=1800,
        generation_intent={'intent': 'batch'},
        current_story_repair_payload=None,
        retry_count=0,
        max_retries=2,
        default_candidate_limit=2,
        ai_service=SimpleNamespace(name='ai'),
        generate_kwargs={'prompt': 'go'},
        story_runtime_contract={'contract': True},
        db_session=None,
        heartbeat_interval_seconds=1.0,
        build_quality_runtime_context_fn=lambda **kwargs: {'runtime': True},
        compute_story_quality_metrics_fn=lambda **kwargs: {'overall_score': 88},
        resolve_quality_gate_execution_plan_fn=lambda *args, **kwargs: {'action': 'continue'},
        candidate_generator_fn=fake_candidate_generator,
        attach_story_runtime_contract_fn=lambda metrics, contract: {'metrics': metrics, 'contract': contract},
        build_candidate_quality_hooks_fn=fake_build_candidate_quality_hooks,
        build_selected_candidate_result_fn=fake_build_selected_candidate_result,
        emit_selected_candidate_events_fn=fake_emit_selected_candidate_events,
    )

    assert hook_calls[0]['retry_count'] == 0
    assert generator_calls[0]['source'] == 'batch'
    assert generator_calls[0]['generation_label'] == 'chapter=2'
    assert generator_calls[0]['max_candidates'] == 2
    assert result_calls[0]['story_runtime_contract'] == {'contract': True}
    assert emit_calls[0]['candidate_word_count'] == 4
    assert flow_result.selected_candidate['full_content'] == '????'
    assert flow_result.selected_candidate_result['story_runtime_contract'] == {'contract': True}



@pytest.mark.asyncio
async def test_should_execute_batch_generation_generation_stage_with_stream_progress():
    publish_calls = []
    execute_calls = []

    async def fake_publish_stream_event(stream_task_id, payload, db_session=None):
        publish_calls.append({
            'stream_task_id': stream_task_id,
            'payload': payload,
            'db_session': db_session,
        })

    async def fake_execute_candidate_flow(**kwargs):
        execute_calls.append(kwargs)
        return batch_execution_service.BatchGenerationCandidateFlowResult(
            selected_candidate={'full_content': '??'},
            selected_candidate_result={'word_count': 12},
        )

    result = await batch_execution_service.execute_batch_generation_generation_stage(
        stream_task_id='task-1',
        stream_chunks=True,
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=4, title='???'),
        effective_story_packet=SimpleNamespace(source='prepared'),
        project=Project(id='project-1', title='??', user_id='user-1'),
        chapter_context=SimpleNamespace(chapter_outline='outline'),
        target_word_count=2000,
        generation_intent={'intent': 'batch'},
        current_story_repair_payload=None,
        retry_count=0,
        max_retries=2,
        default_candidate_limit=2,
        ai_service=SimpleNamespace(name='ai'),
        generate_kwargs={'prompt': 'go'},
        story_runtime_contract={'contract': True},
        db_session='db',
        heartbeat_interval_seconds=1.0,
        build_quality_runtime_context_fn=lambda **kwargs: {'runtime': True},
        compute_story_quality_metrics_fn=lambda **kwargs: {'overall_score': 90},
        resolve_quality_gate_execution_plan_fn=lambda *args, **kwargs: {'action': 'continue'},
        candidate_generator_fn=lambda **kwargs: None,
        attach_story_runtime_contract_fn=lambda metrics, contract: metrics,
        publish_stream_event_fn=fake_publish_stream_event,
        execute_candidate_flow_fn=fake_execute_candidate_flow,
    )

    assert publish_calls[0]['stream_task_id'] == 'task-1'
    assert publish_calls[0]['payload']['progress'] == 35
    assert publish_calls[0]['payload']['chapter_number'] == 4
    assert execute_calls[0]['default_candidate_limit'] == 2
    assert execute_calls[0]['generate_kwargs'] == {'prompt': 'go'}
    assert result.selected_candidate_result['word_count'] == 12



@pytest.mark.asyncio
async def test_should_execute_batch_generation_prompt_stage():
    prompt_calls = []
    payload_calls = []

    async def fake_build_prompt(**kwargs):
        prompt_calls.append(kwargs)
        return batch_execution_service.BatchGenerationPrompt(
            chapter_perspective='????',
            base_prompt='base',
            prompt='styled-prompt',
        )

    def fake_build_request_payload(**kwargs):
        payload_calls.append(kwargs)
        return batch_execution_service.BatchGenerationRequestPayload(
            system_prompt='system-prompt',
            max_tokens=1234,
            generate_kwargs={'prompt': kwargs['prompt'], 'temperature': 0.7},
        )

    result = await batch_execution_service.execute_batch_generation_prompt_stage(
        db_session=None,
        chapter=Chapter(id='chapter-1', project_id='project-1', chapter_number=6, title='???'),
        project=Project(id='project-1', title='??', user_id='user-1'),
        chapter_context=SimpleNamespace(chapter_outline='outline'),
        outline_mode='one-to-many',
        current_user_id='user-1',
        target_word_count=2400,
        temp_narrative_perspective='????',
        previous_summary_context='summary',
        prompt_quality_kwargs={'quality_notes': 'tight'},
        style_content='????',
        style_name='??',
        style_preset_id='preset-9',
        ai_service=SimpleNamespace(name='ai'),
        custom_model='model-x',
        story_runtime_contract={'contract': True},
        research_assets=['asset-1'],
        get_template_fn=lambda *args, **kwargs: None,
        format_prompt_fn=lambda *args, **kwargs: 'unused',
        apply_style_to_prompt_fn=lambda prompt, style: prompt,
        build_runtime_system_prompt_fn=lambda **kwargs: 'unused',
        calculate_max_tokens_fn=lambda target_word_count: 1,
        build_request_options_fn=lambda ai_service: None,
        detect_style_profile_fn=lambda **kwargs: 'profile',
        resolve_generation_temperature_fn=lambda style_profile: 0.7,
        build_prompt_fn=fake_build_prompt,
        build_request_payload_fn=fake_build_request_payload,
    )

    assert prompt_calls[0]['outline_mode'] == 'one-to-many'
    assert prompt_calls[0]['style_content'] == '????'
    assert payload_calls[0]['prompt'] == 'styled-prompt'
    assert payload_calls[0]['style_name'] == '??'
    assert payload_calls[0]['research_assets'] == ['asset-1']
    assert result.batch_prompt.prompt == 'styled-prompt'
    assert result.request_payload.system_prompt == 'system-prompt'
    assert result.prompt == 'styled-prompt'
    assert result.system_prompt == 'system-prompt'
    assert result.max_tokens == 1234
    assert result.generate_kwargs == {'prompt': 'styled-prompt', 'temperature': 0.7}
