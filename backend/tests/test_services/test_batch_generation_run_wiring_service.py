import pytest

from app.services import batch_generation_run_wiring_service as wiring_service


@pytest.mark.asyncio
async def test_should_execute_batch_generation_in_order_with_default_wiring(monkeypatch):
    captured = {}

    async def fake_workflow(**kwargs):
        captured.update(kwargs)
        return {"ok": True}

    async def get_db_write_lock(_user_id: str):
        return object()

    async def run_generation(**_kwargs):
        return {"chapter_id": "chapter-1"}

    async def await_generation_result(**_kwargs):
        return {"done": True}

    async def run_batch_analysis(**_kwargs):
        return True, None

    monkeypatch.setattr(
        wiring_service,
        'execute_batch_generation_in_order_workflow',
        fake_workflow,
    )

    result = await wiring_service.execute_batch_generation_in_order_with_default_wiring(
        batch_id='batch-1',
        user_id='user-1',
        ai_service=object(),
        custom_model='gpt-test',
        story_repair_summary='repair summary',
        get_db_write_lock_fn=get_db_write_lock,
        run_generation_fn=run_generation,
        await_generation_result_fn=await_generation_result,
        run_batch_analysis_fn=run_batch_analysis,
    )

    assert result == {"ok": True}
    assert captured['batch_id'] == 'batch-1'
    assert captured['user_id'] == 'user-1'
    assert captured['custom_model'] == 'gpt-test'
    assert captured['story_repair_summary'] == 'repair summary'
    assert captured['get_db_write_lock_fn'] is get_db_write_lock
    assert captured['run_generation_fn'] is run_generation
    assert captured['await_generation_result_fn'] is await_generation_result
    assert captured['run_batch_analysis_fn'] is run_batch_analysis
    assert captured['resolve_story_repair_prompt_kwargs_fn'] is wiring_service.resolve_story_repair_prompt_kwargs
    assert captured['clone_quality_profile_fn'] is wiring_service.clone_chapter_quality_profile
    assert captured['resolve_story_repair_state_fn'] is wiring_service.resolve_generation_story_repair_state_for_batch
    assert captured['sync_task_story_repair_state_fn'] is wiring_service.sync_task_story_repair_state
    assert captured['publish_task_stream_event_fn'] is wiring_service.publish_task_stream_event
