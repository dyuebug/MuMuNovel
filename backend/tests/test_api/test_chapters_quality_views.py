import json
from datetime import datetime, timedelta
from typing import Any

import pytest

from app.api import chapters as chapters_api
from app.models.generation_history import GenerationHistory
from app.models.memory import PlotAnalysis, StoryMemory
from tests.test_api.chapters_test_support import (
    _build_quality_history_payload,
    chapters_client,
    chapters_session_factory,
    create_chapter,
    create_project,
    fake_ai_service,
    mock_side_effect_services,
    reset_chapters_runtime_caches,
)

pytestmark = pytest.mark.asyncio

async def test_should_include_latest_chapter_quality_metrics_in_project_quality_trend(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="评分章节",
        content="正文内容",
        status="completed",
    )

    now = datetime.utcnow()
    async with chapters_session_factory() as session:
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="旧记录",
                generated_content="纯文本旧数据",
                model="default",
                created_at=now - timedelta(minutes=5),
            )
        )
        session.add(
            GenerationHistory(
                project_id=project.id,
                chapter_id=chapter.id,
                prompt="新记录",
                generated_content=json.dumps(
                    {
                        "log_type": "chapter_generation_quality_v1",
                        "quality_metrics": {
                            "overall_score": 78.5,
                            "conflict_chain_hit_rate": 70.0,
                            "rule_grounding_hit_rate": 82.0,
                            "outline_alignment_rate": 75.0,
                            "dialogue_naturalness_rate": 68.0,
                        },
                    },
                    ensure_ascii=False,
                ),
                model="default",
                created_at=now - timedelta(minutes=1),
            )
        )
        await session.commit()

    response = await chapters_client.get(
        f"/api/chapters/project/{project.id}/quality-trend",
        params={"limit": 1},
    )
    assert response.status_code == 200
    body = response.json()
    assert body["project_id"] == project.id
    assert body["has_metrics"] is True
    assert body["analyzed_chapters"] == 1
    assert len(body["items"]) == 1
    assert body["items"][0]["chapter_id"] == chapter.id
    assert body["items"][0]["latest_quality_metrics"]["overall_score"] == 78.5
    assert body["items"][0]["latest_quality_metrics"]["conflict_chain_hit_rate"] == 70.0
    assert body["items"][0]["latest_quality_metrics"]["rule_grounding_hit_rate"] == 82.0
    assert body["items"][0]["latest_quality_metrics"]["repair_guidance"]["summary"]
    assert body["items"][0]["latest_quality_metrics"]["repair_guidance"]["focus_areas"]
    assert body["quality_metrics_summary"]["last_generated_at"] is not None

async def test_should_get_project_chapter_quality_trend(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter_one = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="Chapter 1",
        content="Chapter 1 body",
    )
    chapter_two = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="Chapter 2",
        content="Chapter 2 body",
    )
    await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="Chapter 3",
        content="Chapter 3 body",
    )

    now = datetime.utcnow()
    async with chapters_session_factory() as session:
        session.add_all(
            [
                GenerationHistory(
                    chapter_id=chapter_one.id,
                    project_id=project.id,
                    prompt="chapter one quality",
                    generated_content=json.dumps(
                        {
                            "log_type": "chapter_generation_quality_v1",
                            "quality_metrics": {
                                "overall_score": 78.0,
                                "conflict_chain_hit_rate": 62.0,
                                "rule_grounding_hit_rate": 80.0,
                                "outline_alignment_rate": 64.0,
                                "dialogue_naturalness_rate": 79.0,
                                "opening_hook_rate": 72.0,
                                "payoff_chain_rate": 58.0,
                                "cliffhanger_rate": 84.0,
                                "pacing_score": 6.9,
                                "quality_runtime_context": {
                                    "plot_stage": "development",
                                    "chapter_count": 12,
                                    "current_chapter_number": 9,
                                    "foreshadow_payoff_plan": ["王城密钥"],
                                    "foreshadow_state_ledger": ["王城密钥仍未现身", "苏离盟约尚未兑现"],
                                },
                            },
                        },
                        ensure_ascii=False,
                    ),
                    model="default",
                    created_at=now - timedelta(minutes=2),
                ),
                GenerationHistory(
                    chapter_id=chapter_two.id,
                    project_id=project.id,
                    prompt="chapter two quality",
                    generated_content=json.dumps(
                        {
                            "log_type": "chapter_generation_quality_v1",
                            "quality_metrics": {
                                "overall_score": 77.0,
                                "conflict_chain_hit_rate": 60.0,
                                "rule_grounding_hit_rate": 78.0,
                                "outline_alignment_rate": 63.0,
                                "dialogue_naturalness_rate": 77.0,
                                "opening_hook_rate": 70.0,
                                "payoff_chain_rate": 56.0,
                                "cliffhanger_rate": 86.0,
                                "pacing_score": 6.7,
                                "quality_runtime_context": {
                                    "plot_stage": "development",
                                    "chapter_count": 12,
                                    "current_chapter_number": 10,
                                    "foreshadow_payoff_plan": ["王城密钥", "苏离盟约"],
                                    "foreshadow_state_ledger": ["王城密钥仍未现身", "苏离盟约尚未兑现", "档案馆真相仍被压住"],
                                },
                            },
                        },
                        ensure_ascii=False,
                    ),
                    model="default",
                    created_at=now - timedelta(minutes=1),
                ),
            ]
        )
        await session.commit()

    response = await chapters_client.get(f"/api/chapters/project/{project.id}/quality-trend", params={"limit": 2})
    assert response.status_code == 200
    body = response.json()
    assert body["project_id"] == project.id
    assert body["has_metrics"] is True
    assert body["total_chapters"] == 3
    assert body["analyzed_chapters"] == 2
    assert len(body["items"]) == 2
    assert body["items"][0]["chapter_id"] == chapter_one.id
    assert body["items"][1]["chapter_id"] == chapter_two.id
    assert body["items"][1]["latest_quality_metrics"]["repair_guidance"]["summary"]
    assert body["quality_metrics_summary"]["chapter_count"] == 2
    assert body["quality_metrics_summary"]["total_chapters"] == 3
    assert body["quality_metrics_summary"]["analyzed_chapters"] == 2
    assert body["quality_metrics_summary"]["last_generated_at"] is not None
    assert body["quality_metrics_summary"]["pacing_imbalance"]["status"] in {"watch", "warning"}
    assert body["quality_metrics_summary"]["pacing_imbalance"]["signals"]
    assert body["quality_metrics_summary"]["volume_goal_completion"]["completion_rate"] > 0
    assert body["quality_metrics_summary"]["volume_goal_completion"]["profile_summary"]
    assert body["quality_metrics_summary"]["foreshadow_payoff_delay"]["delay_index"] > 0
    assert body["quality_metrics_summary"]["repair_effectiveness"]["success_rate"] >= 0

async def test_should_reuse_project_quality_trend_cached_summary_state(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter_one = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="Chapter 1",
        content="Chapter 1 body",
    )
    chapter_two = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="Chapter 2",
        content="Chapter 2 body",
    )

    now = datetime.utcnow()
    async with chapters_session_factory() as session:
        session.add_all(
            [
                GenerationHistory(
                    chapter_id=chapter_one.id,
                    project_id=project.id,
                    prompt="chapter one quality",
                    generated_content=_build_quality_history_payload(
                        {
                            "overall_score": 78.0,
                            "conflict_chain_hit_rate": 62.0,
                            "rule_grounding_hit_rate": 80.0,
                            "outline_alignment_rate": 64.0,
                            "dialogue_naturalness_rate": 79.0,
                            "opening_hook_rate": 72.0,
                            "payoff_chain_rate": 58.0,
                            "cliffhanger_rate": 84.0,
                            "pacing_score": 6.9,
                        }
                    ),
                    model="default",
                    created_at=now - timedelta(minutes=2),
                ),
                GenerationHistory(
                    chapter_id=chapter_two.id,
                    project_id=project.id,
                    prompt="chapter two quality",
                    generated_content=_build_quality_history_payload(
                        {
                            "overall_score": 81.0,
                            "conflict_chain_hit_rate": 67.0,
                            "rule_grounding_hit_rate": 82.0,
                            "outline_alignment_rate": 69.0,
                            "dialogue_naturalness_rate": 80.0,
                            "opening_hook_rate": 75.0,
                            "payoff_chain_rate": 61.0,
                            "cliffhanger_rate": 85.0,
                            "pacing_score": 7.1,
                        }
                    ),
                    model="default",
                    created_at=now - timedelta(minutes=1),
                ),
            ]
        )
        await session.commit()

    original_build_state = chapters_api.build_quality_metrics_summary_state
    original_advance_state = chapters_api.advance_quality_metrics_summary_state
    calls = {"build": 0, "advance": 0}

    def counting_build_state(*args, **kwargs):
        calls["build"] += 1
        return original_build_state(*args, **kwargs)

    def counting_advance_state(*args, **kwargs):
        calls["advance"] += 1
        return original_advance_state(*args, **kwargs)

    monkeypatch.setattr(chapters_api, "build_quality_metrics_summary_state", counting_build_state)
    monkeypatch.setattr(chapters_api, "advance_quality_metrics_summary_state", counting_advance_state)

    first_response = await chapters_client.get(
        f"/api/chapters/project/{project.id}/quality-trend",
        params={"limit": 2},
    )
    second_response = await chapters_client.get(
        f"/api/chapters/project/{project.id}/quality-trend",
        params={"limit": 2},
    )

    assert first_response.status_code == 200
    assert second_response.status_code == 200
    assert second_response.json()["quality_metrics_summary"]["chapter_count"] == 2
    assert calls["build"] == 1
    assert calls["advance"] == 0

async def test_should_incrementally_slide_project_quality_trend_cache_when_window_moves(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter_one = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="Chapter 1",
        content="Chapter 1 body",
    )
    chapter_two = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="Chapter 2",
        content="Chapter 2 body",
    )
    chapter_three = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=3,
        title="Chapter 3",
        content="Chapter 3 body",
    )

    now = datetime.utcnow()
    async with chapters_session_factory() as session:
        session.add_all(
            [
                GenerationHistory(
                    chapter_id=chapter_one.id,
                    project_id=project.id,
                    prompt="chapter one quality",
                    generated_content=_build_quality_history_payload(
                        {
                            "overall_score": 76.0,
                            "conflict_chain_hit_rate": 60.0,
                            "rule_grounding_hit_rate": 79.0,
                            "outline_alignment_rate": 63.0,
                            "dialogue_naturalness_rate": 77.0,
                            "opening_hook_rate": 71.0,
                            "payoff_chain_rate": 57.0,
                            "cliffhanger_rate": 82.0,
                            "pacing_score": 6.8,
                        }
                    ),
                    model="default",
                    created_at=now - timedelta(minutes=3),
                ),
                GenerationHistory(
                    chapter_id=chapter_two.id,
                    project_id=project.id,
                    prompt="chapter two quality",
                    generated_content=_build_quality_history_payload(
                        {
                            "overall_score": 80.0,
                            "conflict_chain_hit_rate": 66.0,
                            "rule_grounding_hit_rate": 81.0,
                            "outline_alignment_rate": 68.0,
                            "dialogue_naturalness_rate": 79.0,
                            "opening_hook_rate": 74.0,
                            "payoff_chain_rate": 60.0,
                            "cliffhanger_rate": 84.0,
                            "pacing_score": 7.0,
                        }
                    ),
                    model="default",
                    created_at=now - timedelta(minutes=2),
                ),
            ]
        )
        await session.commit()

    original_build_state = chapters_api.build_quality_metrics_summary_state
    original_advance_state = chapters_api.advance_quality_metrics_summary_state
    calls = {"build": 0, "advance": 0}

    def counting_build_state(*args, **kwargs):
        calls["build"] += 1
        return original_build_state(*args, **kwargs)

    def counting_advance_state(*args, **kwargs):
        calls["advance"] += 1
        return original_advance_state(*args, **kwargs)

    monkeypatch.setattr(chapters_api, "build_quality_metrics_summary_state", counting_build_state)
    monkeypatch.setattr(chapters_api, "advance_quality_metrics_summary_state", counting_advance_state)

    first_response = await chapters_client.get(
        f"/api/chapters/project/{project.id}/quality-trend",
        params={"limit": 2},
    )
    assert first_response.status_code == 200
    assert first_response.json()["items"][0]["chapter_id"] == chapter_one.id
    assert calls["build"] == 1

    async with chapters_session_factory() as session:
        session.add(
            GenerationHistory(
                chapter_id=chapter_three.id,
                project_id=project.id,
                prompt="chapter three quality",
                generated_content=_build_quality_history_payload(
                    {
                        "overall_score": 84.0,
                        "conflict_chain_hit_rate": 72.0,
                        "rule_grounding_hit_rate": 84.0,
                        "outline_alignment_rate": 74.0,
                        "dialogue_naturalness_rate": 82.0,
                        "opening_hook_rate": 78.0,
                        "payoff_chain_rate": 66.0,
                        "cliffhanger_rate": 87.0,
                        "pacing_score": 7.4,
                    }
                ),
                model="default",
                created_at=now - timedelta(minutes=1),
            )
        )
        await session.commit()

    second_response = await chapters_client.get(
        f"/api/chapters/project/{project.id}/quality-trend",
        params={"limit": 2},
    )
    assert second_response.status_code == 200
    body = second_response.json()
    assert body["items"][0]["chapter_id"] == chapter_two.id
    assert body["items"][1]["chapter_id"] == chapter_three.id
    assert body["analyzed_chapters"] == 2
    assert body["quality_metrics_summary"]["chapter_count"] == 2
    assert calls["build"] == 1
    assert calls["advance"] >= 1

async def test_should_return_chapter_annotations_with_analysis_metadata(
    chapters_client,
    chapters_session_factory,
    mock_user,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter_content = "???????????????????????"
    chapter = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="????",
        content=chapter_content,
        status="completed",
    )

    analysis = PlotAnalysis(
        project_id=project.id,
        chapter_id=chapter.id,
        plot_stage="??",
        hooks=[{"type": "??", "content": "????", "keyword": "??", "strength": 8, "position": "??"}],
        hooks_count=1,
        foreshadows=[{"content": "????", "type": "planted", "keyword": "????", "strength": 7}],
        foreshadows_planted=1,
        plot_points=[{"content": "????????", "keyword": "??", "importance": 0.9, "type": "conflict"}],
        plot_points_count=1,
        overall_quality_score=8.2,
        pacing_score=8.0,
        engagement_score=8.3,
        coherence_score=8.1,
        word_count=len(chapter_content),
        created_at=datetime.utcnow(),
    )

    async with chapters_session_factory() as session:
        session.add(analysis)
        session.add_all(
            [
                StoryMemory(
                    project_id=project.id,
                    chapter_id=chapter.id,
                    memory_type="hook",
                    title="????",
                    content="????",
                    story_timeline=1,
                    chapter_position=-1,
                    text_length=0,
                    importance_score=0.9,
                    tags=["??"],
                ),
                StoryMemory(
                    project_id=project.id,
                    chapter_id=chapter.id,
                    memory_type="foreshadow",
                    title="????",
                    content="????",
                    story_timeline=1,
                    chapter_position=7,
                    text_length=4,
                    importance_score=0.8,
                    is_foreshadow=1,
                    related_locations=["??"],
                ),
                StoryMemory(
                    project_id=project.id,
                    chapter_id=chapter.id,
                    memory_type="plot_point",
                    title="????",
                    content="????????",
                    story_timeline=1,
                    chapter_position=-1,
                    text_length=0,
                    importance_score=0.7,
                ),
            ]
        )
        await session.commit()

    response = await chapters_client.get(f"/api/chapters/{chapter.id}/annotations")
    assert response.status_code == 200
    body = response.json()
    assert body["chapter_id"] == chapter.id
    assert body["has_analysis"] is True
    assert body["summary"]["total_annotations"] == 3
    assert body["summary"]["hooks"] == 1
    assert body["summary"]["foreshadows"] == 1
    assert body["summary"]["plot_points"] == 1

    hook_annotation = next(item for item in body["annotations"] if item["type"] == "hook")
    foreshadow_annotation = next(item for item in body["annotations"] if item["type"] == "foreshadow")
    plot_annotation = next(item for item in body["annotations"] if item["type"] == "plot_point")

    assert hook_annotation["position"] == chapter_content.find("??")
    assert hook_annotation["length"] == len("??")
    assert hook_annotation["metadata"]["strength"] == 8
    assert hook_annotation["metadata"]["position_desc"] == "??"

    assert foreshadow_annotation["position"] == 7
    assert foreshadow_annotation["length"] == 4
    assert foreshadow_annotation["metadata"]["foreshadow_type"] == "planted"
    assert foreshadow_annotation["metadata"]["strength"] == 7
    assert foreshadow_annotation["metadata"]["related_locations"] == ["??"]

    assert plot_annotation["position"] == chapter_content.find("??")
    assert plot_annotation["length"] == len("??")

async def test_should_restore_project_quality_trend_from_persisted_snapshot_after_cache_clear(
    chapters_client,
    chapters_session_factory,
    mock_user,
    monkeypatch,
):
    project = await create_project(chapters_session_factory, user_id=mock_user.user_id)
    chapter_one = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=1,
        title="Trend A",
        content="Trend A body",
    )
    chapter_two = await create_chapter(
        chapters_session_factory,
        project_id=project.id,
        chapter_number=2,
        title="Trend B",
        content="Trend B body",
    )

    now = datetime.utcnow()
    async with chapters_session_factory() as session:
        session.add_all(
            [
                GenerationHistory(
                    chapter_id=chapter_one.id,
                    project_id=project.id,
                    prompt="trend one",
                    generated_content=_build_quality_history_payload(
                        {
                            "overall_score": 78.0,
                            "conflict_chain_hit_rate": 64.0,
                            "rule_grounding_hit_rate": 82.0,
                            "outline_alignment_rate": 68.0,
                            "dialogue_naturalness_rate": 79.0,
                            "opening_hook_rate": 74.0,
                            "payoff_chain_rate": 60.0,
                            "cliffhanger_rate": 83.0,
                            "pacing_score": 6.9,
                        }
                    ),
                    model="default",
                    created_at=now - timedelta(minutes=2),
                ),
                GenerationHistory(
                    chapter_id=chapter_two.id,
                    project_id=project.id,
                    prompt="trend two",
                    generated_content=_build_quality_history_payload(
                        {
                            "overall_score": 84.0,
                            "conflict_chain_hit_rate": 70.0,
                            "rule_grounding_hit_rate": 86.0,
                            "outline_alignment_rate": 73.0,
                            "dialogue_naturalness_rate": 82.0,
                            "opening_hook_rate": 78.0,
                            "payoff_chain_rate": 66.0,
                            "cliffhanger_rate": 87.0,
                            "pacing_score": 7.4,
                        }
                    ),
                    model="default",
                    created_at=now - timedelta(minutes=1),
                ),
            ]
        )
        await session.commit()

    original_build_state = chapters_api.build_quality_metrics_summary_state
    persisted_snapshots: dict[tuple[str, int], dict[str, Any]] = {}
    calls = {"build": 0, "load": 0, "persist": 0}

    def counting_build_state(*args, **kwargs):
        calls["build"] += 1
        return original_build_state(*args, **kwargs)

    def fake_persist_snapshot(project_id: str, limit: int, snapshot: dict[str, Any]):
        calls["persist"] += 1
        persisted_snapshots[(project_id, limit)] = json.loads(json.dumps(snapshot, ensure_ascii=False))

    def fake_load_snapshot(project_id: str, limit: int):
        calls["load"] += 1
        snapshot = persisted_snapshots.get((project_id, limit))
        return json.loads(json.dumps(snapshot, ensure_ascii=False)) if snapshot is not None else None

    monkeypatch.setattr(chapters_api, "build_quality_metrics_summary_state", counting_build_state)
    monkeypatch.setattr(chapters_api, "persist_project_quality_trend_snapshot", fake_persist_snapshot)
    monkeypatch.setattr(chapters_api, "load_project_quality_trend_snapshot", fake_load_snapshot)

    first_response = await chapters_client.get(
        f"/api/chapters/project/{project.id}/quality-trend",
        params={"limit": 2},
    )
    assert first_response.status_code == 200
    assert calls["build"] == 1
    assert calls["persist"] == 1

    chapters_api.project_quality_trend_cache.clear()

    second_response = await chapters_client.get(
        f"/api/chapters/project/{project.id}/quality-trend",
        params={"limit": 2},
    )
    assert second_response.status_code == 200
    assert second_response.json()["quality_metrics_summary"]["chapter_count"] == 2
    assert calls["build"] == 1
    assert calls["load"] == 2
    assert calls["persist"] == 2
