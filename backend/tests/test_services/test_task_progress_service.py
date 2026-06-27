import pytest

from tests.test_support.task_system import (
    infer_workflow_phase,
    resolve_progress_phase,
    resolve_stage_code_for_phase,
)


@pytest.mark.parametrize(
    "event_type,progress,message,expected",
    [
        ("error", None, None, "failed"),
        ("done", None, None, "complete"),
        ("chapter_start", None, None, "generating"),
        ("analysis_started", None, None, "parsing"),
        ("progress", None, "取消", "cancelled"),
        ("progress", 95, None, "saving"),
        ("progress", 30, None, "generating"),
    ],
)
def test_infer_workflow_phase(event_type, progress, message, expected):
    assert (
        infer_workflow_phase(
            event_type=event_type,
            progress=progress,
            message=message,
        )
        == expected
    )


def test_resolve_progress_phase_keeps_monotonic_without_retry():
    assert (
        resolve_progress_phase(
            message="解析中",
            progress=86,
            stage_code="6.writing.parsing",
        )
        == "parsing"
    )
    assert (
        resolve_progress_phase(
            message="加载中",
            progress=6,
            stage_code="6.writing.parsing",
        )
        == "parsing"
    )


def test_resolve_progress_phase_allows_retry_to_go_back():
    assert (
        resolve_progress_phase(
            message="retry loading",
            progress=6,
            stage_code="6.writing.parsing",
        )
        == "loading"
    )


def test_resolve_stage_code_for_phase():
    assert (
        resolve_stage_code_for_phase(
            task_type="chapters_batch_generate",
            stage_code=None,
            phase="loading",
        )
        == "6.writing.loading"
    )
    assert (
        resolve_stage_code_for_phase(
            task_type="chapters_batch_generate",
            stage_code="6.writing.parsing",
            phase="saving",
        )
        == "6.writing.saving"
    )
