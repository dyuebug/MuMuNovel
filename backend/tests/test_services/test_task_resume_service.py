from dataclasses import dataclass
from datetime import datetime, timezone

from tests.test_support.task_system import recover_orphan_tasks_on_boot, touch_checkpoint


@dataclass
class DummyRecord:
    task_id: str
    status: str
    progress: int
    message: str
    error: str | None = None
    stage_code: str | None = None
    checkpoint: dict | None = None
    started_at: datetime | None = None
    completed_at: datetime | None = None
    updated_at: datetime | None = None


def test_recover_orphan_tasks_on_boot_marks_failed_and_sets_checkpoint():
    now = datetime(2026, 1, 1, tzinfo=timezone.utc)
    tasks = {
        "t1": DummyRecord(
            task_id="t1",
            status="running",
            progress=12,
            message="x",
            stage_code="6.writing.loading",
            updated_at=now,
        )
    }

    result = recover_orphan_tasks_on_boot(
        tasks,
        touch_checkpoint_fn=touch_checkpoint,
        now=now,
    )
    assert result.changed is True
    record = tasks["t1"]
    assert record.status == "failed"
    assert record.error
    assert record.checkpoint
    assert record.checkpoint["event"] == "failed"
    assert record.checkpoint["stage_code"] == "6.writing.loading"
