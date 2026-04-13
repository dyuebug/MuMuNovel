from types import SimpleNamespace

import pytest

from app.models.chapter import Chapter
from app.services import chapter_regeneration_stream_service as regeneration_stream_service


def test_should_resolve_regeneration_estimated_total_from_effective_request_first():
    context = regeneration_stream_service.ChapterRegenerationStreamContext(
        chapter=Chapter(id="chapter-1", project_id="project-1", chapter_number=1, title="x", content="legacy"),
        analysis=None,
        user_id="user-1",
        regenerate_request=SimpleNamespace(target_word_count=400, auto_apply=False),
        effective_regenerate_request=SimpleNamespace(target_word_count=600),
        project_context={},
        style_content="",
        style_id=None,
        story_runtime_contract=None,
    )

    assert regeneration_stream_service.resolve_chapter_regeneration_estimated_total(context) == 600


def test_should_sanitize_regeneration_content_and_raise_on_empty():
    with pytest.raises(ValueError) as exc_info:
        regeneration_stream_service.sanitize_chapter_regeneration_content(
            "raw-content",
            chapter_id="chapter-1",
            sanitize_generated_text=lambda text: ("   ", 2),
            contains_workflow_meta_text=lambda text: False,
        )
    assert "?????" in str(exc_info.value)


def test_should_sanitize_regeneration_content_and_raise_on_meta_text():
    with pytest.raises(ValueError) as exc_info:
        regeneration_stream_service.sanitize_chapter_regeneration_content(
            "raw-content",
            chapter_id="chapter-1",
            sanitize_generated_text=lambda text: ("valid content", 0),
            contains_workflow_meta_text=lambda text: True,
        )
    assert "??????" in str(exc_info.value)


def test_should_finalize_regeneration_completion_and_build_result_payload():
    regeneration_task = SimpleNamespace(
        id="task-1",
        version_number=3,
        status="running",
        regenerated_content=None,
        regenerated_word_count=0,
        completed_at=None,
    )

    class FakeRegenerator:
        def calculate_content_diff(self, original_content, new_content):
            return {"similarity": 12.5, "difference": 87.5}

    completion = regeneration_stream_service.finalize_chapter_regeneration_completion(
        regeneration_task=regeneration_task,
        original_content="legacy",
        regenerated_content="new content",
        regenerator=FakeRegenerator(),
        regenerate_request=SimpleNamespace(auto_apply=False),
        story_runtime_contract={"contract": True},
        build_result_payload_fn=lambda **kwargs: {"ok": True, **kwargs},
    )

    assert regeneration_task.status == "completed"
    assert regeneration_task.regenerated_content == "new content"
    assert regeneration_task.regenerated_word_count == len("new content")
    assert regeneration_task.completed_at is not None
    assert completion.word_count == len("new content")
    assert completion.diff_stats == {"similarity": 12.5, "difference": 87.5}
    assert completion.result_payload["task_id"] == "task-1"
    assert completion.result_payload["story_runtime_contract"] == {"contract": True}


def test_should_build_chapter_regeneration_emission_plan_in_expected_order():
    plan = regeneration_stream_service.build_chapter_regeneration_emission_plan(
        result_payload={"task_id": "task-1", "word_count": 500},
    )

    assert [step.kind for step in plan] == [
        "tracker_saving",
        "tracker_complete",
        "tracker_result",
        "tracker_done",
    ]
    assert plan[0].progress == 0.9
    assert plan[2].payload["task_id"] == "task-1"


@pytest.mark.asyncio
async def test_should_emit_chapter_regeneration_plan_in_expected_order():
    plan = [
        regeneration_stream_service.ChapterRegenerationEmissionStep(kind="tracker_saving", message="saving", progress=0.9),
        regeneration_stream_service.ChapterRegenerationEmissionStep(kind="tracker_complete", message="done"),
        regeneration_stream_service.ChapterRegenerationEmissionStep(kind="tracker_result", payload={"task_id": "task-1"}),
        regeneration_stream_service.ChapterRegenerationEmissionStep(kind="tracker_done"),
    ]
    calls: list[tuple[str, object]] = []

    async def fake_saving(message: str, progress: float):
        calls.append(("saving", (message, progress)))
        return f"saving:{message}:{progress}"

    async def fake_complete(message: str):
        calls.append(("complete", message))
        return f"complete:{message}"

    async def fake_result(payload: dict[str, object]):
        calls.append(("result", payload))
        return f"result:{payload['task_id']}"

    async def fake_done():
        calls.append(("done", None))
        return "done"

    emitted = []
    async for item in regeneration_stream_service.emit_chapter_regeneration_plan(
        emission_plan=plan,
        tracker_saving_fn=fake_saving,
        tracker_complete_fn=fake_complete,
        tracker_result_fn=fake_result,
        tracker_done_fn=fake_done,
    ):
        emitted.append(item)

    assert emitted == [
        "saving:saving:0.9",
        "complete:done",
        "result:task-1",
        "done",
    ]
    assert [name for name, _ in calls] == [
        "saving",
        "complete",
        "result",
        "done",
    ]


@pytest.mark.asyncio
async def test_should_handle_chapter_regeneration_failure_via_mark_failed_helper():
    calls: dict[str, object] = {}

    async def fake_mark_failed(db_session, *, chapter_id: str, error_message: str):
        calls["db_session"] = db_session
        calls["chapter_id"] = chapter_id
        calls["error_message"] = error_message

    db_session = SimpleNamespace(name="db")
    await regeneration_stream_service.handle_chapter_regeneration_failure(
        db_session,
        chapter_id="chapter-1",
        error_message="boom",
        mark_failed_fn=fake_mark_failed,
    )

    assert calls == {
        "db_session": db_session,
        "chapter_id": "chapter-1",
        "error_message": "boom",
    }


@pytest.mark.asyncio
async def test_should_stream_regeneration_feedback_chunks_and_update_state():
    class FakeRegenerator:
        async def regenerate_with_feedback(self, **kwargs):
            yield {"type": "chunk", "content": "a" * 500}
            yield {"type": "chunk", "content": "tail"}

    calls: list[tuple[str, object]] = []

    async def fake_generating_chunk(chunk: str):
        calls.append(("chunk", chunk))
        return f"chunk:{len(chunk)}"

    async def fake_preparing(message: str):
        calls.append(("preparing", message))
        return f"preparing:{message}"

    async def fake_generating(**kwargs):
        calls.append(("generating", kwargs))
        return f"generating:{kwargs['current_chars']}"

    async def fake_parsing(message: str):
        calls.append(("parsing", message))
        return f"parsing:{message}"

    state = regeneration_stream_service.ChapterRegenerationStreamingState()
    context = regeneration_stream_service.ChapterRegenerationStreamContext(
        chapter=Chapter(id="chapter-1", project_id="project-1", chapter_number=1, title="x", content="legacy"),
        analysis=None,
        user_id="user-1",
        regenerate_request=SimpleNamespace(target_word_count=500, auto_apply=False),
        effective_regenerate_request=SimpleNamespace(target_word_count=500),
        project_context={},
        style_content="",
        style_id=None,
        story_runtime_contract=None,
    )

    emitted = []
    async for item in regeneration_stream_service.stream_chapter_regeneration_feedback(
        regenerator=FakeRegenerator(),
        context=context,
        db_session=SimpleNamespace(),
        estimated_total=500,
        streaming_state=state,
        tracker_generating_chunk_fn=fake_generating_chunk,
        tracker_preparing_fn=fake_preparing,
        tracker_generating_fn=fake_generating,
        tracker_parsing_fn=fake_parsing,
    ):
        emitted.append(item)

    assert state.full_content == ("a" * 500) + "tail"
    assert emitted == ["chunk:500", "generating:500", "chunk:4"]
    assert calls[1][0] == "generating"
    assert calls[1][1]["current_chars"] == 500


@pytest.mark.asyncio
async def test_should_stream_regeneration_feedback_progress_to_matching_tracker_stage():
    class FakeRegenerator:
        async def regenerate_with_feedback(self, **kwargs):
            yield {"type": "progress", "progress": 10, "message": "prep"}
            yield {"type": "progress", "progress": 40, "message": "gen"}
            yield {"type": "progress", "progress": 90, "message": "parse"}

    calls: list[tuple[str, object]] = []

    async def fake_generating_chunk(chunk: str):
        calls.append(("chunk", chunk))
        return chunk

    async def fake_preparing(message: str):
        calls.append(("preparing", message))
        return f"preparing:{message}"

    async def fake_generating(**kwargs):
        calls.append(("generating", kwargs))
        return f"generating:{kwargs['message']}"

    async def fake_parsing(message: str):
        calls.append(("parsing", message))
        return f"parsing:{message}"

    state = regeneration_stream_service.ChapterRegenerationStreamingState(full_content="existing")
    context = regeneration_stream_service.ChapterRegenerationStreamContext(
        chapter=Chapter(id="chapter-1", project_id="project-1", chapter_number=1, title="x", content="legacy"),
        analysis=None,
        user_id="user-1",
        regenerate_request=SimpleNamespace(target_word_count=500, auto_apply=False),
        effective_regenerate_request=SimpleNamespace(target_word_count=500),
        project_context={},
        style_content="",
        style_id=None,
        story_runtime_contract=None,
    )

    emitted = []
    async for item in regeneration_stream_service.stream_chapter_regeneration_feedback(
        regenerator=FakeRegenerator(),
        context=context,
        db_session=SimpleNamespace(),
        estimated_total=500,
        streaming_state=state,
        tracker_generating_chunk_fn=fake_generating_chunk,
        tracker_preparing_fn=fake_preparing,
        tracker_generating_fn=fake_generating,
        tracker_parsing_fn=fake_parsing,
    ):
        emitted.append(item)

    assert emitted == ["preparing:prep", "generating:gen", "parsing:parse"]
    assert [name for name, _ in calls] == ["preparing", "generating", "parsing"]
