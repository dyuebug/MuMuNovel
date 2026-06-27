import pytest

from tests.test_support.chapter_generation_stream_types import (
    ChapterGenerationStreamCandidateStageResult,
)
from tests.test_support import single_generation_stream_orchestration_test_adapter as stream_orchestration_service


@pytest.mark.asyncio
async def test_should_build_chapter_generation_event_stream_with_explicit_wiring():
    captured = {}

    class FakeTracker:
        def __init__(self, name):
            captured["tracker_name"] = name

        async def start(self):
            return "start"

        async def loading(self, message, progress):
            return f"loading:{message}:{progress}"

        async def preparing(self, message):
            return f"preparing:{message}"

        async def generating(self, **kwargs):
            return f"generating:{kwargs['current_chars']}"

        async def heartbeat(self):
            return "heartbeat"

        async def generating_chunk(self, chunk):
            return f"chunk:{chunk}"

        async def saving(self, message, progress):
            return f"saving:{message}:{progress}"

        async def complete(self, message):
            return f"complete:{message}"

        async def result(self, payload):
            return f"result:{payload['type']}"

        async def done(self):
            return "done"

        async def error(self, detail, error_code=None):
            return f"error:{detail}:{error_code}"

    async def fake_db_session_source():
        class FakeDbSession:
            def in_transaction(self):
                return False

        yield FakeDbSession()

    async def fake_prepare_stream_execution_fn(**kwargs):
        captured["prepare_kwargs"] = kwargs
        return "execution-setup"

    async def fake_execute_candidate_stage_fn(**kwargs):
        captured["candidate_kwargs"] = kwargs

        class Result:
            chunk_payloads = ["chunk-payload-1", "chunk-payload-2"]

        return Result()

    async def fake_finalize_stream_result_fn(**kwargs):
        captured["finalize_kwargs"] = kwargs
        return "saving-payload", ["plan-step"]

    async def fake_emit_stream_plan_fn(**kwargs):
        captured["emit_plan_kwargs"] = kwargs
        yield "final-payload"

    def fake_format_sse(payload):
        return payload

    async def fake_send_event(**kwargs):
        return kwargs

    emitted = []
    async for payload in (
        stream_orchestration_service.build_chapter_generation_event_stream_with_explicit_wiring(
            db_session_source=fake_db_session_source,
            chapter_id="chapter-1",
            current_user_id="user-1",
            generate_request="generate-request",
            background_tasks="bg",
            user_ai_service="ai-service",
            target_word_count=1200,
            enable_analysis=True,
            heartbeat_interval_seconds=0.25,
            custom_model="gpt-test",
            temp_narrative_perspective="first_person",
            style_id=7,
            dependencies=type(
                "Deps",
                (),
                {
                    "execution": "execution-deps",
                    "candidate": "candidate-deps",
                    "finalize": "finalize-deps",
                },
            )(),
            prepare_stream_execution_fn=fake_prepare_stream_execution_fn,
            execute_candidate_stage_fn=fake_execute_candidate_stage_fn,
            finalize_stream_result_fn=fake_finalize_stream_result_fn,
            emit_stream_plan_fn=fake_emit_stream_plan_fn,
            tracker_factory=FakeTracker,
            format_sse_fn=fake_format_sse,
            send_event_fn=fake_send_event,
            build_progress_kwargs_fn="progress-fn",
            result_type="result-type",
        )
    ):
        emitted.append(payload)

    assert emitted == [
        "start",
        "loading:Loading generation context...:0.2",
        "loading:Chapter context built:0.8",
        "preparing:Preparing AI prompts...",
        "generating:0",
        "chunk-payload-1",
        "chunk-payload-2",
        "saving-payload",
        "final-payload",
    ]
    assert captured["tracker_name"] == "章节生成"
    assert captured["prepare_kwargs"]["chapter_id"] == "chapter-1"
    assert captured["prepare_kwargs"]["dependencies"] == "execution-deps"
    assert captured["candidate_kwargs"]["execution_setup"] == "execution-setup"
    assert captured["candidate_kwargs"]["dependencies"] == "candidate-deps"
    assert captured["candidate_kwargs"]["build_progress_kwargs_fn"] == "progress-fn"
    assert captured["candidate_kwargs"]["result_type"] == "result-type"
    assert captured["finalize_kwargs"]["execution_setup"] == "execution-setup"
    assert captured["finalize_kwargs"]["dependencies"] == "finalize-deps"
    assert captured["emit_plan_kwargs"]["emission_plan"] == ["plan-step"]


@pytest.mark.asyncio
async def test_should_build_chapter_generation_event_stream_with_default_wiring(monkeypatch):
    captured = {}

    async def fake_build_with_explicit_wiring(**kwargs):
        captured.update(kwargs)
        yield "stream-payload"

    monkeypatch.setattr(
        stream_orchestration_service,
        "build_chapter_generation_event_stream_with_explicit_wiring",
        fake_build_with_explicit_wiring,
    )
    monkeypatch.setattr(
        stream_orchestration_service,
        "prepare_chapter_generation_stream_execution",
        "prepare-fn",
    )
    monkeypatch.setattr(
        "tests.test_support.single_generation_stream_candidate_test_adapter.execute_chapter_generation_candidate_stage",
        "candidate-fn",
    )
    monkeypatch.setattr(
        stream_orchestration_service,
        "finalize_chapter_generation_stream_result",
        "finalize-fn",
    )
    monkeypatch.setattr(
        stream_orchestration_service,
        "emit_chapter_generation_stream_plan",
        "emit-plan-fn",
    )

    emitted = []
    async for payload in stream_orchestration_service.build_chapter_generation_event_stream_with_default_wiring(
        db_session_source="db-source",
        chapter_id="chapter-1",
        current_user_id="user-1",
        generate_request="generate-request",
        background_tasks="bg",
        user_ai_service="ai-service",
        target_word_count=1200,
        enable_analysis=True,
        heartbeat_interval_seconds=1.5,
        custom_model="gpt-test",
        temp_narrative_perspective="first_person",
        style_id=7,
        dependencies="deps",
    ):
        emitted.append(payload)

    assert emitted == ["stream-payload"]
    assert captured["db_session_source"] == "db-source"
    assert captured["chapter_id"] == "chapter-1"
    assert captured["dependencies"] == "deps"
    assert captured["prepare_stream_execution_fn"] == "prepare-fn"
    assert captured["execute_candidate_stage_fn"] == "candidate-fn"
    assert captured["finalize_stream_result_fn"] == "finalize-fn"
    assert captured["emit_stream_plan_fn"] == "emit-plan-fn"
    assert captured["build_progress_kwargs_fn"].__name__ == "build_chapter_generation_progress_kwargs"
    assert (
        captured["result_type"]
        == ChapterGenerationStreamCandidateStageResult
    )
