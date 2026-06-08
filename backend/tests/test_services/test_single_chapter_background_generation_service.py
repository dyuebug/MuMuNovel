from datetime import datetime, timezone

from app.models.batch_generation_task import BatchGenerationTask
from app.services.batch_generation_orchestration_service import (
    recover_stale_single_chapter_background_task_if_needed,
    single_chapter_background_task_contains_chapter,
)


def test_should_match_single_chapter_background_task_for_string_or_object_chapter_ids():
    task = BatchGenerationTask(
        chapter_ids=['chapter-1', {'id': 'chapter-2', 'title': 'second'}],
    )

    assert single_chapter_background_task_contains_chapter(task, 'chapter-1') is True
    assert single_chapter_background_task_contains_chapter(task, 'chapter-2') is True
    assert single_chapter_background_task_contains_chapter(task, 'chapter-9') is False


def test_should_not_recover_fresh_pending_task_created_with_utc_timestamp():
    task = BatchGenerationTask(
        status='pending',
        created_at=datetime.now(timezone.utc).replace(tzinfo=None),
        chapter_ids=['chapter-1'],
    )

    assert recover_stale_single_chapter_background_task_if_needed(task) is False
    assert task.status == 'pending'
