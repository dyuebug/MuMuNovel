import pytest

from tests.test_support.task_system import TaskStreamHub


pytestmark = pytest.mark.asyncio


async def test_task_stream_hub_subscribe_unsubscribe_and_fanout():
    hub = TaskStreamHub()
    q1 = await hub.subscribe("t1", maxsize=10)
    q2 = await hub.subscribe("t1", maxsize=10)

    result = await hub.fanout("t1", {"type": "progress", "progress": 10})
    assert result.delivered == 2

    await hub.unsubscribe("t1", q1)
    result = await hub.fanout("t1", {"type": "progress", "progress": 20})
    assert result.delivered == 1

    await hub.unsubscribe("t1", q2)
    result = await hub.fanout("t1", {"type": "progress", "progress": 30})
    assert result.delivered == 0
