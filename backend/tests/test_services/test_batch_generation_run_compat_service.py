import asyncio

import pytest

from app.services import batch_generation_run_compat_service as compat_service


@pytest.mark.asyncio
async def test_should_return_generation_result_when_not_cancelled():
    class Task:
        status = 'running'

    class DBSession:
        def __init__(self):
            self.refresh_calls = []

        async def refresh(self, task):
            self.refresh_calls.append(task.status)

    async def generation_coro():
        await asyncio.sleep(0)
        return {'ok': True}

    result = await compat_service.await_cancelable_batch_generation_result(
        generation_coro=generation_coro(),
        task=Task(),
        db_session=DBSession(),
        poll_interval_seconds=0.01,
    )

    assert result == {'ok': True}


@pytest.mark.asyncio
async def test_should_cancel_generation_when_task_marked_cancelled():
    class Task:
        def __init__(self):
            self.status = 'running'

    class DBSession:
        def __init__(self, task):
            self.task = task
            self.refresh_count = 0

        async def refresh(self, task):
            self.refresh_count += 1
            self.task.status = 'cancelled'

    async def generation_coro():
        await asyncio.sleep(1)
        return {'ok': False}

    task = Task()
    db_session = DBSession(task)

    with pytest.raises(asyncio.CancelledError):
        await compat_service.await_cancelable_batch_generation_result(
            generation_coro=generation_coro(),
            task=task,
            db_session=db_session,
            poll_interval_seconds=0.01,
        )

    assert db_session.refresh_count >= 1
