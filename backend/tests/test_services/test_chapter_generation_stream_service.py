import asyncio
from types import SimpleNamespace
from typing import Any
from unittest.mock import AsyncMock

import pytest

from app.models.chapter import Chapter
from app.models.outline import Outline
from app.models.project import Project
from app.services import chapter_generation_stream_service as generation_stream_service
from app.services.story_repair_payload_service import normalize_story_repair_payload


EXPECTED_STREAM_SERVICE_PUBLIC_API = {
    "ChapterGenerationAnalysisFollowupPlan",
    "ChapterGenerationAnalysisScheduling",
    "ChapterGenerationCandidateExecution",
    "ChapterGenerationCandidateQualityHooks",
    "ChapterGenerationEmissionStep",
    "ChapterGenerationPersistencePreparation",
    "ChapterGenerationPostPersistEffects",
    "ChapterGenerationSelectedCandidateOutcome",
    "ChapterGenerationStreamBuiltContext",
    "ChapterGenerationStreamCandidateStageResult",
    "ChapterGenerationStreamDependencies",
    "ChapterGenerationStreamExecutionSetup",
    "ChapterGenerationStreamPreparation",
    "ChapterGenerationStreamPrompt",
    "ChapterGenerationStreamRequestPayload",
    "ChapterGenerationStreamResponseArtifacts",
    "ChapterGenerationStreamRuntimeContext",
    "apply_chapter_generation_outcome_and_build_history",
    "build_chapter_generation_analysis_followup_plan",
    "build_chapter_generation_candidate_quality_hooks",
    "build_chapter_generation_event_stream",
    "build_chapter_generation_selected_candidate_outcome",
    "build_chapter_generation_stream_context",
    "build_chapter_generation_stream_emission_plan",
    "build_chapter_generation_stream_prompt",
    "build_chapter_generation_stream_request_payload",
    "build_chapter_generation_stream_response_artifacts",
    "create_chapter_generation_candidate_execution",
    "finalize_chapter_generation_stream_result",
    "load_chapter_generation_stream_runtime_context",
    "prepare_chapter_generation_analysis_scheduling",
    "prepare_chapter_generation_stream_execution",
    "prepare_chapter_generation_stream_request",
    "run_chapter_generation_post_persist_effects",
    "wait_for_chapter_generation_candidate",
}


def test_should_expose_expected_public_api_contract():
    assert set(generation_stream_service.__all__) == EXPECTED_STREAM_SERVICE_PUBLIC_API
    assert len(generation_stream_service.__all__) == len(EXPECTED_STREAM_SERVICE_PUBLIC_API)
    for symbol_name in EXPECTED_STREAM_SERVICE_PUBLIC_API:
        assert hasattr(generation_stream_service, symbol_name)


class _ScalarResult:
    def __init__(self, value):
        self._value = value

    def scalar_one_or_none(self):
        return self._value


@pytest.mark.asyncio
async def test_should_load_generation_stream_runtime_context(monkeypatch):
    chapter = Chapter(
        id="chapter-1",
        project_id="project-1",
        chapter_number=3,
        title="Test Chapter",
        outline_id="outline-1",
    )
    project = Project(
        id="project-1",
        title="Test Project",
        user_id="user-1",
        outline_mode="one-to-one",
    )
    outline = Outline(
        id="outline-1",
        project_id="project-1",
        order_index=3,
        content="outline notes",
    )
    db_session = AsyncMock()
    db_session.execute = AsyncMock(
        side_effect=[
            _ScalarResult(chapter),
            _ScalarResult(project),
            _ScalarResult(outline),
        ]
    )
    quality_profile = {
        "resolved_style_id": 7,
        "style_content": "style guide",
        "style_name": "noir",
        "style_preset_id": "preset-7",
    }
    story_packet = SimpleNamespace(guidance={"creative_mode": "hook"})
    resolve_quality_profile = AsyncMock(return_value=quality_profile)
    build_story_packet = AsyncMock(return_value=story_packet)
    repair_payload = normalize_story_repair_payload(
        summary="repair brief",
        targets=["tighten pace"],
        strengths=["keep tension"],
    )
    resolve_story_repair_state = AsyncMock(return_value={"payload": repair_payload})
    cancelled_projects: list[str] = []

    def cancel_outline_postprocess_tasks(project_id: str) -> int:
        cancelled_projects.append(project_id)
        return 2

    monkeypatch.setattr(
        generation_stream_service,
        "resolve_chapter_quality_profile",
        resolve_quality_profile,
    )
    monkeypatch.setattr(
        generation_stream_service,
        "build_story_generation_packet_with_project_continuity",
        build_story_packet,
    )

    request = SimpleNamespace(
        enable_mcp=False,
        story_repair_summary="repair brief",
        story_repair_targets=["tighten pace"],
        story_preserve_strengths=["keep tension"],
    )

    context = await generation_stream_service.load_chapter_generation_stream_runtime_context(
        db_session,
        chapter_id=chapter.id,
        user_id="user-1",
        generate_request=request,
        style_id=7,
        resolve_story_repair_state_fn=resolve_story_repair_state,
        cancel_outline_postprocess_tasks_fn=cancel_outline_postprocess_tasks,
    )

    assert context.chapter is chapter
    assert context.project is project
    assert context.outline is outline
    assert context.outline_mode == "one-to-one"
    assert context.quality_profile == quality_profile
    assert context.story_packet is story_packet
    assert context.generation_guidance == {"creative_mode": "hook"}
    assert context.story_repair_payload == repair_payload
    assert context.resolved_style_id == 7
    assert context.style_content == "style guide"
    assert context.style_name == "noir"
    assert context.style_preset_id == "preset-7"
    assert cancelled_projects == ["project-1"]
    resolve_quality_profile.assert_awaited_once()
    build_story_packet.assert_awaited_once()
    resolve_story_repair_state.assert_awaited_once()


@pytest.mark.asyncio
async def test_should_raise_when_project_missing_while_loading_generation_stream_runtime_context():
    chapter = Chapter(
        id="chapter-1",
        project_id="project-1",
        chapter_number=2,
        title="Test Chapter",
    )
    db_session = AsyncMock()
    db_session.execute = AsyncMock(
        side_effect=[
            _ScalarResult(chapter),
            _ScalarResult(None),
        ]
    )

    with pytest.raises(ValueError) as exc_info:
        await generation_stream_service.load_chapter_generation_stream_runtime_context(
            db_session,
            chapter_id=chapter.id,
            user_id="user-1",
            generate_request=SimpleNamespace(enable_mcp=True),
            style_id=None,
            resolve_story_repair_state_fn=AsyncMock(return_value={}),
            cancel_outline_postprocess_tasks_fn=lambda _project_id: 0,
        )

    assert str(exc_info.value) == "项目不存在"


@pytest.mark.asyncio
@pytest.mark.parametrize(
    ("outline_mode", "expected_builder"),
    [
        ("one-to-one", "one"),
        ("one-to-many", "many"),
    ],
)
async def test_should_build_generation_stream_context_with_expected_builder(
    outline_mode: str,
    expected_builder: str,
):
    runtime_context = generation_stream_service.ChapterGenerationStreamRuntimeContext(
        chapter=Chapter(id="chapter-1", project_id="project-1", chapter_number=3, title="Test Chapter"),
        project=Project(id="project-1", title="Test Project", user_id="user-1", outline_mode=outline_mode),
        outline=Outline(id="outline-1", project_id="project-1", order_index=3, content="outline notes"),
        outline_mode=outline_mode,
        quality_profile={"resolved_style_id": 7},
        story_packet=SimpleNamespace(guidance={"creative_mode": "hook"}),
        generation_guidance={"creative_mode": "hook"},
        story_repair_state={"payload": None},
        story_repair_payload=None,
        resolved_style_id=7,
        style_content="style guide",
        style_name="noir",
        style_preset_id="preset-7",
    )
    calls = {"one": 0, "many": 0}

    class FakeContext:
        context_stats = {
            "outline_length": 12,
            "previous_content_length": 6,
            "characters_length": 8,
            "foreshadow_length": 4,
            "memories_length": 3,
            "total_length": 33,
            "continuation_length": 5,
            "skeleton_length": 9,
        }

    class FakeOneToOneBuilder:
        def __init__(self, *args, **kwargs):
            self.kwargs = kwargs

        async def build(self, **kwargs):
            calls["one"] += 1
            assert kwargs["target_word_count"] == 1800
            return FakeContext()

    class FakeOneToManyBuilder:
        def __init__(self, *args, **kwargs):
            self.kwargs = kwargs

        async def build(self, **kwargs):
            calls["many"] += 1
            assert kwargs["target_word_count"] == 1800
            assert kwargs["style_content"] == "style guide"
            assert kwargs["temp_narrative_perspective"] == "第三人称"
            return FakeContext()

    generation_runtime = SimpleNamespace(
        generation_intent={"intent": "ok"},
        prompt_quality_kwargs={"quality_preset": "plot_drive"},
        story_runtime_contract={"contract": True},
    )

    built_context = await generation_stream_service.build_chapter_generation_stream_context(
        db_session=AsyncMock(),
        runtime_context=runtime_context,
        user_id="user-1",
        target_word_count=1800,
        temp_narrative_perspective="第三人称",
        memory_service=object(),
        foreshadow_service=object(),
        one_to_one_builder_cls=FakeOneToOneBuilder,
        one_to_many_builder_cls=FakeOneToManyBuilder,
        build_outline_structure_runtime_sources_fn=lambda outline: {"outline_id": outline.id} if outline else None,
        build_generation_runtime_bundle_fn=lambda **kwargs: generation_runtime,
    )

    assert calls[expected_builder] == 1
    unexpected_builder = "many" if expected_builder == "one" else "one"
    assert calls[unexpected_builder] == 0
    assert isinstance(built_context.chapter_context, FakeContext)
    assert built_context.generation_intent == {"intent": "ok"}
    assert built_context.prompt_quality_kwargs == {"quality_preset": "plot_drive"}
    assert built_context.story_runtime_contract == {"contract": True}


@pytest.mark.asyncio
async def test_should_build_one_to_one_next_prompt_with_style():
    runtime_context = generation_stream_service.ChapterGenerationStreamRuntimeContext(
        chapter=Chapter(id="chapter-1", project_id="project-1", chapter_number=2, title="Test Chapter"),
        project=Project(id="project-1", title="Test Project", user_id="user-1", outline_mode="one-to-one"),
        outline=Outline(id="outline-1", project_id="project-1", order_index=2, content="outline notes"),
        outline_mode="one-to-one",
        quality_profile={"resolved_style_id": 7},
        story_packet=SimpleNamespace(guidance={"creative_mode": "hook"}),
        generation_guidance={"creative_mode": "hook"},
        story_repair_state={"payload": None},
        story_repair_payload=None,
        resolved_style_id=7,
        style_content="style guide",
        style_name="noir",
        style_preset_id="preset-7",
    )
    chapter_context = SimpleNamespace(
        chapter_outline="outline",
        continuation_point="cliffhanger",
        previous_chapter_summary="summary",
        chapter_characters="角色A",
        chapter_careers="职业A",
        foreshadow_reminders="伏笔A",
        relevant_memories="记忆A",
        recent_chapters_context="",
    )
    built_context = generation_stream_service.ChapterGenerationStreamBuiltContext(
        chapter_context=chapter_context,
        generation_intent={"intent": "ok"},
        prompt_quality_kwargs={"quality_preset": "plot_drive"},
        story_runtime_contract={"contract": True},
    )
    template_calls: list[str] = []
    format_calls: list[dict[str, Any]] = []

    async def fake_get_template(template_key: str, user_id: str, db_session: Any) -> str:
        template_calls.append(template_key)
        return f"template:{template_key}"

    def fake_format_prompt(template: str, **kwargs: Any) -> str:
        format_calls.append({"template": template, **kwargs})
        return f"formatted:{template}"

    def fake_apply_style(prompt: str, style_content: str) -> str:
        return f"styled::{style_content}::{prompt}"

    result = await generation_stream_service.build_chapter_generation_stream_prompt(
        db_session=AsyncMock(),
        runtime_context=runtime_context,
        built_context=built_context,
        current_user_id="user-1",
        target_word_count=1800,
        temp_narrative_perspective="第三人称",
        get_template_fn=fake_get_template,
        format_prompt_fn=fake_format_prompt,
        apply_style_to_prompt_fn=fake_apply_style,
    )

    assert template_calls == ["CHAPTER_GENERATION_ONE_TO_ONE_NEXT"]
    assert result.chapter_perspective == "第三人称"
    assert result.base_prompt == "formatted:template:CHAPTER_GENERATION_ONE_TO_ONE_NEXT"
    assert result.prompt == "styled::style guide::formatted:template:CHAPTER_GENERATION_ONE_TO_ONE_NEXT"
    assert format_calls[0]["previous_chapter_content"] == "cliffhanger"
    assert format_calls[0]["previous_chapter_summary"] == "summary"
    assert format_calls[0]["narrative_perspective"] == "第三人称"


@pytest.mark.asyncio
async def test_should_build_one_to_many_first_chapter_prompt_without_style():
    runtime_context = generation_stream_service.ChapterGenerationStreamRuntimeContext(
        chapter=Chapter(id="chapter-1", project_id="project-1", chapter_number=1, title="Test Chapter"),
        project=Project(id="project-1", title="Test Project", user_id="user-1", outline_mode="one-to-many"),
        outline=Outline(id="outline-1", project_id="project-1", order_index=1, content="outline notes"),
        outline_mode="one-to-many",
        quality_profile={"resolved_style_id": None},
        story_packet=SimpleNamespace(guidance={"creative_mode": "hook"}),
        generation_guidance={"creative_mode": "hook"},
        story_repair_state={"payload": None},
        story_repair_payload=None,
        resolved_style_id=None,
        style_content="",
        style_name="",
        style_preset_id="",
    )
    chapter_context = SimpleNamespace(
        chapter_outline="outline",
        continuation_point=None,
        previous_chapter_summary="",
        chapter_characters="角色A",
        chapter_careers="职业A",
        foreshadow_reminders="伏笔A",
        relevant_memories="记忆A",
        recent_chapters_context="recent recap",
    )
    built_context = generation_stream_service.ChapterGenerationStreamBuiltContext(
        chapter_context=chapter_context,
        generation_intent={"intent": "ok"},
        prompt_quality_kwargs={"quality_notes": "tight prose"},
        story_runtime_contract={"contract": True},
    )
    template_calls: list[str] = []

    async def fake_get_template(template_key: str, user_id: str, db_session: Any) -> str:
        template_calls.append(template_key)
        return f"template:{template_key}"

    def fake_format_prompt(template: str, **kwargs: Any) -> str:
        return f"formatted:{template}:{kwargs['chapter_number']}"

    result = await generation_stream_service.build_chapter_generation_stream_prompt(
        db_session=AsyncMock(),
        runtime_context=runtime_context,
        built_context=built_context,
        current_user_id="user-1",
        target_word_count=1500,
        temp_narrative_perspective=None,
        get_template_fn=fake_get_template,
        format_prompt_fn=fake_format_prompt,
        apply_style_to_prompt_fn=lambda prompt, style: f"styled::{style}::{prompt}",
    )

    assert template_calls == ["CHAPTER_GENERATION_ONE_TO_MANY"]
    assert result.chapter_perspective == "第三人称"
    assert result.base_prompt == "formatted:template:CHAPTER_GENERATION_ONE_TO_MANY:1"
    assert result.prompt == result.base_prompt



def test_should_build_generation_stream_request_payload_with_request_options_and_custom_model():
    runtime_context = generation_stream_service.ChapterGenerationStreamRuntimeContext(
        chapter=Chapter(id="chapter-1", project_id="project-1", chapter_number=2, title="Test Chapter"),
        project=Project(id="project-1", title="Test Project", user_id="user-1", outline_mode="one-to-one"),
        outline=Outline(id="outline-1", project_id="project-1", order_index=2, content="outline notes"),
        outline_mode="one-to-one",
        quality_profile={"resolved_style_id": 7},
        story_packet=SimpleNamespace(guidance={"creative_mode": "hook"}),
        generation_guidance={"creative_mode": "hook"},
        story_repair_state={"payload": None},
        story_repair_payload=None,
        resolved_style_id=7,
        style_content="style guide",
        style_name="noir",
        style_preset_id="preset-7",
    )
    built_context = generation_stream_service.ChapterGenerationStreamBuiltContext(
        chapter_context=SimpleNamespace(
            chapter_outline="outline",
            previous_chapter_summary="summary",
        ),
        generation_intent={"intent": "ok"},
        prompt_quality_kwargs={"quality_preset": "plot_drive"},
        story_runtime_contract={"contract": True},
    )
    stream_prompt = generation_stream_service.ChapterGenerationStreamPrompt(
        chapter_perspective="first-person",
        base_prompt="base prompt",
        prompt="final prompt",
    )

    payload = generation_stream_service.build_chapter_generation_stream_request_payload(
        runtime_context=runtime_context,
        built_context=built_context,
        stream_prompt=stream_prompt,
        project=runtime_context.project,
        target_word_count=1800,
        enable_mcp=False,
        custom_model="gpt-custom",
        ai_service=object(),
        build_runtime_system_prompt_fn=lambda **kwargs: "system prompt",
        calculate_max_tokens_fn=lambda target_word_count: target_word_count + 123,
        build_request_options_fn=lambda ai_service: {"provider": "mock"},
        detect_style_profile_fn=lambda **kwargs: "low_ai_serial",
        resolve_generation_temperature_fn=lambda style_profile: 0.42,
    )

    assert payload.system_prompt == "system prompt"
    assert payload.max_tokens == 1923
    assert payload.generate_kwargs["prompt"] == "final prompt"
    assert payload.generate_kwargs["system_prompt"] == "system prompt"
    assert payload.generate_kwargs["auto_mcp"] is False
    assert payload.generate_kwargs["temperature"] == 0.42
    assert payload.generate_kwargs["request_options"] == {"provider": "mock"}
    assert payload.generate_kwargs["model"] == "gpt-custom"


def test_should_build_generation_stream_request_payload_without_optional_fields():
    runtime_context = generation_stream_service.ChapterGenerationStreamRuntimeContext(
        chapter=Chapter(id="chapter-1", project_id="project-1", chapter_number=1, title="Test Chapter"),
        project=Project(id="project-1", title="Test Project", user_id="user-1", outline_mode="one-to-many"),
        outline=None,
        outline_mode="one-to-many",
        quality_profile={"resolved_style_id": None},
        story_packet=SimpleNamespace(guidance={"creative_mode": "hook"}),
        generation_guidance={"creative_mode": "hook"},
        story_repair_state={"payload": None},
        story_repair_payload=None,
        resolved_style_id=None,
        style_content="",
        style_name="",
        style_preset_id="",
    )
    built_context = generation_stream_service.ChapterGenerationStreamBuiltContext(
        chapter_context=SimpleNamespace(
            chapter_outline="outline",
            previous_chapter_summary="",
        ),
        generation_intent={"intent": "ok"},
        prompt_quality_kwargs={},
        story_runtime_contract=None,
    )
    stream_prompt = generation_stream_service.ChapterGenerationStreamPrompt(
        chapter_perspective="first-person",
        base_prompt="base prompt",
        prompt="final prompt",
    )

    payload = generation_stream_service.build_chapter_generation_stream_request_payload(
        runtime_context=runtime_context,
        built_context=built_context,
        stream_prompt=stream_prompt,
        project=runtime_context.project,
        target_word_count=1500,
        enable_mcp=True,
        custom_model=None,
        ai_service=object(),
        build_runtime_system_prompt_fn=lambda **kwargs: "system prompt",
        calculate_max_tokens_fn=lambda target_word_count: 2048,
        build_request_options_fn=lambda ai_service: None,
        detect_style_profile_fn=lambda **kwargs: "default",
        resolve_generation_temperature_fn=lambda style_profile: 0.8,
    )

    assert payload.generate_kwargs == {
        "prompt": "final prompt",
        "system_prompt": "system prompt",
        "tool_choice": "auto",
        "auto_mcp": True,
        "max_tokens": 2048,
        "temperature": 0.8,
    }



@pytest.mark.asyncio
async def test_should_create_chapter_generation_candidate_execution():
    recorded_kwargs: dict[str, Any] = {}

    async def fake_candidate_generator(**kwargs: Any) -> dict[str, Any]:
        recorded_kwargs.update(kwargs)
        return {"full_content": "draft", "candidate_index": 1}

    execution = generation_stream_service.create_chapter_generation_candidate_execution(
        ai_service=object(),
        generate_kwargs={"prompt": "hello"},
        target_word_count=1800,
        chapter_id="chapter-1",
        quality_evaluator=lambda content: {"overall_score": 90.0},
        quality_gate_plan_builder=lambda metrics, attempt_offset: {"action": "continue"},
        max_candidates=2,
        candidate_generator_fn=fake_candidate_generator,
    )

    assert execution.runtime_state["candidate_total"] == 2
    assert execution.runtime_state["candidate_index"] == 1
    result = await execution.selected_candidate_task
    assert result["full_content"] == "draft"
    assert recorded_kwargs["generation_label"] == "chapter_id=chapter-1"
    assert recorded_kwargs["runtime_state"] is execution.runtime_state


@pytest.mark.asyncio
async def test_should_wait_for_candidate_and_emit_heartbeat_progress():
    runtime_state = {
        "candidate_total": 2,
        "candidate_index": 2,
        "current_chars": 321,
    }
    events: list[tuple[str, Any]] = []

    async def fake_selected_candidate() -> dict[str, Any]:
        await asyncio.sleep(0.03)
        return {"full_content": "draft", "candidate_index": 2}

    async def emit_generating(**kwargs: Any) -> None:
        events.append(("generating", kwargs))

    async def emit_heartbeat() -> None:
        events.append(("heartbeat", None))

    task = asyncio.create_task(fake_selected_candidate())
    result = await generation_stream_service.wait_for_chapter_generation_candidate(
        selected_candidate_task=task,
        runtime_state=runtime_state,
        target_word_count=1800,
        heartbeat_interval_seconds=0.01,
        default_candidate_total=2,
        emit_generating_fn=emit_generating,
        emit_heartbeat_fn=emit_heartbeat,
    )

    assert result["full_content"] == "draft"
    assert events[0][0] == "generating"
    assert events[0][1]["current_chars"] == 321
    assert events[0][1]["retry_count"] == 1
    assert events[1] == ("heartbeat", None)



def test_should_build_candidate_quality_hooks_and_evaluate_metrics():
    runtime_context = generation_stream_service.ChapterGenerationStreamRuntimeContext(
        chapter=Chapter(id="chapter-1", project_id="project-1", chapter_number=2, title="Test Chapter"),
        project=Project(id="project-1", title="Test Project", user_id="user-1", outline_mode="one-to-one", world_rules="ruleA"),
        outline=None,
        outline_mode="one-to-one",
        quality_profile={"resolved_style_id": 7},
        story_packet=SimpleNamespace(guidance={"creative_mode": "hook"}),
        generation_guidance={"creative_mode": "hook"},
        story_repair_state={"payload": {"summary": "repair brief"}},
        story_repair_payload={"summary": "repair brief"},
        resolved_style_id=7,
        style_content="style guide",
        style_name="noir",
        style_preset_id="preset-7",
    )
    built_context = generation_stream_service.ChapterGenerationStreamBuiltContext(
        chapter_context=SimpleNamespace(chapter_outline="outline"),
        generation_intent={"intent": "ok"},
        prompt_quality_kwargs={},
        story_runtime_contract=None,
    )
    captured_runtime_kwargs: dict[str, Any] = {}
    captured_metric_kwargs: dict[str, Any] = {}

    def fake_build_quality_runtime_context(**kwargs: Any) -> dict[str, Any]:
        captured_runtime_kwargs.update(kwargs)
        return {"runtime": True}

    def fake_compute_story_quality_metrics(**kwargs: Any) -> dict[str, Any]:
        captured_metric_kwargs.update(kwargs)
        return {
            "overall_score": 91.0,
            "conflict_chain_hit_rate": 88.0,
            "rule_grounding_hit_rate": 90.0,
        }

    hooks = generation_stream_service.build_chapter_generation_candidate_quality_hooks(
        runtime_context=runtime_context,
        built_context=built_context,
        target_word_count=1800,
        build_quality_runtime_context_fn=fake_build_quality_runtime_context,
        compute_story_quality_metrics_fn=fake_compute_story_quality_metrics,
        resolve_quality_gate_execution_plan_fn=lambda *args, **kwargs: {"action": "continue"},
    )

    metrics = hooks.quality_evaluator("draft text")

    assert metrics["overall_score"] == 91.0
    assert captured_runtime_kwargs["target_word_count"] == 1800
    assert captured_runtime_kwargs["generation_intent"] == {"intent": "ok"}
    assert captured_metric_kwargs["content"] == "draft text"
    assert captured_metric_kwargs["chapter_outline"] == "outline"
    assert captured_metric_kwargs["world_rules"] == "ruleA"
    assert captured_metric_kwargs["quality_runtime_context"] == {"runtime": True}


def test_should_build_candidate_quality_gate_plan_from_hooks():
    runtime_context = generation_stream_service.ChapterGenerationStreamRuntimeContext(
        chapter=Chapter(id="chapter-1", project_id="project-1", chapter_number=2, title="Test Chapter"),
        project=Project(id="project-1", title="Test Project", user_id="user-1", outline_mode="one-to-one"),
        outline=None,
        outline_mode="one-to-one",
        quality_profile={"resolved_style_id": 7},
        story_packet=SimpleNamespace(guidance={"creative_mode": "hook"}),
        generation_guidance={"creative_mode": "hook"},
        story_repair_state={"payload": {"summary": "repair brief"}},
        story_repair_payload={"summary": "repair brief"},
        resolved_style_id=7,
        style_content="style guide",
        style_name="noir",
        style_preset_id="preset-7",
    )
    built_context = generation_stream_service.ChapterGenerationStreamBuiltContext(
        chapter_context=SimpleNamespace(chapter_outline="outline"),
        generation_intent={"intent": "ok"},
        prompt_quality_kwargs={},
        story_runtime_contract=None,
    )
    captured_gate_kwargs: dict[str, Any] = {}

    def fake_resolve_quality_gate_execution_plan(candidate_metrics: Any, **kwargs: Any) -> dict[str, Any]:
        captured_gate_kwargs["candidate_metrics"] = candidate_metrics
        captured_gate_kwargs.update(kwargs)
        return {"action": "retry", "quality_gate": {"decision": "auto_repair"}}

    hooks = generation_stream_service.build_chapter_generation_candidate_quality_hooks(
        runtime_context=runtime_context,
        built_context=built_context,
        target_word_count=1800,
        build_quality_runtime_context_fn=lambda **kwargs: {"runtime": True},
        compute_story_quality_metrics_fn=lambda **kwargs: {"overall_score": 91.0},
        resolve_quality_gate_execution_plan_fn=fake_resolve_quality_gate_execution_plan,
        retry_count=0,
        max_retries=1,
        scope="chapter",
    )

    plan = hooks.quality_gate_plan_builder({"overall_score": 81.0}, 0)

    assert plan["action"] == "retry"
    assert captured_gate_kwargs["candidate_metrics"] == {"overall_score": 81.0}
    assert captured_gate_kwargs["retry_count"] == 0
    assert captured_gate_kwargs["max_retries"] == 1
    assert captured_gate_kwargs["current_story_repair_payload"] == {"summary": "repair brief"}
    assert captured_gate_kwargs["scope"] == "chapter"



def test_should_build_selected_candidate_outcome_for_followup_retry():
    draft_calls: dict[str, Any] = {}

    def fake_build_draft_attempt(**kwargs: Any) -> dict[str, Any]:
        draft_calls.update(kwargs)
        return {"draft": True, **kwargs}

    outcome = generation_stream_service.build_chapter_generation_selected_candidate_outcome(
        selected_candidate={
            "full_content": "draft",
            "word_count": 1234,
            "candidate_chunks": ["片段A", "片段B"],
            "quality_metrics": {"overall_score": 76.0},
            "quality_gate_plan": {
                "action": "retry",
                "message": "needs review",
                "quality_gate": {"decision": "auto_repair", "status": "repairable"},
                "active_story_repair_payload": {"summary": "repair brief"},
            },
        },
        story_runtime_contract={"contract": True},
        previous_content="",
        previous_word_count=0,
        project_id="project-1",
        chapter_id="chapter-1",
        build_draft_attempt_fn=fake_build_draft_attempt,
        attach_story_runtime_contract_fn=lambda metrics, contract: {**(metrics or {}), "story_runtime_contract": contract},
    )

    assert outcome.full_content == "draft"
    assert outcome.candidate_word_count == 1234
    assert outcome.candidate_chunks == ["片段A", "片段B"]
    assert outcome.quality_gate_action == "retry"
    assert outcome.quality_gate_requires_followup is True
    assert outcome.content_applied is False
    assert outcome.provisional_draft_allowed is True
    assert outcome.quality_metrics["quality_gate"]["decision"] == "auto_repair"
    assert outcome.quality_metrics["story_runtime_contract"] == {"contract": True}
    assert outcome.draft_attempt["draft"] is True
    assert draft_calls["repair_payload"] == {"summary": "repair brief"}


def test_should_build_selected_candidate_outcome_for_applied_content():
    outcome = generation_stream_service.build_chapter_generation_selected_candidate_outcome(
        selected_candidate={
            "full_content": "draft",
            "candidate_chunks": ["片段A"],
            "quality_metrics": {"overall_score": 92.0},
            "quality_gate_plan": {
                "action": "continue",
                "message": "ok",
                "quality_gate": {"decision": "allow_save", "status": "pass"},
            },
        },
        story_runtime_contract=None,
        previous_content="draft",
        previous_word_count=800,
        project_id="project-1",
        chapter_id="chapter-1",
        build_draft_attempt_fn=lambda **kwargs: {"draft": True},
        attach_story_runtime_contract_fn=lambda metrics, contract: metrics,
    )

    assert outcome.candidate_word_count == len("draft")
    assert outcome.quality_gate_action == "continue"
    assert outcome.quality_gate_requires_followup is False
    assert outcome.content_applied is True
    assert outcome.attempt_state == "applied"
    assert outcome.provisional_draft_allowed is False
    assert outcome.draft_attempt is None


def test_should_apply_generation_outcome_and_build_history_for_completed_content():
    chapter = Chapter(
        id="chapter-1",
        project_id="project-1",
        chapter_number=3,
        title="Applied Chapter",
        content="old",
        word_count=3,
        status="draft",
    )
    project = Project(id="project-1", title="测试项目", user_id="user-1", current_words=3)
    payload_calls: dict[str, Any] = {}
    outcome = generation_stream_service.ChapterGenerationSelectedCandidateOutcome(
        full_content="final text",
        candidate_word_count=5,
        candidate_chunks=["片段A"],
        quality_metrics={"overall_score": 95.0},
        quality_gate_plan={"action": "continue"},
        quality_gate_action="continue",
        quality_gate_requires_followup=False,
        quality_gate_message=None,
        quality_gate_snapshot=None,
        content_applied=True,
        attempt_state="applied",
        draft_attempt=None,
        provisional_draft_allowed=False,
    )

    def fake_build_generation_history_payload(full_content: str, quality_metrics: Any, **kwargs: Any) -> str:
        payload_calls.update({
            "full_content": full_content,
            "quality_metrics": quality_metrics,
            **kwargs,
        })
        return "history-payload"

    preparation = generation_stream_service.apply_chapter_generation_outcome_and_build_history(
        chapter=chapter,
        project=project,
        outcome=outcome,
        story_runtime_contract={"contract": True},
        build_generation_history_payload_fn=fake_build_generation_history_payload,
        history_model="test-model",
    )

    assert chapter.content == "final text"
    assert chapter.word_count == 5
    assert chapter.status == "completed"
    assert project.current_words == 5
    assert preparation.previous_content == "old"
    assert preparation.previous_word_count == 3
    assert preparation.previous_status == "draft"
    assert preparation.saved_word_count == 5
    assert preparation.provisional_draft_saved is False
    assert preparation.history.project_id == "project-1"
    assert preparation.history.chapter_id == "chapter-1"
    assert preparation.history.generated_content == "history-payload"
    assert preparation.history.model == "test-model"
    assert payload_calls["full_content"] == "final text"
    assert payload_calls["content_applied"] is True
    assert payload_calls["attempt_state"] == "applied"
    assert payload_calls["story_runtime_contract"] == {"contract": True}


def test_should_apply_generation_outcome_as_provisional_draft_when_retry_allows_save():
    chapter = Chapter(
        id="chapter-1",
        project_id="project-1",
        chapter_number=1,
        title="Retry Chapter",
        content=None,
        word_count=0,
        status=None,
    )
    project = Project(id="project-1", title="测试项目", user_id="user-1", current_words=0)
    payload_calls: dict[str, Any] = {}
    outcome = generation_stream_service.ChapterGenerationSelectedCandidateOutcome(
        full_content="draft fix",
        candidate_word_count=5,
        candidate_chunks=["片段A"],
        quality_metrics={"overall_score": 78.0},
        quality_gate_plan={"action": "retry"},
        quality_gate_action="retry",
        quality_gate_requires_followup=True,
        quality_gate_message="needs review",
        quality_gate_snapshot={"decision": "auto_repair"},
        content_applied=False,
        attempt_state="retry",
        draft_attempt={"draft": True},
        provisional_draft_allowed=True,
    )

    def fake_build_generation_history_payload(full_content: str, quality_metrics: Any, **kwargs: Any) -> str:
        payload_calls.update({
            "full_content": full_content,
            "quality_metrics": quality_metrics,
            **kwargs,
        })
        return "history-payload"

    preparation = generation_stream_service.apply_chapter_generation_outcome_and_build_history(
        chapter=chapter,
        project=project,
        outcome=outcome,
        story_runtime_contract=None,
        build_generation_history_payload_fn=fake_build_generation_history_payload,
    )

    assert chapter.content == "draft fix"
    assert chapter.word_count == 5
    assert chapter.status == "draft"
    assert project.current_words == 5
    assert preparation.previous_content == ""
    assert preparation.previous_word_count == 0
    assert preparation.previous_status is None
    assert preparation.saved_word_count == 5
    assert preparation.provisional_draft_saved is True
    assert payload_calls["content_applied"] is False
    assert payload_calls["attempt_state"] == "retry"


def test_should_build_analysis_followup_plan_for_quality_gate_retry():
    plan = generation_stream_service.build_chapter_generation_analysis_followup_plan(
        enable_analysis=False,
        quality_gate_action="retry",
        quality_gate_requires_followup=True,
        full_content="draft",
        candidate_word_count=456,
    )

    assert plan.should_schedule_analysis is True
    assert plan.analysis_reason == "quality_gate_auto_repair"
    assert plan.chapter_content_override == "draft"
    assert plan.chapter_word_count_override == 456
    assert plan.completion_message == "章节生成完成，已转入质量修复"
    assert plan.analysis_started_message == "质量修复分析任务已启动"


def test_should_build_stream_response_artifacts_with_candidate_draft_and_analysis_event():
    chapter = Chapter(
        id="chapter-1",
        project_id="project-1",
        chapter_number=2,
        title="Blocked Chapter",
        status="draft",
    )
    chapter.updated_at = None
    draft_calls: dict[str, Any] = {}
    result_calls: dict[str, Any] = {}

    def fake_build_candidate_draft_payload(**kwargs: Any) -> dict[str, Any]:
        draft_calls.update(kwargs)
        return {"attempt_id": "draft-1", "can_apply": True}

    def fake_build_stream_result_payload(**kwargs: Any) -> dict[str, Any]:
        result_calls.update(kwargs)
        return {"ok": True, **kwargs}

    artifacts = generation_stream_service.build_chapter_generation_stream_response_artifacts(
        chapter=chapter,
        draft_attempt={"draft": True},
        quality_metrics={"overall_score": 80.0},
        quality_gate_action="manual_review",
        quality_gate_message="needs review",
        quality_gate_snapshot={"decision": "manual_review", "status": "blocked"},
        quality_gate_requires_followup=True,
        content_applied=False,
        saved_word_count=321,
        task_id="task-1",
        story_runtime_contract={"contract": True},
        analysis_started_message="人工复核分析任务已启动",
        build_candidate_draft_payload_fn=fake_build_candidate_draft_payload,
        build_stream_result_payload_fn=fake_build_stream_result_payload,
    )

    assert artifacts.quality_metrics_event_payload["type"] == "quality_metrics"
    assert artifacts.quality_metrics_event_payload["overall_score"] == 80.0
    assert artifacts.quality_gate_event_payload["type"] == "quality_gate_blocked"
    assert artifacts.quality_gate_event_payload["progress"] == 95
    assert artifacts.result_payload["ok"] is True
    assert artifacts.result_payload["candidate_draft"]["attempt_id"] == "draft-1"
    assert artifacts.analysis_started_event_data == {
        "task_id": "task-1",
        "message": "人工复核分析任务已启动",
    }
    assert draft_calls["draft_attempt"] == {"draft": True}
    assert result_calls["saved_word_count"] == 321
    assert result_calls["hard_gate_blocked"] is True
    assert result_calls["candidate_draft"]["attempt_id"] == "draft-1"


@pytest.mark.asyncio
async def test_should_prepare_analysis_scheduling_with_background_kwargs():
    db_session = AsyncMock()
    created_calls: dict[str, Any] = {}
    followup_plan = generation_stream_service.ChapterGenerationAnalysisFollowupPlan(
        should_schedule_analysis=True,
        analysis_reason="quality_gate_auto_repair",
        chapter_content_override="draft",
        chapter_word_count_override=456,
        completion_message="章节生成完成，已转入质量修复",
        analysis_started_message="质量修复分析任务已启动",
    )

    async def fake_create_analysis_task(*args: Any, **kwargs: Any) -> Any:
        created_calls.update(kwargs)
        return SimpleNamespace(id="task-1")

    scheduling = await generation_stream_service.prepare_chapter_generation_analysis_scheduling(
        db_session,
        chapter_id="chapter-1",
        user_id="user-1",
        project_id="project-1",
        followup_plan=followup_plan,
        ai_service="ai-service",
        quality_profile={"name": "default"},
        story_packet={"packet": True},
        create_analysis_task_fn=fake_create_analysis_task,
    )

    assert scheduling.task_id == "task-1"
    assert created_calls["log_context"] == "stream:quality_gate_auto_repair"
    assert scheduling.background_task_kwargs["chapter_id"] == "chapter-1"
    assert scheduling.background_task_kwargs["task_id"] == "task-1"
    assert scheduling.background_task_kwargs["chapter_content_override"] == "draft"
    assert scheduling.background_task_kwargs["chapter_word_count_override"] == 456


@pytest.mark.asyncio
async def test_should_skip_analysis_scheduling_when_followup_not_needed():
    scheduling = await generation_stream_service.prepare_chapter_generation_analysis_scheduling(
        AsyncMock(),
        chapter_id="chapter-1",
        user_id="user-1",
        project_id="project-1",
        followup_plan=generation_stream_service.ChapterGenerationAnalysisFollowupPlan(
            should_schedule_analysis=False,
            analysis_reason=None,
            chapter_content_override=None,
            chapter_word_count_override=None,
            completion_message="章节生成完成",
            analysis_started_message=None,
        ),
        ai_service="ai-service",
        quality_profile={},
        story_packet={},
        create_analysis_task_fn=AsyncMock(),
    )

    assert scheduling.task_id is None
    assert scheduling.background_task_kwargs is None


@pytest.mark.asyncio
async def test_should_run_post_persist_effects_and_plant_foreshadows_when_content_applied():
    chapter = Chapter(
        id="chapter-1",
        project_id="project-1",
        chapter_number=2,
        title="Persisted Chapter",
        status="completed",
    )
    project = Project(id="project-1", title="测试项目", user_id="user-1")
    db_session = AsyncMock()
    calls: dict[str, Any] = {}

    async def fake_auto_plant_pending_foreshadows(**kwargs: Any) -> dict[str, Any]:
        calls.update(kwargs)
        return {"planted_count": 2}

    effects = await generation_stream_service.run_chapter_generation_post_persist_effects(
        db_session,
        chapter_id="chapter-1",
        chapter=chapter,
        project=project,
        full_content="draft",
        candidate_word_count=321,
        content_applied=True,
        provisional_draft_saved=False,
        previous_status="draft",
        auto_plant_pending_foreshadows_fn=fake_auto_plant_pending_foreshadows,
    )

    assert effects.planted_count == 2
    assert effects.plant_error is None
    assert calls["db"] is db_session
    assert calls["project_id"] == "project-1"
    assert calls["chapter_id"] == "chapter-1"
    assert calls["chapter_number"] == 2
    assert calls["chapter_content"] == "draft"


@pytest.mark.asyncio
async def test_should_swallow_post_persist_foreshadow_errors():
    chapter = Chapter(
        id="chapter-1",
        project_id="project-1",
        chapter_number=2,
        title="Persisted Chapter",
        status="completed",
    )
    project = Project(id="project-1", title="测试项目", user_id="user-1")

    async def fake_auto_plant_pending_foreshadows(**kwargs: Any) -> dict[str, Any]:
        raise RuntimeError("plant failed")

    effects = await generation_stream_service.run_chapter_generation_post_persist_effects(
        AsyncMock(),
        chapter_id="chapter-1",
        chapter=chapter,
        project=project,
        full_content="draft",
        candidate_word_count=321,
        content_applied=True,
        provisional_draft_saved=False,
        previous_status="draft",
        auto_plant_pending_foreshadows_fn=fake_auto_plant_pending_foreshadows,
    )

    assert effects.planted_count == 0
    assert effects.plant_error == "plant failed"


def test_should_build_stream_emission_plan_in_expected_order():
    response_artifacts = generation_stream_service.ChapterGenerationStreamResponseArtifacts(
        quality_metrics_event_payload={"type": "quality_metrics", "overall_score": 88.0},
        quality_gate_event_payload={"type": "quality_gate_retry", "progress": 88},
        result_payload={"type": "result", "word_count": 500},
        analysis_started_event_data={"task_id": "task-1", "message": "analysis started"},
    )

    plan = generation_stream_service.build_chapter_generation_stream_emission_plan(
        completion_message="章节生成完成，已转入质量修复",
        response_artifacts=response_artifacts,
    )

    assert [step.kind for step in plan] == [
        "tracker_complete",
        "sse_payload",
        "sse_payload",
        "tracker_result",
        "sse_event",
        "tracker_done",
    ]
    assert plan[0].message == "章节生成完成，已转入质量修复"
    assert plan[1].payload["type"] == "quality_metrics"
    assert plan[2].payload["type"] == "quality_gate_retry"
    assert plan[3].payload["word_count"] == 500
    assert plan[4].event == "analysis_started"
    assert plan[4].payload["task_id"] == "task-1"


@pytest.mark.asyncio
async def test_should_emit_stream_plan_in_expected_order():
    emission_plan = [
        generation_stream_service.ChapterGenerationEmissionStep(kind="tracker_complete", message="done generating"),
        generation_stream_service.ChapterGenerationEmissionStep(kind="sse_payload", payload={"type": "quality_metrics"}),
        generation_stream_service.ChapterGenerationEmissionStep(kind="tracker_result", payload={"word_count": 500}),
        generation_stream_service.ChapterGenerationEmissionStep(kind="sse_event", event="analysis_started", payload={"task_id": "task-1"}),
        generation_stream_service.ChapterGenerationEmissionStep(kind="tracker_done"),
    ]
    calls: list[tuple[str, Any]] = []

    async def fake_tracker_complete(message: str) -> str:
        calls.append(("tracker_complete", message))
        return f"complete:{message}"

    async def fake_tracker_result(payload: dict[str, Any]) -> str:
        calls.append(("tracker_result", payload))
        return f"result:{payload['word_count']}"

    async def fake_tracker_done() -> str:
        calls.append(("tracker_done", None))
        return "done"

    def fake_format_sse(payload: dict[str, Any]) -> str:
        calls.append(("sse_payload", payload))
        return f"sse:{payload['type']}"

    async def fake_send_event(*, event: str, data: dict[str, Any]) -> str:
        calls.append(("sse_event", {"event": event, "data": data}))
        return f"event:{event}"

    emitted = []
    async for item in generation_stream_service.emit_chapter_generation_stream_plan(
        emission_plan=emission_plan,
        tracker_complete_fn=fake_tracker_complete,
        tracker_result_fn=fake_tracker_result,
        tracker_done_fn=fake_tracker_done,
        format_sse_fn=fake_format_sse,
        send_event_fn=fake_send_event,
    ):
        emitted.append(item)

    assert emitted == [
        "complete:done generating",
        "sse:quality_metrics",
        "result:500",
        "event:analysis_started",
        "done",
    ]
    assert [name for name, _ in calls] == [
        "tracker_complete",
        "sse_payload",
        "tracker_result",
        "sse_event",
        "tracker_done",
    ]
