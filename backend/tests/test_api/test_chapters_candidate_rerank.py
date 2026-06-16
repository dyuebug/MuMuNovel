from typing import Any

import pytest
from sqlalchemy import select

from app.api import chapters as chapters_api
from app.models.chapter import Chapter
from app.models.generation_history import GenerationHistory
from app.services import batch_generation_single_chapter_entry_service
from tests.test_api.chapters_test_support import (
    chapters_client,
    chapters_session_factory,
    create_chapter,
    create_project,
    fake_ai_service,
    mock_side_effect_services,
    parse_sse_data,
    reset_chapters_runtime_caches,
)

pytestmark = pytest.mark.asyncio

async def test_should_use_near_target_word_budget_repair_candidate_as_targeted_final_repair_seed():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2920
            elif len(self.calls) == 2:
                yield "B" * 1957
            elif len(self.calls) == 3:
                yield "C" * 1422
            else:
                yield "D" * 1336

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        failed_metrics: list[dict[str, str]]
        if content.startswith("A"):
            decision = "manual_review"
            overall_score = 74.0
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        elif content.startswith("B"):
            decision = "manual_review"
            overall_score = 88.0
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Conflict chain", "focus_area": "conflict"},
            ]
        elif content.startswith("C"):
            decision = "manual_review"
            overall_score = 79.0
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        else:
            decision = "allow_save"
            overall_score = 90.0
            failed_metrics = []
        return {
            "overall_score": overall_score,
            "pacing_score": 8.1,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": failed_metrics,
                "continuity_warning_count": 0,
                "overall_score": overall_score,
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need repair",
            "active_story_repair_payload": {
                "summary": "Strengthen the chapter landing without regrowing the scene.",
                "repair_targets": ["Sharpen the chapter-ending pressure"],
                "preserve_strengths": ["Keep the current continuity beats"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-targeted-repair-seed",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 4
    assert "Word-budget repair pass #3" in ai_service.calls[2]["prompt"]
    assert "Targeted quality repair pass #4" in ai_service.calls[3]["prompt"]
    assert ("C" * 120) in ai_service.calls[3]["prompt"]
    assert ("B" * 120) not in ai_service.calls[3]["prompt"]
    assert result["candidate_index"] == 4
    assert result["word_count"] == 1336
    assert result["generation_path"] == "targeted_quality_repair"
    assert result["attempt_kind"] == "targeted_quality_repair"
    assert result["quality_metrics"]["candidate_selection"]["winner_candidate_index"] == 4
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_candidate_index"] == 3
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_generation_path"] == "word_budget_repair"
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_attempt_kind"] == "word_budget_repair"
    repair_seed_items = [
        item for item in result["quality_metrics"]["candidate_pool_summary"] if item.get("is_repair_seed")
    ]
    assert len(repair_seed_items) == 1
    assert repair_seed_items[0]["candidate_index"] == 3
    targeted_repair_items = [
        item
        for item in result["quality_metrics"]["candidate_pool_summary"]
        if item.get("generation_path") == "targeted_quality_repair"
    ]
    assert len(targeted_repair_items) == 1
    assert targeted_repair_items[0]["repair_seed_candidate_index"] == 3
    assert targeted_repair_items[0]["repair_seed_generation_path"] == "word_budget_repair"
    assert targeted_repair_items[0]["repair_seed_attempt_kind"] == "word_budget_repair"

async def test_should_keep_word_budget_seed_when_targeted_final_repair_candidate_is_not_better():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2920
            elif len(self.calls) == 2:
                yield "B" * 1790
            elif len(self.calls) == 3:
                yield "C" * 1422
            else:
                yield "D" * 1433

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        failed_metrics: list[dict[str, str]]
        if content.startswith("A"):
            decision = "manual_review"
            overall_score = 74.0
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        elif content.startswith("B"):
            decision = "auto_repair"
            overall_score = 98.3
            failed_metrics = []
        elif content.startswith("C"):
            decision = "manual_review"
            overall_score = 93.1
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        else:
            decision = "manual_review"
            overall_score = 94.0
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        return {
            "overall_score": overall_score,
            "pacing_score": 8.1,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": failed_metrics,
                "continuity_warning_count": 0,
                "overall_score": overall_score,
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need repair",
            "active_story_repair_payload": {
                "summary": "Strengthen the chapter landing without regrowing the scene.",
                "repair_targets": ["Sharpen the chapter-ending pressure"],
                "preserve_strengths": ["Keep the current continuity beats"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-targeted-repair-adoption-gate",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 5
    assert "Word-budget repair pass #3" in ai_service.calls[2]["prompt"]
    assert "Targeted quality repair pass #4" in ai_service.calls[3]["prompt"]
    assert "Targeted quality repair pass #5" in ai_service.calls[4]["prompt"]
    assert ("C" * 120) in ai_service.calls[3]["prompt"]
    assert ("B" * 120) not in ai_service.calls[3]["prompt"]
    assert ("D" * 120) in ai_service.calls[4]["prompt"]
    assert ("C" * 120) not in ai_service.calls[4]["prompt"]
    assert result["candidate_index"] == 3
    assert result["generation_path"] == "word_budget_repair"
    assert result["attempt_kind"] == "word_budget_repair"
    assert result["quality_metrics"]["candidate_selection"]["winner_candidate_index"] == 3
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_candidate_index"] == 2
    targeted_repair_items = [
        item
        for item in result["quality_metrics"]["candidate_pool_summary"]
        if item.get("generation_path") == "targeted_quality_repair"
    ]
    assert len(targeted_repair_items) == 2
    assert targeted_repair_items[0]["repair_seed_candidate_index"] == 3
    assert targeted_repair_items[1]["repair_seed_candidate_index"] == 4
    assert all(item["is_winner"] is False for item in targeted_repair_items)

async def test_should_run_followup_targeted_final_repair_from_deferred_cliffhanger_seed():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2920
            elif len(self.calls) == 2:
                yield "B" * 1790
            elif len(self.calls) == 3:
                yield "C" * 1422
            elif len(self.calls) == 4:
                yield "D" * 1433
            else:
                yield "E" * 1368

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        failed_metrics: list[dict[str, str]]
        if content.startswith("A"):
            decision = "manual_review"
            overall_score = 74.0
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        elif content.startswith("B"):
            decision = "auto_repair"
            overall_score = 98.3
            failed_metrics = []
        elif content.startswith("C"):
            decision = "manual_review"
            overall_score = 93.1
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        elif content.startswith("D"):
            decision = "manual_review"
            overall_score = 90.4
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        else:
            decision = "allow_save"
            overall_score = 94.2
            failed_metrics = []
        return {
            "overall_score": overall_score,
            "pacing_score": 8.1,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": failed_metrics,
                "continuity_warning_count": 0,
                "overall_score": overall_score,
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need repair",
            "active_story_repair_payload": {
                "summary": "Strengthen the chapter landing without regrowing the scene.",
                "repair_targets": ["Sharpen the chapter-ending pressure"],
                "preserve_strengths": ["Keep the current continuity beats"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-deferred-followup-cliffhanger-repair",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 5
    assert "Targeted quality repair pass #4" in ai_service.calls[3]["prompt"]
    assert "Targeted quality repair pass #5" in ai_service.calls[4]["prompt"]
    assert "Cliffhanger hard rule" in ai_service.calls[4]["prompt"]
    assert "Cliffhanger escalation rule" in ai_service.calls[4]["prompt"]
    assert "Cliffhanger framing rule" in ai_service.calls[4]["prompt"]
    assert ("D" * 120) in ai_service.calls[4]["prompt"]
    assert ("C" * 120) not in ai_service.calls[4]["prompt"]
    assert result["candidate_index"] == 5
    assert result["word_count"] == 1368
    assert result["generation_path"] == "targeted_quality_repair"
    assert result["attempt_kind"] == "targeted_quality_repair"
    assert result["quality_gate_decision"] == "allow_save"
    assert result["quality_metrics"]["candidate_selection"]["winner_candidate_index"] == 5
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_candidate_index"] == 4
    targeted_repair_items = [
        item
        for item in result["quality_metrics"]["candidate_pool_summary"]
        if item.get("generation_path") == "targeted_quality_repair"
    ]
    assert len(targeted_repair_items) == 2
    assert targeted_repair_items[0]["repair_seed_candidate_index"] == 3
    assert targeted_repair_items[0]["is_winner"] is False
    assert targeted_repair_items[-1]["repair_seed_candidate_index"] == 4
    assert targeted_repair_items[-1]["is_winner"] is True

async def test_should_run_followup_targeted_final_repair_for_rule_grounding_and_cliffhanger_gap():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2920
            elif len(self.calls) == 2:
                yield "B" * 1790
            elif len(self.calls) == 3:
                yield "C" * 1422
            elif len(self.calls) == 4:
                yield "D" * 1416
            else:
                yield "E" * 1362

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        failed_metrics: list[dict[str, str]]
        if content.startswith("A"):
            decision = "manual_review"
            overall_score = 74.0
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        elif content.startswith("B"):
            decision = "auto_repair"
            overall_score = 98.3
            failed_metrics = []
        elif content.startswith("C"):
            decision = "manual_review"
            overall_score = 92.0
            failed_metrics = [
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        elif content.startswith("D"):
            decision = "manual_review"
            overall_score = 91.4
            failed_metrics = [
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        else:
            decision = "allow_save"
            overall_score = 94.8
            failed_metrics = []
        return {
            "overall_score": overall_score,
            "pacing_score": 8.1,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": failed_metrics,
                "continuity_warning_count": 0,
                "overall_score": overall_score,
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need repair",
            "active_story_repair_payload": {
                "summary": "Strengthen the chapter landing without regrowing the scene.",
                "repair_targets": ["Make the governing rule trigger the final chapter hook"],
                "preserve_strengths": ["Keep the current continuity beats"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-followup-targeted-repair-rule-and-hook",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 5
    assert "Targeted quality repair pass #4" in ai_service.calls[3]["prompt"]
    assert "Targeted quality repair pass #5" in ai_service.calls[4]["prompt"]
    assert "Joint repair focus" in ai_service.calls[3]["prompt"]
    assert "Joint closing hard rule" in ai_service.calls[4]["prompt"]
    assert ("D" * 120) in ai_service.calls[4]["prompt"]
    assert ("C" * 120) not in ai_service.calls[4]["prompt"]
    assert result["candidate_index"] == 5
    assert result["word_count"] == 1362
    assert result["generation_path"] == "targeted_quality_repair"
    assert result["attempt_kind"] == "targeted_quality_repair"
    assert result["quality_gate_decision"] == "allow_save"
    assert result["quality_metrics"]["candidate_selection"]["winner_candidate_index"] == 5
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_candidate_index"] == 4

async def test_should_prefer_word_budget_repair_candidate_over_severely_overlong_auto_repair_winner():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2903
            elif len(self.calls) == 2:
                yield "B" * 1994
            else:
                yield "C" * 1422

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        failed_metrics: list[dict[str, str]]
        if content.startswith("A"):
            decision = "auto_repair"
            overall_score = 80.5
            failed_metrics = [
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
                {"label": "Conflict chain", "focus_area": "conflict"},
            ]
        elif content.startswith("B"):
            decision = "manual_review"
            overall_score = 80.9
            failed_metrics = [
                {"label": "Conflict chain", "focus_area": "conflict"},
            ]
        else:
            decision = "manual_review"
            overall_score = 68.6
            failed_metrics = [
                {"label": "Conflict chain", "focus_area": "conflict"},
            ]
        return {
            "overall_score": overall_score,
            "pacing_score": 8.1,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": failed_metrics,
                "continuity_warning_count": 0,
                "overall_score": overall_score,
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need repair",
            "active_story_repair_payload": {
                "summary": "Compress the chapter while preserving the core conflict chain.",
                "repair_targets": ["Cut the chapter down to the target window without losing the visible blocker"],
                "preserve_strengths": ["Keep the core plot pressure on-page"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-word-budget-repair-beats-severe-auto-repair",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 3
    assert result["candidate_index"] == 3
    assert result["word_count"] == 1422
    assert result["generation_path"] == "word_budget_repair"
    assert result["attempt_kind"] == "word_budget_repair"
    assert result["quality_metrics"]["candidate_selection"]["winner_candidate_index"] == 3

async def test_should_run_targeted_final_repair_for_opening_conflict_cliffhanger_gap():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2395
            elif len(self.calls) == 2:
                yield "B" * 1801
            elif len(self.calls) == 3:
                yield "C" * 1422
            else:
                yield "D" * 1368

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        failed_metrics: list[dict[str, str]]
        if content.startswith("A"):
            decision = "manual_review"
            overall_score = 70.0
            failed_metrics = [
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Opening", "focus_area": "opening"},
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        elif content.startswith("B"):
            decision = "manual_review"
            overall_score = 60.7
            failed_metrics = [
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Opening", "focus_area": "opening"},
            ]
        elif content.startswith("C"):
            decision = "manual_review"
            overall_score = 90.6
            failed_metrics = [
                {"label": "Opening", "focus_area": "opening"},
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        else:
            decision = "allow_save"
            overall_score = 93.8
            failed_metrics = []
        return {
            "overall_score": overall_score,
            "pacing_score": 8.1,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": failed_metrics,
                "continuity_warning_count": 0,
                "overall_score": overall_score,
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need repair",
            "active_story_repair_payload": {
                "summary": "Tighten the opening, visible blocker, and chapter-ending hook into one clean chain.",
                "repair_targets": ["Make the opening anomaly create the blocker and the blocker create the closing hook"],
                "preserve_strengths": ["Keep the existing chapter mission and continuity beats"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-targeted-repair-opening-conflict-cliffhanger",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 4
    assert "Word-budget repair pass #3" in ai_service.calls[2]["prompt"]
    assert "Targeted quality repair pass #4" in ai_service.calls[3]["prompt"]
    assert "Three-beat repair focus" in ai_service.calls[3]["prompt"]
    assert "Opening repair focus" in ai_service.calls[3]["prompt"]
    assert result["candidate_index"] == 4
    assert result["word_count"] == 1368
    assert result["generation_path"] == "targeted_quality_repair"
    assert result["attempt_kind"] == "targeted_quality_repair"
    assert result["quality_gate_decision"] == "allow_save"
    assert result["quality_metrics"]["candidate_selection"]["winner_candidate_index"] == 4
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_candidate_index"] == 3

async def test_should_run_targeted_final_repair_for_opening_rule_grounding_cliffhanger_gap():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2410
            elif len(self.calls) == 2:
                yield "B" * 1812
            elif len(self.calls) == 3:
                yield "C" * 1421
            else:
                yield "D" * 1372

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        failed_metrics: list[dict[str, str]]
        if content.startswith("A"):
            decision = "manual_review"
            overall_score = 71.0
            failed_metrics = [
                {"label": "Opening", "focus_area": "opening"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        elif content.startswith("B"):
            decision = "manual_review"
            overall_score = 62.5
            failed_metrics = [
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Opening", "focus_area": "opening"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        elif content.startswith("C"):
            decision = "manual_review"
            overall_score = 90.5
            failed_metrics = [
                {"label": "Opening", "focus_area": "opening"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        else:
            decision = "allow_save"
            overall_score = 94.0
            failed_metrics = []
        return {
            "overall_score": overall_score,
            "pacing_score": 8.1,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": failed_metrics,
                "continuity_warning_count": 0,
                "overall_score": overall_score,
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need repair",
            "active_story_repair_payload": {
                "summary": "Fuse the opening hook, grounded rule pressure, and chapter-ending spike into one causal line.",
                "repair_targets": ["Make the opening anomaly reveal the governing rule and let that same rule detonate the final unresolved hook"],
                "preserve_strengths": ["Keep the existing chapter mission and continuity beats"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-targeted-repair-opening-rule-grounding-cliffhanger",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 4
    assert "Word-budget repair pass #3" in ai_service.calls[2]["prompt"]
    assert "Targeted quality repair pass #4" in ai_service.calls[3]["prompt"]
    assert "Opening repair focus" in ai_service.calls[3]["prompt"]
    assert "Rule-grounding repair focus" in ai_service.calls[3]["prompt"]
    assert "Cliffhanger hard rule" in ai_service.calls[3]["prompt"]
    assert "Joint repair focus: make the opening anomaly or urgent demand" in ai_service.calls[3]["prompt"]
    assert "Joint triad hard rule" in ai_service.calls[3]["prompt"]
    assert result["candidate_index"] == 4
    assert result["word_count"] == 1372
    assert result["generation_path"] == "targeted_quality_repair"
    assert result["attempt_kind"] == "targeted_quality_repair"
    assert result["quality_gate_decision"] == "allow_save"
    assert result["quality_metrics"]["candidate_selection"]["winner_candidate_index"] == 4
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_candidate_index"] == 3

async def test_should_run_targeted_final_repair_for_dialogue_and_cliffhanger_gap():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2920
            elif len(self.calls) == 2:
                yield "B" * 1795
            elif len(self.calls) == 3:
                yield "C" * 1428
            else:
                yield "D" * 1366

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        failed_metrics: list[dict[str, str]]
        if content.startswith("A"):
            decision = "manual_review"
            overall_score = 74.0
            failed_metrics = [
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        elif content.startswith("B"):
            decision = "auto_repair"
            overall_score = 98.3
            failed_metrics = []
        elif content.startswith("C"):
            decision = "manual_review"
            overall_score = 91.2
            failed_metrics = [
                {"label": "Dialogue", "focus_area": "dialogue"},
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        else:
            decision = "allow_save"
            overall_score = 94.4
            failed_metrics = []
        return {
            "overall_score": overall_score,
            "pacing_score": 8.1,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": failed_metrics,
                "continuity_warning_count": 0,
                "overall_score": overall_score,
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need repair",
            "active_story_repair_payload": {
                "summary": "Sharpen the decisive exchange and let it detonate the chapter-ending hook.",
                "repair_targets": ["Make the leverage shift inside dialogue create the final unresolved spike"],
                "preserve_strengths": ["Keep the current chapter mission and continuity beats"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-targeted-repair-dialogue-cliffhanger",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 4
    assert "Word-budget repair pass #3" in ai_service.calls[2]["prompt"]
    assert "Targeted quality repair pass #4" in ai_service.calls[3]["prompt"]
    assert "Joint repair focus" in ai_service.calls[3]["prompt"]
    assert "Joint dialogue-cliffhanger hard rule" in ai_service.calls[3]["prompt"]
    assert ("C" * 120) in ai_service.calls[3]["prompt"]
    assert ("B" * 120) not in ai_service.calls[3]["prompt"]
    assert result["candidate_index"] == 4
    assert result["word_count"] == 1366
    assert result["generation_path"] == "targeted_quality_repair"
    assert result["attempt_kind"] == "targeted_quality_repair"
    assert result["quality_gate_decision"] == "allow_save"
    assert result["quality_metrics"]["candidate_selection"]["winner_candidate_index"] == 4
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_candidate_index"] == 3

async def test_should_run_targeted_final_repair_for_opening_and_rule_grounding_gap():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2821
            elif len(self.calls) == 2:
                yield "B" * 1883
            elif len(self.calls) == 3:
                yield "C" * 1420
            else:
                yield "D" * 1364

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        failed_metrics: list[dict[str, str]]
        if content.startswith("A"):
            decision = "manual_review"
            overall_score = 78.8
            failed_metrics = [
                {"label": "Opening", "focus_area": "opening"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        elif content.startswith("B"):
            decision = "manual_review"
            overall_score = 72.6
            failed_metrics = [
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
                {"label": "Opening", "focus_area": "opening"},
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        elif content.startswith("C"):
            decision = "manual_review"
            overall_score = 90.2
            failed_metrics = [
                {"label": "Opening", "focus_area": "opening"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        else:
            decision = "allow_save"
            overall_score = 93.4
            failed_metrics = []
        return {
            "overall_score": overall_score,
            "pacing_score": 8.1,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": failed_metrics,
                "continuity_warning_count": 0,
                "overall_score": overall_score,
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need repair",
            "active_story_repair_payload": {
                "summary": "Hook the chapter immediately and ground the governing rule on-page.",
                "repair_targets": ["Make the opening hook expose the active rule within the first two paragraphs"],
                "preserve_strengths": ["Keep the current chapter mission and continuity"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-targeted-repair-opening-rule-grounding",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 4
    assert "Word-budget repair pass #3" in ai_service.calls[2]["prompt"]
    assert "Targeted quality repair pass #4" in ai_service.calls[3]["prompt"]
    assert "Joint opening hard rule" in ai_service.calls[3]["prompt"]
    assert result["candidate_index"] == 4
    assert result["word_count"] == 1364
    assert result["generation_path"] == "targeted_quality_repair"
    assert result["attempt_kind"] == "targeted_quality_repair"
    assert result["quality_gate_decision"] == "allow_save"
    assert result["quality_metrics"]["candidate_selection"]["winner_candidate_index"] == 4
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_candidate_index"] == 3

async def test_should_fallback_to_overlong_cliffhanger_winner_for_targeted_final_repair_after_budget_repair_collapse():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2717
            elif len(self.calls) == 2:
                yield "B" * 1944
            elif len(self.calls) == 3:
                yield "C" * 1422
            else:
                yield "D" * 1405

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        failed_metrics: list[dict[str, str]]
        if content.startswith("A"):
            decision = "manual_review"
            overall_score = 88.7
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        elif content.startswith("B"):
            decision = "manual_review"
            overall_score = 90.8
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        elif content.startswith("C"):
            decision = "manual_review"
            overall_score = 46.1
            failed_metrics = [
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        else:
            decision = "allow_save"
            overall_score = 91.4
            failed_metrics = []
        return {
            "overall_score": overall_score,
            "pacing_score": 8.1,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": failed_metrics,
                "continuity_warning_count": 0,
                "overall_score": overall_score,
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need repair",
            "active_story_repair_payload": {
                "summary": "Tighten the chapter ending without regrowing the scene.",
                "repair_targets": ["Sharpen the unresolved closing hook"],
                "preserve_strengths": ["Keep the current continuity beats"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-fallback-overlong-cliffhanger-targeted-repair",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 4
    assert "Targeted quality repair pass #3" in ai_service.calls[3]["prompt"]
    assert "Cliffhanger hard rule" in ai_service.calls[3]["prompt"]
    assert ("B" * 120) in ai_service.calls[3]["prompt"]
    assert ("C" * 120) not in ai_service.calls[3]["prompt"]
    assert result["candidate_index"] == 3
    assert result["word_count"] == 1405
    assert result["generation_path"] == "targeted_quality_repair"
    assert result["attempt_kind"] == "targeted_quality_repair"
    assert result["quality_gate_decision"] == "allow_save"
    assert result["quality_metrics"]["candidate_selection"]["winner_candidate_index"] == 3
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_candidate_index"] == 2
    word_budget_items = [
        item
        for item in result["quality_metrics"]["candidate_pool_summary"]
        if item.get("generation_path") == "word_budget_repair"
    ]
    assert len(word_budget_items) == 0

async def test_should_drop_collapsed_targeted_final_repair_candidate_from_pool():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2838
            elif len(self.calls) == 2:
                yield "B" * 1884
            elif len(self.calls) == 3:
                yield "C" * 1422
            elif len(self.calls) == 4:
                yield "D" * 554
            else:
                yield "E" * 1434

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        failed_metrics: list[dict[str, str]]
        if content.startswith("A"):
            decision = "auto_repair"
            overall_score = 88.8
            failed_metrics = [
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        elif content.startswith("B"):
            decision = "manual_review"
            overall_score = 79.8
            failed_metrics = [
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Dialogue", "focus_area": "dialogue"},
            ]
        elif content.startswith("C"):
            decision = "manual_review"
            overall_score = 88.7
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        elif content.startswith("D"):
            decision = "manual_review"
            overall_score = 67.6
            failed_metrics = [
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
                {"label": "Outline", "focus_area": "outline"},
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        else:
            decision = "manual_review"
            overall_score = 88.7
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
            ]
        return {
            "overall_score": overall_score,
            "pacing_score": 8.1,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": failed_metrics,
                "continuity_warning_count": 0,
                "overall_score": overall_score,
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need repair",
            "active_story_repair_payload": {
                "summary": "Tighten the chapter ending without regrowing the scene.",
                "repair_targets": ["Sharpen the unresolved closing hook"],
                "preserve_strengths": ["Keep the current continuity beats"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-drop-collapsed-targeted-repair",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 5
    assert result["candidate_index"] == 3
    assert result["word_count"] == 1422
    targeted_repair_items = [
        item
        for item in result["quality_metrics"]["candidate_pool_summary"]
        if item.get("generation_path") == "targeted_quality_repair"
    ]
    assert len(targeted_repair_items) == 1
    assert targeted_repair_items[0]["candidate_index"] == 4
    assert targeted_repair_items[0]["word_count"] == 1434

async def test_should_run_followup_targeted_final_repair_for_rule_grounding_only_winner():
    class StubAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            if len(self.calls) == 1:
                yield "A" * 2920
            elif len(self.calls) == 2:
                yield "B" * 1957
            elif len(self.calls) == 3:
                yield "C" * 1422
            elif len(self.calls) == 4:
                yield "D" * 1434
            else:
                yield "E" * 1388

    ai_service = StubAIService()

    def evaluate_candidate_quality(content: str) -> dict[str, Any]:
        failed_metrics: list[dict[str, str]]
        if content.startswith("A"):
            decision = "manual_review"
            overall_score = 74.0
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        elif content.startswith("B"):
            decision = "manual_review"
            overall_score = 88.0
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Conflict chain", "focus_area": "conflict"},
            ]
        elif content.startswith("C"):
            decision = "manual_review"
            overall_score = 79.0
            failed_metrics = [
                {"label": "Cliffhanger", "focus_area": "cliffhanger"},
                {"label": "Conflict chain", "focus_area": "conflict"},
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        elif content.startswith("D"):
            decision = "manual_review"
            overall_score = 95.3
            failed_metrics = [
                {"label": "Rule grounding", "focus_area": "rule_grounding"},
            ]
        else:
            decision = "allow_save"
            overall_score = 96.1
            failed_metrics = []
        return {
            "overall_score": overall_score,
            "pacing_score": 8.1,
            "quality_runtime_context": {
                "quality_preset": "plot_drive",
                "creative_mode": "hook",
                "story_focus": "advance_plot",
            },
            "quality_gate": {
                "status": "blocked" if decision != "allow_save" else "pass",
                "decision": decision,
                "failed_metrics": failed_metrics,
                "continuity_warning_count": 0,
                "overall_score": overall_score,
            },
        }

    def build_candidate_quality_gate_plan(metrics: dict[str, Any], _attempt_offset: int) -> dict[str, Any]:
        quality_gate = metrics.get("quality_gate") if isinstance(metrics.get("quality_gate"), dict) else {}
        if quality_gate.get("decision") == "allow_save":
            return {"quality_gate": quality_gate, "message": "passed"}
        return {
            "quality_gate": quality_gate,
            "message": "need repair",
            "active_story_repair_payload": {
                "summary": "Strengthen the chapter landing without regrowing the scene.",
                "repair_targets": ["Sharpen the chapter-ending pressure"],
                "preserve_strengths": ["Keep the current continuity beats"],
            },
        }

    result = await chapters_api._generate_best_ranked_candidate(
        ai_service=ai_service,
        base_generate_kwargs={"prompt": "base prompt", "temperature": 0.8},
        target_word_count=1200,
        source="chapter",
        generation_label="test-followup-targeted-repair",
        quality_evaluator=evaluate_candidate_quality,
        quality_gate_plan_builder=build_candidate_quality_gate_plan,
        max_candidates=2,
    )

    assert len(ai_service.calls) == 5
    assert "Targeted quality repair pass #4" in ai_service.calls[3]["prompt"]
    assert "Targeted quality repair pass #5" in ai_service.calls[4]["prompt"]
    assert "Rule-grounding hard rule" in ai_service.calls[4]["prompt"]
    assert ("D" * 120) in ai_service.calls[4]["prompt"]
    assert ("C" * 120) not in ai_service.calls[4]["prompt"]
    assert result["candidate_index"] == 5
    assert result["word_count"] == 1388
    assert result["generation_path"] == "targeted_quality_repair"
    assert result["attempt_kind"] == "targeted_quality_repair"
    assert result["quality_gate_decision"] == "allow_save"
    assert result["quality_metrics"]["candidate_selection"]["winner_candidate_index"] == 5
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_candidate_index"] == 4
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_generation_path"] == "targeted_quality_repair"
    assert result["quality_metrics"]["candidate_selection"]["repair_seed_attempt_kind"] == "targeted_quality_repair"
    targeted_repair_items = [
        item
        for item in result["quality_metrics"]["candidate_pool_summary"]
        if item.get("generation_path") == "targeted_quality_repair"
    ]
    assert len(targeted_repair_items) == 2
    assert targeted_repair_items[-1]["repair_seed_candidate_index"] == 4
    assert targeted_repair_items[-1]["repair_seed_generation_path"] == "targeted_quality_repair"
    assert targeted_repair_items[-1]["repair_seed_attempt_kind"] == "targeted_quality_repair"

def test_should_retry_when_quality_gate_is_repairable_and_retry_budget_available():
    current_payload = chapters_api.normalize_story_repair_payload(
        "Manual summary",
        ["Keep scene pressure tangible"],
        ["Preserve character voice"],
    )
    metrics = {
        "overall_score": 80.5,
        "conflict_chain_hit_rate": 78.0,
        "rule_grounding_hit_rate": 82.0,
        "outline_alignment_rate": 84.0,
        "dialogue_naturalness_rate": 79.0,
        "opening_hook_rate": 83.0,
        "payoff_chain_rate": 77.0,
        "cliffhanger_rate": 81.0,
        "pacing_score": 7.8,
    }

    plan = chapters_api._resolve_quality_gate_execution_plan(
        metrics,
        retry_count=0,
        max_retries=2,
        current_story_repair_payload=current_payload,
        scope="batch",
    )

    assert plan["action"] == "retry"
    assert plan["quality_gate"]["decision"] == "auto_repair"
    assert plan["quality_gate"]["recommended_action"]
    assert "Recommended repair action" in (plan["message"] or "")
    assert plan["repair_payload"] is not None
    assert plan["active_story_repair_payload"]["quality_gate_decision"] == "auto_repair"

def test_should_switch_to_manual_review_when_quality_gate_blocks_or_retry_budget_is_exhausted():
    blocked_metrics = {
        "overall_score": 66.0,
        "conflict_chain_hit_rate": 48.0,
        "rule_grounding_hit_rate": 52.0,
        "outline_alignment_rate": 50.0,
        "dialogue_naturalness_rate": 75.0,
        "opening_hook_rate": 70.0,
        "payoff_chain_rate": 46.0,
        "cliffhanger_rate": 68.0,
        "pacing_score": 6.1,
    }
    blocked_plan = chapters_api._resolve_quality_gate_execution_plan(
        blocked_metrics,
        retry_count=0,
        max_retries=2,
        current_story_repair_payload=None,
        scope="batch",
    )

    exhausted_plan = chapters_api._resolve_quality_gate_execution_plan(
        {
            "overall_score": 79.0,
            "conflict_chain_hit_rate": 77.0,
            "rule_grounding_hit_rate": 81.0,
            "outline_alignment_rate": 80.0,
            "dialogue_naturalness_rate": 79.0,
            "opening_hook_rate": 82.0,
            "payoff_chain_rate": 78.0,
            "cliffhanger_rate": 80.0,
            "pacing_score": 7.5,
        },
        retry_count=1,
        max_retries=1,
        current_story_repair_payload=None,
        scope="batch",
    )

    assert blocked_plan["action"] == "manual_review"
    assert blocked_plan["quality_gate"]["decision"] == "manual_review"
    assert "Recommended repair action" in (blocked_plan["message"] or "")
    assert exhausted_plan["action"] == "manual_review"
    assert exhausted_plan["quality_gate"]["decision"] == "auto_repair"
    assert "Recommended repair action" in (exhausted_plan["message"] or "")

async def test_should_rerank_generate_stream_candidates_and_save_best_winner(
    chapters_client,
    chapters_session_factory,
    fake_ai_service,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="rerank-stream",
    )

    class FakeContext:
        chapter_outline = "Outline anchor"
        continuation_point = None
        previous_chapter_summary = ""
        chapter_characters = "Character Ledger\n- Alex protects the hidden key"
        chapter_careers = "Alex: courier"
        foreshadow_reminders = "Foreshadow Ledger\n- Preserve the hidden-key pressure"
        relevant_memories = ""
        recent_chapters_context = ""
        context_stats = {}

    class FakeOneToManyBuilder:
        def __init__(self, *args, **kwargs):
            pass

        async def build(self, **kwargs):
            return FakeContext()

    class FakeOneToOneBuilder(FakeOneToManyBuilder):
        pass

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        return "mock-generate-prompt"

    def fake_compute_story_quality_metrics(**kwargs):
        content = kwargs["content"]
        if content == "draft-one":
            return {
                "overall_score": 79.0,
                "conflict_chain_hit_rate": 72.0,
                "rule_grounding_hit_rate": 80.0,
                "outline_alignment_rate": 76.0,
                "dialogue_naturalness_rate": 78.0,
                "opening_hook_rate": 74.0,
                "payoff_chain_rate": 69.0,
                "cliffhanger_rate": 82.0,
                "pacing_score": 7.0,
            }
        return {
            "overall_score": 92.0,
            "conflict_chain_hit_rate": 88.0,
            "rule_grounding_hit_rate": 90.0,
            "outline_alignment_rate": 91.0,
            "dialogue_naturalness_rate": 89.0,
            "opening_hook_rate": 90.0,
            "payoff_chain_rate": 87.0,
            "cliffhanger_rate": 91.0,
            "pacing_score": 8.7,
        }

    def fake_resolve_quality_gate_execution_plan(
        quality_metrics,
        *,
        retry_count,
        max_retries,
        current_story_repair_payload,
        scope,
    ):
        overall_score = float((quality_metrics or {}).get("overall_score") or 0.0)
        if overall_score >= 90.0:
            return {
                "action": "continue",
                "message": "winner accepted",
                "quality_gate": {
                    "decision": "allow_save",
                    "status": "pass",
                    "failed_metrics": [],
                },
            }
        return {
            "action": "retry",
            "message": "need rerank retry",
            "quality_gate": {
                "decision": "auto_repair",
                "status": "warn",
                "failed_metrics": [{"label": "Conflict chain"}],
                "recommended_action": "repair_conflict",
            },
            "active_story_repair_payload": {
                "summary": "Strengthen conflict payoff",
                "repair_targets": ["conflict payoff"],
                "preserve_strengths": ["voice"],
            },
        }

    generation_calls: list[dict[str, Any]] = []
    responses = [["draft-", "one"], ["draft-", "two"], ["draft-", "two"]]

    async def fake_generate_text_stream(**kwargs):
        generation_calls.append(kwargs)
        for chunk in responses[len(generation_calls) - 1]:
            yield chunk

    monkeypatch.setattr(chapters_api, "OneToManyContextBuilder", FakeOneToManyBuilder)
    monkeypatch.setattr(chapters_api, "OneToOneContextBuilder", FakeOneToOneBuilder)
    monkeypatch.setattr(chapters_api.PromptService, "get_template", fake_get_template)
    monkeypatch.setattr(chapters_api.PromptService, "format_prompt", fake_format_prompt)
    monkeypatch.setattr(chapters_api, "compute_story_quality_metrics", fake_compute_story_quality_metrics)
    monkeypatch.setattr(chapters_api, "_resolve_quality_gate_execution_plan", fake_resolve_quality_gate_execution_plan)
    monkeypatch.setattr(fake_ai_service, "generate_text_stream", fake_generate_text_stream)

    response = await chapters_client.post(
        f"/api/chapters/{chapter.id}/generate-stream",
        json={"target_word_count": 500, "enable_analysis": False},
    )
    assert response.status_code == 200

    events = parse_sse_data(response.text)
    result_event = next(event for event in events if event.get("type") == "result")
    result_data = result_event["data"]

    assert len(generation_calls) >= 2
    assert "Revision attempt #2" in generation_calls[1]["prompt"]
    assert result_data["quality_gate_action"] == "continue"
    assert result_data["quality_metrics"]["candidate_selection"]["candidate_count"] >= 2
    assert result_data["quality_metrics"]["candidate_selection"]["candidate_index"] >= 2
    assert result_data["quality_metrics"]["candidate_selection"]["generation_path"] in {"rerank_retry", "targeted_quality_repair"}
    assert result_data["quality_metrics"]["candidate_selection"]["attempt_kind"] in {"rerank_candidate", "targeted_quality_repair"}
    assert result_data["quality_metrics"]["candidate_selection"]["word_budget_repair_used"] is False
    assert (
        result_data["quality_metrics"]["candidate_selection"]["winner_candidate_index"]
        == result_data["quality_metrics"]["candidate_selection"]["candidate_index"]
    )
    assert len(result_data["quality_metrics"]["candidate_pool_summary"]) >= 2
    assert result_data["quality_metrics"]["candidate_pool_summary"][0]["candidate_index"] == 1
    assert result_data["quality_metrics"]["candidate_pool_summary"][-1]["candidate_index"] >= 2
    assert any(item["is_winner"] is True for item in result_data["quality_metrics"]["candidate_pool_summary"])

    async with chapters_session_factory() as session:
        saved_chapter = await session.get(Chapter, chapter.id)
        history_result = await session.execute(
            select(GenerationHistory).where(GenerationHistory.chapter_id == chapter.id)
        )
        histories = history_result.scalars().all()

        assert saved_chapter is not None
        assert saved_chapter.status == "completed"
        assert saved_chapter.content == "draft-two"
        assert histories
        assert "draft-two" in histories[0].generated_content

async def test_generate_single_chapter_for_batch_should_rerank_candidates_before_returning(
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="rerank-batch-helper",
    )

    class FakeContext:
        chapter_outline = "Outline anchor"
        continuation_point = None
        previous_chapter_summary = ""
        chapter_characters = "Character Ledger\n- Alex protects the hidden key"
        chapter_careers = "Alex: courier"
        foreshadow_reminders = "Foreshadow Ledger\n- Preserve the hidden-key pressure"
        relevant_memories = ""
        recent_chapters_context = ""
        context_stats = {}

    class FakeOneToManyBuilder:
        def __init__(self, *args, **kwargs):
            pass

        async def build(self, **kwargs):
            return FakeContext()

    async def fake_resolve_chapter_quality_profile(**kwargs):
        return {
            "resolved_style_id": None,
            "style_content": "",
            "style_name": "",
            "style_preset_id": "",
        }

    async def fake_get_template(*args, **kwargs):
        return "template"

    def fake_format_prompt(template, **kwargs):
        return "mock-batch-generate-prompt"

    def fake_build_chapter_runtime_system_prompt(**kwargs):
        return "mock-batch-system-prompt"

    def fake_compute_story_quality_metrics(**kwargs):
        content = kwargs["content"]
        if content == "draft-one":
            return {
                "overall_score": 78.0,
                "conflict_chain_hit_rate": 70.0,
                "rule_grounding_hit_rate": 79.0,
                "outline_alignment_rate": 75.0,
                "dialogue_naturalness_rate": 77.0,
                "opening_hook_rate": 74.0,
                "payoff_chain_rate": 68.0,
                "cliffhanger_rate": 81.0,
                "pacing_score": 6.9,
            }
        return {
            "overall_score": 93.0,
            "conflict_chain_hit_rate": 89.0,
            "rule_grounding_hit_rate": 91.0,
            "outline_alignment_rate": 92.0,
            "dialogue_naturalness_rate": 90.0,
            "opening_hook_rate": 88.0,
            "payoff_chain_rate": 87.0,
            "cliffhanger_rate": 92.0,
            "pacing_score": 8.8,
        }

    def fake_resolve_quality_gate_execution_plan(
        quality_metrics,
        *,
        retry_count,
        max_retries,
        current_story_repair_payload,
        scope,
    ):
        overall_score = float((quality_metrics or {}).get("overall_score") or 0.0)
        if overall_score >= 90.0:
            return {
                "action": "continue",
                "message": "winner accepted",
                "quality_gate": {
                    "decision": "allow_save",
                    "status": "pass",
                    "failed_metrics": [],
                },
            }
        return {
            "action": "retry",
            "message": "need rerank retry",
            "quality_gate": {
                "decision": "auto_repair",
                "status": "warn",
                "failed_metrics": [{"label": "Conflict chain"}],
                "recommended_action": "repair_conflict",
            },
            "active_story_repair_payload": {
                "summary": "Strengthen conflict payoff",
                "repair_targets": ["conflict payoff"],
                "preserve_strengths": ["voice"],
            },
        }

    class SequencedAIService:
        def __init__(self):
            self.calls: list[dict[str, Any]] = []
            self.responses = [["draft-", "one"], ["draft-", "two"], ["draft-", "two"]]

        async def generate_text_stream(self, **kwargs):
            self.calls.append(kwargs)
            for chunk in self.responses[len(self.calls) - 1]:
                yield chunk

    ai_service = SequencedAIService()

    monkeypatch.setattr(chapters_api.chapter_web_research_service, "is_enabled", lambda *_args, **_kwargs: False)
    monkeypatch.setattr(chapters_api, "OneToManyContextBuilder", FakeOneToManyBuilder)
    monkeypatch.setattr(chapters_api, "resolve_chapter_quality_profile", fake_resolve_chapter_quality_profile)
    monkeypatch.setattr(chapters_api.PromptService, "get_template", fake_get_template)
    monkeypatch.setattr(chapters_api.PromptService, "format_prompt", fake_format_prompt)
    monkeypatch.setattr(chapters_api, "_build_chapter_runtime_system_prompt", fake_build_chapter_runtime_system_prompt)
    monkeypatch.setattr(chapters_api, "compute_story_quality_metrics", fake_compute_story_quality_metrics)
    monkeypatch.setattr(chapters_api, "_resolve_quality_gate_execution_plan", fake_resolve_quality_gate_execution_plan)

    async with chapters_session_factory() as session:
        db_chapter = await session.get(Chapter, chapter.id)
        assert db_chapter is not None

        result = await batch_generation_single_chapter_entry_service.generate_single_chapter_for_batch(
            db_session=session,
            chapter=db_chapter,
            user_id=mock_user.user_id,
            style_id=None,
            target_word_count=600,
            ai_service=ai_service,
            write_lock=chapters_api.Lock(),
            retry_count=0,
            max_retries=1,
        )

    assert len(ai_service.calls) >= 2
    assert "Revision attempt #2" in ai_service.calls[1]["prompt"]
    assert result["full_content"] == "draft-two"
    assert result["candidate_count"] >= 2
    assert result["quality_gate_plan"]["action"] == "continue"
    assert result["quality_gate_plan"]["quality_gate"]["decision"] == "auto_repair"
    assert result["quality_metrics"]["candidate_selection"]["candidate_count"] == result["candidate_count"]
    assert result["quality_metrics"]["candidate_selection"]["candidate_index"] >= 2
    assert result["quality_metrics"]["candidate_selection"]["generation_path"] in {"rerank_retry", "targeted_quality_repair"}
    assert result["quality_metrics"]["candidate_selection"]["attempt_kind"] in {"rerank_candidate", "targeted_quality_repair"}
    assert result["quality_metrics"]["candidate_selection"]["word_budget_repair_used"] is False
    assert (
        result["quality_metrics"]["candidate_selection"]["winner_candidate_index"]
        == result["quality_metrics"]["candidate_selection"]["candidate_index"]
    )

def test_should_recompute_execution_quality_gate_from_latest_candidate_selection():
    metrics = {
        "overall_score": 96.9,
        "conflict_chain_hit_rate": 100.0,
        "rule_grounding_hit_rate": 100.0,
        "outline_alignment_rate": 100.0,
        "dialogue_naturalness_rate": 74.0,
        "opening_hook_rate": 100.0,
        "payoff_chain_rate": 100.0,
        "cliffhanger_rate": 100.0,
        "quality_gate": {
            "status": "pass",
            "decision": "allow_save",
            "allow_save": True,
        },
        "candidate_selection": {
            "word_count": 2699,
            "target_word_count": 1200,
            "candidate_index": 1,
            "candidate_count": 1,
        },
        "quality_runtime_context": {
            "plot_stage": "development",
            "quality_preset": "plot_drive",
            "story_focus": "advance_plot",
            "creative_mode": "hook",
        },
    }

    quality_gate_plan = chapters_api._resolve_quality_gate_execution_plan(
        metrics,
        retry_count=0,
        max_retries=1,
        current_story_repair_payload=None,
        scope="chapter",
    )

    assert quality_gate_plan["action"] == "retry"
    assert quality_gate_plan["quality_gate"]["decision"] == "auto_repair"
    assert quality_gate_plan["quality_gate"]["allow_save"] is False
