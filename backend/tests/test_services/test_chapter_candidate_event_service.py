from app.models.chapter import Chapter
from app.services.chapter_candidate_event_service import (
    build_batch_generation_candidate_progress_event,
    build_batch_generation_chunk_event,
    build_batch_generation_selected_candidate_progress_event,
    build_batch_generation_start_progress_event,
    build_chapter_generation_progress_kwargs,
)
from app.services.chapter_candidate_runtime_state_service import ChapterCandidateRuntimeStateSnapshot
from app.services.chapter_candidate_view_service import ChapterCandidateView


def _chapter() -> Chapter:
    return Chapter(id="chapter-1", project_id="project-1", chapter_number=3, title="Title")


def test_should_build_batch_generation_start_progress_event():
    payload = build_batch_generation_start_progress_event(chapter=_chapter())

    assert payload["type"] == "progress"
    assert payload["progress"] == 35
    assert payload["message"] == "Generating chapter 3"


def test_should_build_batch_generation_candidate_progress_event():
    snapshot = ChapterCandidateRuntimeStateSnapshot(
        candidate_total=4, candidate_count=4, candidate_index=2, current_chars=900, word_count=900, chunk_count=3,
        generation_path="rerank_retry", attempt_kind="rerank_candidate", rerank_used=True,
        word_budget_repair_used=False, winner_candidate_index=None,
    )
    payload = build_batch_generation_candidate_progress_event(
        chapter=_chapter(), runtime_snapshot=snapshot, target_word_count=1800
    )

    assert payload["candidate_index"] == 2
    assert payload["candidate_count"] == 4
    assert payload["generation_path"] == "rerank_retry"
    assert payload["progress"] >= 40


def test_should_build_batch_generation_selected_candidate_progress_event_and_chunk_event():
    candidate_view = ChapterCandidateView(
        candidate_index=2, candidate_count=4, winner_candidate_index=3, word_count=1500,
        generation_path="word_budget_repair", attempt_kind="word_budget_repair", rerank_used=False,
        word_budget_repair_used=True, full_content="content", candidate_chunks=["a", "b"],
        quality_metrics={}, quality_gate_plan={},
    )
    payload = build_batch_generation_selected_candidate_progress_event(
        chapter=_chapter(), selected_candidate_view=candidate_view, candidate_word_count=1500,
        chapter_context_stats={"compaction_applied": True},
    )
    chunk_payload = build_batch_generation_chunk_event(chapter=_chapter(), chunk="abc")

    assert payload["winner_candidate_index"] == 3
    assert payload["word_budget_repair_used"] is True
    assert chunk_payload == {"type": "chunk", "chapter_id": "chapter-1", "chapter_number": 3, "content": "abc"}


def test_should_build_chapter_generation_progress_kwargs():
    snapshot = ChapterCandidateRuntimeStateSnapshot(
        candidate_total=3, candidate_count=3, candidate_index=2, current_chars=321, word_count=321, chunk_count=2,
        generation_path="rerank_retry", attempt_kind="rerank_candidate", rerank_used=True,
        word_budget_repair_used=False, winner_candidate_index=None,
    )
    payload = build_chapter_generation_progress_kwargs(runtime_snapshot=snapshot, target_word_count=1600)

    assert payload["current_chars"] == 321
    assert payload["estimated_total"] == 1600
    assert payload["retry_count"] == 1
    assert payload["max_retries"] == 2
