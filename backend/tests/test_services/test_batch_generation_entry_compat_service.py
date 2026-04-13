import pytest

from app.services import batch_generation_entry_compat_service as compat_service
from app.services.batch_generation_entry_compat_service import (
    execute_batch_generation_in_order,
    generate_single_chapter_for_batch,
)


@pytest.mark.asyncio
async def test_should_delegate_execute_batch_generation_in_order(monkeypatch):
    captured = {}

    async def fake_wiring(**kwargs):
        captured.update(kwargs)
        return {'ok': True}

    monkeypatch.setattr(
        compat_service,
        'execute_batch_generation_in_order_with_default_wiring',
        fake_wiring,
    )

    result = await execute_batch_generation_in_order(
        batch_id='batch-1',
        user_id='user-1',
        ai_service='ai',
        custom_model='model-x',
        get_db_write_lock_fn='lock',
        run_generation_fn='generate',
        await_generation_result_fn='awaiter',
        run_batch_analysis_fn='analyze',
        resolve_story_repair_state_fn='repair',
        sync_task_story_repair_state_fn='sync',
        publish_task_stream_event_fn='publish',
    )

    assert result == {'ok': True}
    assert captured['batch_id'] == 'batch-1'
    assert captured['user_id'] == 'user-1'
    assert captured['ai_service'] == 'ai'
    assert captured['custom_model'] == 'model-x'
    assert captured['get_db_write_lock_fn'] == 'lock'
    assert captured['run_generation_fn'] == 'generate'
    assert captured['await_generation_result_fn'] == 'awaiter'
    assert captured['run_batch_analysis_fn'] == 'analyze'
    assert captured['resolve_story_repair_state_fn'] == 'repair'
    assert captured['sync_task_story_repair_state_fn'] == 'sync'
    assert captured['publish_task_stream_event_fn'] == 'publish'


@pytest.mark.asyncio
async def test_should_delegate_generate_single_chapter_for_batch(monkeypatch):
    captured = {}

    async def fake_wiring(**kwargs):
        captured.update(kwargs)
        return {'content': 'ok'}

    monkeypatch.setattr(
        compat_service,
        'generate_single_chapter_for_batch_with_default_wiring',
        fake_wiring,
    )

    result = await generate_single_chapter_for_batch(
        db_session='db',
        chapter='chapter',
        user_id='user-1',
        style_id=None,
        target_word_count=1200,
        ai_service='ai',
        write_lock='lock',
        candidate_generator_fn='candidate',
        default_candidate_limit=3,
        heartbeat_interval_seconds=0.5,
        chapter_web_research_service='research',
        publish_task_stream_event_fn='publish',
        resolve_quality_profile_fn='quality',
        one_to_one_builder_cls='one',
        one_to_many_builder_cls='many',
        get_template_fn='template',
        format_prompt_fn='format',
        build_runtime_system_prompt_fn='prompt',
        compute_story_quality_metrics_fn='metrics',
        resolve_quality_gate_execution_plan_fn='gate',
    )

    assert result == {'content': 'ok'}
    assert captured['db_session'] == 'db'
    assert captured['chapter'] == 'chapter'
    assert captured['user_id'] == 'user-1'
    assert captured['target_word_count'] == 1200
    assert captured['ai_service'] == 'ai'
    assert captured['write_lock'] == 'lock'
    assert captured['candidate_generator_fn'] == 'candidate'
    assert captured['default_candidate_limit'] == 3
    assert captured['heartbeat_interval_seconds'] == 0.5
    assert captured['chapter_web_research_service'] == 'research'
    assert captured['publish_task_stream_event_fn'] == 'publish'
    assert captured['resolve_quality_profile_fn'] == 'quality'
    assert captured['one_to_one_builder_cls'] == 'one'
    assert captured['one_to_many_builder_cls'] == 'many'
    assert captured['get_template_fn'] == 'template'
    assert captured['format_prompt_fn'] == 'format'
    assert captured['build_runtime_system_prompt_fn'] == 'prompt'
    assert captured['compute_story_quality_metrics_fn'] == 'metrics'
    assert captured['resolve_quality_gate_execution_plan_fn'] == 'gate'
