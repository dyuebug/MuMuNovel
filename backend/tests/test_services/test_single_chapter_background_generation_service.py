from datetime import datetime, timezone
from types import SimpleNamespace

from tests.test_support.single_generation_background_orchestration_test_adapter import (
    recover_stale_single_chapter_background_task_if_needed,
    single_chapter_background_task_contains_chapter,
)


def BatchGenerationTask(**kwargs):
    defaults = {
        'status': None,
        'created_at': None,
        'chapter_ids': [],
        'error_message': None,
        'completed_at': None,
    }
    defaults.update(kwargs)
    return SimpleNamespace(**defaults)


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
