from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest


MODULE_PATH = Path(__file__).resolve().parents[2] / "tools" / "run_novel_autopilot_smoke.py"
TOOLS_PATH = str(MODULE_PATH.parent)


def load_smoke_module():
    if TOOLS_PATH not in sys.path:
        sys.path.insert(0, TOOLS_PATH)
    spec = importlib.util.spec_from_file_location("backend_tools_run_novel_autopilot_smoke", MODULE_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("unable to load run_novel_autopilot_smoke.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_long_chapter_matches_real_generation_quality_fixture_contract():
    smoke = load_smoke_module()

    content = smoke.long_chapter("SMOKE_GENERATED_CHAPTER_1", 1, "返修说明。")

    assert 2_700 <= len(content) <= 3_600
    assert "SMOKE_GENERATED_CHAPTER_1" in content
    assert "钟声会暴露被点名者的身份" in content
    assert (
        "林澈在雾港推进第1阶段调查，目标受到守钟规则阻碍；"
        "她选择留下可验证证据，并立即承担暴露行踪的后果。"
    ) in content
    assert "核对第1组记录" in content
    assert "触发第1次钟声规则" in content
    assert content.count("选择") >= 6
    assert content.count("代价") >= 6
    assert content.count("“") >= 12
    ending = content[-360:]
    assert "真相" in ending
    assert "脚步声" in ending
    assert "逼近" in ending
    assert "下一秒" in ending
    assert content.rstrip().endswith("？")


def test_run_failure_summary_includes_latest_quality_failure():
    smoke = load_smoke_module()
    run = {
        "status": "waiting_human",
        "current_phase": "chapter_loop",
        "current_step": None,
        "version": 38,
        "last_error_code": "chapter_generation_attempts_exhausted",
    }
    steps = [
        {
            "step_key": "chapter:0001:generate",
            "attempt": 3,
            "status": "failed",
            "error_code": "chapter_quality_retry",
            "quality_decision": "retry",
            "created_at": "2026-07-19T20:03:00",
        },
        {
            "step_key": "chapter:0001:generate",
            "attempt": 4,
            "status": "failed",
            "error_code": "chapter_generation_attempts_exhausted",
            "quality_decision": "retry",
            "created_at": "2026-07-19T20:03:01",
        },
    ]

    summary = smoke.run_failure_summary(run, steps)

    assert summary == {
        "status": "waiting_human",
        "phase": "chapter_loop",
        "step": "chapter:0001:generate",
        "version": 38,
        "last_error_code": "chapter_generation_attempts_exhausted",
        "step_status": "failed",
        "step_error_code": "chapter_generation_attempts_exhausted",
        "attempt": 4,
        "quality_decision": "retry",
    }


def test_wait_for_run_fails_fast_when_background_task_is_terminal(monkeypatch):
    smoke = load_smoke_module()
    run = {
        "status": "running",
        "current_phase": "book_polish",
        "current_step": "completion:book_polish:chapter:0002:chapter-2",
        "version": 54,
        "active_background_task_id": "task-book-polish",
    }
    steps = [
        {
            "step_key": run["current_step"],
            "attempt": 1,
            "status": "running",
            "created_at": "2026-07-19T20:26:00",
        }
    ]
    task = {
        "task_id": "task-book-polish",
        "task_type": "novel_book_autopilot",
        "status": "failed",
        "stage_code": "book_review",
        "updated_at": "2026-07-19T20:26:22",
        "error": "sensitive provider output must not be included",
        "data": {"content": "sensitive generated chapter"},
    }
    monkeypatch.setattr(smoke, "get_run", lambda *args, **kwargs: run)
    monkeypatch.setattr(smoke, "list_steps", lambda *args, **kwargs: steps)
    monkeypatch.setattr(smoke, "get_background_task", lambda *args, **kwargs: task)

    with pytest.raises(smoke.SmokeFailure) as caught:
        smoke.wait_for_run(
            object(),
            base_url="http://localhost:8005",
            project_id="project-1",
            run_id="run-1",
            timeout=60,
            poll_interval=0,
            predicate=lambda value: value.get("status") == "completed",
            label="complete-book run completion",
        )

    message = str(caught.value)
    assert "background task stopped before run convergence" in message
    assert "task-book-polish" in message
    assert "book_review" in message
    assert "sensitive provider output" not in message
    assert "sensitive generated chapter" not in message

def test_extract_chapter_number_prefers_task_sentence_over_parameter_guidance():
    smoke = load_smoke_module()
    prompt = """
    撰写第3章《旧塔回声》的完整正文。
    - 若chapter_number为2或3，尽量遵循黄金三章分工。
    全书章节上下文：第1章、第2章、第3章。
    """

    assert smoke.extract_chapter_number(prompt, total_chapters=3) == 3


def test_extract_chapter_number_uses_smoke_marker_for_polish_prompt():
    smoke = load_smoke_module()
    prompt = """
    <prompt_template_key value="AI_DENOISING" />
    【原文】SMOKE_GENERATED_CHAPTER_2
    全书章节上下文：第1章、第2章、第3章。
    """

    assert smoke.extract_chapter_number(prompt, total_chapters=3) == 2


def test_smoke_project_payload_keeps_foundation_incomplete():
    smoke = load_smoke_module()

    payload = smoke.build_smoke_project_payload(total_chapters=3)

    assert payload["target_words"] == 3600
    assert payload["outline_mode"] == "one-to-many"
    assert "description" not in payload
    assert "theme" not in payload
    assert "genre" not in payload
