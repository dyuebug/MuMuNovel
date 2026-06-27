from datetime import datetime

from tests.test_support.task_system import touch_checkpoint


def test_touch_checkpoint_merges_and_sets_fields():
    now = datetime(2026, 1, 1, 0, 0, 0)
    checkpoint = touch_checkpoint(
        {"foo": "bar"},
        event="progress",
        progress=10,
        message="ok",
        extra={"x": 1},
        now=now,
    )
    assert checkpoint["foo"] == "bar"
    assert checkpoint["event"] == "progress"
    assert checkpoint["progress"] == 10
    assert checkpoint["message"] == "ok"
    assert checkpoint["x"] == 1
    assert checkpoint["updated_at"].startswith("2026-01-01")
