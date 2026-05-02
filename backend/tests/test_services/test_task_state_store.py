import asyncio

import pytest

from app.services.task_system.task_state_store import TaskWorkflowRuntimeStateStore


pytestmark = pytest.mark.asyncio


async def test_state_store_set_get_clear():
    store = TaskWorkflowRuntimeStateStore()
    await store.set("t1", {"phase": "loading"})
    assert await store.get("t1") == {"phase": "loading"}
    await store.clear("t1")
    assert await store.get("t1") == {}


async def test_state_store_update_is_atomic():
    store = TaskWorkflowRuntimeStateStore()

    async def worker():
        for _ in range(50):
            await store.update("t1", lambda cur: {"n": int(cur.get("n", 0)) + 1})

    await asyncio.gather(worker(), worker())
    assert (await store.get("t1"))["n"] == 100
